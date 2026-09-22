//! A menu of commands, opened over the screen by whatever asked for it.
//!
//! The menu is a list of entries and nothing more: it has no idea what it is
//! a menu of, where it was opened from, or how it is dismissed. An entry
//! without a message is an entry that does not apply right now — it is shown
//! greyed rather than left out, so a menu keeps the same shape every time it
//! is opened.

use crate::div::{Div, h_flex, v_flex};
use crate::element::{Element, IntoElement};
use crate::overlay::beside;
use crate::style::Styled;
use crate::text::text;
use crate::theme::Theme;

/// Narrowest a menu is drawn, however short its entries are.
const MIN_WIDTH: f32 = 190.0;

/// What a line with a menu of its own carries at its right edge.
const ARROW: &str = "›";

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
    /// A menu of its own, opened from this line.
    Submenu {
        /// What the line is called.
        label: String,
        /// What opening it sends, which is what puts `expanded` the other way.
        message: Option<M>,
        /// Whether it is open right now.
        expanded: bool,
        /// The lines it opens onto.
        items: Vec<MenuItem<M>>,
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

/// A line called `label` that opens `items` beside it when it is expanded.
///
/// Whether it is open is the caller's to hold, as everything else a screen
/// shows is: the line asks to be opened by sending `message`, and is drawn
/// open the next frame because the caller said so.
pub fn menu_submenu<M>(
    label: impl Into<String>,
    message: Option<M>,
    expanded: bool,
    items: Vec<MenuItem<M>>,
) -> MenuItem<M> {
    MenuItem::Submenu {
        label: label.into(),
        message,
        expanded,
        items,
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
fn entry<M: Clone + 'static>(theme: &Theme, item: MenuItem<M>) -> Box<dyn Element<M>> {
    let (label, message, opening) = match item {
        MenuItem::Separator => return separator(theme).into_element(),
        MenuItem::Entry { label, message } => (label, message, None),
        MenuItem::Submenu {
            label,
            message,
            expanded,
            items,
        } => (label, message, Some((expanded, items))),
    };
    let color = match message {
        Some(_) => theme.colors.text,
        None => theme.colors.text_subtle,
    };

    let line = h_flex()
        .h_px(theme.size.row)
        .px(2)
        .gap(2)
        .items_center()
        .when_some(message, |entry, message| {
            entry
                .hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(message)
        })
        .when(opening.is_some(), Div::justify_between)
        .child(text(label).text_sm().font_light().color(color))
        .when(opening.is_some(), |line| {
            line.child(
                text(ARROW)
                    .text_sm()
                    .font_light()
                    .color(theme.colors.text_subtle),
            )
        });

    match opening {
        Some((true, items)) => beside(line, menu(theme, items)).into_element(),
        _ => line.into_element(),
    }
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
