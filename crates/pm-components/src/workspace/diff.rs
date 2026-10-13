//! The diff view: every changed file stacked, the open one showing its lines.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, bg, border, border_b, border_color, cursor_pointer, flex, flex_1,
    flex_col, flex_none, font_medium, font_mono, gap, h, items_center, leading_ratio, min_h,
    min_w_px, mt, mx, overflow_hidden, px, rounded, text_color, text_px, w_full,
};
use pm_ui::{Node, node, text};

use super::chat::tally;
use super::sample::{Change, Kind, Session};
use super::state::{FILE_BASE, VIEWED_BASE, WorkspaceState};
use crate::parts::{check_box, glyph, line, spacer};

/// One line of a diff: tinted by what it does, written in the code font.
fn diff_line(kind: Kind, content: &str) -> Node {
    let (marker, fill, tone) = match kind {
        Kind::Hunk => (" ", Some(Color::from(Role::Surface)), Role::Subtle),
        Kind::Context => (" ", None, Role::Muted),
        Kind::Add => (
            "+",
            Some(Color::from(Role::Success).alpha(0.12)),
            Role::Text,
        ),
        Kind::Remove => ("-", Some(Color::from(Role::Danger).alpha(0.12)), Role::Text),
    };
    let marker = if kind == Kind::Hunk { "" } else { marker };
    let mut row = node().styles([flex(), w_full(), h(5), px(3), items_center()]);
    if let Some(fill) = fill {
        row = row.styles([bg(fill)]);
    }
    row.child(
        text(format!("{marker}{}", content.replace('\t', "    "))).styles([
            text_px(12),
            leading_ratio(1.4),
            font_mono(),
            text_color(tone),
        ]),
    )
}

/// One changed file: its header, and, while it is the open one, its diff.
fn file(index: usize, change: &Change, open: bool) -> Node {
    let mut header = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(10),
            px(3),
            bg(Role::Surface),
        ])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(2),
                    flex_1(),
                    min_w_px(0),
                    overflow_hidden(),
                    cursor_pointer(),
                ])
                .focusable(FocusId(FILE_BASE + index as u64))
                .child(glyph(
                    if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    },
                    3.5,
                    Role::Muted,
                ))
                .child(glyph(IconName::FileCode, 3.5, Role::Subtle))
                .child(
                    line(change.name(), 13, 1.4, Role::Text).styles([font_medium(), flex_none()]),
                )
                .child(text(change.directory().to_owned()).styles([
                    text_px(11),
                    leading_ratio(1.4),
                    font_mono(),
                    text_color(Role::Subtle),
                    pm_style::truncate(),
                ])),
        )
        .child(tally(change.added(), change.removed(), 11))
        .child(check_box(
            change.viewed,
            FocusId(VIEWED_BASE + index as u64),
        ));
    if open {
        header = header.styles([border_b(), border_color(Role::Border)]);
    }
    let mut section = node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            mx(4),
            mt(4),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            overflow_hidden(),
        ])
        .child(header);
    if open {
        section = section.children(
            change
                .diff
                .iter()
                .map(|line| diff_line(line.kind, &line.text)),
        );
    }
    section
}

/// The diff of `session`: a summary strip, then each changed file.
pub(super) fn diff(state: &WorkspaceState, session: &Session) -> Node {
    let viewed = session
        .changes
        .iter()
        .filter(|change| change.viewed)
        .count();
    let count = session.changes.len();
    let mut body = node()
        .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
        .children(
            session
                .changes
                .iter()
                .enumerate()
                .map(|(index, change)| file(index, change, index == state.file.unwrap_or(0))),
        );
    if session.changes.is_empty() {
        body = body.child(node().styles([px(4), pm_style::py(4)]).child(line(
            "No uncommitted changes.",
            13,
            1.4,
            Role::Subtle,
        )));
    }
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(3),
                    h(10),
                    px(4),
                    flex_none(),
                    border_b(),
                    border_color(Role::Border),
                ])
                .child(line(
                    format!("{count} file{} changed", if count == 1 { "" } else { "s" }),
                    12,
                    1.4,
                    Role::Muted,
                ))
                .child(tally(session.added(), session.removed(), 12))
                .child(spacer())
                .child(line(
                    format!("{viewed} of {count} viewed"),
                    12,
                    1.4,
                    Role::Subtle,
                )),
        )
        .child(body)
}
