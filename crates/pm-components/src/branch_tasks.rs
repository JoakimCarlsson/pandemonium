//! The questions and lists for publishing a branch, choosing what it follows, renaming, deleting, merging and rebasing it.

use crate::areas::GitEvent;
use crate::events;
use rustle::{Alignment, Arrangement, TextOverflow};

use crate::target::Target;
use crate::theme;
use rustle::Modifier;
use rustle::TextFieldState;

use crate::theme::Role;

use crate::button::Variant;
use crate::git_prompts::Prompt;
use crate::parts::{line, wrap};
use crate::popover::row;
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Checkbox, Input};

/// How many rows a list shows at once.
const ROWS: usize = 12;

/// How wide a question is, in logical pixels.
const WIDTH: f32 = 400.0;

/// What is being done to a branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchTask {
    /// Pushing the checked out branch to a remote under a name, and following it.
    Publish,
    /// Choosing the remote branch the checked out branch follows.
    Upstream,
    /// Giving the checked out branch another name.
    Rename,
    /// Removing a local branch.
    Delete,
    /// Merging a branch into the checked out one.
    Merge,
    /// Replaying the checked out branch on top of another.
    Rebase,
}

/// What the open question or list about a branch has been told and what has been chosen in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Branching {
    /// What is being done.
    pub task: BranchTask,
    /// What the server said about the remotes and the branch chosen, once it has.
    pub info: Option<pm_git::BranchInfo>,
    /// The branch chosen from a list to be deleted, once it is.
    pub chosen: Option<String>,
    /// Which of the remotes to publish to, by position.
    pub remote: usize,
    /// Whether to carry the rename or deletion over to the branch on the remote.
    pub with_remote: bool,
    /// The name typed, or what the list is searched for.
    pub field: TextFieldState,
}

impl BranchTask {
    /// What the title of the question says.
    fn title(self) -> &'static str {
        match self {
            Self::Publish => "Publish branch",
            Self::Upstream => "Follow a remote branch",
            Self::Rename => "Rename branch",
            Self::Delete => "Delete a branch",
            Self::Merge => "Merge a branch into this one",
            Self::Rebase => "Rebase this branch onto",
        }
    }
}

impl WorkspaceState {
    /// Opens the question or list for `task`, asks the server for what it needs, and says why when it cannot be done.
    pub(crate) fn branch_task(&mut self, task: BranchTask) {
        let Some(session) = self.review_index() else {
            return;
        };
        let current = self.current_branch_name();
        let field = match task {
            BranchTask::Publish | BranchTask::Rename => {
                TextFieldState::new(current.as_deref().unwrap_or(""))
            }
            _ => TextFieldState::default(),
        };
        self.git.branching = Some(Branching {
            task,
            info: None,
            chosen: None,
            remote: 0,
            with_remote: false,
            field,
        });
        self.ask(Prompt::Branch(task));
        self.outbox.push(Outgoing::BranchInfo {
            session,
            branch: None,
        });
        self.outbox.push(Outgoing::SessionBranches {
            session,
            submodule: self.git.repository.clone(),
        });
    }

    /// The state with the question or list for `task` open.
    pub fn branch_asked(mut self, task: BranchTask) -> Self {
        self.branch_task(task);
        self
    }

    /// The branch the panel's repository has checked out, when it is on one.
    fn current_branch_name(&self) -> Option<String> {
        self.review_session()
            .filter(|session| session.on_branch)
            .and_then(|_| self.head_of(self.git.repository.as_deref()))
            .filter(|name| !name.is_empty() && !name.starts_with('@'))
    }

    /// The state after the server answered the question about `branch` of the session at `session` with `info`.
    pub fn with_branch_info(
        mut self,
        session: usize,
        branch: Option<String>,
        info: pm_git::BranchInfo,
    ) -> Self {
        if self.review_index() == Some(session)
            && let Some(open) = self.git.branching.as_mut()
            && open.chosen == branch
        {
            open.info = Some(info);
        }
        self
    }

    /// The rows the open list offers: each branch's name, what to say under it and whether it is the one now chosen.
    fn branch_choices(&self) -> Vec<(String, String, bool)> {
        let Some(open) = self.git.branching.as_ref() else {
            return Vec::new();
        };
        let Some(info) = open.info.as_ref() else {
            return Vec::new();
        };
        if open.task == BranchTask::Publish {
            return if info.remotes.len() > 1 {
                info.remotes
                    .iter()
                    .enumerate()
                    .map(|(position, remote)| {
                        (remote.clone(), String::new(), position == open.remote)
                    })
                    .collect()
            } else {
                Vec::new()
            };
        }
        if open.chosen.is_some() || matches!(open.task, BranchTask::Rename) {
            return Vec::new();
        }
        let current = self.current_branch_name().unwrap_or_default();
        let following = self
            .review_session()
            .and_then(|session| session.tracking.upstream.clone());
        let wanted = open.field.text().trim().to_lowercase();
        let names: Vec<&String> = match open.task {
            BranchTask::Upstream => info.remote_branches.iter().collect(),
            BranchTask::Delete => self
                .git
                .branches
                .iter()
                .filter(|name| !info.remote_branches.contains(name))
                .collect(),
            _ => self.git.branches.iter().collect(),
        };
        names
            .into_iter()
            .filter(|name| **name != current && name.to_lowercase().contains(&wanted))
            .take(ROWS)
            .map(|name| {
                (
                    name.clone(),
                    String::new(),
                    following.as_ref() == Some(name),
                )
            })
            .collect()
    }

    /// Carries out `action` if it presses something in the question or list about a branch, and says whether it did.
    pub(crate) fn press_branch_task(&mut self, action: Target) -> bool {
        if self.git.branching.is_none() || !matches!(self.git.prompt, Some(Prompt::Branch(_))) {
            return false;
        }
        match action {
            Target::Git(GitEvent::BranchTaskField) => {}
            Target::Git(GitEvent::BranchTaskRemote) => {
                if let Some(open) = self.git.branching.as_mut() {
                    open.with_remote = !open.with_remote;
                }
            }
            _ => {
                let Target::Git(GitEvent::BranchTaskRow(row)) = action else {
                    return false;
                };
                if row >= ROWS {
                    return false;
                }
                self.choose_branch(row);
            }
        }
        true
    }

    /// Does what the list open asks of the branch at its `row`-th row.
    fn choose_branch(&mut self, row: usize) {
        let Some((name, _, _)) = self.branch_choices().into_iter().nth(row) else {
            return;
        };
        let session = self.review_index();
        let Some(task) = self.git.branching.as_ref().map(|open| open.task) else {
            return;
        };
        match task {
            BranchTask::Publish => {
                if let Some(open) = self.git.branching.as_mut() {
                    open.remote = row;
                }
            }
            BranchTask::Delete => {
                if let Some(open) = self.git.branching.as_mut() {
                    open.chosen = Some(name.clone());
                    open.info = None;
                }
                if let Some(session) = session {
                    self.outbox.push(Outgoing::BranchInfo {
                        session,
                        branch: Some(name),
                    });
                }
            }
            BranchTask::Upstream => self.branch_operation(
                "set_upstream",
                "Updating upstream…",
                String::new(),
                Some(name),
                None,
                false,
            ),
            BranchTask::Merge => {
                self.branch_operation("merge", "Merging…", name, None, None, false)
            }
            BranchTask::Rebase => {
                self.branch_operation("rebase", "Rebasing…", name, None, None, false)
            }
            BranchTask::Rename => {}
        }
    }

    /// Closes the question or list and asks the server to do `action` to `branch`, saying `doing` meanwhile.
    pub(crate) fn branch_operation(
        &mut self,
        action: &'static str,
        doing: &'static str,
        branch: String,
        target: Option<String>,
        remote: Option<String>,
        with_remote: bool,
    ) {
        let Some(session) = self.review_index() else {
            return;
        };
        self.git.prompt = None;
        self.git.branching = None;
        self.await_git(doing);
        self.outbox.push(Outgoing::BranchOperation {
            session,
            action,
            branch,
            target,
            remote,
            with_remote,
        });
    }

    /// Takes the button that says yes in the question or list about `task`.
    pub(crate) fn confirm_branch_task(&mut self, task: BranchTask) {
        let Some(open) = self.git.branching.clone() else {
            return;
        };
        let typed = open.field.text().trim().to_owned();
        let current = self.current_branch_name().unwrap_or_default();
        match task {
            BranchTask::Publish => {
                let remote = open
                    .info
                    .as_ref()
                    .and_then(|info| info.remotes.get(open.remote).cloned());
                let name = if typed.is_empty() { current } else { typed };
                if remote.is_some() {
                    self.branch_operation(
                        "publish",
                        "Publishing…",
                        String::new(),
                        Some(name),
                        remote,
                        false,
                    );
                } else {
                    self.git.prompt = Some(Prompt::Branch(task));
                }
            }
            BranchTask::Rename if !typed.is_empty() && typed != current => self.branch_operation(
                "rename",
                "Renaming…",
                current,
                Some(typed),
                None,
                open.with_remote,
            ),
            BranchTask::Delete => match (open.chosen, open.info) {
                (Some(branch), Some(info)) if info.elsewhere.is_none() => self.branch_operation(
                    "delete",
                    "Deleting…",
                    branch,
                    None,
                    None,
                    open.with_remote && info.follows.is_some(),
                ),
                (None, _) => self.choose_branch(0),
                _ => self.git.prompt = Some(Prompt::Branch(task)),
            },
            BranchTask::Upstream | BranchTask::Merge | BranchTask::Rebase => self.choose_branch(0),
            BranchTask::Rename => self.git.prompt = Some(Prompt::Branch(task)),
        }
    }
}

/// `count` commits, worded for one or many.
fn commits(count: usize) -> String {
    if count == 1 {
        "1 commit is".to_owned()
    } else {
        format!("{count} commits are")
    }
}

/// Emits a row with a check box that says `label`, ticked while `checked`.
#[track_caller]
fn tick(label: &str, checked: bool) {
    rustle::row_arranged(
        events::pressed(Modifier::new(), Target::Git(GitEvent::BranchTaskRemote))
            .padding_xy(theme::sp(2), 0.0),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            Checkbox::new(checked, Target::Git(GitEvent::BranchTaskRemote)).build();
            line(label, 13, 1.4, Role::Text);
        },
    );
}

/// Emits `words` wrapped to the width of a question, one muted line after another.
#[track_caller]
fn say(words: &str) {
    for words in wrap(words, 13, WIDTH - 40.0) {
        line(words, 13, 1.5, Role::Muted);
    }
}

/// Emits the rows of `choices`, each a branch or remote ticked while chosen, answering to the row ids in order.
#[track_caller]
fn choice_rows(choices: &[(String, String, bool)]) {
    for (position, (name, description, chosen)) in choices.iter().enumerate() {
        row(
            name,
            description,
            *chosen,
            Target::Git(GitEvent::BranchTaskRow(position)),
        );
    }
}

/// Emits a line of `words` set as a title, `pixels` tall, in the text colour and medium weight.
#[track_caller]
fn title_line(words: impl Into<String>, pixels: u32) {
    rustle::basic_text(
        Modifier::new(),
        words,
        theme::strong(pixels, Role::Text),
        TextOverflow::Ellipsis,
    );
}

/// Emits the box the name of a branch is typed in, or a list is searched in, showing `placeholder` while it is empty.
#[track_caller]
fn field(open: &Branching, placeholder: &str) {
    Input::new(&open.field, Target::Git(GitEvent::BranchTaskField))
        .modifier(Modifier::new().fill_max_width())
        .placeholder(placeholder)
        .size(Size::Md)
        .build();
}

/// Emits the question or list about a branch that is open, in a modal; nothing when none is open.
pub fn branch_prompt(state: &WorkspaceState, task: BranchTask) {
    let Some(open) = state.git.branching.as_ref() else {
        return;
    };
    let choices = state.branch_choices();
    let loading = open.info.is_none();
    let current = state.current_branch_name().unwrap_or_default();
    let upstream = state
        .review_session()
        .and_then(|session| session.tracking.upstream.clone());
    crate::git_prompts::git_modal(|| {
        let mut confirm: Option<(&str, Variant)> = None;
        rustle::column_arranged(
            Modifier::new().padding(theme::sp(5)),
            Arrangement::SpacedBy(theme::sp(3)),
            Alignment::Start,
            || {
                title_line(task.title(), 15);
                if loading {
                    line("Reading the remotes…", 13, 1.4, Role::Subtle);
                } else if let Some(info) = open.info.as_ref() {
                    match task {
                        BranchTask::Publish if info.remotes.is_empty() => {
                            say("This repository has no remote to publish to.");
                        }
                        BranchTask::Publish => {
                            if info.remotes.len() > 1 {
                                line("Remote", 12, 1.4, Role::Subtle);
                                choice_rows(&choices);
                            }
                            line("Branch name on the remote", 12, 1.4, Role::Subtle);
                            field(open, &current);
                            confirm = Some(("Publish", Variant::Default));
                        }
                        BranchTask::Rename => {
                            field(open, "New name");
                            if let Some(upstream) = &upstream {
                                tick(
                                    &format!("Also rename {upstream} on the remote"),
                                    open.with_remote,
                                );
                            }
                            confirm = Some(("Rename", Variant::Default));
                        }
                        BranchTask::Delete if open.chosen.is_some() => {
                            let name = open.chosen.clone().unwrap_or_default();
                            title_line(format!("Delete {name}?"), 13);
                            if let Some(path) = &info.elsewhere {
                                say(&format!(
                                    "{name} is checked out in {path}, so it cannot be deleted from here."
                                ));
                            } else {
                                let mut warnings = Vec::new();
                                if info.unmerged > 0 {
                                    warnings.push(format!(
                                        "{} not merged into {current}",
                                        commits(info.unmerged)
                                    ));
                                }
                                if info.unpushed > 0 {
                                    warnings.push(format!("{} not pushed", commits(info.unpushed)));
                                }
                                let words = if warnings.is_empty() {
                                    "The branch is removed. Its commits are all in other branches."
                                        .to_owned()
                                } else {
                                    format!(
                                        "{}. They are lost when it is deleted.",
                                        warnings.join(", ")
                                    )
                                };
                                say(&words);
                                if let Some(follows) = &info.follows {
                                    tick(
                                        &format!("Also delete {follows} on the remote"),
                                        open.with_remote,
                                    );
                                }
                                confirm = Some(("Delete", Variant::Destructive));
                            }
                        }
                        _ => {
                            field(open, "Search branches");
                            if choices.is_empty() {
                                line(
                                    if task == BranchTask::Upstream && info.remotes.is_empty() {
                                        "This repository has no remote."
                                    } else {
                                        "No branches."
                                    },
                                    13,
                                    1.4,
                                    Role::Subtle,
                                );
                            }
                            choice_rows(&choices);
                        }
                    }
                }
            },
        );
        crate::git_prompts::buttons(
            Target::Git(GitEvent::PromptCancel),
            confirm
                .filter(|_| !loading)
                .map(|(label, variant)| (label, Target::Git(GitEvent::PromptConfirm), variant)),
        );
    });
}
