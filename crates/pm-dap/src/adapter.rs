//! The debug adapters the editor knows how to run, and which one a scenario
//! means.
//!
//! An adapter is named by the scenario that wants it, in whatever words the
//! editor that wrote the scenario used: VS Code's `cppdbg` and `lldb`, `debug.json`'s
//! `CodeLLDB` and `GDB`. Each adapter answers to every name it can stand
//! in for, so a `launch.json` written for another editor debugs here too.
//! The adapters come from the [`Catalog`] a server registers: those the
//! installed languages declare and those extensions bring.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde_json::{Map, Value};

/// The placeholder in an adapter's arguments that the port it listens on
/// replaces.
pub(crate) const PORT: &str = "{port}";

/// How the editor reaches an adapter once it is running.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Connect {
    /// Over the adapter's standard input and output.
    Stdio,
    /// Over a socket on the loopback address, at a port the editor picks and
    /// hands the adapter in its arguments.
    Tcp,
}

/// One debug adapter, and how it is run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Adapter {
    /// The name it is known by in the protocol's handshake.
    pub id: String,
    /// The name a reader knows it by.
    pub name: String,
    /// The programs it may be installed as, the one to prefer first.
    pub programs: Vec<String>,
    /// What each of them is started with.
    pub arguments: Vec<String>,
    /// How the editor talks to it once it has started.
    pub connect: Connect,
    /// The names a scenario may call it by, in lower case.
    pub types: Vec<String>,
    /// Why it may not be run, for an adapter an extension brings that the reader has not allowed to run programs.
    pub denied: Option<String>,
    /// Whether the extension that brings it says which program it is, in place of `programs`.
    pub provided: bool,
    /// The starts of the names of the programs it attaches to; an empty one stands for any program, and the longest that fits a name picks the adapter.
    pub attach_names: Vec<String>,
    /// What it is always told besides what a scenario says, by key.
    pub set: Map<String, Value>,
    /// What it is told when a scenario says nothing, or `auto`, for the key.
    pub default: Map<String, Value>,
}

/// A program an extension says is a debug adapter, with what it is started with.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Binary {
    /// The program.
    pub program: PathBuf,
    /// What it is started with, in place of the adapter's arguments; `{port}` stands for the port it listens on.
    pub arguments: Vec<String>,
    /// Variables added to its environment.
    pub env: BTreeMap<String, String>,
}

/// Whoever knows which program an extension's debug adapter is, who may fetch it.
pub trait BinaryProvider: Send + Sync {
    /// The program of the adapter `adapter`: where it is, fetching it first when `install` and the machine does not have it; nothing when it is not anywhere. Fails with the reason when the extension cannot say, or is not allowed to.
    fn binary(&self, adapter: &str, install: bool) -> Result<Option<Binary>, String>;
}

/// Who says which program the adapters extensions bring are.
static BINARIES: RwLock<Option<Arc<dyn BinaryProvider>>> = RwLock::new(None);

/// Has `provider` say which program the adapters that extensions provide are.
pub fn set_binary_provider(provider: Arc<dyn BinaryProvider>) {
    if let Ok(mut held) = BINARIES.write() {
        *held = Some(provider);
    }
}

/// Whoever knows which adapters there are and what a file in front can be debugged as: the server of a machine, from the languages and extensions installed on it.
pub trait Catalog: Send + Sync {
    /// Every adapter, the one to prefer first.
    fn adapters(&self) -> Vec<Adapter>;

    /// What a file named `name` can be debugged as without a word written down.
    fn offers(&self, name: &str) -> Vec<Offer>;
}

/// A way to debug a file with no configuration written: a language says it for the files it reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Offer {
    /// What a reader calls it.
    pub label: String,
    /// The name of the adapter that runs it, as a scenario would call it.
    pub kind: String,
    /// What the adapter is told, with `${file}`, `${fileDirname}` and `${workspaceFolder}` filled in.
    pub config: Value,
}

/// Who knows the adapters and the offers.
static CATALOG: RwLock<Option<Arc<dyn Catalog>>> = RwLock::new(None);

/// Has `catalog` say which adapters there are and what files can be debugged as.
pub fn set_catalog(catalog: Arc<dyn Catalog>) {
    if let Ok(mut held) = CATALOG.write() {
        *held = Some(catalog);
    }
}

/// The catalog, when there is one.
fn catalog() -> Option<Arc<dyn Catalog>> {
    CATALOG.read().ok().and_then(|held| held.clone())
}

/// What the file named `name` can be debugged as without a word written down.
pub(crate) fn offers(name: &str) -> Vec<Offer> {
    catalog().map(|held| held.offers(name)).unwrap_or_default()
}

/// Every adapter the editor can run, in the order they are offered.
pub fn adapters() -> Vec<Adapter> {
    catalog().map(|held| held.adapters()).unwrap_or_default()
}

impl Adapter {
    /// The adapter a scenario calling it `kind` means, if the editor has one.
    pub fn find(kind: &str) -> Option<Self> {
        let kind = kind.to_lowercase();
        adapters()
            .into_iter()
            .find(|adapter| adapter.types.contains(&kind))
    }

    /// The adapter called `id` in the protocol's handshake, if the editor has one.
    pub fn with_id(id: &str) -> Option<Self> {
        adapters().into_iter().find(|adapter| adapter.id == id)
    }

    /// The program this adapter is installed as, where it is installed; for an adapter an extension provides, where the extension says it is without having it fetched.
    pub fn program(&self) -> Option<PathBuf> {
        if self.provided {
            return self.binary(false).ok().flatten().map(|held| held.program);
        }
        self.programs
            .iter()
            .find_map(|program| pm_tools::installed(program))
    }

    /// The program of an adapter an extension provides, as the extension says, fetching it first when `install` and the machine does not have it.
    fn binary(&self, install: bool) -> Result<Option<Binary>, String> {
        let provider = BINARIES.read().ok().and_then(|held| held.clone());
        match provider {
            Some(provider) => provider.binary(&self.id, install),
            None => Ok(None),
        }
    }

    /// The program to start for this adapter, fetched first when the extension that provides it says to, with what it is started with when `port` is where it listens and what is added to its environment.
    pub(crate) fn launch(&self, port: u16) -> Result<Binary, String> {
        if self.provided {
            let held = self
                .binary(true)?
                .ok_or_else(|| format!("{} is not installed", self.name))?;
            let arguments = if held.arguments.is_empty() {
                self.arguments_for(port)
            } else {
                held.arguments
                    .iter()
                    .map(|argument| argument.replace(PORT, &port.to_string()))
                    .collect()
            };
            return Ok(Binary {
                program: held.program,
                arguments,
                env: held.env,
            });
        }
        let program = self
            .program()
            .ok_or_else(|| format!("{} is not installed", self.name))?;
        Ok(Binary {
            program,
            arguments: self.arguments_for(port),
            env: BTreeMap::new(),
        })
    }

    /// Whether this adapter is installed.
    pub fn installed(&self) -> bool {
        self.program().is_some()
    }

    /// The arguments it is started with, listening on `port` where it
    /// listens on one.
    pub(crate) fn arguments_for(&self, port: u16) -> Vec<String> {
        self.arguments
            .iter()
            .map(|argument| argument.replace(PORT, &port.to_string()))
            .collect()
    }

    /// Settles what a scenario asks for into what this adapter takes.
    ///
    /// A scenario is written for the editor that wrote it, and a few of its
    /// words mean something this editor does not do. What the adapter's
    /// declaration says to set, and to default, turns those into what this
    /// adapter does on its own.
    pub(crate) fn prepare(&self, config: &mut Map<String, Value>) {
        for (key, value) in &self.default {
            let guessed = config.get(key).is_none_or(|held| held == "auto");
            if guessed {
                config.insert(key.clone(), value.clone());
            }
        }
        for (key, value) in &self.set {
            config.insert(key.clone(), value.clone());
        }
    }
}
