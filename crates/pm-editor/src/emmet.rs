//! Emmet abbreviations: a short line such as `ul>li*3` written out as the markup or the style rules it stands for.

mod markup;
mod snippet;
mod style;

/// The kind of text an abbreviation is written for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dialect {
    /// Tags, written as HTML.
    Markup,
    /// Properties and values, written as style rules.
    Style,
}

/// The snippet `abbreviation` expands to in `dialect`, its lines indented with `indent` and with tab stops for the places left to fill in; nothing when it is not an abbreviation.
pub fn expand(abbreviation: &str, dialect: Dialect, indent: &str) -> Option<String> {
    if abbreviation.is_empty() || abbreviation.chars().any(char::is_control) {
        return None;
    }
    match dialect {
        Dialect::Markup => markup::expand(abbreviation, indent),
        Dialect::Style => style::expand(abbreviation),
    }
}
