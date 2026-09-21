//! Which language a file is written in, and what that brings with it.
//!
//! A language is the one place a file extension turns into everything the
//! editor knows about a file: the grammar its syntax tree is parsed with, the
//! query its highlights come from, and the language server that has opinions
//! about it. Nothing else matches on an extension.

use std::fmt::{self, Debug, Formatter};
use std::path::Path;

use tree_sitter::Language as Grammar;
use tree_sitter_language::LanguageFn;

/// A language the editor knows about.
#[derive(Clone, Copy)]
pub struct Language {
    /// The name the status bar shows.
    name: &'static str,
    /// The grammar the syntax tree is parsed with.
    grammar: LanguageFn,
    /// The query the highlights are captured by.
    highlights: &'static str,
    /// The language server to run for this language, when there is one.
    server: Option<Server>,
}

/// A language server, as the command that starts one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Server {
    /// The program to run.
    pub command: &'static str,
    /// The arguments to run it with.
    pub arguments: &'static [&'static str],
    /// The language identifier the server is told a document is in.
    pub language_id: &'static str,
}

/// Rust: the grammar, its highlights and rust-analyzer.
const RUST: Language = Language {
    name: "Rust",
    grammar: tree_sitter_rust::LANGUAGE,
    highlights: tree_sitter_rust::HIGHLIGHTS_QUERY,
    server: Some(Server {
        command: "rust-analyzer",
        arguments: &[],
        language_id: "rust",
    }),
};

/// JSON: the grammar and its highlights.
const JSON: Language = Language {
    name: "JSON",
    grammar: tree_sitter_json::LANGUAGE,
    highlights: tree_sitter_json::HIGHLIGHTS_QUERY,
    server: None,
};

/// TOML: the grammar and its highlights.
const TOML: Language = Language {
    name: "TOML",
    grammar: tree_sitter_toml_ng::LANGUAGE,
    highlights: tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
    server: None,
};

/// Markdown: the block grammar and the highlights of its blocks.
const MARKDOWN: Language = Language {
    name: "Markdown",
    grammar: tree_sitter_md::LANGUAGE,
    highlights: tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
    server: None,
};

impl Debug for Language {
    /// Writes the language's name, the grammar behind it being a pointer.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name)
    }
}

impl Language {
    /// The language a file at `path` is written in, judged by its extension.
    pub fn of(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?;
        match extension {
            "rs" => Some(RUST),
            "json" => Some(JSON),
            "toml" => Some(TOML),
            "md" | "markdown" => Some(MARKDOWN),
            _ => None,
        }
    }

    /// The name the status bar shows.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// The grammar the syntax tree is parsed with.
    pub fn grammar(self) -> Grammar {
        Grammar::new(self.grammar)
    }

    /// The query the highlights are captured by.
    pub const fn highlights(self) -> &'static str {
        self.highlights
    }

    /// The language server to run for this language, when there is one.
    pub const fn server(self) -> Option<Server> {
        self.server
    }
}
