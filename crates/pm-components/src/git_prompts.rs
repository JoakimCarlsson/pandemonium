//! The questions the source control panel asks before it does something that cannot be taken back, and the lists it asks a choice from.

use pm_input::FocusId;
use pm_style::{Role, flex, flex_col, gap, items_center, p, pb, px};
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::parts::{line, spacer, wrap};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::tree_actions::{EntryChange, TreeTarget, name_of};
use crate::{Button, Input, Modal};

/// The dimmed window behind a question.
pub const PROMPT_OUTSIDE: FocusId = FocusId(7015);

/// The panel of a question.
pub const PROMPT_INSIDE: FocusId = FocusId(7016);

/// Does what the question asks.
pub const PROMPT_CONFIRM: FocusId = FocusId(7017);

/// Does not.
pub const PROMPT_CANCEL: FocusId = FocusId(7018);

/// The box a session is renamed in.
pub const RENAME_FIELD: FocusId = FocusId(7085);

/// The box a stash is named in.
pub const STASH_FIELD: FocusId = FocusId(7075);

/// The first of the rows of a list of stashes; the rest follow it.
pub const PROMPT_ROW_BASE: u64 = 8750;

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
    /// Whether to close the session at this position, which has changes that are not committed, and remove its worktree with them.
    Close(usize),
    /// What to call the session at this position now.
    Rename(usize),
    /// Whether to close the window while agents are working.
    Quit,
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
            self.git.stash_message = pm_text::Field::default();
        }
        self.git.prompt = Some(prompt);
    }

    /// Carries out `action` if it presses a question or one of its rows, and says whether it did.
    pub(crate) fn press_prompt(&mut self, action: FocusId) -> bool {
        match action {
            PROMPT_INSIDE | STASH_FIELD => {}
            PROMPT_CANCEL | PROMPT_OUTSIDE => self.git.prompt = None,
            PROMPT_CONFIRM => self.confirm_prompt(),
            _ => {
                let Some(row) = action
                    .0
                    .checked_sub(PROMPT_ROW_BASE)
                    .map(|row| row as usize)
                    .filter(|row| *row < STASH_ROWS)
                else {
                    return false;
                };
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
            Prompt::Stashes(_) | Prompt::Close(_) | Prompt::Rename(_) | Prompt::Quit => {}
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
        self.git.trouble = None;
        self.git.busy = Some(match action {
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

/// A button of a question, saying `label`.
fn button(label: &str, id: FocusId, variant: Variant) -> Node {
    Button::new(label, id)
        .variant(variant)
        .size(Size::Sm)
        .build()
}

/// What a question answers to: the dimmed window behind it, its panel, and the buttons that say yes and no.
#[derive(Clone, Copy, Debug)]
pub(crate) struct QuestionIds {
    /// The dimmed window behind the question.
    pub outside: FocusId,
    /// The panel of the question.
    pub inside: FocusId,
    /// The button that says yes.
    pub confirm: FocusId,
    /// The button that says no.
    pub cancel: FocusId,
}

/// The names of the questions of the source control panel.
const GIT_QUESTION: QuestionIds = QuestionIds {
    outside: PROMPT_OUTSIDE,
    inside: PROMPT_INSIDE,
    confirm: PROMPT_CONFIRM,
    cancel: PROMPT_CANCEL,
};

/// A question of the source control panel: a title, the words that explain it, and the button that says yes beside the one that says no.
fn question(title: String, words: &str, confirm: &str, variant: Variant) -> Node {
    question_of(GIT_QUESTION, title, words, confirm, variant)
}

/// A question answering to `ids`: a title, the words that explain it, and the button that says yes beside the one that says no.
pub(crate) fn question_of(
    ids: QuestionIds,
    title: String,
    words: &str,
    confirm: &str,
    variant: Variant,
) -> Node {
    Modal::new(ids.outside, ids.inside)
        .width(WIDTH)
        .child(
            node()
                .styles([flex(), flex_col(), gap(2), p(5)])
                .children(wrap(&title, 15, WIDTH - 40.0).iter().map(|title| {
                    line(title, 15, 1.4, Role::Text).styles([pm_style::font_medium()])
                }))
                .children(
                    wrap(words, 13, WIDTH - 40.0)
                        .iter()
                        .map(|words| line(words, 13, 1.5, Role::Muted)),
                ),
        )
        .child(
            node()
                .styles([flex(), items_center(), gap(2), px(5), pb(5)])
                .child(spacer())
                .child(button("Cancel", ids.cancel, Variant::Ghost))
                .child(button(confirm, ids.confirm, variant)),
        )
        .build()
}

/// A box named `title` for `field` to be typed in, with the button `confirm` that accepts it.
fn naming_box(
    title: &str,
    field: &pm_text::Field,
    id: FocusId,
    placeholder: &str,
    confirm: &str,
) -> Node {
    Modal::new(PROMPT_OUTSIDE, PROMPT_INSIDE)
        .width(WIDTH)
        .child(
            node()
                .styles([flex(), flex_col(), gap(3), p(5)])
                .child(line(title, 15, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .child(
                    Input::new(field, id)
                        .placeholder(placeholder)
                        .size(Size::Md)
                        .build(),
                ),
        )
        .child(
            node()
                .styles([flex(), items_center(), gap(2), px(5), pb(5)])
                .child(spacer())
                .child(button("Cancel", PROMPT_CANCEL, Variant::Ghost))
                .child(button(confirm, PROMPT_CONFIRM, Variant::Default)),
        )
        .build()
}

/// The list of stashes to `action` one of, with the button that leaves it.
fn stash_list(state: &WorkspaceState, action: StashAction) -> Node {
    let mut panel = node().styles([flex(), flex_col(), gap(1), p(3)]).child(
        node()
            .styles([px(2), pb(2)])
            .child(line(action.title(), 15, 1.4, Role::Text).styles([pm_style::font_medium()])),
    );
    if state.git.stashes.is_empty() {
        panel = panel.child(node().styles([px(2), pb(2)]).child(line(
            "There are no stashes.",
            13,
            1.4,
            Role::Subtle,
        )));
    }
    panel = panel.children(state.git.stashes.iter().take(STASH_ROWS).enumerate().map(
        |(row, stash)| {
            crate::popover::row(
                &format!("stash@{{{}}}", stash.index),
                &stash.message,
                false,
                FocusId(PROMPT_ROW_BASE + row as u64),
            )
        },
    ));
    Modal::new(PROMPT_OUTSIDE, PROMPT_INSIDE)
        .width(WIDTH)
        .child(panel)
        .child(
            node()
                .styles([flex(), items_center(), gap(2), px(5), pb(5)])
                .child(spacer())
                .child(button("Cancel", PROMPT_CANCEL, Variant::Ghost)),
        )
        .build()
}

/// The question or list open, as a node.
pub fn prompt(state: &WorkspaceState, open: &Prompt) -> Node {
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
                Variant::Destructive,
            )
        }
        Prompt::Amend => question(
            "Amend the last commit?".to_owned(),
            "The last commit is rewritten with what is staged and this message. If it was pushed, the branch will have to be force pushed.",
            "Amend",
            Variant::Destructive,
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
                Variant::Destructive,
            )
        }
        Prompt::Skip => question(
            "Skip this commit?".to_owned(),
            "The commit it stopped on is left out, and the rest go on.",
            "Skip",
            Variant::Destructive,
        ),
        Prompt::StashMessage => naming_box(
            "Stash changes",
            &state.git.stash_message,
            STASH_FIELD,
            "Message (optional)",
            "Stash",
        ),
        Prompt::Rename(_) => naming_box(
            "Rename session",
            &state.git.rename,
            RENAME_FIELD,
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
                Variant::Destructive,
            )
        }
        Prompt::Close(position) => {
            let title = state
                .sessions
                .get(*position)
                .map_or("this session", |session| session.title.as_str());
            question(
                format!("Close {title} with changes that are not committed?"),
                "Its worktree is removed with the changes in it. They are lost when it is closed.",
                "Close",
                Variant::Destructive,
            )
        }
        Prompt::Remove(targets) => {
            let (title, words) = match targets.as_slice() {
                [target] if target.folder => (
                    format!("Delete {}?", name_of(&target.path)),
                    "The folder and everything in it is taken off the disk. This cannot be undone.",
                ),
                [target] => (
                    format!("Delete {}?", name_of(&target.path)),
                    "The file is taken off the disk. This cannot be undone.",
                ),
                _ => (
                    format!("Delete {} items?", targets.len()),
                    "They are taken off the disk, each folder with everything in it. This cannot be undone.",
                ),
            };
            question(title, words, "Delete", Variant::Destructive)
        }
        Prompt::Stashes(action) => stash_list(state, *action),
        Prompt::Quit => question(
            "Close the window while agents are working?".to_owned(),
            "The agents keep working on the server; open the window again to see what they did.",
            "Close Window",
            Variant::Default,
        ),
    }
}
