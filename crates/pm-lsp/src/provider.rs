//! What an extension's code says about the language servers of the languages it brought: which program starts one, what it is told, and what its completions and symbols are called in a list.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::manifest::ServerSpec;
use crate::types::Completion;

/// A program an extension says starts a language server.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProvidedCommand {
    /// The program.
    pub command: String,
    /// What it is started with.
    pub args: Vec<String>,
    /// Variables added to its environment.
    pub env: BTreeMap<String, String>,
}

/// What an extension says about one language server.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Provided {
    /// The program that starts it, when the extension names one.
    pub command: Option<ProvidedCommand>,
    /// The options it is told in its `initialize` request, when the extension says.
    pub init: Option<Value>,
    /// The settings it is told when it asks for its configuration, when the extension says.
    pub settings: Option<Value>,
}

/// A symbol a language server found, as the extension is told of it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SymbolLabel {
    /// What it is called.
    pub name: String,
    /// What kind of thing it is, in a word.
    pub kind: String,
    /// What holds it, when something does.
    pub container: Option<String>,
}

/// Whoever knows the extensions that brought languages: the server of a machine, which asks the extension's code.
pub trait Provider: Send + Sync {
    /// What the extension that brought `language` says about the server `spec` for the project at `root`; the program is only asked for when `command` is set. Fails with the reason when the extension cannot say, or is not allowed to.
    fn provide(
        &self,
        language: &str,
        spec: &ServerSpec,
        root: &Path,
        command: bool,
    ) -> Result<Provided, String>;

    /// What each of `items`, completions of the server `spec` for files of `language`, is called in a list; nothing for one that keeps its name.
    fn label_completions(
        &self,
        language: &str,
        spec: &ServerSpec,
        items: &[Completion],
    ) -> Vec<Option<String>>;

    /// What each of `items`, symbols of the server `spec` for files of `language`, is called in a list; nothing for one that keeps its name.
    fn label_symbols(
        &self,
        language: &str,
        spec: &ServerSpec,
        items: &[SymbolLabel],
    ) -> Vec<Option<String>>;
}
