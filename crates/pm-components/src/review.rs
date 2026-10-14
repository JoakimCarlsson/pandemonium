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
use crate::sample::Session;
use crate::state::{
    COMMIT, COMMIT_MESSAGE, REVIEW, REVIEW_CHANGES, REVIEW_FILES, ReviewTab, STAGE_ALL,
    TREE_SCROLL, WorkspaceState,
};
use crate::{Button, Input};
use crate::{Checkbox, Tally};
use crate::{change_tree, file_tree};

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

/// The box that holds the rows of the panel's list and scrolls them.
fn scrolling_list(state: &WorkspaceState) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            pm_style::flex_1(),
            min_h(0),
            overflow_hidden(),
            pm_style::py(1),
        ])
        .scroll_from_start(TREE_SCROLL, state.tree_scroll)
}

/// A line telling the list has nothing to show.
fn nothing(message: &str) -> Node {
    node()
        .styles([px(4), pm_style::py(3)])
        .child(line(message, 13, 1.4, Role::Subtle))
}

/// One of the two tabs at the top of the panel, filled while it is the one shown.
fn panel_tab(label: &str, id: FocusId, shown: bool) -> Node {
    let mut styles = vec![
        flex(),
        items_center(),
        h(8),
        px(3),
        rounded(Radius::Lg),
        cursor_pointer(),
    ];
    let tone = if shown {
        styles.push(bg(Role::SurfaceSelected));
        Role::Text
    } else {
        styles.push(bg(Role::SurfaceHover).hover());
        Role::Muted
    };
    node()
        .styles(styles)
        .child(line(label, 13, 1.4, tone))
        .focusable(id)
}

/// Every file of the project, as a tree.
fn files(state: &WorkspaceState) -> Node {
    let mut list = scrolling_list(state).children(file_tree::rows(state));
    if state.files.is_empty() {
        list = list.child(nothing("No files yet."));
    }
    node()
        .styles([flex(), flex_col(), pm_style::flex_1(), min_h(0)])
        .child(list)
}

/// The changes of `session` and the commits made from it.
fn changes(state: &WorkspaceState, session: &Session) -> Node {
    let staged = session
        .changes
        .iter()
        .filter(|change| change.staged)
        .count();
    let all = !session.changes.is_empty() && staged == session.changes.len();
    let mut list = scrolling_list(state).children(change_tree::rows(state, session));
    if session.changes.is_empty() {
        list = list.child(nothing("No uncommitted changes."));
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
                .child(
                    Checkbox::new(all, STAGE_ALL)
                        .mixed(staged > 0 && !all)
                        .build(),
                )
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
                .child(panel_tab(
                    "Changes",
                    REVIEW_CHANGES,
                    state.review_tab == ReviewTab::Changes,
                ))
                .child(panel_tab(
                    "Files",
                    REVIEW_FILES,
                    state.review_tab == ReviewTab::Files,
                ))
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
        .child(match state.review_tab {
            ReviewTab::Changes => changes(state, session),
            ReviewTab::Files => files(state),
        })
}
