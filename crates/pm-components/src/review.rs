//! The source control panel: the branch, the commit message and its button, the changed files in groups, and the graph of commits.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, animate_spin, bg, border, border_b, border_color, border_l,
    cursor_pointer, flex, flex_1, flex_col, flex_none, font_medium, font_mono, gap, h, h_full,
    items_center, justify_center, min_h, min_w_px, overflow_hidden, pb, px, py, relative, rounded,
    w_full, w_px,
};
use pm_text::Shaper;
use pm_ui::{Node, node};

use crate::branch_menu::branch_menu;
use crate::button::Size;
use crate::git_controls::{
    BRANCH_BUTTON, CHANGES_MORE, CHANGES_REFRESH, CHANGES_TOGGLE, COMMIT_CHEVRON, Primary,
    TROUBLE_DISMISS,
};
use crate::graph::graph;
use crate::parts::{clipped, glyph, line, spacer, wrap};
use crate::sample::Session;
use crate::state::{
    COMMIT, COMMIT_MESSAGE, OPEN_DIFF, REVIEW, REVIEW_CHANGES, REVIEW_FILES, ReviewTab,
    TREE_SCROLL, WorkspaceState,
};
use crate::{Button, Input, input};
use crate::{change_list, file_tree};

/// How wide the review panel is, in logical pixels.
pub const WIDTH: f32 = 380.0;

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

/// The words and the icon on the button under the commit message, for what it does now.
fn primary_face(primary: &Primary) -> Node {
    let enabled = !matches!(
        primary,
        Primary::Commit {
            stopped: Some(_),
            ..
        } | Primary::Busy(_)
    );
    let tone = if enabled { Role::Text } else { Role::Subtle };
    let (icon, label) = match primary {
        Primary::Commit { title, stopped } => (IconName::Check, stopped.unwrap_or(title)),
        Primary::Sync { .. } => (IconName::Refresh, "Sync Changes"),
        Primary::Publish => (IconName::GitPush, "Publish Branch"),
        Primary::Busy(doing) => (IconName::Loading, *doing),
    };
    let mut face = node().styles([flex(), items_center(), gap(2)]).child(
        if matches!(primary, Primary::Busy(_)) {
            glyph(icon, 3.5, tone).styles([animate_spin()])
        } else {
            glyph(icon, 3.5, tone)
        },
    );
    face = face.child(line(label, 13, 1.4, tone).styles([font_medium()]));
    if let Primary::Sync { ahead, behind } = primary {
        for (count, arrow) in [(*behind, IconName::ArrowDown), (*ahead, IconName::ArrowUp)] {
            if count > 0 {
                face = face
                    .child(line(count.to_string(), 13, 1.4, tone))
                    .child(glyph(arrow, 3.0, tone));
            }
        }
    }
    face
}

/// The control that commits, or syncs or publishes once there is nothing to commit, with the arrow that offers what else it can do.
///
/// What stops it is written where the words would be, so a reader who cannot commit is told why.
fn commit_button(primary: &Primary) -> Node {
    let enabled = !matches!(
        primary,
        Primary::Commit {
            stopped: Some(_),
            ..
        } | Primary::Busy(_)
    );
    let mut main = node()
        .styles([flex(), items_center(), justify_center(), flex_1(), h_full()])
        .child(primary_face(primary));
    let mut arrow = node()
        .styles([
            flex(),
            items_center(),
            px(2),
            h_full(),
            border_l(),
            border_color(Role::Border),
        ])
        .child(glyph(
            IconName::ChevronDown,
            3.0,
            if enabled { Role::Text } else { Role::Subtle },
        ));
    let mut control = node().styles([
        flex(),
        items_center(),
        h(9),
        w_full(),
        rounded(Radius::Md),
        bg(Role::SurfaceSelected),
    ]);
    if enabled {
        main = main
            .styles([cursor_pointer(), bg(Role::SurfaceHover).hover()])
            .focusable(COMMIT);
        arrow = arrow
            .styles([cursor_pointer(), bg(Role::SurfaceHover).hover()])
            .focusable(COMMIT_CHEVRON);
    }
    control = control.child(main).child(arrow);
    node().styles([px(3), pb(2), flex_none()]).child(control)
}

/// The box the commit message is written in: several lines, wrapped to the panel.
fn message_box(state: &WorkspaceState, shaper: &mut Shaper) -> Node {
    node().styles([px(3), py(2), flex_none()]).child(
        node()
            .styles([
                relative(),
                flex(),
                flex_col(),
                w_full(),
                rounded(Radius::Lg),
                border(),
                border_color(Role::Border),
                border_color(Role::Accent).focus(),
                bg(Role::Background),
            ])
            .focused_with(COMMIT_MESSAGE)
            .child(
                Input::new(&state.message, COMMIT_MESSAGE)
                    .variant(input::Variant::Multiline)
                    .placeholder("Message (Ctrl+Enter to commit)")
                    .rows(4)
                    .wrap(input::inner_width(WIDTH - 24.0), shaper)
                    .build(),
            ),
    )
}

/// The heading of the changes section: a chevron that folds it, its name, and what can be done from it.
fn section_heading(state: &WorkspaceState, primary: &Primary) -> Node {
    let open = state.git.changes_open;
    let committable = matches!(primary, Primary::Commit { stopped: None, .. });
    node()
        .styles([flex(), items_center(), gap(1), h(8), px(2), flex_none()])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(1),
                    flex_1(),
                    h_full(),
                    cursor_pointer(),
                ])
                .child(glyph(
                    if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    },
                    3.5,
                    Role::Subtle,
                ))
                .child(line("Changes", 13, 1.4, Role::Text))
                .focusable(CHANGES_TOGGLE),
        )
        .child(
            Button::icon(IconName::GitCommit, COMMIT)
                .size(Size::Icon)
                .disabled(!committable)
                .build()
                .tooltip("Commit"),
        )
        .child(
            Button::icon(IconName::Refresh, CHANGES_REFRESH)
                .size(Size::Icon)
                .spinning(state.git.working)
                .build()
                .tooltip("Refresh"),
        )
        .child(
            Button::icon(IconName::More, CHANGES_MORE)
                .size(Size::Icon)
                .build()
                .tooltip("More Actions…"),
        )
}

/// The banner saying what git refused to do, with a button that dismisses it.
fn trouble(said: &str) -> Node {
    node().styles([px(3), pb(2), flex_none()]).child(
        node()
            .styles([
                flex(),
                gap(2),
                px(3),
                py(2),
                rounded(Radius::Md),
                bg(Color::from(Role::Danger).alpha(0.12)),
            ])
            .child(
                node()
                    .styles([flex(), flex_col(), flex_1(), min_w_px(0)])
                    .children(
                        wrap(said, 12, WIDTH - 24.0 - 56.0)
                            .iter()
                            .map(|words| line(words, 12, 1.5, Role::Danger)),
                    ),
            )
            .child(
                Button::icon(IconName::Close, TROUBLE_DISMISS)
                    .size(Size::Icon)
                    .build(),
            ),
    )
}

/// The changes of `session`: the commit message and its button, then the files in groups, and the graph under them.
fn changes(state: &WorkspaceState, session: &Session, shaper: &mut Shaper) -> Node {
    if !session.on_branch
        && session.branch.is_empty()
        && session.changes.is_empty()
        && state.git.history.is_empty()
    {
        return nothing("This folder is not a git repository.");
    }
    let primary = state.primary();
    let mut list = scrolling_list(state).children(change_list::rows(state, session));
    if session.changes.is_empty() {
        list = list.child(nothing("No uncommitted changes."));
    }
    let mut section = node()
        .styles([flex(), flex_col(), pm_style::flex_1(), min_h(0)])
        .child(section_heading(state, &primary))
        .children(state.git.trouble.as_deref().map(trouble));
    if state.git.changes_open {
        section = section
            .child(message_box(state, shaper))
            .child(commit_button(&primary))
            .child(list);
    } else {
        section = section.child(spacer());
    }
    node()
        .styles([flex(), flex_col(), pm_style::flex_1(), min_h(0)])
        .child(section)
        .child(graph(state))
}

/// The button that shows the branch `session` is on and opens the list of branches, or, outside a repository, only says so.
fn branch_button(session: &Session, working: bool) -> Node {
    if session.branch.is_empty() {
        return clipped("Not a Git repository", 12, Role::Subtle);
    }
    node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(8),
            px(2),
            pm_style::flex_1(),
            pm_style::min_w_px(0),
            rounded(Radius::Lg),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .child(glyph(IconName::GitFork, 3.5, Role::Muted))
        .child(clipped(&session.branch, 12, Role::Text).styles([font_mono()]))
        .child(if working {
            glyph(IconName::Loading, 3.5, Role::Muted).styles([animate_spin()])
        } else {
            glyph(IconName::ChevronDown, 3.5, Role::Muted)
        })
        .focusable(BRANCH_BUTTON)
}

/// The review panel for `session`.
pub fn review(state: &WorkspaceState, session: &Session, shaper: &mut Shaper) -> Node {
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
            relative(),
        ])
        .child(
            band(12)
                .styles([px(3)])
                .child(panel_tab(
                    "Source Control",
                    REVIEW_CHANGES,
                    state.review_tab == ReviewTab::Changes,
                ))
                .child(panel_tab(
                    "Files",
                    REVIEW_FILES,
                    state.review_tab == ReviewTab::Files,
                ))
                .child(spacer())
                .children(
                    state
                        .git
                        .operation
                        .as_ref()
                        .filter(|_| state.review_tab == ReviewTab::Changes)
                        .map(|waiting| line(waiting.label(), 12, 1.4, Role::Muted)),
                )
                .children(
                    (state.review_tab == ReviewTab::Changes && !session.changes.is_empty()).then(
                        || {
                            Button::icon(IconName::GitCompare, OPEN_DIFF)
                                .build()
                                .tooltip("Open Changes")
                        },
                    ),
                )
                .child(Button::icon(IconName::PanelRight, REVIEW).build()),
        )
        .child(band(10).child(branch_button(session, state.git.working)))
        .child(match state.review_tab {
            ReviewTab::Changes => changes(state, session, shaper),
            ReviewTab::Files => files(state),
        })
        .children(
            state
                .git
                .branch_menu
                .then(|| branch_menu(state, &session.branch)),
        )
}
