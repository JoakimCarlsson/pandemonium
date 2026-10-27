//! What Emmet abbreviations know about a language: the tags of its markup and the properties of its style rules, as the language's folder says.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The tables of a language's `emmet.toml`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Tables {
    /// What markup abbreviations know.
    #[serde(default)]
    pub markup: Markup,
    /// What style abbreviations know.
    #[serde(default)]
    pub style: Style,
}

impl Tables {
    /// The tables written in `text`, or nothing when it is not a valid `emmet.toml`.
    pub fn parse(text: &str) -> Option<Self> {
        toml::from_str(text).ok()
    }
}

/// What markup abbreviations know about tag names.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Markup {
    /// What `!` stands for, where `{indent}` stands for one level of indentation.
    #[serde(default)]
    pub document: String,
    /// Tags written without a closing tag.
    #[serde(default)]
    pub void: Vec<String>,
    /// Tags that sit in a line of text instead of standing on a line of their own.
    #[serde(default)]
    pub inline: Vec<String>,
    /// Parents whose children are inline when no name is written.
    #[serde(default)]
    pub inline_parents: Vec<String>,
    /// What a plain word before the caret is taken to be an abbreviation for.
    #[serde(default)]
    pub words: Vec<String>,
    /// Short names and the tags they stand for.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    /// Tags whose children are named after them when no name is written, and the tag that is used.
    #[serde(default)]
    pub implicit_children: BTreeMap<String, String>,
    /// The tag of an element written without a name where nothing else says which, and inside an inline parent.
    #[serde(default)]
    pub default_tag: String,
    /// The tag inside an inline parent.
    #[serde(default)]
    pub inline_tag: String,
    /// Attributes a tag is written with: a name alone leaves a tab stop to fill it in, a name and a value gives one.
    #[serde(default)]
    pub defaults: BTreeMap<String, Vec<Vec<String>>>,
}

/// The short keywords of a group of properties.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Keywords {
    /// The properties the short names are for.
    #[serde(default)]
    pub properties: Vec<String>,
    /// Short names and the keywords they stand for.
    #[serde(default)]
    pub words: BTreeMap<String, String>,
}

/// What style abbreviations know about properties, units and keywords.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct Style {
    /// Short names and the properties they stand for.
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
    /// Properties whose bare numbers are written without a unit.
    #[serde(default)]
    pub unitless: Vec<String>,
    /// The unit a bare number is written with when its property takes one.
    #[serde(default)]
    pub default_unit: String,
    /// Units a number may be followed by, with the long form of the short ones.
    #[serde(default)]
    pub units: BTreeMap<String, String>,
    /// Units that only mean something for a duration.
    #[serde(default)]
    pub durations: BTreeMap<String, String>,
    /// What a property must start with to take durations.
    #[serde(default)]
    pub timed: Vec<String>,
    /// The short keywords of each group of properties.
    #[serde(default)]
    pub keywords: Vec<Keywords>,
}
