//! What a highlighted stretch of code is, as the capture names of a highlights query say.

/// What a stretch of code is, for choosing its colour.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    /// A word of the language: `fn`, `if`, `return`.
    Keyword,
    /// A function, method or macro, named or called.
    Function,
    /// A type, module or namespace.
    Type,
    /// A string, character or escape inside one.
    String,
    /// A number, boolean or named constant.
    Constant,
    /// A comment.
    Comment,
    /// A field, property or attribute.
    Property,
    /// An operator.
    Operator,
    /// Brackets, delimiters and other punctuation.
    Punctuation,
    /// A variable or parameter, drawn as plain text.
    Variable,
    /// A heading, emphasis or other markup of prose.
    Markup,
    /// A link or address.
    Link,
}

impl Kind {
    /// The kind a capture named `name` stands for, decided by the first part of the name, or nothing when it names none.
    pub fn of_capture(name: &str) -> Option<Self> {
        let (head, rest) = name.split_once('.').unwrap_or((name, ""));
        Some(match head {
            "keyword" | "conditional" | "repeat" | "include" | "exception" | "storageclass"
            | "tag" => Self::Keyword,
            "function" | "method" | "constructor" | "macro" => Self::Function,
            "type" | "namespace" | "module" => Self::Type,
            "string" | "character" | "escape" => Self::String,
            "number" | "float" | "boolean" | "constant" | "label" => Self::Constant,
            "comment" => Self::Comment,
            "property" | "field" | "attribute" => Self::Property,
            "operator" => Self::Operator,
            "punctuation" => Self::Punctuation,
            "variable" if rest.starts_with("builtin") => Self::Keyword,
            "variable" | "parameter" => Self::Variable,
            "markup" if rest.starts_with("link") => Self::Link,
            "markup" if rest.starts_with("raw") => Self::String,
            "markup" => Self::Markup,
            _ => return None,
        })
    }
}
