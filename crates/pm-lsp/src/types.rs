//! What a language server says about a file, as clients are told it.

use pm_buffer::{Position, Span};
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
    /// What its documentation says, in markdown, when the server sent it with the list.
    #[serde(default)]
    pub documentation: Option<String>,
    /// The edits choosing it makes besides writing its text, such as the line that imports it, as the server sent them with the list or when it resolved the item.
    #[serde(default)]
    pub additional_edits: Vec<TextChange>,
    /// The item as its server wrote it, to hand back to resolve what the list left out, while the server has more to say about it.
    #[serde(default)]
    pub resolve: Option<String>,
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

/// Where something is defined, as a client is told it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Location {
    /// The file: from the root of the project when it is inside it, and absolute otherwise.
    pub path: String,
    /// The stretch of the file it names.
    pub span: Span,
    /// Whether the file is inside the project, so that it is read and edited like any other of its files.
    #[serde(default)]
    pub inside: bool,
    /// What a file outside the project holds, which the client cannot read through the project, to be shown read only.
    #[serde(default)]
    pub text: Option<String>,
    /// The installed language a file outside the project is written in, when one is.
    #[serde(default)]
    pub language: Option<String>,
    /// What the place is called, as a function calling or called is named.
    #[serde(default)]
    pub name: Option<String>,
    /// The line the stretch starts on, as the file holds it, to list the place by.
    #[serde(default)]
    pub line: Option<String>,
}

/// Something a language server says is wrong with a file of a project.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Problem {
    /// The file, from the root of the project.
    pub path: String,
    /// What is wrong, and where.
    pub diagnostic: Diagnostic,
}

/// What is looked for from a place in a file.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Seek {
    /// Where the thing is defined.
    #[default]
    Definition,
    /// Where the thing is declared.
    Declaration,
    /// Where the type of the thing is defined.
    TypeDefinition,
    /// Where the thing is implemented.
    Implementation,
    /// Every place the thing is used.
    References,
    /// The functions that call the function.
    IncomingCalls,
    /// The functions the function calls.
    OutgoingCalls,
    /// The types the type derives from.
    Supertypes,
    /// The types that derive from the type.
    Subtypes,
}

impl Seek {
    /// What a list of the places found is titled.
    pub fn title(self) -> &'static str {
        match self {
            Self::Definition => "Definitions",
            Self::Declaration => "Declarations",
            Self::TypeDefinition => "Type definitions",
            Self::Implementation => "Implementations",
            Self::References => "References",
            Self::IncomingCalls => "Incoming calls",
            Self::OutgoingCalls => "Outgoing calls",
            Self::Supertypes => "Supertypes",
            Self::Subtypes => "Subtypes",
        }
    }
}

/// The characters after which a language server says something without being asked.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Triggers {
    /// Those after which it offers completions.
    pub completion: Vec<String>,
    /// Those after which it tells what the call the caret is in looks like.
    pub signature: Vec<String>,
    /// Those after which it formats what was just typed.
    #[serde(default)]
    pub format: Vec<String>,
    /// Whether it formats a stretch of a file.
    #[serde(default)]
    pub formats_ranges: bool,
    /// Whether it says which blocks of a file can be folded.
    #[serde(default)]
    pub folds: bool,
    /// Whether it says which stretches of a file are edited together.
    #[serde(default)]
    pub links: bool,
    /// Whether it says what each name of a file is, so that the names can be coloured by it.
    #[serde(default)]
    pub semantic_tokens: bool,
    /// Whether it says what to show inside a line, such as the type of a name or the name of a parameter.
    #[serde(default)]
    pub inlay_hints: bool,
    /// Whether it says what to show above a line, such as how many times a function is used.
    #[serde(default)]
    pub code_lens: bool,
    /// Whether it says which names of a file to show the values of while a program is paused.
    #[serde(default)]
    pub inline_values: bool,
    /// Whether it says which stretches of a file lead somewhere when pressed, such as web addresses and paths.
    #[serde(default)]
    pub document_links: bool,
    /// Whether it says which stretches of a file spell a colour.
    #[serde(default)]
    pub colors: bool,
    /// Whether it says what each name of a stretch of a file is, without the rest of the file.
    #[serde(default)]
    pub semantic_ranges: bool,
}

/// What a language server says a stretch of a file shows while the program being debugged is paused.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "text", rename_all = "snake_case")]
pub enum InlineValueKind {
    /// Text to show as it is.
    Text(String),
    /// The value of the variable of this name, or of the one the stretch spells when no name is given.
    Variable(Option<String>),
    /// What this expression comes to, or the stretch when no expression is given.
    Expression(Option<String>),
}

/// A stretch of a file whose value is shown at the end of its line while the program being debugged is paused.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InlineValue {
    /// The stretch.
    pub span: pm_buffer::Span,
    /// What it shows.
    pub kind: InlineValueKind,
}

/// What a name of a file is, as a language server says and as it is coloured.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    /// A keyword or a modifier.
    Keyword,
    /// A function, a method or a macro.
    Function,
    /// A type, a namespace or a trait.
    Type,
    /// A string or a regular expression.
    String,
    /// A number, a constant or a member of an enumeration.
    Constant,
    /// A comment.
    Comment,
    /// A field or a property.
    Property,
    /// An operator.
    Operator,
    /// A variable or a parameter.
    #[default]
    Variable,
}

/// A stretch of one line that a language server says is a name of a kind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticToken {
    /// The line, counted from 0.
    pub line: u32,
    /// Where it starts in the line, in UTF-16 code units.
    pub start: u32,
    /// How long it is, in UTF-16 code units.
    pub length: u32,
    /// What it is.
    pub kind: TokenKind,
}

/// What an inlay hint tells about the place it stands at.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HintKind {
    /// The type of the name before it.
    Type,
    /// The name of the parameter the value after it is passed as.
    Parameter,
    /// Anything else.
    #[default]
    Other,
    /// What a name holds while the program being debugged is paused.
    Value,
}

/// Text a language server says to show inside a line, at a place, that is not part of the file.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct InlayHint {
    /// The place it stands at.
    pub position: pm_buffer::Position,
    /// What it shows.
    pub label: String,
    /// What it tells.
    #[serde(default)]
    pub kind: HintKind,
    /// Whether a space stands before it.
    #[serde(default)]
    pub pad_left: bool,
    /// Whether a space stands after it.
    #[serde(default)]
    pub pad_right: bool,
}

/// Where a link of a file leads.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkTarget {
    /// A web address, to open in the browser.
    Url {
        /// The address.
        address: String,
    },
    /// A file of the project, to open.
    File {
        /// The file, from the root of the project.
        path: String,
        /// The place in it the link names, when it does.
        #[serde(default)]
        position: Option<Position>,
    },
    /// A link the server gave without saying where it leads, which it says when asked: the link as the server wrote it, to hand back.
    Unresolved {
        /// The link as its server wrote it.
        link: String,
    },
}

/// A stretch of a file a language server says leads somewhere when pressed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DocumentLink {
    /// The stretch.
    pub span: Span,
    /// Where it leads.
    pub target: LinkTarget,
}

/// A stretch of a file a language server says spells a colour.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ColorSwatch {
    /// The stretch.
    pub span: Span,
    /// How much red, from 0 to 255.
    pub red: u8,
    /// How much green, from 0 to 255.
    pub green: u8,
    /// How much blue, from 0 to 255.
    pub blue: u8,
    /// How opaque, from 0 to 255.
    pub alpha: u8,
}

/// One way a colour can be written in a file, as a language server offers it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ColorPresentation {
    /// What it is called, as the language writes the colour.
    pub label: String,
    /// The stretch it replaces.
    pub span: Span,
    /// What it writes there.
    pub text: String,
    /// The other edits that writing it makes, such as an import.
    #[serde(default)]
    pub additional_edits: Vec<TextChange>,
}

/// A line of text a language server says to show with a stretch of a file, such as how many times a function is used.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodeLens {
    /// The stretch it is about.
    pub span: Span,
    /// What it says.
    pub title: String,
    /// Whether pressing it does something.
    #[serde(default)]
    pub pressable: bool,
}

/// A block of lines a language server says can be folded: the line it starts on and the line it ends on, counted from 0.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FoldRange {
    /// The first line.
    pub start: u32,
    /// The last line.
    pub end: u32,
}

/// What a language server says about the call the caret is in: the signature of what is called, and which of its parameters the caret is at.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Signature {
    /// The signature, as the language writes it.
    pub label: String,
    /// The bytes of `label` that name the parameter the caret is at, when the server says.
    #[serde(default)]
    pub active: Option<(usize, usize)>,
    /// What the documentation says of it, in markdown.
    #[serde(default)]
    pub documentation: Option<String>,
    /// How many other signatures there are, as for an overloaded function.
    #[serde(default)]
    pub others: usize,
}

/// What a language server says can be renamed at a place: the stretch of text that holds the name, and the name it starts the new one from, each when the server says.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenameTarget {
    /// The stretch that holds the name; nothing when the server leaves it to the word at the place.
    #[serde(default)]
    pub span: Option<Span>,
    /// The name as the server writes it, when it is not the text of the stretch.
    #[serde(default)]
    pub placeholder: Option<String>,
}

/// The changes to one file that an edit across a project makes.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileEdit {
    /// The file: from the root of the project when it is inside it, and absolute otherwise.
    pub path: String,
    /// The stretches replaced, each as the file is before any of them.
    pub changes: Vec<TextChange>,
}

/// A change a language server offers to make at a place, such as a fix for what is wrong there.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodeAction {
    /// What it does.
    pub title: String,
    /// What sort of change it is, such as `quickfix` or `refactor.extract`, when the server says.
    #[serde(default)]
    pub kind: Option<String>,
    /// Whether the server thinks it is the one to take.
    #[serde(default)]
    pub preferred: bool,
    /// Why it cannot be taken now, when it cannot.
    #[serde(default)]
    pub disabled: Option<String>,
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
