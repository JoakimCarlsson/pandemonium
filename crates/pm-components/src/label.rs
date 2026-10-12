//! The label: a line of text and how it is styled.

use pm_style::{Style, Utility};

/// A line of text and the style it is drawn with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Label {
    /// The words shown.
    text: String,
    /// How the label is laid out, filled and typeset.
    style: Style,
}

/// A label showing `text`, with no style yet.
pub fn label(text: impl Into<String>) -> Label {
    Label {
        text: text.into(),
        style: Style::default(),
    }
}

impl Label {
    /// Applies utilities in order, after whatever was applied before.
    pub fn styles(mut self, utilities: impl IntoIterator<Item = Utility>) -> Self {
        self.style.extend(utilities);
        self
    }

    /// The words shown.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The style the label is drawn with.
    pub fn style(&self) -> &Style {
        &self.style
    }
}
