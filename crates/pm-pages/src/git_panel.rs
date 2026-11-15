//! The Git tab of the right panel: the branch with its menu, the commit
//! message and button, the changed files in groups with boxes that stage
//! them, the menus on them, the questions asked before throwing work away,
//! and the commit graph with the remote tasks.

use std::rc::Rc;

use rustle::Modifiers;
use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, ChangeRow, ChangeRowVariant, CheckState, ConfirmDialog,
    ConfirmDialogVariant, Divider, FoldHeader, FoldHeaderVariant, GraphLine, GraphRow,
    GraphRowVariant, IconButton, Label, LabelVariant, Menu, OptionList, OptionListVariant, Popover,
    Size, TextArea, TextInput, extended_colors,
};
use pm_shell::{
    BranchTask, ChangeGroup, ChangedFile, GitAction, GitMenu, GitPrompt, GitSection, GitSummary,
    GraphCommit, Pick, Primary,
};

use crate::actions::Actions;

/// How tall the graph section is while unfolded.
const GRAPH_HEIGHT: f32 = 230.0;

/// How wide the panel's popovers are.
const MENU_WIDTH: f32 = 240.0;

/// Emits the Git tab for `git`, calling `actions` when the person acts.
pub fn git_panel(git: &GitSummary, actions: &Actions) {
    stack_with(Modifier::new().fill_max_size(), || {
        column_with(Modifier::new().fill_max_size().padding_xy(8.0, 0.0), || {
            if git.folder.is_empty() {
                note("Open a session to see its changes.");
                return;
            }
            if !git.loaded {
                note("Reading git…");
                return;
            }
            if !git.repository {
                note("This folder is not a git repository.");
                return;
            }
            branch_button(git, actions);
            trouble(git, actions);
            changes_section(git, actions);
            graph_section(git, actions);
        });
        if let Some(prompt) = &git.prompt {
            question(git, prompt, actions);
        }
    });
}

/// A closure that sends `action` to the panel.
fn send(actions: &Actions, action: GitAction) -> impl Fn() + 'static {
    let git = actions.git.clone();
    move || git(action.clone())
}

/// A muted line standing in for the panel's content.
fn note(message: &str) {
    Label::new(message)
        .variant(LabelVariant::Subtle)
        .size(Size::Xs)
        .modifier(Modifier::new().padding_xy(4.0, 8.0))
        .show();
}

/// The branch button, saying how far the branch and the one it follows are apart, with the branch menu under it.
fn branch_button(git: &GitSummary, actions: &Actions) {
    let label = if git.branch.is_empty() {
        git.head
            .as_ref()
            .map(|head| format!("Detached at {head}"))
            .unwrap_or_else(|| "No branch".to_owned())
    } else {
        git.branch.clone()
    };
    let label = match (git.ahead, git.behind) {
        (0, 0) => label,
        (ahead, 0) => format!("{label}  ↑{ahead}"),
        (0, behind) => format!("{label}  ↓{behind}"),
        (ahead, behind) => format!("{label}  ↑{ahead} ↓{behind}"),
    };
    let open = git.branch_menu.is_some();
    column_with(Modifier::new().fill_max_width(), || {
        Button::new(label)
            .icon(phosphor::GIT_BRANCH)
            .trailing_icon(phosphor::CARET_DOWN)
            .variant(if open {
                ButtonVariant::Secondary
            } else {
                ButtonVariant::Ghost
            })
            .modifier(Modifier::new().fill_max_width())
            .on_click(send(actions, GitAction::BranchMenu(!open)))
            .show();
        if open {
            branch_menu(git, actions);
        }
    });
}

/// The branch menu: a box narrowing the branches, the branches to check out, and making a new one.
fn branch_menu(git: &GitSummary, actions: &Actions) {
    let filter = git
        .branch_menu
        .as_ref()
        .map(|menu| menu.filter.clone())
        .unwrap_or_default();
    let choices = git.branch_choices();
    let current = choices.iter().position(|name| *name == git.branch);
    let narrow = actions.git.clone();
    let create = actions.git.clone();
    let pick = actions.git.clone();
    let named = filter.trim().to_owned();
    let picked = choices.clone();
    Popover::new()
        .width(SIDE_MENU_WIDTH)
        .on_dismiss(send(actions, GitAction::BranchMenu(false)))
        .show(move || {
            let submitted = named.clone();
            let first = picked.first().cloned();
            let submit = create.clone();
            TextInput::new(filter.clone())
                .placeholder("Find or create a branch")
                .size(Size::Xs)
                .modifier(Modifier::new().fill_max_width())
                .on_change(move |text| narrow(GitAction::BranchFilter(text)))
                .on_submit(move |_| match &first {
                    Some(branch) if submitted.is_empty() || *branch == submitted => {
                        submit(GitAction::Checkout(branch.clone()))
                    }
                    _ => submit(GitAction::CreateBranch(submitted.clone())),
                })
                .show();
            spacer(Modifier::new().height(pm_components::spacing(4.0)));
            if !named.is_empty() && !picked.contains(&named) {
                let create = create.clone();
                let name = named.clone();
                Button::new(format!("Create branch “{named}”"))
                    .icon(phosphor::PLUS)
                    .size(Size::Xs)
                    .modifier(Modifier::new().fill_max_width())
                    .on_click(move || create(GitAction::CreateBranch(name.clone())))
                    .show();
            }
            if picked.is_empty() {
                return;
            }
            let shown = picked.clone();
            OptionList::new(picked.clone(), current)
                .variant(OptionListVariant::Flat)
                .size(Size::Xs)
                .max_height(260.0)
                .modifier(Modifier::new().fill_max_width())
                .on_pick(move |place| {
                    if let Some(branch) = shown.get(place) {
                        pick(GitAction::Checkout(branch.clone()));
                    }
                })
                .show();
        });
}

/// The width of the branch menu, the panel's width less its padding.
const SIDE_MENU_WIDTH: f32 = 314.0;

/// The line saying what git refused, and the merge or rebase the folder is stopped in.
fn trouble(git: &GitSummary, actions: &Actions) {
    let scheme = color_scheme();
    let extended = extended_colors();
    if let Some(operation) = &git.operation {
        row_arranged(
            Modifier::new().fill_max_width().padding_xy(4.0, 4.0),
            Arrangement::SpacedBy(pm_components::spacing(6.0)),
            Alignment::Center,
            || {
                icon(phosphor::GIT_MERGE, 12.0, extended.warning);
                Label::new(operation.clone())
                    .colour(extended.warning)
                    .size(Size::Xs)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                let conflicted = git.changes.iter().any(|change| change.conflicted);
                Button::new("Abort")
                    .variant(ButtonVariant::Ghost)
                    .size(Size::Xs)
                    .on_click(send(actions, GitAction::Step("abort".to_owned())))
                    .show();
                if git.skippable {
                    Button::new("Skip")
                        .variant(ButtonVariant::Ghost)
                        .size(Size::Xs)
                        .on_click(send(actions, GitAction::Step("skip".to_owned())))
                        .show();
                }
                Button::new("Continue")
                    .variant(ButtonVariant::Secondary)
                    .size(Size::Xs)
                    .disabled(conflicted)
                    .on_click(send(actions, GitAction::Step("continue".to_owned())))
                    .show();
            },
        );
    }
    if let Some(notice) = &git.notice {
        row_arranged(
            Modifier::new()
                .fill_max_width()
                .background_rounded(scheme.surface_container, 6.0)
                .padding_xy(6.0, 4.0),
            Arrangement::SpacedBy(pm_components::spacing(6.0)),
            Alignment::Start,
            || {
                icon(phosphor::WARNING_CIRCLE, 12.0, scheme.error);
                Label::new(notice.clone())
                    .colour(scheme.error)
                    .size(Size::Xs)
                    .wrap()
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                IconButton::new(phosphor::X)
                    .size(Size::Xs)
                    .on_click(send(actions, GitAction::Dismiss))
                    .show();
            },
        );
    }
}

/// The Changes section: its header with the panel's menu, the commit box and button, and the files in groups.
fn changes_section(git: &GitSummary, actions: &Actions) {
    column_with(Modifier::new().fill_max_width().weight(1.0), || {
        let commit = send(actions, GitAction::Commit { push: false });
        let refresh = send(actions, GitAction::Refresh);
        let source = send(
            actions,
            GitAction::Menu(match git.menu {
                Some(GitMenu::Source) => None,
                _ => Some(GitMenu::Source),
            }),
        );
        FoldHeader::new("Changes", git.changes_open)
            .count(git.changes.len())
            .on_toggle(send(actions, GitAction::Fold(GitSection::Changes)))
            .trailing(move || {
                IconButton::new(phosphor::CHECK)
                    .size(Size::Xs)
                    .on_click(commit)
                    .show();
                IconButton::new(phosphor::ARROWS_CLOCKWISE)
                    .size(Size::Xs)
                    .on_click(refresh)
                    .show();
                IconButton::new(phosphor::DOTS_THREE)
                    .size(Size::Xs)
                    .on_click(source)
                    .show();
            })
            .show();
        if git.menu == Some(GitMenu::Stashes) {
            stashes_menu(git, actions);
        }
        if git.menu == Some(GitMenu::Source) {
            source_menu(git, actions);
        }
        if !git.changes_open {
            return;
        }
        let typed = actions.git.clone();
        let submit = actions.git.clone();
        TextArea::new(git.message.clone())
            .placeholder("Message (Ctrl+Enter to commit)")
            .size(Size::Xs)
            .height(60.0)
            .modifier(Modifier::new().fill_max_width())
            .on_change(move |text| typed(GitAction::Message(text)))
            .on_submit(move |_| submit(GitAction::Commit { push: false }))
            .show();
        spacer(Modifier::new().height(pm_components::spacing(6.0)));
        primary_row(git, actions);
        spacer(Modifier::new().height(pm_components::spacing(6.0)));
        let scroll = remember_scroll_state();
        column_with(
            Modifier::new()
                .fill_max_width()
                .weight(1.0)
                .vertical_scroll(&scroll),
            || {
                if git.changes.is_empty() {
                    note("No uncommitted changes.");
                }
                for group in ChangeGroup::ALL {
                    let members = git.group(group);
                    if members.is_empty() {
                        continue;
                    }
                    key(group.title(), || {
                        change_group(git, group, &members, actions)
                    });
                }
            },
        );
    });
}

/// The main button under the commit message, with the commit menu beside it.
fn primary_row(git: &GitSummary, actions: &Actions) {
    let primary = git.primary();
    let (label, glyph, enabled) = match &primary {
        Primary::Commit { ready } => (
            if git.changes.is_empty() {
                "Nothing to commit".to_owned()
            } else {
                "Commit".to_owned()
            },
            phosphor::CHECK,
            *ready,
        ),
        Primary::Sync { ahead, behind } => (
            match (ahead, behind) {
                (0, behind) => format!("Pull {behind}"),
                (ahead, 0) => format!("Push {ahead}"),
                (ahead, behind) => format!("Sync ↑{ahead} ↓{behind}"),
            },
            phosphor::ARROWS_DOWN_UP,
            true,
        ),
        Primary::Publish => ("Publish branch".to_owned(), phosphor::CLOUD_ARROW_UP, true),
        Primary::Busy(word) => (format!("{word}…"), phosphor::SPINNER, false),
    };
    let commit_menu = git.menu == Some(GitMenu::Commit);
    column_with(Modifier::new().fill_max_width(), || {
        row_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::SpacedBy(pm_components::spacing(4.0)),
            Alignment::Center,
            || {
                Button::new(label)
                    .icon(glyph)
                    .variant(if enabled {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .size(Size::Xs)
                    .disabled(!enabled)
                    .modifier(Modifier::new().weight(1.0))
                    .on_click(send(actions, GitAction::Primary))
                    .show();
                IconButton::new(phosphor::CARET_DOWN)
                    .size(Size::Xs)
                    .active(commit_menu)
                    .on_click(send(
                        actions,
                        GitAction::Menu((!commit_menu).then_some(GitMenu::Commit)),
                    ))
                    .show();
            },
        );
        if commit_menu {
            let ready = !git.changes.is_empty() && !git.message.trim().is_empty();
            let mut menu = Menu::new()
                .width(MENU_WIDTH)
                .offset(SIDE_MENU_WIDTH - MENU_WIDTH, 4.0)
                .on_dismiss(send(actions, GitAction::Menu(None)));
            menu = if ready {
                menu.icon_item(
                    phosphor::CHECK,
                    "Commit",
                    send(actions, GitAction::Commit { push: false }),
                )
                .hint("Ctrl+Enter")
                .icon_item(
                    phosphor::CLOUD_ARROW_UP,
                    "Commit and push",
                    send(actions, GitAction::Commit { push: true }),
                )
            } else {
                menu.disabled_item(phosphor::CHECK, "Commit")
                    .disabled_item(phosphor::CLOUD_ARROW_UP, "Commit and push")
            };
            menu.separator()
                .icon_item(
                    phosphor::PENCIL_SIMPLE,
                    "Amend last commit",
                    send(actions, GitAction::Ask(GitPrompt::Amend)),
                )
                .show();
        }
    });
}

/// The panel's own menu: remote tasks, staging everything and the checked out branch's tasks.
fn source_menu(git: &GitSummary, actions: &Actions) {
    let branch = git.branch.clone();
    let task = |task: BranchTask| {
        send(
            actions,
            GitAction::Ask(GitPrompt::Branch {
                task,
                branch: branch.clone(),
                name: if task == BranchTask::Delete {
                    String::new()
                } else {
                    branch.clone()
                },
            }),
        )
    };
    let mut menu = Menu::new()
        .width(MENU_WIDTH)
        .offset(SIDE_MENU_WIDTH - MENU_WIDTH, 0.0)
        .on_dismiss(send(actions, GitAction::Menu(None)));
    if let Some(base) = &git.base {
        menu = menu.heading("Review").icon_item(
            phosphor::GIT_DIFF,
            format!("Changes since {base}"),
            send(
                actions,
                GitAction::ShowRevision {
                    title: format!("{} since {base}", git.branch),
                    from: Some(base.clone()),
                    to: "HEAD".to_owned(),
                },
            ),
        );
    }
    menu = menu
        .heading("Remote")
        .icon_item(
            phosphor::ARROW_DOWN,
            "Pull",
            send(actions, GitAction::Sync("pull".to_owned())),
        )
        .icon_item(
            phosphor::ARROW_DOWN,
            "Pull (rebase)",
            send(actions, GitAction::Sync("pull_rebase".to_owned())),
        )
        .icon_item(
            phosphor::ARROW_UP,
            "Push",
            send(actions, GitAction::Sync("push".to_owned())),
        )
        .destructive_item(
            phosphor::ARROW_FAT_UP,
            "Force push",
            send(actions, GitAction::Sync("force_push".to_owned())),
        )
        .icon_item(
            phosphor::CLOUD_ARROW_DOWN,
            "Fetch",
            send(actions, GitAction::Sync("fetch".to_owned())),
        )
        .separator()
        .heading("Changes")
        .icon_item(
            phosphor::PLUS,
            "Stage all",
            send(actions, GitAction::StageAll(true)),
        )
        .icon_item(
            phosphor::MINUS,
            "Unstage all",
            send(actions, GitAction::StageAll(false)),
        )
        .destructive_item(
            phosphor::ARROW_COUNTER_CLOCKWISE,
            "Discard all",
            send(
                actions,
                GitAction::Discard(
                    git.changes
                        .iter()
                        .map(|change| change.path.clone())
                        .collect(),
                ),
            ),
        )
        .separator()
        .heading("Stash");
    if !git.changes.is_empty() {
        menu = menu.icon_item(
            phosphor::ARCHIVE,
            "Stash changes",
            send(actions, GitAction::Stash("push".to_owned(), None)),
        );
    }
    if let Some((latest, _)) = git.stashes.first() {
        menu = menu
            .icon_item(
                phosphor::ARCHIVE,
                "Pop latest stash",
                send(actions, GitAction::Stash("pop".to_owned(), Some(*latest))),
            )
            .icon_item(
                phosphor::LIST,
                format!("Stashes ({})…", git.stashes.len()),
                send(actions, GitAction::Menu(Some(GitMenu::Stashes))),
            );
    } else if git.changes.is_empty() {
        menu = menu.disabled_item(phosphor::ARCHIVE, "No changes to stash");
    }
    menu = menu
        .separator()
        .heading("Branch")
        .icon_item(
            phosphor::GIT_BRANCH,
            "New branch…",
            send(actions, GitAction::Ask(GitPrompt::NewBranch(String::new()))),
        );
    if !branch.is_empty() {
        if git.upstream.is_none() && !git.remotes.is_empty() {
            menu = menu.icon_item(
                phosphor::CLOUD_ARROW_UP,
                "Publish branch…",
                task(BranchTask::Publish),
            );
        }
        if git.upstream.is_some() {
            menu = menu.icon_item(
                phosphor::LINK_BREAK,
                "Stop following upstream",
                send(actions, GitAction::UnsetUpstream),
            );
        }
        menu = menu.icon_item(
            phosphor::PENCIL_SIMPLE,
            "Rename branch…",
            task(BranchTask::Rename),
        );
    }
    let others: Vec<&String> = git
        .branches
        .iter()
        .filter(|name| **name != git.branch)
        .take(6)
        .collect();
    for other in others {
        let ask = |task| {
            send(
                actions,
                GitAction::Ask(GitPrompt::Branch {
                    task,
                    branch: other.clone(),
                    name: String::new(),
                }),
            )
        };
        menu = menu
            .separator()
            .heading(other.clone())
            .icon_item(
                phosphor::GIT_MERGE,
                format!("Merge into {}", git.branch),
                ask(BranchTask::Merge),
            )
            .icon_item(
                phosphor::GIT_PULL_REQUEST,
                format!("Rebase {} onto it", git.branch),
                ask(BranchTask::Rebase),
            )
            .destructive_item(phosphor::TRASH, "Delete", ask(BranchTask::Delete));
    }
    menu.show();
}

/// Emits the menu of the stashes, each with its message as a heading and
/// rows that apply, pop or drop it.
fn stashes_menu(git: &GitSummary, actions: &Actions) {
    let mut menu = Menu::new()
        .width(MENU_WIDTH)
        .offset(SIDE_MENU_WIDTH - MENU_WIDTH, 0.0)
        .on_dismiss(send(actions, GitAction::Menu(None)));
    for (index, message) in &git.stashes {
        let stash = |action: &str| send(actions, GitAction::Stash(action.to_owned(), Some(*index)));
        menu = menu
            .heading(format!("stash@{{{index}}} {message}"))
            .icon_item(phosphor::ARROW_LINE_DOWN, "Apply", stash("apply"))
            .icon_item(phosphor::ARCHIVE, "Pop", stash("pop"))
            .destructive_item(phosphor::TRASH, "Drop", stash("drop"));
    }
    menu.show();
}

/// A group of changed files: its header, whose box stages or unstages the whole group, then its files.
fn change_group(git: &GitSummary, group: ChangeGroup, members: &[&ChangedFile], actions: &Actions) {
    let staged = group == ChangeGroup::Staged;
    let paths: Vec<String> = members.iter().map(|change| change.path.clone()).collect();
    let mut header = FoldHeader::new(group.title(), true)
        .variant(FoldHeaderVariant::Group)
        .size(Size::Xs)
        .count(members.len());
    if group != ChangeGroup::Conflicted {
        header = header.check(
            if staged {
                CheckState::On
            } else {
                CheckState::Off
            },
            send(actions, GitAction::Stage(paths, !staged)),
        );
    }
    header.show();
    for change in members {
        key(change.path.clone(), || change_row(git, change, staged, actions));
    }
}

/// One changed file, listed among the staged files when `in_staged`, with
/// the menu on it under it while that menu is open; a conflicted file has
/// no box to stage it until it is resolved.
fn change_row(git: &GitSummary, change: &ChangedFile, in_staged: bool, actions: &Actions) {
    let path = change.path.clone();
    let staged = change.staged || in_staged;
    let acting = git.acting(&path);
    let variant = match change.status {
        _ if change.conflicted => ChangeRowVariant::Conflicted,
        'D' => ChangeRowVariant::Deleted,
        'A' | '?' => ChangeRowVariant::Added,
        _ => ChangeRowVariant::Modified,
    };
    let picking = actions.git.clone();
    let picked = path.clone();
    let mut row = ChangeRow::new(change.name(), change.folder(), change.status)
        .tally(change.added, change.removed)
        .checked(staged)
        .checkable(!change.conflicted)
        .selected(git.selected.contains(&path) || git.diff.as_ref() == Some(&path))
        .variant(variant)
        .size(Size::Xs)
        .on_click(move |held: Modifiers| {
            let pick = if held.shift {
                Pick::Range
            } else if held.ctrl || held.meta {
                Pick::Toggle
            } else {
                Pick::Only
            };
            picking(GitAction::Pick(picked.clone(), pick));
            if pick == Pick::Only {
                picking(GitAction::Open(picked.clone()));
            }
        })
        .on_toggle(send(actions, GitAction::Stage(vec![path.clone()], !staged)))
        .on_menu(send(
            actions,
            GitAction::Menu(Some(GitMenu::Change(path.clone()))),
        ))
        .action(
            phosphor::FILE_TEXT,
            send(actions, GitAction::Open(path.clone())),
        );
    if !change.untracked || change.staged {
        row = row.action(
            phosphor::ARROW_COUNTER_CLOCKWISE,
            send(actions, GitAction::Discard(acting.clone())),
        );
    }
    if !change.conflicted {
        row = row.action(
            if staged {
                phosphor::MINUS
            } else {
                phosphor::PLUS
            },
            send(actions, GitAction::Stage(acting.clone(), !staged)),
        );
    }
    column_with(Modifier::new().fill_max_width(), || {
        row.show();
        if git.menu == Some(GitMenu::Change(path.clone())) {
            change_menu(change, &acting, actions);
        }
    });
}

/// The menu on a changed file, acting on every file in `acting`.
fn change_menu(change: &ChangedFile, acting: &[String], actions: &Actions) {
    let several = acting.len() > 1;
    let count = |word: &str| {
        if several {
            format!("{word} {} files", acting.len())
        } else {
            word.to_owned()
        }
    };
    let acting = acting.to_vec();
    Menu::new()
        .width(MENU_WIDTH)
        .offset(24.0, 0.0)
        .on_dismiss(send(actions, GitAction::Menu(None)))
        .icon_item(
            phosphor::GIT_DIFF,
            "Open changes",
            send(actions, GitAction::Open(change.path.clone())),
        )
        .separator()
        .icon_item(
            if change.staged {
                phosphor::MINUS
            } else {
                phosphor::PLUS
            },
            count(if change.staged { "Unstage" } else { "Stage" }),
            send(actions, GitAction::Stage(acting.clone(), !change.staged)),
        )
        .destructive_item(
            phosphor::ARROW_COUNTER_CLOCKWISE,
            count("Discard"),
            send(actions, GitAction::Discard(acting.clone())),
        )
        .separator()
        .icon_item(
            phosphor::EYE_SLASH,
            count("Add to .gitignore"),
            send(actions, GitAction::Ignore(acting)),
        )
        .show();
}

/// The Graph section: its header with the scope and the remote tasks, then the commits.
fn graph_section(git: &GitSummary, actions: &Actions) {
    let all = git.graph_all;
    let scope = send(actions, GitAction::GraphAll(!all));
    let fetch = send(actions, GitAction::Sync("fetch".to_owned()));
    let pull = send(actions, GitAction::Sync("pull".to_owned()));
    let push = send(actions, GitAction::Sync("push".to_owned()));
    let refresh = send(actions, GitAction::Refresh);
    column_with(Modifier::new().fill_max_width(), || {
        Divider::new().show();
        FoldHeader::new("Graph", git.graph_open)
            .on_toggle(send(actions, GitAction::Fold(GitSection::Graph)))
            .trailing(move || {
                Button::new(if all { "All" } else { "Auto" })
                    .icon(phosphor::GIT_FORK)
                    .trailing_icon(phosphor::CARET_DOWN)
                    .size(Size::Xs)
                    .on_click(scope)
                    .show();
                IconButton::new(phosphor::CLOUD_ARROW_DOWN)
                    .size(Size::Xs)
                    .on_click(fetch)
                    .show();
                IconButton::new(phosphor::ARROW_DOWN)
                    .size(Size::Xs)
                    .on_click(pull)
                    .show();
                IconButton::new(phosphor::ARROW_UP)
                    .size(Size::Xs)
                    .on_click(push)
                    .show();
                IconButton::new(phosphor::ARROWS_CLOCKWISE)
                    .size(Size::Xs)
                    .on_click(refresh)
                    .show();
            })
            .show();
        if !git.graph_open {
            return;
        }
        let scroll = remember_scroll_state();
        let columns = git
            .commits
            .iter()
            .map(|commit| commit.width.max(commit.lane + 1))
            .max()
            .unwrap_or(1);
        column_with(
            Modifier::new()
                .fill_max_width()
                .height(GRAPH_HEIGHT)
                .vertical_scroll(&scroll),
            || {
                match &git.graph_trouble {
                    Some(trouble) => note(trouble),
                    None if git.commits.is_empty() => note("No commits to show."),
                    None => {}
                }
                for commit in &git.commits {
                    key(commit.id.clone(), || {
                        graph_row(git, commit, columns, actions)
                    });
                }
            },
        );
    });
}

/// One commit of the graph, with the menu on it under it while that menu is open.
fn graph_row(git: &GitSummary, commit: &GraphCommit, columns: usize, actions: &Actions) {
    let head = git.head.as_deref() == Some(commit.id.as_str())
        || commit.refs.iter().any(|name| name.starts_with("HEAD"));
    let open = git.menu == Some(GitMenu::History(commit.id.clone()));
    column_with(Modifier::new().fill_max_width(), || {
        GraphRow::new(commit.id.clone(), commit.summary.clone(), commit.lane)
            .refs(commit.refs.clone())
            .color(commit.color)
            .columns(columns)
            .lines(
                commit
                    .edges
                    .iter()
                    .map(|edge| GraphLine {
                        upper: edge.upper,
                        from: edge.from,
                        to: edge.to,
                        color: edge.color,
                    })
                    .collect(),
            )
            .variant(if head {
                GraphRowVariant::Head
            } else {
                GraphRowVariant::Plain
            })
            .size(Size::Xs)
            .on_menu(send(
                actions,
                GitAction::Menu(Some(GitMenu::History(commit.id.clone()))),
            ))
            .show();
        if open {
            let branches: Vec<String> = commit
                .refs
                .iter()
                .map(|name| name.trim_start_matches("HEAD -> ").to_owned())
                .filter(|name| !name.starts_with("tag: ") && *name != git.branch)
                .collect();
            let mut menu = Menu::new()
                .width(MENU_WIDTH)
                .offset(24.0, 0.0)
                .on_dismiss(send(actions, GitAction::Menu(None)));
            for branch in branches {
                menu = menu.icon_item(
                    phosphor::GIT_BRANCH,
                    format!("Check out {branch}"),
                    send(actions, GitAction::Checkout(branch.clone())),
                );
            }
            menu.icon_item(
                phosphor::GIT_DIFF,
                "Show changes",
                send(
                    actions,
                    GitAction::ShowRevision {
                        title: format!("Commit {} \u{2014} {}", commit.id, commit.summary),
                        from: None,
                        to: commit.id.clone(),
                    },
                ),
            )
            .icon_item(
                phosphor::GIT_COMMIT,
                format!("Check out {}", commit.id),
                send(actions, GitAction::Checkout(commit.id.clone())),
            )
            .show();
        }
    });
}

/// The question the panel asks before `prompt` acts, over the whole panel.
fn question(git: &GitSummary, prompt: &GitPrompt, actions: &Actions) {
    let confirm = send(actions, GitAction::Confirm);
    let cancel = send(actions, GitAction::Cancel);
    let typed: Rc<dyn Fn(GitAction)> = actions.git.clone();
    let name_changed = move |text: String| typed(GitAction::PromptText(text));
    let dialog = match prompt {
        GitPrompt::Discard(paths) => ConfirmDialog::new(
            if paths.len() == 1 {
                "Discard changes?".to_owned()
            } else {
                format!("Discard changes to {} files?", paths.len())
            },
            if paths.len() == 1 {
                format!(
                    "{} goes back to how it is in the last commit; a new file is deleted. This cannot be undone.",
                    paths[0]
                )
            } else {
                "The files go back to how they are in the last commit; new files are deleted. This cannot be undone.".to_owned()
            },
            "Discard",
        )
        .variant(ConfirmDialogVariant::Destructive),
        GitPrompt::Amend => ConfirmDialog::new(
            "Amend the last commit?",
            if git.message.trim().is_empty() {
                "What is staged is added to the last commit, which keeps its message.".to_owned()
            } else {
                "What is staged is added to the last commit, and its message is replaced with the one typed.".to_owned()
            },
            "Amend",
        ),
        GitPrompt::ForcePush => ConfirmDialog::new(
            "Force push?",
            format!(
                "{} on the remote is overwritten with the local branch; commits only the remote has are lost.",
                git.upstream.clone().unwrap_or_else(|| git.branch.clone())
            ),
            "Force push",
        )
        .variant(ConfirmDialogVariant::Destructive),
        GitPrompt::NewBranch(name) => ConfirmDialog::new(
            "New branch",
            format!(
                "Made at {} and checked out.",
                git.head.clone().unwrap_or_else(|| git.branch.clone())
            ),
            "Create",
        )
        .naming(name.clone(), "Branch name", name_changed),
        GitPrompt::Branch { task, branch, name } => {
            let current = &git.branch;
            match task {
                BranchTask::Publish => ConfirmDialog::new(
                    "Publish branch",
                    format!(
                        "Pushes {branch} to {} under the name below and follows it.",
                        git.remotes.first().cloned().unwrap_or_else(|| "the remote".to_owned())
                    ),
                    "Publish",
                )
                .naming(name.clone(), "Name on the remote", name_changed),
                BranchTask::Rename => ConfirmDialog::new(
                    "Rename branch",
                    format!("Renames {branch}."),
                    "Rename",
                )
                .naming(name.clone(), "New name", name_changed),
                BranchTask::Delete => ConfirmDialog::new(
                    format!("Delete {branch}?"),
                    "Commits only this branch has can no longer be reached by name.",
                    "Delete",
                )
                .variant(ConfirmDialogVariant::Destructive),
                BranchTask::Merge => ConfirmDialog::new(
                    format!("Merge {branch}?"),
                    format!("Merges {branch} into {current}."),
                    "Merge",
                ),
                BranchTask::Rebase => ConfirmDialog::new(
                    format!("Rebase onto {branch}?"),
                    format!("Replays the commits of {current} on top of {branch}."),
                    "Rebase",
                ),
            }
        }
    };
    dialog.on_confirm(confirm).on_cancel(cancel).show();
}
