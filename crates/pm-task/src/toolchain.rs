//! The toolchains installed on a machine, and what starting a program with one changes.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

/// One installed toolchain of a language: a version of its compiler or interpreter, or a project's own virtual environment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Toolchain {
    /// The language it is for, by family: named by the language folders that keep its toolchains, such as `rust` or `javascript`.
    pub language: String,
    /// What it is chosen by; `system` is the one the machine's own path finds.
    pub id: String,
    /// What a reader calls it.
    pub name: String,
    /// Where it is, or which version it is.
    pub detail: String,
    /// The folders put in front of the path programs are found on.
    pub path: Vec<PathBuf>,
    /// What the environment holds besides, which names it.
    pub env: BTreeMap<String, String>,
}

/// The id of the toolchain the machine's own path finds.
pub(crate) const SYSTEM: &str = "system";

impl Toolchain {
    /// The environment a program is started with when this toolchain is chosen: its own variables, and the path with its folders in front of `path`.
    pub fn environment(&self, path: Option<OsString>) -> BTreeMap<String, String> {
        let mut env = self.env.clone();
        if !self.path.is_empty() {
            let rest = path.as_deref().map(std::env::split_paths);
            let joined =
                std::env::join_paths(self.path.iter().cloned().chain(rest.into_iter().flatten()));
            if let Ok(joined) = joined {
                env.insert("PATH".to_owned(), joined.to_string_lossy().into_owned());
            }
        }
        env
    }
}
