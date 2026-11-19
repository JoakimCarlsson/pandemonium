//! The source control panel: the branch, the commit message and its button, the changed files in groups, and the graph of commits.

use crate::events::reduce;
use crate::git_menus::MenuKind;
use crate::theme;
use crate::theme::sp;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};
use rustle::{Icon, phosphor};
use std::rc::Rc;

use crate::events::Interactive;
use crate::focus::FocusId;
use crate::icons::spinner;
use crate::theme::{Radius, Role, Tone};

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
    COMMIT, COMMIT_MESSAGE, OPEN_DIFF, REVIEW_CHANGES, REVIEW_FILES, ReviewTab, WorkspaceState,
};
use crate::tree_actions::{TREE_COLLAPSE, TREE_NEW_FILE, TREE_NEW_FOLDER, TREE_REFRESH};
use crate::{Button, Input, Tile, input};
use crate::{change_list, file_tree};

/// How wide the review panel is until it is dragged wider or narrower, in logical pixels.
pub const WIDTH: f32 = 380.0;

/// How narrow and how wide the review panel can be dragged, in logical pixels.
pub const WIDTHS: (f32, f32) = (260.0, 760.0);

/// Emits a row of the panel `height` steps tall, with a rule beneath it, holding what `content` emits.
#[track_caller]
fn band(height: u32, content: impl FnOnce()) {
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(height))
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        content,
    );
}

/// Emits the box that holds the rows of the panel's list, which `content` emits, and scrolls them.
#[track_caller]
fn scrolling_list(state: &WorkspaceState, content: impl FnOnce()) {
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .vertical_scroll(&state.tree_scroll)
            .fill_max_width()
            .padding_xy(0.0, theme::sp(1)),
        Arrangement::Start,
        Alignment::Start,
        content,
    );
}

/// Emits a line telling the list has nothing to show.
#[track_caller]
pub(crate) fn nothing(message: &str) {
    row_arranged(
        Modifier::new()
            .padding_xy(theme::sp(4), 0.0)
            .padding_xy(0.0, theme::sp(3)),
        Arrangement::Start,
        Alignment::Start,
        || line(message, 13, 1.4, Role::Subtle),
    );
}

/// Emits one of the tabs at the top of the panel, `icon`, a glyph that says
/// its name when the pointer rests on it, drawn as `filled` while it is the one shown.
#[track_caller]
fn panel_tab(label: &str, icon: Icon, filled: Icon, id: FocusId, shown: bool) {
    let mark = if shown { filled } else { icon.clone() };
    let tone = if shown { Role::Text } else { Role::Muted };
    Button::icon(icon, id)
        .size(Size::Md)
        .mark(move || glyph(mark.clone(), 5.0, tone))
        .tooltip(label)
        .build();
}

/// Emits a small button showing only `icon`, answering to `id`, turning while `spinning`, saying what it does with `tooltip`.
#[track_caller]
fn small_icon_button(icon: Icon, id: FocusId, spinning: bool, tooltip: &str) {
    Button::icon(icon, id)
        .size(Size::Sm)
        .spinning(spinning)
        .tooltip(tooltip)
        .build();
}

/// Emits the line above the tree: what it lists, and the buttons that make a file or folder, read it again and fold it.
#[track_caller]
fn files_header(state: &WorkspaceState) {
    band(10, || {
        label_with_grow("Files");
        small_icon_button(phosphor::FILE_PLUS, TREE_NEW_FILE, false, "New File…");
        small_icon_button(phosphor::FOLDER_PLUS, TREE_NEW_FOLDER, false, "New Folder…");
        small_icon_button(
            phosphor::ARROWS_CLOCKWISE,
            TREE_REFRESH,
            state.git.working,
            "Refresh",
        );
        small_icon_button(
            phosphor::ARROWS_IN_LINE_VERTICAL,
            TREE_COLLAPSE,
            false,
            "Collapse Folders",
        );
    });
}

/// Emits `words` as the name of a list, taking the room left in its row.
#[track_caller]
fn label_with_grow(words: &str) {
    rustle::basic_text(
        Modifier::new().weight(1.0),
        words,
        theme::text_style(13, 1.4)
            .color(theme::color(Role::Subtle))
            .max_lines(1),
        rustle::TextOverflow::Ellipsis,
    );
}

/// Emits every file of the project, as a tree, under the buttons that act on it.
#[track_caller]
fn files(state: &WorkspaceState) {
    column_arranged(
        Modifier::new().fill_max_width().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            files_header(state);
            file_tree::list(state);
        },
    );
}

/// Emits the words and the icon on the button under the commit message, for what it does now.
#[track_caller]
fn primary_face(primary: &Primary) {
    let enabled = !matches!(
        primary,
        Primary::Commit {
            stopped: Some(_),
            ..
        } | Primary::Busy(_)
    );
    let tone = if enabled { Role::Text } else { Role::Subtle };
    let (icon, words) = match primary {
        Primary::Commit { title, stopped } => (phosphor::CHECK, stopped.unwrap_or(title)),
        Primary::Sync { .. } => (phosphor::ARROWS_CLOCKWISE, "Sync Changes"),
        Primary::Publish => (phosphor::ARROW_LINE_UP, "Publish Branch"),
        Primary::Busy(doing) => (phosphor::CIRCLE_DASHED, *doing),
    };
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            if matches!(primary, Primary::Busy(_)) {
                spinner(3.5, tone);
            } else {
                glyph(icon, 3.5, tone);
            }
            rustle::basic_text(
                Modifier::new(),
                words,
                theme::text_style(13, 1.4)
                    .font_weight(rustle::FontWeight::MEDIUM)
                    .color(theme::color(tone))
                    .max_lines(1),
                rustle::TextOverflow::Ellipsis,
            );
            if let Primary::Sync { ahead, behind } = primary {
                for (count, arrow) in [
                    (*behind, phosphor::ARROW_DOWN),
                    (*ahead, phosphor::ARROW_UP),
                ] {
                    if count > 0 {
                        line(count.to_string(), 13, 1.4, tone);
                        glyph(arrow, 3.0, tone);
                    }
                }
            }
        },
    );
}

/// Emits the control that commits, or syncs or publishes once there is nothing to commit, with the arrow that offers what else it can do.
///
/// What stops it is written where the words would be, so a reader who cannot commit is told why.
#[track_caller]
fn commit_button(primary: &Primary) {
    let enabled = !matches!(
        primary,
        Primary::Commit {
            stopped: Some(_),
            ..
        } | Primary::Busy(_)
    );
    let pressable = |own: Modifier, id: FocusId, on_click: Rc<dyn Fn()>| {
        if enabled {
            own.interaction_focused(crate::focus::is_held(id))
                .interaction_background(
                    (theme::InteractionTones::new().hover(Role::SurfaceHover)).colors(),
                )
                .cursor(CursorIcon::Pointer)
                .control(id, move || on_click())
        } else {
            own
        }
    };
    let main = pressable(
        Modifier::new().weight(1.0).fill_max_height(),
        COMMIT,
        Rc::new(reduce(COMMIT)),
    );
    let operations = crate::events::operations();
    let arrow = pressable(
        Modifier::new()
            .fill_max_height()
            .draw_behind(theme::edge(
                theme::Side::Left,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(1.0, 0.0, 0.0, 0.0),
        COMMIT_CHEVRON,
        Rc::new(move || operations.open_git_menu(MenuKind::Commit, Some(COMMIT_CHEVRON))),
    )
    .padding_xy(theme::sp(2), 0.0);
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(theme::sp(3), 0.0)
            .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .height(theme::sp(9))
                    .fill_max_width()
                    .background_rounded(theme::color(Role::SurfaceSelected), Radius::Md.px()),
                Arrangement::Start,
                Alignment::Center,
                || {
                    row_arranged(main, Arrangement::Center, Alignment::Center, || {
                        primary_face(primary)
                    });
                    row_arranged(arrow, Arrangement::Start, Alignment::Center, || {
                        glyph(
                            phosphor::CARET_DOWN,
                            3.0,
                            if enabled { Role::Text } else { Role::Subtle },
                        )
                    });
                },
            );
        },
    );
}

/// Emits the box the commit message is written in: several lines, wrapped to the panel.
#[track_caller]
fn message_box(state: &WorkspaceState) {
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(theme::sp(3), 0.0)
            .padding_xy(0.0, theme::sp(2)),
        Arrangement::Start,
        Alignment::Start,
        || {
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .background_rounded(theme::color(Role::Background), Radius::Lg.px())
                    .interaction_focused(crate::focus::is_held(COMMIT_MESSAGE))
                    .draw_behind_interactive(theme::outline_by_focus(
                        theme::color(Role::Border),
                        theme::color(Role::Accent),
                        Radius::Lg,
                    ))
                    .padding(1.0)
                    .interaction_focused(crate::focus::is_held(COMMIT_MESSAGE)),
                Arrangement::Start,
                Alignment::Start,
                || {
                    Input::new(&state.message, COMMIT_MESSAGE)
                        .modifier(Modifier::new().fill_max_width())
                        .variant(input::Variant::Multiline)
                        .placeholder(&match &state.git.repository {
                            Some(submodule) => {
                                format!("Message (Ctrl+Enter to commit in {submodule})")
                            }
                            None => "Message (Ctrl+Enter to commit)".to_owned(),
                        })
                        .rows(4)
                        .build();
                },
            );
        },
    );
}

/// Emits the heading of the changes section: a chevron that folds it, its name, and what can be done from it.
#[track_caller]
fn section_heading(state: &WorkspaceState, primary: &Primary) {
    let open = state.git.changes_open;
    let committable = matches!(primary, Primary::Commit { stopped: None, .. });
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(8))
            .padding_xy(theme::sp(2), 0.0),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new()
                    .weight(1.0)
                    .fill_max_height()
                    .cursor(CursorIcon::Pointer)
                    .control(CHANGES_TOGGLE, reduce(CHANGES_TOGGLE)),
                Arrangement::SpacedBy(sp(1)),
                Alignment::Center,
                || {
                    glyph(
                        if open {
                            phosphor::CARET_DOWN
                        } else {
                            phosphor::CARET_RIGHT
                        },
                        3.5,
                        Role::Subtle,
                    );
                    line("Changes", 13, 1.4, Role::Text);
                },
            );
            Button::icon(phosphor::GIT_COMMIT, COMMIT)
                .size(Size::Sm)
                .disabled(!committable)
                .tooltip("Commit")
                .build();
            small_icon_button(
                phosphor::ARROWS_CLOCKWISE,
                CHANGES_REFRESH,
                state.git.working,
                "Refresh",
            );
            let operations = crate::events::operations();
            Button::icon(phosphor::DOTS_THREE, CHANGES_MORE)
                .on_click(move || operations.open_git_menu(MenuKind::Source, Some(CHANGES_MORE)))
                .size(Size::Sm)
                .tooltip("More Actions…")
                .build();
        },
    );
}

/// Emits the banner saying what git refused to do, with a button that dismisses it, in a panel `width` logical pixels wide.
#[track_caller]
fn trouble(said: &str, width: f32) {
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(theme::sp(3), 0.0)
            .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .weight(1.0)
                    .background_rounded(
                        theme::color(Tone::from(Role::Danger).alpha(0.12)),
                        Radius::Md.px(),
                    )
                    .padding_xy(theme::sp(3), 0.0)
                    .padding_xy(0.0, theme::sp(2)),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Start,
                || {
                    column_arranged(
                        Modifier::new().weight(1.0),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            for words in wrap(said, 12, width - 24.0 - 56.0) {
                                line(words, 12, 1.5, Role::Danger);
                            }
                        },
                    );
                    Button::icon(phosphor::X, TROUBLE_DISMISS)
                        .size(Size::Sm)
                        .build();
                },
            );
        },
    );
}

/// Emits the changes of `session`: the commit message and its button, then the files in groups, and the graph under them.
#[track_caller]
fn changes(state: &WorkspaceState, session: &Session) {
    if session.unknown {
        nothing("Reading the folder…");
        return;
    }
    if !session.on_branch
        && session.branch.is_empty()
        && session.changes.is_empty()
        && state.git.history.is_empty()
    {
        nothing("This folder is not a git repository.");
        return;
    }
    let primary = state.primary();
    column_arranged(
        Modifier::new().fill_max_width().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            column_arranged(
                Modifier::new().fill_max_width().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    section_heading(state, &primary);
                    if let Some(said) = state.git.trouble.as_deref() {
                        trouble(said, state.review_width);
                    }
                    if state.git.changes_open {
                        message_box(state);
                        commit_button(&primary);
                        scrolling_list(state, || {
                            change_list::rows(state, session);
                            if session.changes.is_empty() {
                                nothing("No uncommitted changes.");
                            }
                        });
                    } else {
                        spacer();
                    }
                },
            );
            graph(state);
        },
    );
}

/// Emits the rows of the list of repositories of `session`'s folder, when it has submodules: its own and each submodule, nested ones set in under the one they are in, each with what it has checked out on a button that opens its branches; the one the panel works on is picked.
#[track_caller]
fn repositories(state: &WorkspaceState, session: &Session) {
    let repositories = state.repositories();
    if repositories.len() < 2 {
        return;
    }
    let name = |repository: &Option<String>| match repository {
        Some(path) => path.clone(),
        None => state
            .project_of(session)
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
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(1), 0.0)
            .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .height(theme::sp(7))
                    .padding_xy(theme::sp(2), 0.0),
                Arrangement::Start,
                Alignment::Center,
                || line("Repositories", 12, 1.4, Role::Subtle),
            );
            for (place, repository) in repositories.iter().enumerate() {
                let changed = session
                    .changes
                    .iter()
                    .filter(|change| change.is_listed_in(repository.as_deref()))
                    .count();
                let head = state.head_of(repository.as_deref()).unwrap_or_default();
                Tile::new(FocusId(REPOSITORY_BASE + place as u64))
                    .selected(state.git.repository == *repository)
                    .height(7.0)
                    .gap(2.0)
                    .radius(Radius::Sm)
                    .modifier(Modifier::new().fill_max_width())
                    .inner(
                        Modifier::new()
                            .padding_each(
                                theme::sp(2.0 + depth(repository) as f32 * 3.5),
                                0.0,
                                0.0,
                                0.0,
                            )
                            .padding_each(0.0, 0.0, theme::sp(1), 0.0),
                    )
                    .build_with(|| {
                        glyph(
                            if repository.is_some() {
                                phosphor::GIT_FORK
                            } else {
                                phosphor::GIT_BRANCH
                            },
                            3.5,
                            Role::Muted,
                        );
                        clipped(name(repository), 13, Role::Text);
                        if changed > 0 {
                            line(changed.to_string(), 11, 1.4, Role::Subtle);
                        }
                        Button::new(&head, FocusId(REPOSITORY_BRANCH_BASE + place as u64))
                            .variant(Variant::Ghost)
                            .size(Size::Sm)
                            .monospace()
                            .tooltip("Checkout Branch…")
                            .build();
                    });
            }
        },
    );
}

/// Emits the button that shows the branch the repository the panel works on is on and opens the list of branches, or, outside a repository, only says so.
#[track_caller]
fn branch_button(state: &WorkspaceState, session: &Session, working: bool) {
    if session.branch.is_empty() {
        let said = if session.unknown {
            "Reading the folder…"
        } else {
            "Not a Git repository"
        };
        clipped(said, 12, Role::Subtle);
        return;
    }
    let head = state
        .head_of(state.git.repository.as_deref())
        .unwrap_or_else(|| session.branch.clone());
    Button::new(&head, BRANCH_BUTTON)
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .tone(Role::Text)
        .with_icon(phosphor::GIT_FORK)
        .monospace()
        .fill()
        .trailing_icon(if working {
            phosphor::CIRCLE_DASHED
        } else {
            phosphor::CARET_DOWN
        })
        .spinning(working)
        .build();
}

/// Emits the review panel for `session`. A chat has no branch, worktree or diff, so its panel leaves out source control: it lists the files of its scratch folder, or searches them.
#[track_caller]
pub fn review(state: &WorkspaceState, session: &Session) {
    let versioned = !session.is_chat();
    let shown = match state.review_tab {
        ReviewTab::Changes if !versioned => ReviewTab::Files,
        tab => tab,
    };
    rustle::stack_with(
        Modifier::new()
            .width(state.review_width)
            .fill_max_height()
            .background(theme::color(Role::Background))
            .draw_behind(theme::edge(
                theme::Side::Left,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(1.0, 0.0, 0.0, 0.0),
        || {
            column_arranged(
                Modifier::new().fill_max_size(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    band(12, || {
                        row_arranged(
                            Modifier::new().weight(1.0).clip(),
                            Arrangement::SpacedBy(sp(2)),
                            Alignment::Center,
                            || {
                                if versioned {
                                    panel_tab(
                                        "Source Control",
                                        phosphor::GIT_BRANCH,
                                        phosphor::fill::GIT_BRANCH,
                                        REVIEW_CHANGES,
                                        shown == ReviewTab::Changes,
                                    );
                                }
                                panel_tab(
                                    "Files",
                                    phosphor::FOLDER,
                                    phosphor::fill::FOLDER,
                                    REVIEW_FILES,
                                    shown == ReviewTab::Files,
                                );
                                panel_tab(
                                    "Search",
                                    phosphor::MAGNIFYING_GLASS,
                                    phosphor::fill::MAGNIFYING_GLASS,
                                    crate::project_search::REVIEW_SEARCH,
                                    shown == ReviewTab::Search,
                                );
                            },
                        );
                        if shown == ReviewTab::Changes && !session.changes.is_empty() {
                            Button::icon(phosphor::GIT_DIFF, OPEN_DIFF)
                                .tooltip("Open Diff")
                                .build();
                        }
                    });
                    if versioned {
                        band(10, || branch_button(state, session, state.git.working));
                    }
                    if shown == ReviewTab::Changes {
                        repositories(state, session);
                    }
                    match shown {
                        ReviewTab::Changes => changes(state, session),
                        ReviewTab::Files => files(state),
                        ReviewTab::Search => crate::project_search::panel(state),
                    }
                },
            );
            if versioned && state.git.branch_menu {
                branch_menu(state);
            }
        },
    );
}
