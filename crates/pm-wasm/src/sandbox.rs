//! Compiling a module and running a call of it.

use std::sync::Arc;

use pm_plugin_api::{
    EXPORT_ALLOC, EXPORT_COMMAND, EXPORT_MEMORY, HOST_IMPORT, HOST_MODULE, Host, HostCall,
    HostReply, pack, unpack,
};
use wasmtime::{
    Caller, Config, Engine, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
};

/// How much one call of a module may use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// How many units of work the call may do before it is stopped.
    pub fuel: u64,
    /// How many bytes of memory the module may grow to.
    pub memory: usize,
    /// How many bytes one message across the boundary may be.
    pub message: usize,
}

impl Default for Limits {
    /// A budget for a command that fetches a page or runs a program: a billion units of work, 64 MiB of memory and 16 MiB a message.
    fn default() -> Self {
        Self {
            fuel: 1_000_000_000,
            memory: 64 << 20,
            message: 16 << 20,
        }
    }
}

/// What a running call holds: the host that answers the module and the limits it runs under.
struct Running {
    /// Who answers what the module asks for.
    host: Arc<dyn Host>,
    /// The ceiling of memory the module may grow to.
    limits: StoreLimits,
    /// How big one message across the boundary may be.
    message: usize,
}

/// A compiled extension module, ready to run calls.
pub struct Sandbox {
    /// The engine the module was compiled by.
    engine: Engine,
    /// The module.
    module: Module,
    /// What a call may use.
    limits: Limits,
}

/// The memory a running module exports.
fn memory_of(caller: &mut Caller<'_, Running>) -> Result<Memory, String> {
    caller
        .get_export(EXPORT_MEMORY)
        .and_then(wasmtime::Extern::into_memory)
        .ok_or_else(|| format!("the module exports no {EXPORT_MEMORY}"))
}

/// Copies `bytes` into the module's memory through its allocator and returns where, packed.
fn put(caller: &mut Caller<'_, Running>, bytes: &[u8]) -> Result<i64, String> {
    let alloc = caller
        .get_export(EXPORT_ALLOC)
        .and_then(wasmtime::Extern::into_func)
        .ok_or_else(|| format!("the module exports no {EXPORT_ALLOC}"))?
        .typed::<i32, i32>(&caller)
        .map_err(|error| error.to_string())?;
    let pointer = alloc
        .call(&mut *caller, bytes.len() as i32)
        .map_err(|error| error.to_string())?;
    memory_of(caller)?
        .write(&mut *caller, pointer as usize, bytes)
        .map_err(|error| error.to_string())?;
    Ok(pack(pointer as u32, bytes.len() as u32))
}

/// The `length` bytes the module has at `pointer`.
fn take(caller: &mut Caller<'_, Running>, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
    if length as usize > caller.data().message {
        return Err("the message is larger than a call may carry".to_owned());
    }
    let memory = memory_of(caller)?;
    let mut bytes = vec![0; length as usize];
    memory
        .read(&*caller, pointer as usize, &mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

/// What the host answers the module's request at `pointer` with, as the text of a [`HostReply`].
fn answer(caller: &mut Caller<'_, Running>, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
    let bytes = take(caller, pointer, length)?;
    let reply = match serde_json::from_slice::<HostCall>(&bytes) {
        Ok(call) => match caller.data().host.call(call) {
            Ok(value) => HostReply::Ok(value),
            Err(reason) => HostReply::Error(reason),
        },
        Err(error) => HostReply::Error(format!("not a request the host knows: {error}")),
    };
    serde_json::to_vec(&reply).map_err(|error| error.to_string())
}

impl Sandbox {
    /// Compiles the module in `wasm`, to run its calls under `limits`.
    pub fn new(wasm: &[u8], limits: Limits) -> Result<Self, String> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|error| error.to_string())?;
        let module = Module::new(&engine, wasm).map_err(|error| error.to_string())?;
        Ok(Self {
            engine,
            module,
            limits,
        })
    }

    /// Whether the module exports what the contract asks of it.
    pub fn exports_commands(&self) -> bool {
        [EXPORT_MEMORY, EXPORT_ALLOC, EXPORT_COMMAND]
            .into_iter()
            .all(|name| self.module.get_export(name).is_some())
    }

    /// Runs the module's command export on `input`, answering what it asks of the host through `host`, and returns what it wrote back.
    pub fn command(&self, input: &[u8], host: Arc<dyn Host>) -> Result<Vec<u8>, String> {
        let mut store = Store::new(
            &self.engine,
            Running {
                host,
                limits: StoreLimitsBuilder::new()
                    .memory_size(self.limits.memory)
                    .build(),
                message: self.limits.message,
            },
        );
        store.limiter(|running| &mut running.limits);
        store
            .set_fuel(self.limits.fuel)
            .map_err(|error| error.to_string())?;
        let mut linker = Linker::new(&self.engine);
        linker
            .func_wrap(
                HOST_MODULE,
                HOST_IMPORT,
                |mut caller: Caller<'_, Running>,
                 pointer: i32,
                 length: i32|
                 -> wasmtime::Result<i64> {
                    let reply = answer(&mut caller, pointer as u32, length as u32)
                        .map_err(wasmtime::Error::msg)?;
                    put(&mut caller, &reply).map_err(wasmtime::Error::msg)
                },
            )
            .map_err(|error| error.to_string())?;
        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|error| error.to_string())?;
        let memory = instance
            .get_memory(&mut store, EXPORT_MEMORY)
            .ok_or_else(|| format!("the module exports no {EXPORT_MEMORY}"))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, EXPORT_ALLOC)
            .map_err(|error| error.to_string())?;
        let run = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, EXPORT_COMMAND)
            .map_err(|error| error.to_string())?;
        let pointer = alloc
            .call(&mut store, input.len() as i32)
            .map_err(|error| error.to_string())?;
        memory
            .write(&mut store, pointer as usize, input)
            .map_err(|error| error.to_string())?;
        let packed = run
            .call(&mut store, (pointer, input.len() as i32))
            .map_err(|error| error.to_string())?;
        let (at, length) = unpack(packed);
        if length as usize > self.limits.message {
            return Err("the module answered with more than a call may carry".to_owned());
        }
        let mut reply = vec![0; length as usize];
        memory
            .read(&store, at as usize, &mut reply)
            .map_err(|error| error.to_string())?;
        Ok(reply)
    }
}
