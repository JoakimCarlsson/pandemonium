//! The review panel: the branch, every changed file with a check box, the commits and the commit box.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border_b, border_color, border_l, border_t, cursor_pointer, flex, flex_col,
    flex_none, font_mono, gap, h, h_full, items_center, leading_ratio, min_h, overflow_hidden, p,
    px, rounded, text_color, text_px, w_full, w_px,
};
use pm_ui::{Node, node, text};

use crate::button::{Size, Variant};
use crate::parts::{clipped, glyph, line, spacer};
use crate::sample::{Change, Session};
use crate::state::{
    COMMIT, COMMIT_MESSAGE, FILE_BASE, REVIEW, STAGE_ALL, STAGE_BASE, WorkspaceState,
};
use crate::{Button, Input};
use crate::{Checkbox, Tally};

/// How wide the review panel is, in logical pixels.
pub const WIDTH: f32 = 380.0;

/// Pushes the commits made so far.
const PUSH: FocusId = FocusId(319);

/// A row of the panel `height` steps tall, with a rule beneath it.
fn band(height: u32) -> Node {
    node().styles([
        flex(),
        items_center(),
        gap(2),
        h(height),
        px(3),
        flex_none(),
        border_b(),
        border_color(Role::Border),
    ])
}

/// The row of one changed file: whether it is staged, its name and where it is, and what it changed.
fn change_row(index: usize, change: &Change, selected: bool) -> Node {
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(8),
        px(4),
        cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ];
    if selected {
        styles.push(bg(Role::SurfaceSelected));
    }
    let tone = if change.viewed {
        Role::Subtle
    } else {
        Role::Text
    };
    node()
        .styles(styles)
        .child(Checkbox::new(change.staged, FocusId(STAGE_BASE + index as u64)).build())
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(2),
                    pm_style::flex_1(),
                    pm_style::min_w_px(0),
                    overflow_hidden(),
                ])
                .focusable(FocusId(FILE_BASE + index as u64))
                .child(glyph(IconName::FileCode, 3.5, Role::Subtle))
                .child(line(change.name(), 13, 1.4, tone))
                .child(text(change.directory().to_owned()).styles([
                    text_px(11),
                    leading_ratio(1.4),
                    font_mono(),
                    text_color(Role::Subtle),
                    pm_style::truncate(),
                ])),
        )
        .child(
            Tally::new(change.added(), change.removed())
                .size(11)
                .build(),
        )
}

/// The changes of `session` and the commits made from it.
fn changes(state: &WorkspaceState, session: &Session) -> Node {
    let staged = session
        .changes
        .iter()
        .filter(|change| change.staged)
        .count();
    let all = !session.changes.is_empty() && staged == session.changes.len();
    let mut list = node()
        .styles([
            flex(),
            flex_col(),
            pm_style::flex_1(),
            min_h(0),
            overflow_hidden(),
            pm_style::py(1),
        ])
        .children(
            session
                .changes
                .iter()
                .enumerate()
                .map(|(index, change)| change_row(index, change, state.file == Some(index))),
        );
    if session.changes.is_empty() {
        list = list.child(node().styles([px(4), pm_style::py(3)]).child(line(
            "No uncommitted changes.",
            13,
            1.4,
            Role::Subtle,
        )));
    }
    let mut commits = node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            border_t(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([flex(), items_center(), gap(2), h(9), px(4)])
                .child(line("Commits", 12, 1.4, Role::Muted))
                .child(text(session.commits.len().to_string()).styles([
                    text_px(12),
                    leading_ratio(1.4),
                    font_mono(),
                    text_color(Role::Subtle),
                ])),
        );
    commits = commits.children(session.commits.iter().rev().take(3).map(|message| {
        node()
            .styles([flex(), items_center(), gap(2), h(8), px(4)])
            .child(clipped(message, 12, Role::Text))
            .child(line("not pushed", 11, 1.4, Role::Subtle))
    }));
    let ready = staged > 0 && !state.message.text().trim().is_empty();
    node()
        .styles([flex(), flex_col(), pm_style::flex_1(), min_h(0)])
        .child(
            band(10)
                .styles([px(4), gap(3)])
                .child(Checkbox::new(all, STAGE_ALL).build())
                .child(line("Changes", 12, 1.4, Role::Muted))
                .child(
                    Tally::new(session.added(), session.removed())
                        .size(12)
                        .build(),
                )
                .child(spacer())
                .child(line(
                    format!("{staged} of {} staged", session.changes.len()),
                    12,
                    1.4,
                    Role::Subtle,
                )),
        )
        .child(list)
        .child(commits)
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    gap(2),
                    p(3),
                    flex_none(),
                    border_t(),
                    border_color(Role::Border),
                ])
                .child(
                    Input::new(&state.message, COMMIT_MESSAGE)
                        .placeholder("Commit message")
                        .build(),
                )
                .child(
                    Button::new(
                        if staged > 0 {
                            format!("Commit {staged} file{}", if staged == 1 { "" } else { "s" })
                        } else {
                            "Commit".to_owned()
                        },
                        COMMIT,
                    )
                    .disabled(!ready)
                    .build()
                    .styles([w_full()]),
                ),
        )
}

/// The review panel for `session`.
pub fn review(state: &WorkspaceState, session: &Session) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            w_px(WIDTH),
            h_full(),
            bg(Role::Background),
            border_l(),
            border_color(Role::Border),
        ])
        .child(
            band(12)
                .styles([px(3)])
                .child(
                    node()
                        .styles([
                            flex(),
                            items_center(),
                            h(8),
                            px(3),
                            rounded(Radius::Lg),
                            bg(Role::SurfaceSelected),
                        ])
                        .child(line("Changes", 13, 1.4, Role::Text)),
                )
                .child(spacer())
                .child(
                    Button::icon(IconName::PanelRight, REVIEW)
                        .build()
                        .styles([text_color(Role::Text)]),
                ),
        )
        .child(
            band(10)
                .child(glyph(IconName::GitFork, 3.5, Role::Muted))
                .child(if session.branch.is_empty() {
                    clipped("Not a Git repository", 12, Role::Subtle)
                } else {
                    clipped(&session.branch, 12, Role::Text).styles([font_mono()])
                })
                .child(
                    Button::new("Push", PUSH)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .with_icon(IconName::ArrowUp)
                        .disabled(true)
                        .build(),
                ),
        )
        .child(changes(state, session))
}
