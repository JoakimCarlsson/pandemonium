//! What is true when a key is pressed, and the `when` clause that reads it.
//!
//! A [`Context`] is the window's state flattened to keys and values — which
//! pane has focus, what kind of item it holds, whether the project has
//! sessions. A [`When`] clause is the condition a binding carries; the
//! resolver only offers a binding whose clause holds.

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

/// The value a context key carries when it is set without one.
const TRUE: &str = "true";

/// What is true where the key was pressed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Context {
    /// Every key that is set, and the value it carries.
    values: BTreeMap<String, String>,
}

impl Context {
    /// An empty context, in which only bare flags are false.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `key` to `value`.
    pub fn set(&mut self, key: &str, value: &str) -> &mut Self {
        self.values.insert(key.to_owned(), value.to_owned());
        self
    }

    /// Sets `key` as a flag, present when `held` and absent otherwise.
    pub fn flag(&mut self, key: &str, held: bool) -> &mut Self {
        if held {
            self.set(key, TRUE);
        } else {
            self.values.remove(key);
        }
        self
    }

    /// The value `key` carries, if it is set at all.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }
}

/// The condition under which a binding applies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum When {
    /// The binding always applies.
    Always,
    /// The binding never applies.
    Never,
    /// The key is set to anything but `false`.
    Defined(String),
    /// The key carries this value.
    Equals(String, String),
    /// The inner clause does not hold.
    Not(Box<When>),
    /// Every clause holds.
    All(Vec<When>),
    /// At least one clause holds.
    Any(Vec<When>),
}

impl When {
    /// Whether the clause holds in `context`.
    pub fn evaluate(&self, context: &Context) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Defined(key) => context.get(key).is_some_and(|value| value != "false"),
            Self::Equals(key, value) => context.get(key) == Some(value.as_str()),
            Self::Not(clause) => !clause.evaluate(context),
            Self::All(clauses) => clauses.iter().all(|clause| clause.evaluate(context)),
            Self::Any(clauses) => clauses.iter().any(|clause| clause.evaluate(context)),
        }
    }

    /// The clause with `key` known to carry `value`, folded as far as that
    /// takes it.
    ///
    /// A keymap is written once for every platform and read on one, so the
    /// platform's own key is settled as the keymap is put in force: a
    /// binding for another platform settles to [`When::Never`] and is left
    /// out, and one for this platform loses the part that said so.
    pub fn settle(self, key: &str, value: &str) -> Self {
        match self {
            Self::Defined(named) if named == key => match value {
                "false" => Self::Never,
                _ => Self::Always,
            },
            Self::Equals(named, wanted) if named == key => match wanted == value {
                true => Self::Always,
                false => Self::Never,
            },
            Self::Not(clause) => match clause.settle(key, value) {
                Self::Always => Self::Never,
                Self::Never => Self::Always,
                clause => Self::Not(Box::new(clause)),
            },
            Self::All(clauses) => fold(clauses, key, value, Self::Always, Self::Never, Self::All),
            Self::Any(clauses) => fold(clauses, key, value, Self::Never, Self::Always, Self::Any),
            clause => clause,
        }
    }
}

/// `clauses` settled one by one and joined again by `combine`.
///
/// A clause that settles to `neutral` drops out, one that settles to
/// `decisive` decides the whole, and what is left collapses to its one
/// clause or to `neutral` when nothing is.
fn fold(
    clauses: Vec<When>,
    key: &str,
    value: &str,
    neutral: When,
    decisive: When,
    combine: fn(Vec<When>) -> When,
) -> When {
    let mut kept = Vec::new();
    for clause in clauses {
        match clause.settle(key, value) {
            settled if settled == decisive => return decisive,
            settled if settled == neutral => {}
            settled => kept.push(settled),
        }
    }
    match kept.len() {
        0 => neutral,
        _ => collapse(kept, combine),
    }
}

impl Display for When {
    /// Writes the clause the way a keymap spells it.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Always => formatter.write_str("true"),
            Self::Never => formatter.write_str("false"),
            Self::Defined(key) => formatter.write_str(key),
            Self::Equals(key, value) => write!(formatter, "{key} == {value}"),
            Self::Not(clause) => write!(formatter, "!({clause})"),
            Self::All(clauses) => write_joined(formatter, clauses, " && "),
            Self::Any(clauses) => write_joined(formatter, clauses, " || "),
        }
    }
}

/// Writes `clauses` in parentheses, joined by `separator`.
fn write_joined(formatter: &mut Formatter<'_>, clauses: &[When], separator: &str) -> fmt::Result {
    formatter.write_str("(")?;
    for (index, clause) in clauses.iter().enumerate() {
        if index > 0 {
            formatter.write_str(separator)?;
        }
        write!(formatter, "{clause}")?;
    }
    formatter.write_str(")")
}

impl FromStr for When {
    type Err = ParseWhenError;

    /// Reads a clause of keys joined by `&&`, `||`, `!`, `==` and `!=`.
    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let mut parser = Parser {
            rest: source.trim(),
        };
        let clause = parser.any()?;
        if parser.rest.is_empty() {
            Ok(clause)
        } else {
            Err(ParseWhenError::Trailing(parser.rest.to_owned()))
        }
    }
}

/// A recursive-descent reader over the text of a `when` clause.
struct Parser<'a> {
    /// What is left to read.
    rest: &'a str,
}

impl<'a> Parser<'a> {
    /// Reads clauses joined by `||`, the loosest binding operator.
    fn any(&mut self) -> Result<When, ParseWhenError> {
        let mut clauses = vec![self.all()?];
        while self.eat("||") {
            clauses.push(self.all()?);
        }
        Ok(collapse(clauses, When::Any))
    }

    /// Reads clauses joined by `&&`.
    fn all(&mut self) -> Result<When, ParseWhenError> {
        let mut clauses = vec![self.unary()?];
        while self.eat("&&") {
            clauses.push(self.unary()?);
        }
        Ok(collapse(clauses, When::All))
    }

    /// Reads a clause, negated by any number of leading `!`.
    fn unary(&mut self) -> Result<When, ParseWhenError> {
        if self.eat("!") {
            return Ok(When::Not(Box::new(self.unary()?)));
        }
        self.atom()
    }

    /// Reads a parenthesised clause, or a key and the value it is compared to.
    fn atom(&mut self) -> Result<When, ParseWhenError> {
        if self.eat("(") {
            let clause = self.any()?;
            if !self.eat(")") {
                return Err(ParseWhenError::Unclosed);
            }
            return Ok(clause);
        }

        let key = self.word()?;
        if self.eat("==") {
            return Ok(When::Equals(key, self.word()?));
        }
        if self.eat("!=") {
            return Ok(When::Not(Box::new(When::Equals(key, self.word()?))));
        }

        Ok(match key.as_str() {
            "true" => When::Always,
            "false" => When::Never,
            _ => When::Defined(key),
        })
    }

    /// Reads one bare word: a key, or the value a key is compared to.
    fn word(&mut self) -> Result<String, ParseWhenError> {
        self.skip_spaces();
        let end = self
            .rest
            .find(|character: char| !is_word(character))
            .unwrap_or(self.rest.len());
        if end == 0 {
            return Err(ParseWhenError::ExpectedKey(self.rest.to_owned()));
        }
        let (word, rest) = self.rest.split_at(end);
        self.rest = rest;
        Ok(word.to_owned())
    }

    /// Takes `token` off the front if it is there, saying whether it was.
    fn eat(&mut self, token: &str) -> bool {
        self.skip_spaces();
        match self.rest.strip_prefix(token) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    /// Steps over the whitespace in front of the next token.
    fn skip_spaces(&mut self) {
        self.rest = self.rest.trim_start();
    }
}

/// Whether `character` can appear in a key or a value.
fn is_word(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '.' | '-' | ':' | '/')
}

/// The one clause itself, or `combine` over all of them.
fn collapse(mut clauses: Vec<When>, combine: fn(Vec<When>) -> When) -> When {
    if clauses.len() == 1 {
        clauses.remove(0)
    } else {
        combine(clauses)
    }
}

/// Why a `when` clause could not be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseWhenError {
    /// A key was expected where this text is.
    ExpectedKey(String),
    /// A parenthesised clause was never closed.
    Unclosed,
    /// The clause ended, and this text came after it.
    Trailing(String),
}

impl Display for ParseWhenError {
    /// Says why the clause could not be read.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedKey(rest) => write!(formatter, "expected a context key at `{rest}`"),
            Self::Unclosed => formatter.write_str("unclosed parenthesis"),
            Self::Trailing(rest) => write!(formatter, "unexpected `{rest}`"),
        }
    }
}

impl std::error::Error for ParseWhenError {}

/// The context keys a `when` clause is written against.
///
/// The vocabulary lives here so that a clause in a keymap table and the window
/// that fills the context in spell the same key.
pub mod keys {
    /// What the focused pane holds: `file`, `diff`, `review`, `agent`,
    /// `debug`, `terminal`, `prompt` — an agent's prompt — `console` — the
    /// debug console — or `commit`.
    pub const PANE_KIND: &str = "pane.kind";
    /// Set while a project has the focus.
    pub const PROJECT_FOCUSED: &str = "project.focused";
    /// Set while the focused item belongs to a session.
    pub const SESSION_FOCUSED: &str = "session.focused";
    /// Set while a palette is open over everything else.
    pub const PALETTE_OPEN: &str = "palette.open";
    /// Set while the setup screen is up.
    pub const SETUP_OPEN: &str = "setup.open";
    /// Set while a file's text has the keyboard: not its search bar, not a
    /// box of text beside it.
    pub const EDITOR_FOCUSED: &str = "editor.focused";
    /// Set while any text has the keyboard: a file's, an agent's prompt or
    /// a commit message.
    pub const TEXT_FOCUSED: &str = "text.focused";
    /// Set while a pane's search bar has the keyboard.
    pub const SEARCH_FOCUSED: &str = "search.focused";
    /// Set while it is the search bar's replacement field that has it.
    pub const SEARCH_REPLACING: &str = "search.replacing";
    /// Set while the list of changes has the keyboard.
    pub const CHANGES_FOCUSED: &str = "changes.focused";
    /// The extension of the file the focused pane shows, without its dot.
    pub const FILE_EXTENSION: &str = "file.extension";
    /// The platform the editor runs on: `macos`, `linux` or `windows`.
    pub const OS: &str = "os";
    /// Set while the worktree in front is debugging a program.
    pub const DEBUG_ACTIVE: &str = "debug.active";
    /// Set while that program is paused.
    pub const DEBUG_STOPPED: &str = "debug.stopped";
}
