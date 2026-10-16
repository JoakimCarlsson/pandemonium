//! What a client is told about the languages a server can install, and the grammar it highlights a file with.

use serde::{Deserialize, Serialize};

/// One language: whether it is installed on the server's machine and what installing it is doing.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LanguageInfo {
    /// Its folder's name, which requests name it by.
    pub id: String,
    /// What the reader calls it.
    pub name: String,
    /// Whether it is installed only as part of another, and so left out of the list the reader picks from.
    #[serde(default)]
    pub hidden: bool,
    /// The endings of the file names written in it.
    #[serde(default)]
    pub suffixes: Vec<String>,
    /// Whether it is installed.
    pub installed: bool,
    /// What installing it is doing now, while it is being installed.
    #[serde(default)]
    pub busy: Option<String>,
    /// Why installing it or its language server last failed.
    #[serde(default)]
    pub problem: Option<String>,
    /// What the machine has of its language server, in words for the reader, when it has one.
    #[serde(default)]
    pub server: Option<String>,
}

/// An installed language's grammar and queries, which a client highlights its files with.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GrammarBundle {
    /// The language's folder name.
    pub id: String,
    /// The name the grammar was built under.
    pub symbol: String,
    /// Other names a code fence may call the language by.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// The grammar, compiled to WebAssembly.
    #[serde(with = "crate::bytes")]
    pub wasm: Vec<u8>,
    /// The highlights query.
    pub highlights: String,
    /// The indents query.
    pub indents: String,
    /// The outline query.
    pub outline: String,
    /// The injections query.
    pub injections: String,
    /// The brackets query.
    pub brackets: String,
    /// How many columns one level of indentation is.
    pub indent: usize,
    /// Whether a level of indentation is a tab.
    #[serde(default)]
    pub tabs: bool,
    /// What starts a comment that runs to the end of the line.
    #[serde(default)]
    pub line_comment: Option<String>,
}
