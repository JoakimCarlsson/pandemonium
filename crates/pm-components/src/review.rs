//! The source control panel: the branch, the commit message and its button, the changed files in groups, and the graph of commits.

use crate::areas::GitEvent;
use crate::areas::{DiffEvent, SearchEvent, TreeEvent};
use crate::button;
use crate::events;
use crate::icons;
use crate::parts;
use crate::theme::{FocusOutline, Indicated};
use rustle::TextOverflow;

use crate::events::{Event, send};
use crate::git_menus::MenuKind;
use crate::target::Target;
use crate::theme;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{CursorIcon, Modifier};
use rustle::{Icon, phosphor};
use std::rc::Rc;

use crate::events::Interactive;
use crate::icons::spinner;
use crate::theme::{Radius, Role};

use crate::branch_menu::branch_menu;
use crate::button::Variant;
use crate::git_controls::Primary;
use crate::graph::graph;
use crate::parts::{clipped, glyph, line, spacer, wrapping};
use crate::sample::Session;
use crate::size::Size;
use crate::state::{ReviewTab, WorkspaceState};

use crate::{Button, Input, Tile, input};
use crate::{change_list, file_tree};

/// How wide the review panel is until it is dragged wider or narrower, in logical pixels.
pub const WIDTH: f32 = 380.0;

/// How narrow and how wide the review panel can be dragged, in logical pixels.
pub const WIDTHS: (f32, f32) = (260.0, 760.0);

/// Emits the box that holds the rows of the panel's list, which `content` emits, and scrolls them.
fn scrolling_list(state: &WorkspaceState, content: impl FnOnce()) {
    rustle::column_arranged(
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
    rustle::row_arranged(
        Modifier::new().padding_xy(theme::sp(4), theme::sp(3)),
        Arrangement::Start,
        Alignment::Start,
        || line(message, 13, 1.4, Role::Subtle),
    );
}

/// Emits one of the tabs at the top of the panel, `icon`, a glyph that says
/// its name when the pointer rests on it, drawn as `filled` while it is the one shown;
/// a click gives the keyboard to the box `into` when there is one.
#[track_caller]
fn panel_tab(label: &str, icon: Icon, filled: Icon, id: Target, shown: bool, into: Option<Target>) {
    let mark = if shown { filled } else { icon.clone() };
    let tone = if shown { Role::Text } else { Role::Muted };
    let mut button = Button::icon(icon, id)
        .size(Size::Md)
        .mark(move || glyph(mark.clone(), 5.0, tone))
        .tooltip(label);
    if let Some(field) = into {
        button = button.on_click(crate::focus::pressing_into(id, field));
    }
    button.build();
}

/// Emits a small button showing only `icon`, answering to `id`, turning while `spinning`, saying what it does with `tooltip`.
#[track_caller]
pub(crate) fn small_icon_button(icon: Icon, id: Target, spinning: bool, tooltip: &str) {
    Button::icon(icon, id)
        .spinning(spinning)
        .tooltip(tooltip)
        .build();
}

/// Emits the line above the tree: what it lists, and the buttons that make a file or folder, read it again and fold it.
fn files_header(state: &WorkspaceState) {
    parts::bar(10, 3, 2, || {
        rustle::basic_text(
            Modifier::new().weight(1.0),
            "Files",
            theme::line(13, Role::Subtle),
            TextOverflow::Ellipsis,
        );
        for (icon, id, spinning, tooltip) in [
            (
                phosphor::FILE_PLUS,
                TreeEvent::TreeNewFile,
                false,
                "New File…",
            ),
            (
                phosphor::FOLDER_PLUS,
                TreeEvent::TreeNewFolder,
                false,
                "New Folder…",
            ),
            (
                phosphor::ARROWS_CLOCKWISE,
                TreeEvent::TreeRefresh,
                state.git.working,
                "Refresh",
            ),
            (
                phosphor::ARROWS_IN_LINE_VERTICAL,
                TreeEvent::TreeCollapse,
                false,
                "Collapse Folders",
            ),
        ] {
            small_icon_button(icon, Target::Tree(id), spinning, tooltip);
        }
    });
}

/// Emits every file of the project, as a tree, under the buttons that act on it.
#[track_caller]
fn files(state: &WorkspaceState) {
    rustle::column_arranged(
        Modifier::new().fill_max_width().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            files_header(state);
            file_tree::list(state);
        },
    );
}

/// Whether the button under the commit message does something now: it is neither stopped nor waiting.
fn enabled(primary: &Primary) -> bool {
    !matches!(
        primary,
        Primary::Commit {
            stopped: Some(_),
            ..
        } | Primary::Busy(_)
    )
}

/// Emits the words and the icon on the button under the commit message, for what it does now.
fn primary_face(primary: &Primary) {
    let enabled = enabled(primary);
    let tone = if enabled { Role::Text } else { Role::Subtle };
    let (icon, words) = match primary {
        Primary::Commit { title, stopped } => (phosphor::CHECK, stopped.unwrap_or(title)),
        Primary::Sync { .. } => (phosphor::ARROWS_CLOCKWISE, "Sync Changes"),
        Primary::Publish => (phosphor::ARROW_LINE_UP, "Publish Branch"),
        Primary::Busy(doing) => (phosphor::CIRCLE_DASHED, *doing),
    };
    rustle::row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(theme::sp(2)),
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
                theme::strong(13, tone),
                TextOverflow::Ellipsis,
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
fn commit_button(state: &WorkspaceState, primary: &Primary) {
    let enabled = enabled(primary);
    let pressable = |own: Modifier, id: Target, on_click: Rc<dyn Fn()>| {
        if enabled {
            own.indicated(
                theme::InteractionTones::new().hover(Role::SurfaceHover),
                theme::Focused::Control(crate::focus::is_focused(id)),
            )
            .cursor(CursorIcon::Pointer)
            .control(id, move || on_click())
        } else {
            own
        }
    };
    let main = pressable(
        Modifier::new().weight(1.0).fill_max_height(),
        Target::Git(GitEvent::Commit),
        Rc::new(send(Event::Git(GitEvent::Commit))),
    );
    let operations = crate::events::operations();
    let arrow = pressable(
        Modifier::new().fill_max_height(),
        Target::Git(GitEvent::CommitChevron),
        Rc::new(move || operations.open_git_menu(MenuKind::Commit, None)),
    )
    .padding_xy(theme::sp(2), 0.0);
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(theme::sp(3), 0.0, theme::sp(3), theme::sp(2))
            .height(theme::sp(9))
            .background_rounded(theme::color(Role::SurfaceSelected), Radius::Md.px()),
        Arrangement::Start,
        Alignment::Center,
        || {
            row_arranged(main, Arrangement::Center, Alignment::Center, || {
                primary_face(primary)
            });
            rustle::vertical_divider(Modifier::new(), 1.0, theme::color(Role::Border));
            rustle::row_arranged(arrow, Arrangement::Start, Alignment::Center, || {
                glyph(
                    phosphor::CARET_DOWN,
                    3.0,
                    if enabled { Role::Text } else { Role::Subtle },
                );
                if let Some(menu) = state.git_menu_under(MenuKind::Commit) {
                    menu.build();
                }
            });
        },
    );
}

/// Emits the box the commit message is written in: several lines, wrapped to the panel.
fn message_box(state: &WorkspaceState) {
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(theme::sp(3), theme::sp(2))
            .background_rounded(theme::color(Role::Background), Radius::Lg.px())
            .focus_outline(
                crate::focus::is_focused(Target::Git(GitEvent::CommitMessage)),
                theme::color(Role::Border),
                theme::color(Role::Accent),
                Radius::Lg,
            )
            .padding(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            Input::new(&state.message, Target::Git(GitEvent::CommitMessage))
                .modifier(Modifier::new().fill_max_width())
                .variant(input::Variant::Multiline)
                .placeholder(&match &state.git.repository {
                    Some(submodule) => format!("Message (Ctrl+Enter to commit in {submodule})"),
                    None => "Message (Ctrl+Enter to commit)".to_owned(),
                })
                .rows(4)
                .breaks_lines_on_enter()
                .submits(Target::Git(GitEvent::Commit))
                .hurries(Target::Git(GitEvent::Commit))
                .build();
        },
    );
}

/// Emits the heading of the changes section: a chevron that folds it, its name, and what can be done from it.
fn section_heading(state: &WorkspaceState, primary: &Primary) {
    let open = state.git.changes_open;
    let committable = matches!(primary, Primary::Commit { stopped: None, .. });
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(8))
            .padding_xy(theme::sp(2), 0.0),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            rustle::row_arranged(
                events::pressed(
                    Modifier::new().weight(1.0).fill_max_height(),
                    Target::Git(GitEvent::ChangesToggle),
                ),
                Arrangement::SpacedBy(theme::sp(1)),
                Alignment::Center,
                || {
                    glyph(icons::chevron(open), 3.5, Role::Subtle);
                    line("Changes", 13, 1.4, Role::Text);
                },
            );
            Button::icon(phosphor::GIT_COMMIT, Target::Git(GitEvent::Commit))
                .disabled(!committable)
                .tooltip("Commit")
                .build();
            small_icon_button(
                phosphor::ARROWS_CLOCKWISE,
                Target::Git(GitEvent::ChangesRefresh),
                state.git.working,
                "Refresh",
            );
            let operations = crate::events::operations();
            Button::icon(phosphor::DOTS_THREE, Target::Git(GitEvent::ChangesMore))
                .on_click(move || operations.open_git_menu(MenuKind::Source, None))
                .dropdown(state.git_menu_under(MenuKind::Source))
                .tooltip("More Actions…")
                .build();
        },
    );
}

/// Emits the banner saying what git refused to do, wrapped to the panel's width, with a button that dismisses it.
#[track_caller]
fn trouble(said: &str) {
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(theme::sp(3), 0.0, theme::sp(3), theme::sp(2))
            .background_rounded(theme::color(Role::Danger.alpha(0.12)), Radius::Md.px())
            .padding_xy(theme::sp(3), theme::sp(2)),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Start,
        || {
            rustle::column_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    wrapping(Modifier::new(), said, 12, 1.5, Role::Danger);
                },
            );
            Button::icon(phosphor::X, Target::Git(GitEvent::TroubleDismiss)).build();
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
    rustle::column_arranged(
        Modifier::new().fill_max_width().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::column_arranged(
                Modifier::new().fill_max_width().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    section_heading(state, &primary);
                    if let Some(said) = state.git.trouble.as_deref() {
                        trouble(said);
                    }
                    if state.git.changes_open {
                        message_box(state);
                        commit_button(state, &primary);
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
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .border_bottom(1.0, theme::color(Role::Border))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_each(theme::sp(1), 0.0, theme::sp(1), theme::sp(2)),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::row_arranged(
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
                Tile::new(Target::Git(GitEvent::Repository(place)))
                    .selected(state.git.repository == *repository)
                    .height(7.0)
                    .gap(2.0)
                    .radius(Radius::Sm)
                    .modifier(Modifier::new().fill_max_width())
                    .inner(Modifier::new().padding_each(
                        theme::sp(2.0 + depth(repository) as f32 * 3.5),
                        0.0,
                        theme::sp(1),
                        0.0,
                    ))
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
                        button::small_button(
                            &head,
                            Target::Git(GitEvent::RepositoryBranch(place)),
                            Variant::Ghost,
                        )
                        .monospace()
                        .tooltip("Checkout Branch…")
                        .build();
                    });
            }
        },
    );
}

/// Emits the button that shows the branch the repository the panel works on is on and opens the list of branches, or, outside a repository, only says so.
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
    let branches = Target::Git(GitEvent::BranchButton);
    button::small_button(&head, branches, Variant::Ghost)
        .on_click(crate::focus::pressing_into(
            branches,
            Target::Git(GitEvent::BranchFilter),
        ))
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
    rustle::row_arranged(
        Modifier::new()
            .width(state.review_width)
            .fill_max_height()
            .background(theme::color(Role::Background)),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::vertical_divider(Modifier::new(), 1.0, theme::color(Role::Border));
            rustle::stack_with(Modifier::new().weight(1.0).fill_max_height(), || {
                rustle::column_arranged(
                    Modifier::new().fill_max_size(),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        parts::bar(12, 3, 2, || {
                            rustle::row_arranged(
                                Modifier::new().weight(1.0).clip(),
                                Arrangement::SpacedBy(theme::sp(2)),
                                Alignment::Center,
                                || {
                                    if versioned {
                                        panel_tab(
                                            "Source Control",
                                            phosphor::GIT_BRANCH,
                                            phosphor::fill::GIT_BRANCH,
                                            Target::Diff(DiffEvent::ReviewChanges),
                                            shown == ReviewTab::Changes,
                                            None,
                                        );
                                    }
                                    panel_tab(
                                        "Files",
                                        phosphor::FOLDER,
                                        phosphor::fill::FOLDER,
                                        Target::Diff(DiffEvent::ReviewFiles),
                                        shown == ReviewTab::Files,
                                        None,
                                    );
                                    panel_tab(
                                        "Search",
                                        phosphor::MAGNIFYING_GLASS,
                                        phosphor::fill::MAGNIFYING_GLASS,
                                        Target::Search(SearchEvent::ReviewSearch),
                                        shown == ReviewTab::Search,
                                        Some(Target::Search(SearchEvent::ProjectSearchField)),
                                    );
                                },
                            );
                            if shown == ReviewTab::Changes && !session.changes.is_empty() {
                                Button::icon(phosphor::GIT_DIFF, Target::Diff(DiffEvent::OpenDiff))
                                    .tooltip("Open Diff")
                                    .build();
                            }
                        });
                        if versioned {
                            parts::bar(10, 3, 2, || {
                                branch_button(state, session, state.git.working)
                            });
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
            });
        },
    );
}
