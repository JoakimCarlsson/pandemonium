//! The debug adapters the editor can run, and which one a scenario means.
//!
//! The editor knows no adapter itself: the installed languages and extensions
//! bring them, each answering to the names a scenario may call it by, so that a
//! `launch.json` written for another editor debugs here too when a language
//! brings an adapter that answers to its `type`.

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
    /// Settings every scenario run with it gets, whatever it says.
    pub set: Map<String, Value>,
    /// Settings a scenario gets only when it names none or says `auto`.
    pub default: Map<String, Value>,
    /// The beginnings of the names of programs it attaches to, in lower case; `*` for any other program.
    pub attach: Vec<String>,
    /// How a build a scenario names in place of a program is run, for an adapter that knows only a program.
    pub build: Option<crate::build::Build>,
}

/// The adapters the installed languages and extensions bring.
static ADAPTERS: RwLock<Vec<Adapter>> = RwLock::new(Vec::new());

/// Every adapter the editor can run, in the order they are offered.
pub fn adapters() -> Vec<Adapter> {
    ADAPTERS.read().map(|held| held.clone()).unwrap_or_default()
}

/// Replaces the adapters the editor can run with `adapters`, an adapter whose id an earlier one has being left out.
pub fn set_adapters(adapters: Vec<Adapter>) {
    let mut kept: Vec<Adapter> = Vec::new();
    for adapter in adapters {
        if !kept.iter().any(|held| held.id == adapter.id) {
            kept.push(adapter);
        }
    }
    if let Ok(mut held) = ADAPTERS.write() {
        *held = kept;
    }
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
            .find_map(|program| pm_tools::installed(program))
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

    /// Settles what a scenario asks for into what this adapter takes: what the adapter says every scenario gets is put in, and what it says a scenario gets when it names none, or says `auto`, is put in where there is none.
    pub(crate) fn prepare(&self, config: &mut Map<String, Value>) {
        for (key, value) in &self.set {
            config.insert(key.clone(), value.clone());
        }
        for (key, value) in &self.default {
            if config.get(key).is_none_or(|held| held == "auto") {
                config.insert(key.clone(), value.clone());
            }
        }
    }
}
