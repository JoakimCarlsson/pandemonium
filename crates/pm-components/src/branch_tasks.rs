//! The questions and lists for publishing a branch, choosing what it follows, renaming, deleting, merging and rebasing it.

use pm_input::FocusId;
use pm_style::{Role, flex, flex_col, gap, items_center, p, pb, px};
use pm_text::Field;
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::git_prompts::{PROMPT_CANCEL, PROMPT_CONFIRM, PROMPT_INSIDE, PROMPT_OUTSIDE, Prompt};
use crate::parts::{line, spacer, wrap};
use crate::popover::row;
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Button, Checkbox, Input, Modal};

/// The box a branch is named in, or the list of branches is searched in.
pub const BRANCH_TASK_FIELD: FocusId = FocusId(7110);

/// The check box that carries a rename or a deletion over to the branch on the remote.
pub const BRANCH_TASK_REMOTE: FocusId = FocusId(7111);

/// The first of the rows of the list that is open; the rest follow it.
pub const BRANCH_TASK_ROW_BASE: u64 = 8850;

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
    pub field: Field,
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
                Field::new(current.as_deref().unwrap_or(""))
            }
            _ => Field::default(),
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
    pub(crate) fn press_branch_task(&mut self, action: FocusId) -> bool {
        if self.git.branching.is_none() || !matches!(self.git.prompt, Some(Prompt::Branch(_))) {
            return false;
        }
        match action {
            BRANCH_TASK_FIELD => {}
            BRANCH_TASK_REMOTE => {
                if let Some(open) = self.git.branching.as_mut() {
                    open.with_remote = !open.with_remote;
                }
            }
            _ => {
                let Some(row) = action
                    .0
                    .checked_sub(BRANCH_TASK_ROW_BASE)
                    .map(|row| row as usize)
                    .filter(|row| *row < ROWS)
                else {
                    return false;
                };
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
        self.git.busy = Some(doing);
        self.git.trouble = None;
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

/// A button of a question, saying `label`.
fn button(label: &str, id: FocusId, variant: Variant) -> Node {
    Button::new(label, id)
        .variant(variant)
        .size(Size::Sm)
        .build()
}

/// `count` commits, worded for one or many.
fn commits(count: usize) -> String {
    if count == 1 {
        "1 commit is".to_owned()
    } else {
        format!("{count} commits are")
    }
}

/// A row with a check box that says `label`, ticked while `checked`.
fn tick(label: &str, checked: bool) -> Node {
    node()
        .styles([flex(), items_center(), gap(2), px(2)])
        .child(Checkbox::new(checked, BRANCH_TASK_REMOTE).build())
        .child(line(label, 13, 1.4, Role::Text))
        .focusable(BRANCH_TASK_REMOTE)
}

/// The question or list about a branch that is open, as a node.
pub fn branch_prompt(state: &WorkspaceState, task: BranchTask) -> Node {
    let Some(open) = state.git.branching.as_ref() else {
        return Node::default();
    };
    let mut body = node()
        .styles([flex(), flex_col(), gap(3), p(5)])
        .child(line(task.title(), 15, 1.4, Role::Text).styles([pm_style::font_medium()]));
    let mut confirm: Option<(&str, Variant)> = None;
    let choices = state.branch_choices();
    let loading = open.info.is_none();
    let current = state.current_branch_name().unwrap_or_default();
    let upstream = state
        .review_session()
        .and_then(|session| session.tracking.upstream.clone());
    let say = |words: &str| -> Vec<Node> {
        wrap(words, 13, WIDTH - 40.0)
            .iter()
            .map(|words| line(words, 13, 1.5, Role::Muted))
            .collect()
    };
    if loading {
        body = body.child(line("Reading the remotes…", 13, 1.4, Role::Subtle));
    } else if let Some(info) = open.info.as_ref() {
        match task {
            BranchTask::Publish if info.remotes.is_empty() => {
                body = body.children(say("This repository has no remote to publish to."));
            }
            BranchTask::Publish => {
                if info.remotes.len() > 1 {
                    body = body.child(line("Remote", 12, 1.4, Role::Subtle));
                    body = body.children(choices.iter().enumerate().map(
                        |(position, (name, description, chosen))| {
                            row(
                                name,
                                description,
                                *chosen,
                                FocusId(BRANCH_TASK_ROW_BASE + position as u64),
                            )
                        },
                    ));
                }
                body = body.child(line("Branch name on the remote", 12, 1.4, Role::Subtle));
                body = body.child(
                    Input::new(&open.field, BRANCH_TASK_FIELD)
                        .placeholder(&current)
                        .size(Size::Md)
                        .build(),
                );
                confirm = Some(("Publish", Variant::Default));
            }
            BranchTask::Rename => {
                body = body.child(
                    Input::new(&open.field, BRANCH_TASK_FIELD)
                        .placeholder("New name")
                        .size(Size::Md)
                        .build(),
                );
                if let Some(upstream) = &upstream {
                    body = body.child(tick(
                        &format!("Also rename {upstream} on the remote"),
                        open.with_remote,
                    ));
                }
                confirm = Some(("Rename", Variant::Default));
            }
            BranchTask::Delete if open.chosen.is_some() => {
                let name = open.chosen.clone().unwrap_or_default();
                body = body.child(
                    line(format!("Delete {name}?"), 13, 1.4, Role::Text)
                        .styles([pm_style::font_medium()]),
                );
                if let Some(path) = &info.elsewhere {
                    body = body.children(say(&format!(
                        "{name} is checked out in {path}, so it cannot be deleted from here."
                    )));
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
                        "The branch is removed. Its commits are all in other branches.".to_owned()
                    } else {
                        format!("{}. They are lost when it is deleted.", warnings.join(", "))
                    };
                    body = body.children(say(&words));
                    if let Some(follows) = &info.follows {
                        body = body.child(tick(
                            &format!("Also delete {follows} on the remote"),
                            open.with_remote,
                        ));
                    }
                    confirm = Some(("Delete", Variant::Destructive));
                }
            }
            _ => {
                body = body.child(
                    Input::new(&open.field, BRANCH_TASK_FIELD)
                        .placeholder("Search branches")
                        .size(Size::Md)
                        .build(),
                );
                if choices.is_empty() {
                    body = body.child(line(
                        if task == BranchTask::Upstream && info.remotes.is_empty() {
                            "This repository has no remote."
                        } else {
                            "No branches."
                        },
                        13,
                        1.4,
                        Role::Subtle,
                    ));
                }
                body = body.children(choices.iter().enumerate().map(
                    |(position, (name, description, chosen))| {
                        row(
                            name,
                            description,
                            *chosen,
                            FocusId(BRANCH_TASK_ROW_BASE + position as u64),
                        )
                    },
                ));
            }
        }
    }
    let mut buttons = node()
        .styles([flex(), items_center(), gap(2), px(5), pb(5)])
        .child(spacer())
        .child(button("Cancel", PROMPT_CANCEL, Variant::Ghost));
    if let Some((label, variant)) = confirm.filter(|_| !loading) {
        buttons = buttons.child(button(label, PROMPT_CONFIRM, variant));
    }
    Modal::new(PROMPT_OUTSIDE, PROMPT_INSIDE)
        .width(WIDTH)
        .child(body)
        .child(buttons)
        .build()
}
