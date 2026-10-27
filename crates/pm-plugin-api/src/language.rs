//! What the host asks of an extension's module for the languages it carries, and what it answers.
//!
//! A language is mostly data, a `config.toml` and queries, and the module is
//! asked only for what data cannot say: how to start its language server, what
//! the latest version of a dependency is, and which tasks and toolchains a
//! project has. A module that does not know a hook answers [`LanguageReply::Unsupported`]
//! and the host falls back to the language's data.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What the host asks of the module of the extension that carries `language`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "hook", rename_all = "kebab-case")]
pub enum LanguageCall {
    /// How to start the language server of the language in the project; answered with [`LanguageReply::Command`].
    ServerCommand {
        /// The language, by its folder's name.
        language: String,
        /// The folder of the project; nothing unless the extension may read the project.
        #[serde(default)]
        project: Option<String>,
    },
    /// The latest version of the dependency `name`; answered with [`LanguageReply::Version`].
    LatestVersion {
        /// The language, by its folder's name.
        language: String,
        /// The dependency.
        name: String,
    },
    /// Whether the version a manifest asks for is behind `latest`; answered with [`LanguageReply::Replacement`], the text that asks for `latest` instead when it is.
    Behind {
        /// The language, by its folder's name.
        language: String,
        /// What the manifest asks for, as written.
        requirement: String,
        /// The latest version.
        latest: String,
    },
    /// The tasks the project has; answered with [`LanguageReply::Tasks`].
    Tasks {
        /// The language, by its folder's name.
        language: String,
        /// The folder of the project; nothing unless the extension may read the project.
        #[serde(default)]
        project: Option<String>,
    },
    /// The toolchains the machine and the project have; answered with [`LanguageReply::Toolchains`].
    Toolchains {
        /// The language, by its folder's name.
        language: String,
        /// The folder of the project; nothing unless the extension may read the project.
        #[serde(default)]
        project: Option<String>,
    },
}

/// A program to start, as a module says.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Launch {
    /// The program, a path or a name found on the machine's path.
    pub program: String,
    /// What it is started with; the language's own arguments when empty.
    #[serde(default)]
    pub args: Vec<String>,
    /// What the environment holds besides.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// A task a module says a project has.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TaskItem {
    /// What it is called.
    pub name: String,
    /// What is typed into the shell to run it.
    pub command: String,
    /// `build`, `test`, `run`, or nothing.
    #[serde(default)]
    pub group: String,
    /// The folder it runs in, from the root of the project.
    #[serde(default)]
    pub folder: String,
    /// The manifest it was found in.
    #[serde(default)]
    pub source: String,
}

/// A toolchain a module says is installed.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolchainItem {
    /// What it is chosen by.
    pub id: String,
    /// What the reader calls it.
    pub name: String,
    /// Where it is, or which version it is.
    #[serde(default)]
    pub detail: String,
    /// The folders put in front of the path programs are found on.
    #[serde(default)]
    pub path: Vec<String>,
    /// What the environment holds besides.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// What the module answers to a [`LanguageCall`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguageReply {
    /// The module does not do this; the host uses the language's data.
    Unsupported,
    /// The program that starts the language server.
    Command(Launch),
    /// A version, or nothing when the registry does not say.
    Version(Option<String>),
    /// What a requirement is rewritten as, or nothing when it is not behind.
    Replacement(Option<String>),
    /// The tasks of the project.
    Tasks(Vec<TaskItem>),
    /// The toolchains.
    Toolchains(Vec<ToolchainItem>),
    /// It failed, with the reason.
    Error(String),
}
