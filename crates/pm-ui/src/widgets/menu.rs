//! A menu of commands, opened over the screen by whatever asked for it.
//!
//! The menu is a list of entries and nothing more: it has no idea what it is
//! a menu of, where it was opened from, or how it is dismissed. An entry
//! without a message is an entry that does not apply right now — it is shown
//! greyed rather than left out, so a menu keeps the same shape every time it
//! is opened.

use crate::div::{Div, h_flex, v_flex};
use crate::style::Styled;
use crate::text::text;
use crate::theme::Theme;

/// Narrowest a menu is drawn, however short its entries are.
const MIN_WIDTH: f32 = 190.0;

/// Height of the line between two groups of entries, gaps included.
const SEPARATOR_HEIGHT: f32 = 9.0;

/// One line of a menu.
pub enum MenuItem<M> {
    /// A command, which is greyed out when it carries no message.
    Entry {
        /// What the entry is called.
        label: String,
        /// What choosing it sends, or nothing when it does not apply.
        message: Option<M>,
    },
    /// A line between two groups of commands.
    Separator,
}

/// An entry called `label` that sends `message`, greyed out without one.
pub fn menu_entry<M>(label: impl Into<String>, message: Option<M>) -> MenuItem<M> {
    MenuItem::Entry {
        label: label.into(),
        message,
    }
}

/// A line between two groups of entries.
pub fn menu_separator<M>() -> MenuItem<M> {
    MenuItem::Separator
}

/// A menu of `items`, sized to its content.
pub fn menu<M: Clone + 'static>(theme: &Theme, items: Vec<MenuItem<M>>) -> Div<M> {
    v_flex()
        .py(0.5)
        .w_fit()
        .min_w_px(MIN_WIDTH)
        .items_stretch()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .children(items.into_iter().map(|item| entry(theme, item)))
}

/// Builds one line of the menu.
fn entry<M: Clone + 'static>(theme: &Theme, item: MenuItem<M>) -> Div<M> {
    let (label, message) = match item {
        MenuItem::Separator => return separator(theme),
        MenuItem::Entry { label, message } => (label, message),
    };
    let color = match message {
        Some(_) => theme.colors.text,
        None => theme.colors.text_subtle,
    };

    h_flex()
        .h_px(theme.size.row)
        .px(2)
        .items_center()
        .when_some(message, |entry, message| {
            entry
                .hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(message)
        })
        .child(text(label).text_sm().font_light().color(color))
}

/// Builds the line between two groups of entries.
///
/// The line is stretched across the menu rather than told to be full width:
/// a child that asks for its parent's whole width is measured against
/// whatever the parent was offered, which for a menu is the window.
fn separator<M: Clone + 'static>(theme: &Theme) -> Div<M> {
    v_flex()
        .h_px(SEPARATOR_HEIGHT)
        .py(0.5)
        .w_fit()
        .items_stretch()
        .child(v_flex().h_px(1.0).bg(theme.colors.border_variant))
}
