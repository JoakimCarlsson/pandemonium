//! The questions the host asks an extension's code while it runs a language server or a debug adapter, and the answers.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A program to start, as an extension says how.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Command {
    /// The program, a path or a name on the path.
    pub command: String,
    /// What it is started with.
    #[serde(default)]
    pub args: Vec<String>,
    /// Variables added to its environment.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// One completion a language server offered, as the extension is told of it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompletionItem {
    /// What the server calls it.
    pub label: String,
    /// What kind of thing it is, in a word: `function`, `variable`, `type`, `keyword`, `snippet` or `other`.
    #[serde(default)]
    pub kind: String,
    /// What the server says about it besides its name, such as a signature.
    #[serde(default)]
    pub detail: Option<String>,
}

/// One symbol a language server found, as the extension is told of it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SymbolItem {
    /// What it is called.
    pub name: String,
    /// What kind of thing it is, in a word: `function`, `variable`, `type`, `module` or `other`.
    #[serde(default)]
    pub kind: String,
    /// What holds it, when something does.
    #[serde(default)]
    pub container: Option<String>,
}

/// What the host asks an extension's code, by the name of the hook.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "hook", rename_all = "snake_case")]
pub enum HookCall {
    /// Which program starts the language server `server` for a project: answered with [`HookReply::Command`].
    LanguageServerCommand {
        /// The server's id, as the language's `config.toml` names it.
        server: String,
        /// The language, by folder name, whose file asked for it.
        language: String,
        /// The folder of the project; nothing unless the extension may read the project.
        #[serde(default)]
        project: Option<String>,
        /// The platform the host runs on, as `os-arch`: `linux-x86_64`, `macos-aarch64`, `windows-x86_64`.
        #[serde(default)]
        platform: String,
    },
    /// What the language server is told in its `initialize` request: answered with [`HookReply::Json`], or [`HookReply::None`] to leave the language's own.
    LanguageServerInitializationOptions {
        /// The server's id.
        server: String,
        /// The language whose file asked for it.
        language: String,
        /// The folder of the project; nothing unless the extension may read the project.
        #[serde(default)]
        project: Option<String>,
    },
    /// What the language server is told when it asks for its configuration: answered like the initialization options.
    LanguageServerWorkspaceConfiguration {
        /// The server's id.
        server: String,
        /// The language whose file asked for it.
        language: String,
        /// The folder of the project; nothing unless the extension may read the project.
        #[serde(default)]
        project: Option<String>,
    },
    /// What to show in a list of completions in place of the names a server gave: answered with [`HookReply::Labels`], one for each item, in order.
    LabelForCompletions {
        /// The server's id.
        server: String,
        /// The completions.
        items: Vec<CompletionItem>,
    },
    /// What to show in a list of symbols in place of the names a server gave: answered with [`HookReply::Labels`], one for each item, in order.
    LabelForSymbols {
        /// The server's id.
        server: String,
        /// The symbols.
        items: Vec<SymbolItem>,
    },
    /// Which program is the debug adapter `adapter`: answered with [`HookReply::Command`].
    DebugAdapterBinary {
        /// The adapter's id, as the extension's manifest names it.
        adapter: String,
        /// Whether the program may be fetched when the machine does not have it; when not, it is only said where it is, if it is anywhere.
        #[serde(default)]
        install: bool,
        /// The platform the host runs on, as `os-arch`.
        #[serde(default)]
        platform: String,
    },
}

/// What an extension's code answers a [`HookCall`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookReply {
    /// The program to start.
    Command(Command),
    /// Options or settings, as JSON.
    Json(serde_json::Value),
    /// A label for each item asked about, or nothing for an item to keep the one it has.
    Labels(Vec<Option<String>>),
    /// The extension has nothing to say, and what the language says holds.
    None,
    /// The extension could not answer, for this reason.
    Error(String),
}
