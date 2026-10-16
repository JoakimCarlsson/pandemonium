//! What a language server says about a file, as clients are told it.

use pm_buffer::Span;
use serde::{Deserialize, Serialize};

/// How bad a diagnostic is.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The file is wrong.
    #[default]
    Error,
    /// The file may be wrong.
    Warning,
    /// Something worth knowing.
    Information,
    /// A suggestion.
    Hint,
}

impl Severity {
    /// The severity a language server numbers `number`, an error when it gives none.
    pub(crate) fn of_number(number: Option<u64>) -> Self {
        match number {
            Some(2) => Self::Warning,
            Some(3) => Self::Information,
            Some(4) => Self::Hint,
            _ => Self::Error,
        }
    }
}

/// Something a language server says about a stretch of a file.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The stretch.
    pub span: Span,
    /// How bad it is.
    pub severity: Severity,
    /// What it says.
    pub message: String,
    /// What said it, such as the compiler or a linter, when the server says.
    #[serde(default)]
    pub source: Option<String>,
}

/// What a completion is, for the mark drawn beside it.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionKind {
    /// A function or method.
    Function,
    /// A variable, field or property.
    Variable,
    /// A type, class, interface, struct or enum.
    Type,
    /// A module or namespace.
    Module,
    /// A keyword of the language.
    Keyword,
    /// A constant or a member of an enum.
    Constant,
    /// A snippet of code.
    Snippet,
    /// Anything else.
    #[default]
    Other,
}

impl CompletionKind {
    /// The kind a language server numbers `number`.
    pub(crate) fn of_number(number: Option<u64>) -> Self {
        match number {
            Some(2..=4) => Self::Function,
            Some(5 | 6 | 10) => Self::Variable,
            Some(7 | 8 | 13 | 22 | 25) => Self::Type,
            Some(9) => Self::Module,
            Some(14) => Self::Keyword,
            Some(12 | 20 | 21) => Self::Constant,
            Some(15) => Self::Snippet,
            _ => Self::Other,
        }
    }
}

/// One thing that could be written at a place.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Completion {
    /// What the list shows.
    pub label: String,
    /// More about it, such as its type or signature.
    #[serde(default)]
    pub detail: Option<String>,
    /// What it is.
    #[serde(default)]
    pub kind: CompletionKind,
    /// What choosing it writes.
    pub text: String,
    /// The stretch choosing it replaces, when the server says; otherwise the word before the place.
    #[serde(default)]
    pub span: Option<Span>,
    /// What the word typed so far is matched against.
    pub filter: String,
    /// What it is sorted by.
    pub sort: String,
}

/// What a language server says about the thing at a place.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hover {
    /// What it says, in markdown.
    pub text: String,
    /// The stretch it is about, when the server says.
    #[serde(default)]
    pub span: Option<Span>,
}

/// One stretch of an open file replaced, as a client tells it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextChange {
    /// The stretch replaced, or nothing for the whole text.
    #[serde(default)]
    pub span: Option<Span>,
    /// What replaced it.
    pub text: String,
}
