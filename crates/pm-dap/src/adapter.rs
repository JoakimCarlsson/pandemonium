//! The debug adapters the editor knows how to run, and which one a scenario
//! means.
//!
//! An adapter is named by the scenario that wants it, in whatever words the
//! editor that wrote the scenario used: VS Code's `cppdbg` and `lldb`, Zed's
//! `CodeLLDB` and `GDB`. Each adapter here answers to every name it can stand
//! in for, so a `launch.json` written for another editor debugs here too.

use std::path::PathBuf;
use std::sync::RwLock;

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
}

/// The adapters extensions bring, which are looked for after the ones the editor knows.
static EXTENSION_ADAPTERS: RwLock<Vec<Adapter>> = RwLock::new(Vec::new());

/// Puts together an adapter the editor knows from its parts.
fn known(
    id: &str,
    name: &str,
    programs: &[&str],
    arguments: &[&str],
    connect: Connect,
    types: &[&str],
) -> Adapter {
    let owned = |words: &[&str]| words.iter().map(|word| (*word).to_owned()).collect();
    Adapter {
        id: id.to_owned(),
        name: name.to_owned(),
        programs: owned(programs),
        arguments: owned(arguments),
        connect,
        types: owned(types),
        denied: None,
    }
}

/// Every adapter the editor can run, in the order they are offered: those it knows, then those extensions brought.
pub fn adapters() -> Vec<Adapter> {
    let mut every = vec![
        known(
            "gdb",
            "GDB",
            &["gdb"],
            &["--interpreter=dap", "--quiet"],
            Connect::Stdio,
            &["gdb", "cppdbg"],
        ),
        known(
            "lldb-dap",
            "LLDB",
            &["lldb-dap", "lldb-vscode"],
            &[],
            Connect::Stdio,
            &["lldb", "lldb-dap", "codelldb"],
        ),
        known(
            "debugpy",
            "Debugpy",
            &["python3", "python"],
            &["-m", "debugpy.adapter"],
            Connect::Stdio,
            &["debugpy", "python"],
        ),
        known(
            "delve",
            "Delve",
            &["dlv"],
            &["dap", "--listen", "127.0.0.1:{port}"],
            Connect::Tcp,
            &["go", "delve"],
        ),
    ];
    if let Ok(held) = EXTENSION_ADAPTERS.read() {
        every.extend(held.iter().cloned());
    }
    every
}

/// Replaces the adapters extensions bring with `adapters`, an adapter whose id or names one the editor knows being left out so that an extension adds adapters and never takes one over.
pub fn set_extension_adapters(adapters: Vec<Adapter>) {
    let own = self::adapters_known();
    let kept = adapters
        .into_iter()
        .filter(|adapter| {
            !own.iter().any(|held| {
                held.id == adapter.id || held.types.iter().any(|name| adapter.types.contains(name))
            })
        })
        .collect();
    if let Ok(mut held) = EXTENSION_ADAPTERS.write() {
        *held = kept;
    }
}

/// The adapters the editor knows, without those extensions bring.
fn adapters_known() -> Vec<Adapter> {
    adapters().into_iter().take(4).collect()
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

    /// The program this adapter is installed as, where it is installed.
    pub fn program(&self) -> Option<PathBuf> {
        self.programs
            .iter()
            .find_map(|program| crate::program::installed(program))
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
    /// words mean something this editor does not do: a Python program run in
    /// a terminal the editor would have to open, a Go program whose mode the
    /// other editor's extension guessed before delve ever heard of it. Those
    /// are turned into what this adapter does on its own.
    pub(crate) fn prepare(&self, config: &mut Map<String, Value>) {
        match self.id.as_str() {
            "debugpy" => {
                config.insert("console".into(), "internalConsole".into());
            }
            "delve" => {
                let guessed = config.get("mode").is_none_or(|mode| mode == "auto");
                if guessed {
                    config.insert("mode".into(), "debug".into());
                }
            }
            _ => {}
        }
    }
}
