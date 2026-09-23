//! The bar above a pane of text naming where in the worktree it is.
//!
//! A breadcrumb names two things, one after the other: where the file sits
//! in its worktree, and which declarations the cursor is inside. The second
//! half is read off the syntax tree along the one path from the root to the
//! cursor, so it costs a frame the depth of the file rather than its size,
//! and it follows the cursor as it moves. A declaration in the bar is a
//! place: pressing it goes there.

use std::path::Path;

use pm_text::{Buffer, Position};
use pm_ui::{Div, IconName, IconSize, Styled, Theme, h_flex, icon, text};

use crate::message::Message;
use crate::panes::PaneId;

/// The words a grammar's node kinds carry when they declare something the
/// reader would name: a function, a type, the block implementing one.
const DECLARING: &[&str] = &[
    "function",
    "method",
    "class",
    "struct",
    "impl",
    "trait",
    "enum",
    "interface",
    "module",
    "mod_item",
    "namespace",
    "union",
    "object",
    "protocol",
];

/// The words a node kind carries when it uses a declaration rather than
/// making one: a call is not where the cursor is.
const USING: &[&str] = &["call", "invocation", "parameter", "argument", "identifier"];

/// Most characters a declaration's name is drawn with before it is cut.
const LONGEST_NAME: usize = 40;

/// Where one pane of text is: its path in the worktree, and the declarations
/// the cursor is inside.
#[derive(Clone, Debug, Default)]
pub struct Crumbs {
    /// The directories down to the file, then the file itself.
    pub path: Vec<String>,
    /// The declarations the cursor is inside, outermost first, each with
    /// where it begins.
    pub symbols: Vec<(String, Position)>,
}

impl Crumbs {
    /// Where `buffer`, a file of the worktree at `root`, has its cursor.
    pub fn of(buffer: &Buffer, root: &Path) -> Self {
        let path = buffer
            .path()
            .strip_prefix(root)
            .unwrap_or(buffer.path())
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        let symbols = buffer
            .syntax_around(buffer.selection().head, &is_declaration)
            .into_iter()
            .filter_map(|node| Some((shortened(&node.name?), node.range.start)))
            .collect();

        Self { path, symbols }
    }
}

/// Whether a node of `kind` declares something a breadcrumb names.
fn is_declaration(kind: &str) -> bool {
    DECLARING.iter().any(|word| kind.contains(word))
        && !USING.iter().any(|word| kind.contains(word))
}

/// `name` on one line, cut short when it runs long.
fn shortened(name: &str) -> String {
    let flat = name.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.chars().count() > LONGEST_NAME {
        true => format!("{}…", flat.chars().take(LONGEST_NAME).collect::<String>()),
        false => flat,
    }
}

/// Builds the bar naming where the text of `pane` is.
///
/// The path is read and not pressed; the declarations are pressed, each
/// going to where it begins in `pane`.
pub fn crumb_bar(theme: &Theme, pane: PaneId, crumbs: &Crumbs) -> Div<Message> {
    let last = crumbs.path.len().saturating_sub(1);
    let path = crumbs.path.iter().enumerate().map(|(index, part)| {
        let color = match index == last {
            true => theme.colors.text_muted,
            false => theme.colors.text_subtle,
        };
        h_flex()
            .items_center()
            .gap(0.5)
            .when(index > 0, |crumb| crumb.child(separator(theme)))
            .child(text(part.clone()).text_xs().color(color))
    });
    let symbols = crumbs.symbols.iter().map(|(name, at)| {
        h_flex()
            .items_center()
            .gap(0.5)
            .child(separator(theme))
            .child(
                h_flex()
                    .px(0.5)
                    .rounded(theme.radius.sm)
                    .hover_bg(theme.colors.surface_hover)
                    .active_bg(theme.colors.surface_active)
                    .on_click(Message::JumpTo(pane, *at))
                    .child(
                        text(name.clone())
                            .text_xs()
                            .font_mono()
                            .color(theme.colors.text_muted),
                    ),
            )
    });

    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(1.5)
        .gap(0.5)
        .items_center()
        .overflow_hidden()
        .bg(theme.colors.background)
        .children(path)
        .children(symbols)
}

/// Builds the mark standing between one crumb and the next.
fn separator(theme: &Theme) -> pm_ui::Icon {
    icon(IconName::ChevronRight)
        .size(IconSize::XSmall)
        .color(theme.colors.text_subtle)
}
