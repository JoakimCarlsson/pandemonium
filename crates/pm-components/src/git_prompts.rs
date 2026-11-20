//! The questions the source control panel asks before it does something that cannot be taken back, and the lists it asks a choice from.

use crate::areas::GitEvent;
use crate::kit;
use crate::target::Target;
use crate::theme;
use crate::theme::Role;
use rustle::Modifier;

use crate::button::Variant;
use crate::parts::{line, wrap};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::tree_actions::{EntryChange, TreeTarget, name_of};
use crate::{Input, Modal};

/// How many stashes a list shows.
const STASH_ROWS: usize = 12;

/// How wide a question is, in logical pixels.
const WIDTH: f32 = 400.0;

/// What is done with the stash a list is chosen from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StashAction {
    /// Applies it and keeps it.
    Apply,
    /// Applies it and removes it.
    Pop,
    /// Removes it.
    Drop,
}

impl StashAction {
    /// The word the request carries, and the title of the list.
    fn name(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Pop => "pop",
            Self::Drop => "drop",
        }
    }

    /// What the title of the list says.
    fn title(self) -> &'static str {
        match self {
            Self::Apply => "Apply a stash",
            Self::Pop => "Pop a stash",
            Self::Drop => "Drop a stash",
        }
    }
}

/// What the reader is being asked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Prompt {
    /// Whether to put the files at these positions back as they were at the last commit.
    Discard(Vec<usize>),
    /// Whether to rewrite the last commit.
    Amend,
    /// Whether to give up the merge, rebase or cherry-pick that stopped.
    Abort,
    /// Whether to leave out the commit the rebase or cherry-pick stopped on.
    Skip,
    /// What to call the stash about to be made.
    StashMessage,
    /// Which stash to apply, pop or drop.
    Stashes(StashAction),
    /// Whether to take files and folders off the disk.
    Remove(Vec<TreeTarget>),
    /// Whether to close the tab at this position of the open files, which has changes that were not saved.
    Unsaved(usize),
    /// Whether to close the session at this position, which has changes that are not committed, and remove its worktree with them; for a chat, whether to remove it with its scratch folder.
    Close(usize),
    /// What to call the session at this position now.
    Rename(usize),
    /// Whether to close the window while agents are working.
    Quit,
    /// A question or list about publishing, following, renaming, deleting, merging or rebasing a branch.
    Branch(crate::branch_tasks::BranchTask),
}

impl WorkspaceState {
    /// The state after the reader asked to close the window: it asks first while agents are working and the settings say to, and is otherwise ready to close.
    pub fn close_asked(mut self) -> Self {
        if self.window.general.confirm_close && self.waiting() && !self.quitting {
            self.ask(Prompt::Quit);
        } else {
            self.quitting = true;
        }
        self
    }

    /// The state with `prompt` put to the reader.
    pub fn asked(mut self, prompt: Prompt) -> Self {
        self.ask(prompt);
        self
    }

    /// Asks `prompt` of the reader.
    pub(crate) fn ask(&mut self, prompt: Prompt) {
        self.git.menu = None;
        self.git.branch_menu = false;
        if prompt == Prompt::StashMessage {
            self.git.stash_message = rustle::TextFieldState::default();
        }
        self.git.prompt = Some(prompt);
    }

    /// Carries out `action` if it presses a question or one of its rows, and says whether it did.
    pub(crate) fn press_prompt(&mut self, action: Target) -> bool {
        if self.press_branch_task(action) {
            return true;
        }
        match action {
            Target::Git(GitEvent::PromptInside) | Target::Git(GitEvent::StashField) => {}
            Target::Git(GitEvent::PromptCancel) | Target::Git(GitEvent::PromptOutside) => {
                self.git.prompt = None
            }
            Target::Git(GitEvent::PromptConfirm) => self.confirm_prompt(),
            _ => {
                let Target::Git(GitEvent::StashRow(row)) = action else {
                    return false;
                };
                if row >= STASH_ROWS {
                    return false;
                }
                self.choose_stash(row);
            }
        }
        true
    }

    /// Does what the question open says, and closes it.
    fn confirm_prompt(&mut self) {
        let Some(prompt) = self.git.prompt.take() else {
            return;
        };
        if let Prompt::Close(position) = prompt {
            self.close_session(position, true, true);
            return;
        }
        if prompt == Prompt::Quit {
            self.quitting = true;
            return;
        }
        if let Prompt::Branch(task) = prompt {
            self.confirm_branch_task(task);
            return;
        }
        if let Prompt::Rename(position) = prompt {
            let title = self.git.rename.text().trim().to_owned();
            if let Some(held) = self
                .sessions
                .get_mut(position)
                .filter(|_| !title.is_empty())
            {
                held.title = title.clone();
                self.outbox.push(Outgoing::Rename {
                    session: position,
                    title,
                });
            }
            return;
        }
        let Some(session) = self.review_index() else {
            return;
        };
        self.git.trouble = None;
        match prompt {
            Prompt::Discard(indices) => {
                self.git.busy = Some("Discarding…");
                let paths = self.paths_of(&indices);
                self.outbox.push(Outgoing::Discard { session, paths });
            }
            Prompt::Amend => self.commit_with(false, true),
            Prompt::Abort => self.step("abort", "Aborting…"),
            Prompt::Skip => self.step("skip", "Skipping…"),
            Prompt::StashMessage => {
                let message = self.git.stash_message.text().trim().to_owned();
                self.git.busy = Some("Stashing…");
                self.outbox.push(Outgoing::Stash {
                    session,
                    action: "push",
                    index: None,
                    message: Some(message).filter(|message| !message.is_empty()),
                });
            }
            Prompt::Stashes(_)
            | Prompt::Close(_)
            | Prompt::Rename(_)
            | Prompt::Quit
            | Prompt::Branch(_) => {}
            Prompt::Unsaved(index) => self.discard_file(index),
            Prompt::Remove(targets) => {
                for target in targets {
                    self.drop_viewer_of(&target.path);
                    self.send_entry(EntryChange::Remove { path: target.path });
                }
            }
        }
    }

    /// Does what the list open asks of the stash at its `row`-th row, and closes it.
    fn choose_stash(&mut self, row: usize) {
        let Some(Prompt::Stashes(action)) = self.git.prompt else {
            return;
        };
        let Some(session) = self.review_index() else {
            return;
        };
        let Some(stash) = self.git.stashes.get(row) else {
            return;
        };
        let index = stash.index;
        self.git.prompt = None;
        self.await_git(match action {
            StashAction::Apply => "Applying…",
            StashAction::Pop => "Popping…",
            StashAction::Drop => "Dropping…",
        });
        self.outbox.push(Outgoing::Stash {
            session,
            action: action.name(),
            index: Some(index),
            message: None,
        });
    }

    /// The paths of the files at `indices` of the open session's changes.
    pub(crate) fn paths_of(&self, indices: &[usize]) -> Vec<String> {
        self.review_session().map_or_else(Vec::new, |session| {
            indices
                .iter()
                .filter_map(|&index| session.changes.get(index))
                .map(|change| change.path.clone())
                .collect()
        })
    }
}

/// What a question answers to: the dimmed window behind it, its panel, and the buttons that say yes and no.
#[derive(Clone, Copy, Debug)]
pub(crate) struct QuestionIds {
    /// The dimmed window behind the question.
    pub outside: Target,
    /// The panel of the question.
    pub inside: Target,
    /// The button that says yes.
    pub confirm: Target,
    /// The button that says no.
    pub cancel: Target,
}

/// The names of the questions of the source control panel.
const GIT_QUESTION: QuestionIds = QuestionIds {
    outside: Target::Git(GitEvent::PromptOutside),
    inside: Target::Git(GitEvent::PromptInside),
    confirm: Target::Git(GitEvent::PromptConfirm),
    cancel: Target::Git(GitEvent::PromptCancel),
};

/// Emits a question of the source control panel about something that cannot be taken back: a title, the words that explain it, and the destructive button that says yes beside the one that says no.
#[track_caller]
fn question(title: String, words: &str, confirm: &str) {
    question_of(GIT_QUESTION, title, words, confirm, Variant::Destructive);
}

/// Emits the row of a question's buttons, pushed to its right end: the one that says no, then `confirm` when there is one.
#[track_caller]
pub(crate) fn buttons(cancel: Target, confirm: Option<(&str, Target, Variant)>) {
    kit::row(
        Modifier::new().fill_max_width().padding_each(
            theme::sp(5),
            0.0,
            theme::sp(5),
            theme::sp(5),
        ),
        2,
        || {
            rustle::spacer(rustle::Modifier::new().weight(1.0));
            kit::small_button("Cancel", cancel, Variant::Ghost).build();
            if let Some((words, id, variant)) = confirm {
                kit::small_button(words, id, variant).build();
            }
        },
    );
}

/// Emits a question answering to `ids`: a title, the words that explain it, and the button that says yes beside the one that says no.
#[track_caller]
pub(crate) fn question_of(
    ids: QuestionIds,
    title: String,
    words: &str,
    confirm: &str,
    variant: Variant,
) {
    Modal::new(ids.outside, ids.inside)
        .width(WIDTH)
        .build_with(|| {
            kit::column(Modifier::new().padding(theme::sp(5)), 2, || {
                for title in wrap(&title, 15, WIDTH - 40.0) {
                    kit::text(Modifier::new(), title, kit::strong(15, Role::Text));
                }
                for words in wrap(words, 13, WIDTH - 40.0) {
                    line(words, 13, 1.5, Role::Muted);
                }
            });
            buttons(ids.cancel, Some((confirm, ids.confirm, variant)));
        });
}

/// Emits a dialog of the source control panel holding what `content` emits, answering to the panel's question names.
#[track_caller]
pub(crate) fn git_modal(content: impl FnOnce()) {
    Modal::new(GIT_QUESTION.outside, GIT_QUESTION.inside)
        .width(WIDTH)
        .build_with(content);
}

/// Emits a box named `title` for `field` to be typed in, with the button `confirm` that accepts it.
#[track_caller]
fn naming_box(
    title: &str,
    field: &rustle::TextFieldState,
    id: Target,
    placeholder: &str,
    confirm: &str,
) {
    git_modal(|| {
        kit::column(
            Modifier::new().fill_max_width().padding(theme::sp(5)),
            3,
            || {
                kit::text(Modifier::new(), title, kit::strong(15, Role::Text));
                Input::new(field, id)
                    .modifier(Modifier::new().fill_max_width())
                    .placeholder(placeholder)
                    .size(Size::Md)
                    .build();
            },
        );
        buttons(
            GIT_QUESTION.cancel,
            Some((confirm, GIT_QUESTION.confirm, Variant::Default)),
        );
    });
}

/// Emits `content` inset two steps at either side and below: a line of the list of stashes that is not a row.
#[track_caller]
fn stash_line(content: impl FnOnce()) {
    kit::column(
        Modifier::new().padding_each(theme::sp(2), 0.0, theme::sp(2), theme::sp(2)),
        0,
        content,
    );
}

/// Emits the list of stashes to `action` one of, with the button that leaves it.
fn stash_list(state: &WorkspaceState, action: StashAction) {
    git_modal(|| {
        kit::column(
            Modifier::new().fill_max_width().padding(theme::sp(3)),
            1,
            || {
                stash_line(|| {
                    kit::text(Modifier::new(), action.title(), kit::strong(15, Role::Text))
                });
                if state.git.stashes.is_empty() {
                    stash_line(|| line("There are no stashes.", 13, 1.4, Role::Subtle));
                }
                for (position, stash) in state.git.stashes.iter().take(STASH_ROWS).enumerate() {
                    crate::popover::row(
                        &format!("stash@{{{}}}", stash.index),
                        &stash.message,
                        false,
                        Target::Git(GitEvent::StashRow(position)),
                    );
                }
            },
        );
        buttons(GIT_QUESTION.cancel, None);
    });
}

/// Emits the question or list open.
#[track_caller]
pub fn prompt(state: &WorkspaceState, open: &Prompt) {
    match open {
        Prompt::Discard(indices) => {
            let paths = state.paths_of(indices);
            let created = state
                .review_session()
                .map(|session| {
                    indices
                        .iter()
                        .filter_map(|&index| session.changes.get(index))
                        .all(|change| change.is_created())
                })
                .unwrap_or(false);
            let title = match (paths.as_slice(), created) {
                ([one], true) => format!("Delete {one}?"),
                ([one], false) => format!("Discard changes to {one}?"),
                (many, true) => format!("Delete {} files?", many.len()),
                (many, false) => format!("Discard changes to {} files?", many.len()),
            };
            question(
                title,
                "They go back to how they were at the last commit, and files that are new are deleted. This cannot be undone.",
                if created { "Delete" } else { "Discard" },
            )
        }
        Prompt::Amend => question(
            "Amend the last commit?".to_owned(),
            "The last commit is rewritten with what is staged and this message. If it was pushed, the branch will have to be force pushed.",
            "Amend",
        ),
        Prompt::Abort => {
            let name = state
                .git
                .operation
                .as_ref()
                .map_or("operation", |waiting| waiting.name());
            question(
                format!("Abort the {}?", name.to_lowercase()),
                "Everything done since it began is thrown away, and the branch goes back to how it was.",
                "Abort",
            )
        }
        Prompt::Skip => question(
            "Skip this commit?".to_owned(),
            "The commit it stopped on is left out, and the rest go on.",
            "Skip",
        ),
        Prompt::StashMessage => naming_box(
            "Stash changes",
            &state.git.stash_message,
            Target::Git(GitEvent::StashField),
            "Message (optional)",
            "Stash",
        ),
        Prompt::Rename(_) => naming_box(
            "Rename session",
            &state.git.rename,
            Target::Git(GitEvent::RenameField),
            "Title",
            "Rename",
        ),
        Prompt::Unsaved(index) => {
            let name = state
                .git
                .viewers
                .get(*index)
                .map_or("this file", |viewer| name_of(&viewer.path));
            question(
                format!("Close {name} without saving?"),
                "It has changes that are not saved. They are lost when it is closed.",
                "Discard",
            )
        }
        Prompt::Close(position) => {
            let session = state.sessions.get(*position);
            let title = session.map(|session| session.title.as_str());
            if session.is_some_and(crate::sample::Session::is_chat) {
                question(
                    format!("Remove {}?", title.unwrap_or("this chat")),
                    "Its folder is deleted with the files in it. The conversation stays in the history.",
                    "Remove",
                )
            } else {
                question(
                    format!(
                        "Close {} with changes that are not committed?",
                        title.unwrap_or("this session")
                    ),
                    "Its worktree is removed with the changes in it. They are lost when it is closed.",
                    "Close",
                )
            }
        }
        Prompt::Remove(targets) => {
            let (title, words) = match targets.as_slice() {
                [target] => (
                    format!("Delete {}?", name_of(&target.path)),
                    if target.folder {
                        "The folder and everything in it is taken off the disk. This cannot be undone."
                    } else {
                        "The file is taken off the disk. This cannot be undone."
                    },
                ),
                _ => (
                    format!("Delete {} items?", targets.len()),
                    "They are taken off the disk, each folder with everything in it. This cannot be undone.",
                ),
            };
            question(title, words, "Delete")
        }
        Prompt::Stashes(action) => stash_list(state, *action),
        Prompt::Branch(task) => crate::branch_tasks::branch_prompt(state, *task),
        Prompt::Quit => question_of(
            GIT_QUESTION,
            "Close the window while agents are working?".to_owned(),
            "The agents keep working on the server; open the window again to see what they did.",
            "Close Window",
            Variant::Default,
        ),
    }
}
