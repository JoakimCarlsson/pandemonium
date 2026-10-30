//! The line patterns a language's `config.toml` indents by, besides what its syntax tree says.

use regex::Regex;

/// A regular expression, equal to another when it is written the same.
#[derive(Clone, Debug)]
pub struct Pattern(Regex);

impl Pattern {
    /// The pattern written `source`, or nothing when it is not a regular expression.
    pub fn new(source: &str) -> Option<Self> {
        Regex::new(source).ok().map(Self)
    }

    /// Whether `line` matches.
    pub fn matches(&self, line: &str) -> bool {
        self.0.is_match(line)
    }
}

impl PartialEq for Pattern {
    /// Two patterns are the same when their sources are.
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for Pattern {}

/// A pattern that takes a line back to the level of an earlier construct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Aligned {
    /// What the line is matched against.
    pub pattern: Pattern,
    /// The names of the `@start.<name>` captures of `indents.scm` the line lines up with; the line goes back one level when there are none.
    pub valid_after: Vec<String>,
}

/// What a language's indentation says of the text of a line, besides its syntax.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IndentRules {
    /// A line that matches indents the line after it.
    pub increase: Option<Pattern>,
    /// A line that starts matching goes back one level.
    pub decrease: Option<Pattern>,
    /// Lines that line up with an earlier construct, the first that matches deciding.
    pub aligned: Vec<Aligned>,
}

impl IndentRules {
    /// Whether the line `line` is one the line after it is indented below.
    pub fn increases(&self, line: &str) -> bool {
        self.increase
            .as_ref()
            .is_some_and(|pattern| pattern.matches(line))
    }

    /// Whether the line `line` goes back one level wherever it is.
    pub fn decreases(&self, line: &str) -> bool {
        self.decrease
            .as_ref()
            .is_some_and(|pattern| pattern.matches(line))
    }

    /// The rule that lines `line` up with an earlier construct, when one matches.
    pub fn aligning(&self, line: &str) -> Option<&Aligned> {
        self.aligned.iter().find(|rule| rule.pattern.matches(line))
    }

    /// Whether the language has no rules of its own.
    pub fn is_empty(&self) -> bool {
        self.increase.is_none() && self.decrease.is_none() && self.aligned.is_empty()
    }
}
