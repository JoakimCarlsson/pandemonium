//! The source control panel: the branch, the commit message and its button, the changed files in groups, and the graph of commits.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, animate_spin, bg, border, border_b, border_color, border_l,
    cursor_pointer, flex, flex_1, flex_col, flex_none, font_medium, gap, h, h_full, items_center,
    justify_center, min_h, min_w_px, overflow_hidden, pb, pl, pr, px, py, relative, rounded,
    w_full, w_px,
};
use pm_text::Shaper;
use pm_ui::{Node, node};

use crate::branch_menu::branch_menu;
use crate::button::Variant;
use crate::git_controls::{
    BRANCH_BUTTON, CHANGES_MORE, CHANGES_REFRESH, CHANGES_TOGGLE, COMMIT_CHEVRON, Primary,
    REPOSITORY_BASE, REPOSITORY_BRANCH_BASE, TROUBLE_DISMISS,
};
use crate::graph::graph;
use crate::parts::{clipped, glyph, line, spacer, wrap};
use crate::sample::Session;
use crate::size::Size;
use crate::state::{
    COMMIT, COMMIT_MESSAGE, OPEN_DIFF, REVIEW_CHANGES, REVIEW_FILES, ReviewTab, TREE_SCROLL,
    WorkspaceState,
};
use crate::tree_actions::{
    TREE_COLLAPSE, TREE_NEW_FILE, TREE_NEW_FOLDER, TREE_REFRESH, TREE_SPACE,
};
use crate::{Button, Input, Tile, input};
use crate::{change_list, file_tree};

/// How wide the review panel is until it is dragged wider or narrower, in logical pixels.
pub const WIDTH: f32 = 380.0;

/// How narrow and how wide the review panel can be dragged, in logical pixels.
pub const WIDTHS: (f32, f32) = (260.0, 760.0);

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

/// One of the tabs at the top of the panel, a glyph that says its name when the pointer rests on it, filled while it is the one shown.
fn panel_tab(label: &str, icon: IconName, id: FocusId, shown: bool) -> Node {
    let mark = match (icon, shown) {
        (IconName::Git, true) => IconName::GitFilled,
        (IconName::Folder, true) => IconName::FolderFilled,
        (IconName::Search, true) => IconName::SearchFilled,
        _ => icon,
    };
    let tone = if shown { Role::Text } else { Role::Muted };
    Button::icon(icon, id)
        .size(Size::Md)
        .mark(glyph(mark, 5.0, tone))
        .build()
        .tooltip(label)
}

/// The line above the tree: what it lists, and the buttons that make a file or folder, read it again and fold it.
fn files_header(state: &WorkspaceState) -> Node {
    band(10)
        .child(line("Files", 13, 1.4, Role::Subtle).styles([pm_style::flex_1()]))
        .child(
            Button::icon(IconName::FilePlus, TREE_NEW_FILE)
                .size(Size::Sm)
                .build()
                .tooltip("New File…"),
        )
        .child(
            Button::icon(IconName::FolderPlus, TREE_NEW_FOLDER)
                .size(Size::Sm)
                .build()
                .tooltip("New Folder…"),
        )
        .child(
            Button::icon(IconName::Refresh, TREE_REFRESH)
                .size(Size::Sm)
                .spinning(state.git.working)
                .build()
                .tooltip("Refresh"),
        )
        .child(
            Button::icon(IconName::Collapse, TREE_COLLAPSE)
                .size(Size::Sm)
                .build()
                .tooltip("Collapse Folders"),
        )
}

/// Every file of the project, as a tree, under the buttons that act on it.
fn files(state: &WorkspaceState) -> Node {
    let mut list = scrolling_list(state).children(file_tree::rows(state));
    if state.files.is_empty() {
        list = list.child(nothing("No files yet."));
    }
    list = list.child(node().styles([flex_1(), min_h(32.0)]).focusable(TREE_SPACE));
    node()
        .styles([flex(), flex_col(), pm_style::flex_1(), min_h(0)])
        .child(files_header(state))
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
                    .placeholder(&match &state.git.repository {
                        Some(submodule) => format!("Message (Ctrl+Enter to commit in {submodule})"),
                        None => "Message (Ctrl+Enter to commit)".to_owned(),
                    })
                    .rows(4)
                    .wrap(input::inner_width(state.review_width - 24.0), shaper)
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
                .size(Size::Sm)
                .disabled(!committable)
                .build()
                .tooltip("Commit"),
        )
        .child(
            Button::icon(IconName::Refresh, CHANGES_REFRESH)
                .size(Size::Sm)
                .spinning(state.git.working)
                .build()
                .tooltip("Refresh"),
        )
        .child(
            Button::icon(IconName::More, CHANGES_MORE)
                .size(Size::Sm)
                .build()
                .tooltip("More Actions…"),
        )
}

/// The banner saying what git refused to do, with a button that dismisses it, in a panel `width` logical pixels wide.
fn trouble(said: &str, width: f32) -> Node {
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
                        wrap(said, 12, width - 24.0 - 56.0)
                            .iter()
                            .map(|words| line(words, 12, 1.5, Role::Danger)),
                    ),
            )
            .child(
                Button::icon(IconName::Close, TROUBLE_DISMISS)
                    .size(Size::Sm)
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
        .children(
            state
                .git
                .trouble
                .as_deref()
                .map(|said| trouble(said, state.review_width)),
        );
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

/// The rows of the list of repositories of `session`'s folder, when it has submodules: its own and each submodule, nested ones set in under the one they are in, each with what it has checked out on a button that opens its branches; the one the panel works on is picked.
fn repositories(state: &WorkspaceState, session: &Session) -> Option<Node> {
    let repositories = state.repositories();
    if repositories.len() < 2 {
        return None;
    }
    let name = |repository: &Option<String>| match repository {
        Some(path) => path.clone(),
        None => state
            .projects
            .get(session.project)
            .map_or_else(String::new, |project| project.name.clone()),
    };
    let depth = |repository: &Option<String>| {
        repository.as_deref().map_or(0, |path| {
            1 + state
                .git
                .submodule_heads
                .iter()
                .filter(|(held, _)| {
                    path.strip_prefix(held.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
                })
                .count()
        })
    };
    let rows =
        repositories.iter().enumerate().map(|(row, repository)| {
            let changed = session
                .changes
                .iter()
                .filter(|change| change.submodule == *repository)
                .count();
            let head = state.head_of(repository.as_deref()).unwrap_or_default();
            Tile::new(FocusId(REPOSITORY_BASE + row as u64))
                .selected(state.git.repository == *repository)
                .height(7.0)
                .gap(2.0)
                .radius(Radius::Sm)
                .build()
                .styles([pl(2.0 + depth(repository) as f32 * 3.5), pr(1), flex_none()])
                .child(glyph(
                    if repository.is_some() {
                        IconName::GitFork
                    } else {
                        IconName::Git
                    },
                    3.5,
                    Role::Muted,
                ))
                .child(clipped(name(repository), 13, Role::Text))
                .children((changed > 0).then(|| {
                    line(changed.to_string(), 11, 1.4, Role::Subtle).styles([flex_none()])
                }))
                .child(
                    Button::new(&head, FocusId(REPOSITORY_BRANCH_BASE + row as u64))
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .monospace()
                        .build()
                        .styles([flex_none()])
                        .tooltip("Checkout Branch…"),
                )
        });
    Some(
        node()
            .styles([
                flex(),
                flex_col(),
                px(1),
                pb(2),
                flex_none(),
                border_b(),
                border_color(Role::Border),
            ])
            .child(
                node()
                    .styles([flex(), items_center(), h(7), px(2)])
                    .child(line("Repositories", 12, 1.4, Role::Subtle)),
            )
            .children(rows),
    )
}

/// The button that shows the branch the repository the panel works on is on and opens the list of branches, or, outside a repository, only says so.
fn branch_button(state: &WorkspaceState, session: &Session, working: bool) -> Node {
    if session.branch.is_empty() {
        return clipped("Not a Git repository", 12, Role::Subtle);
    }
    let head = state
        .head_of(state.git.repository.as_deref())
        .unwrap_or_else(|| session.branch.clone());
    Button::new(&head, BRANCH_BUTTON)
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .tone(Role::Text)
        .with_icon(IconName::GitFork)
        .monospace()
        .fill()
        .trailing_icon(if working {
            IconName::Loading
        } else {
            IconName::ChevronDown
        })
        .spinning(working)
        .build()
}

/// The review panel for `session`.
pub fn review(state: &WorkspaceState, session: &Session, shaper: &mut Shaper) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            w_px(state.review_width),
            h_full(),
            bg(Role::Background),
            border_l(),
            border_color(Role::Border),
            relative(),
        ])
        .child(
            band(12)
                .styles([px(3)])
                .child(
                    node()
                        .styles([
                            flex(),
                            items_center(),
                            gap(2),
                            flex_1(),
                            min_w_px(0),
                            overflow_hidden(),
                        ])
                        .child(panel_tab(
                            "Source Control",
                            IconName::Git,
                            REVIEW_CHANGES,
                            state.review_tab == ReviewTab::Changes,
                        ))
                        .child(panel_tab(
                            "Files",
                            IconName::Folder,
                            REVIEW_FILES,
                            state.review_tab == ReviewTab::Files,
                        ))
                        .child(panel_tab(
                            "Search",
                            IconName::Search,
                            crate::project_search::REVIEW_SEARCH,
                            state.review_tab == ReviewTab::Search,
                        )),
                )
                .children(
                    (state.review_tab == ReviewTab::Changes && !session.changes.is_empty()).then(
                        || {
                            Button::icon(IconName::GitCompare, OPEN_DIFF)
                                .build()
                                .styles([flex_none()])
                                .tooltip("Open Changes")
                        },
                    ),
                ),
        )
        .child(band(10).child(branch_button(state, session, state.git.working)))
        .children(
            (state.review_tab == ReviewTab::Changes)
                .then(|| repositories(state, session))
                .flatten(),
        )
        .child(match state.review_tab {
            ReviewTab::Changes => changes(state, session, shaper),
            ReviewTab::Files => files(state),
            ReviewTab::Search => crate::project_search::panel(state),
        })
        .children(state.git.branch_menu.then(|| branch_menu(state)))
}
