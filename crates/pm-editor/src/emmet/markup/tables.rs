//! What the markup abbreviations know about tag names, read from the tables of the language.

use crate::emmet::data::Markup;

/// Attributes a tag is written with, a value of `None` leaving a tab stop to fill it in.
pub(super) type Defaults<'a> = Vec<(&'a str, Option<&'a str>)>;

impl Markup {
    /// Whether `tag` is written without a closing tag.
    pub(super) fn is_void(&self, tag: &str) -> bool {
        self.void.iter().any(|held| held == tag)
    }

    /// Whether `tag` stays in the line of its siblings.
    pub(super) fn is_inline(&self, tag: &str) -> bool {
        self.inline.iter().any(|held| held == tag)
    }

    /// The tag a short `name` stands for, or `name` itself.
    pub(super) fn resolve<'a>(&'a self, name: &'a str) -> &'a str {
        self.aliases.get(name).map_or(name, String::as_str)
    }

    /// The tag of an element written without a name inside `parent`.
    pub(super) fn implicit_name(&self, parent: &str) -> &str {
        if let Some(tag) = self.implicit_children.get(parent) {
            return tag;
        }
        if self.inline_parents.iter().any(|held| held == parent) {
            return &self.inline_tag;
        }
        &self.default_tag
    }

    /// The attributes `tag` is written with unless the abbreviation gives them.
    pub(super) fn defaults(&self, tag: &str) -> Defaults<'_> {
        self.defaults
            .get(tag)
            .map(|held| {
                held.iter()
                    .filter_map(|attribute| {
                        Some((
                            attribute.first()?.as_str(),
                            attribute.get(1).map(String::as_str),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
