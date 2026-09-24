//! The debug adapters the editor knows how to run, and which one a scenario
//! means.
//!
//! An adapter is named by the scenario that wants it, in whatever words the
//! editor that wrote the scenario used: VS Code's `cppdbg` and `lldb`, Zed's
//! `CodeLLDB` and `GDB`. Each adapter here answers to every name it can stand
//! in for, so a `launch.json` written for another editor debugs here too.

use std::path::PathBuf;

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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Adapter {
    /// The name it is known by in the protocol's handshake.
    pub id: &'static str,
    /// The name a reader knows it by.
    pub name: &'static str,
    /// The programs it may be installed as, the one to prefer first.
    pub programs: &'static [&'static str],
    /// What each of them is started with.
    pub arguments: &'static [&'static str],
    /// How the editor talks to it once it has started.
    pub connect: Connect,
    /// The names a scenario may call it by, in lower case.
    pub types: &'static [&'static str],
}

/// Every adapter the editor can run, in the order they are offered.
pub const ADAPTERS: [Adapter; 4] = [
    Adapter {
        id: "gdb",
        name: "GDB",
        programs: &["gdb"],
        arguments: &["--interpreter=dap", "--quiet"],
        connect: Connect::Stdio,
        types: &["gdb", "cppdbg"],
    },
    Adapter {
        id: "lldb-dap",
        name: "LLDB",
        programs: &["lldb-dap", "lldb-vscode"],
        arguments: &[],
        connect: Connect::Stdio,
        types: &["lldb", "lldb-dap", "codelldb"],
    },
    Adapter {
        id: "debugpy",
        name: "Debugpy",
        programs: &["python3", "python"],
        arguments: &["-m", "debugpy.adapter"],
        connect: Connect::Stdio,
        types: &["debugpy", "python"],
    },
    Adapter {
        id: "delve",
        name: "Delve",
        programs: &["dlv"],
        arguments: &["dap", "--listen", "127.0.0.1:{port}"],
        connect: Connect::Tcp,
        types: &["go", "delve"],
    },
];

impl Adapter {
    /// The adapter a scenario calling it `kind` means, if the editor has one.
    pub fn find(kind: &str) -> Option<Self> {
        let kind = kind.to_lowercase();
        ADAPTERS
            .into_iter()
            .find(|adapter| adapter.types.contains(&kind.as_str()))
    }

    /// The program this adapter is installed as, where it is installed.
    pub fn program(self) -> Option<PathBuf> {
        self.programs
            .iter()
            .find_map(|program| pm_text::program::installed(program))
    }

    /// Whether this adapter is installed.
    pub fn installed(self) -> bool {
        self.program().is_some()
    }

    /// The arguments it is started with, listening on `port` where it
    /// listens on one.
    pub(crate) fn arguments_for(self, port: u16) -> Vec<String> {
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
    pub(crate) fn prepare(self, config: &mut Map<String, Value>) {
        match self.id {
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
