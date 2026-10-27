//! What the style abbreviations know about properties, units and keywords, read from the tables of the language.

use crate::emmet::data::Style;

impl Style {
    /// The property a short name stands for.
    pub(super) fn alias(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
    }

    /// Whether `name` is a property that a short name stands for, written in full.
    pub(super) fn is_property(&self, name: &str) -> bool {
        self.properties.values().any(|property| property == name)
    }

    /// The property `name` is, as a short name or in full.
    pub(super) fn property(&self, name: &str) -> Option<&str> {
        self.alias(name).or_else(|| {
            self.properties
                .values()
                .map(String::as_str)
                .find(|property| *property == name)
        })
    }

    /// The keyword a short `word` stands for in `property`, with `a` meaning `auto` wherever nothing else claims it.
    pub(super) fn keyword_alias(&self, property: &str, word: &str) -> Option<&str> {
        let specific = self
            .keywords
            .iter()
            .find(|keywords| keywords.properties.iter().any(|held| held == property))
            .and_then(|keywords| keywords.words.get(word))
            .map(String::as_str);
        specific.or((word == "a").then_some("auto"))
    }

    /// The keyword `word` stands for in `property`, or `word` itself.
    pub(super) fn keyword<'a>(&'a self, property: &str, word: &'a str) -> &'a str {
        self.keyword_alias(property, word).unwrap_or(word)
    }

    /// The unit `letters` write after a number in `property`, nothing when they are not a unit there.
    pub(super) fn unit(&self, property: &str, letters: &str) -> Option<&str> {
        let timed = self
            .timed
            .iter()
            .any(|start| property.starts_with(start.as_str()));
        self.units
            .get(letters)
            .or_else(|| timed.then(|| self.durations.get(letters)).flatten())
            .map(String::as_str)
    }

    /// Whether a bare number in `property` is written without a unit.
    pub(super) fn is_unitless(&self, property: &str) -> bool {
        self.unitless.iter().any(|held| held == property)
    }
}
