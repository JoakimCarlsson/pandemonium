//! Continuing a chat as a session in a project: the chip above a chat's prompt box, the choice of a project and a base branch behind it, and what the server says of how it went.
//!
//! The server asks the chat's agent for a handoff summary and starts the session with it, so the window only says where the work goes on and is told the outcome.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{Role, border_color, border_t, flex, items_center, justify_end, mt, px, py};
use pm_ui::{Node, node};

use crate::Button;
use crate::parts::line;
use crate::popover::{Popover, anchored, heading};
use crate::sample::{Message, Session, Status};
use crate::size::Size;
use crate::state::{BRANCH_BASE, MENU_BODY, Outgoing, Page, Picker, SESSION_BASE, WorkspaceState};

/// The chip above a chat's prompt box that opens the choice of where to continue it.
pub const CONTINUE: FocusId = FocusId(7130);

/// The button of the choice that asks the server to continue the chat in the project chosen.
pub const CONTINUE_START: FocusId = FocusId(7131);

/// The first of the rows choosing the project a chat is continued in, by the project's position; the rest follow it.
pub const CONTINUE_PROJECT_BASE: u64 = 7200;

/// How many projects the choice has rows for.
const MOST_PROJECTS: usize = 200;

/// How wide the choice is, in logical pixels.
const MENU_WIDTH: f32 = 360.0;

/// Where a chat is to be continued, as chosen so far in the choice behind the chip. It is kept apart from the page for starting work, so that neither changes what was chosen on the other.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Choice {
    /// The project chosen, by position.
    pub project: Option<usize>,
    /// The branches of that project, once its machine has listed them; none of them when it holds no repository.
    pub branches: Option<Vec<String>>,
    /// The branch picked to make the worktree from; the first of `branches` while none was picked.
    pub base: Option<String>,
}

impl Choice {
    /// Takes in `branches` as those of the project at `project`, when that is the one chosen, and forgets a picked branch that is not among them.
    pub(crate) fn read(&mut self, project: usize, branches: &[String]) {
        if self.project != Some(project) {
            return;
        }
        if self
            .base
            .as_ref()
            .is_some_and(|base| !branches.contains(base))
        {
            self.base = None;
        }
        self.branches = Some(branches.to_vec());
    }

    /// The branch the worktree is made from: the one picked, or the first one listed.
    pub fn from(&self) -> Option<&String> {
        self.base
            .as_ref()
            .or_else(|| self.branches.as_ref()?.first())
    }
}

/// Whether `id` is the chip, the button or a project row of the choice, which leave the choice open or work it.
pub fn is_continue_part(id: FocusId) -> bool {
    id == CONTINUE
        || id == CONTINUE_START
        || (CONTINUE_PROJECT_BASE..CONTINUE_PROJECT_BASE + MOST_PROJECTS as u64).contains(&id.0)
}

impl WorkspaceState {
    /// The projects `chat` can be continued in, by position: those on the chat's own machine, since the session that continues it is pointed at files in the chat's scratch folder.
    pub fn continue_projects(&self, chat: &Session) -> Vec<usize> {
        let Some(machine) = self.machine_of(chat) else {
            return Vec::new();
        };
        (0..self.projects.len())
            .filter(|project| self.projects[*project].host() == machine)
            .collect()
    }

    /// Whether `chat` can be continued in a project now: its server has made its scratch folder, its agent is not working, nothing waits for the agent, and it is not being continued already.
    pub fn can_continue(&self, chat: &Session) -> bool {
        chat.is_chat()
            && !chat.directory.is_empty()
            && chat.status != Status::Working
            && chat.queued.is_empty()
            && !chat.handing_over
    }

    /// The project chosen to continue the open chat in, when it is one the chat can be continued in.
    pub fn continue_project(&self) -> Option<usize> {
        let chat = self.open()?;
        self.continuing
            .project
            .filter(|project| self.continue_projects(chat).contains(project))
    }

    /// Chooses the project at `project` to continue the open chat in, and asks its machine for its branches.
    fn choose_continue_project(&mut self, project: usize) {
        let Some(held) = self.projects.get(project) else {
            return;
        };
        let ask = Outgoing::Branches {
            project,
            host: held.host(),
            directory: held.directory().to_string_lossy().into_owned(),
        };
        if self.continuing.project != Some(project) {
            self.continuing = Choice {
                project: Some(project),
                ..Choice::default()
            };
        }
        self.outbox.push(ask);
    }

    /// Picks the branch the row `id` names as the one the worktree is made from, while the choice is open, and returns whether `id` was such a row then.
    fn press_continue_branch(&mut self, id: FocusId) -> bool {
        if self.picker != Some(Picker::Continue) {
            return false;
        }
        let picked =
            id.0.checked_sub(BRANCH_BASE)
                .and_then(|row| self.continuing.branches.as_ref()?.get(row as usize))
                .cloned();
        match picked {
            Some(branch) => {
                self.continuing.base = Some(branch);
                true
            }
            None => false,
        }
    }

    /// Works the chip, a project row, a branch row or the button of the choice named by `id`, and returns whether `id` was one of them.
    pub(crate) fn press_continue(&mut self, id: FocusId) -> bool {
        if self.press_continue_branch(id) {
            return true;
        }
        if !is_continue_part(id) {
            return false;
        }
        let Page::Session(position) = self.page else {
            self.picker = None;
            return true;
        };
        let Some(chat) = self.sessions.get(position) else {
            return true;
        };
        let offered = self.continue_projects(chat);
        if !self.can_continue(chat) || offered.is_empty() {
            self.picker = None;
            return true;
        }
        if id == CONTINUE {
            self.picker = (self.picker != Some(Picker::Continue)).then_some(Picker::Continue);
            if self.picker.is_some() {
                let chosen = self.continue_project().unwrap_or(offered[0]);
                self.choose_continue_project(chosen);
            }
        } else if id == CONTINUE_START {
            let chosen = self.continue_project().zip(self.continuing.from().cloned());
            if let Some((project, base)) = chosen {
                self.outbox.push(Outgoing::ContinueChat {
                    session: position,
                    project,
                    base,
                });
                self.sessions[position].handing_over = true;
                self.sessions[position].closed = false;
                self.picker = None;
            }
        } else {
            let project = (id.0 - CONTINUE_PROJECT_BASE) as usize;
            if offered.contains(&project) {
                self.choose_continue_project(project);
            }
        }
        true
    }

    /// The state after the server said how continuing the chat at `chat` went: on the session at `started` when one was started and the chat is still showing, or with why there is none noted in the chat, whose agent may be on another turn by then and is left as it stands.
    pub fn chat_continued(
        mut self,
        chat: usize,
        started: Option<usize>,
        failure: Option<&str>,
    ) -> Self {
        if let Some(held) = self.sessions.get_mut(chat) {
            held.handing_over = false;
        }
        match (started, failure) {
            (Some(session), _) if self.page == Page::Session(chat) => {
                self.after(FocusId(SESSION_BASE + session as u64))
            }
            (None, Some(reason)) => {
                if let Some(held) = self.sessions.get_mut(chat) {
                    let mut note =
                        Message::reply(&format!("Could not continue in a project: {reason}"));
                    note.duration = Some(String::new());
                    held.messages.push(note);
                }
                self
            }
            _ => self,
        }
    }
}

/// The choice of where the open chat is continued: the projects on its machine, the branches of the one chosen to make the worktree from, and the button that starts it.
fn menu(state: &WorkspaceState, chat: &Session) -> Node {
    let chosen = state.continue_project();
    let mut menu = Popover::new()
        .width(MENU_WIDTH)
        .heading("Continue in", None);
    for project in state.continue_projects(chat) {
        let held = &state.projects[project];
        menu = menu.row(
            &held.name,
            &held.directory().to_string_lossy(),
            chosen == Some(project),
            FocusId(CONTINUE_PROJECT_BASE + project as u64),
        );
    }
    let listed = state.continuing.branches.as_deref();
    let ready = chosen.is_some() && listed.is_some_and(|branches| !branches.is_empty());
    let from = node()
        .styles([border_t(), border_color(Role::Border), mt(1)])
        .child(heading("Branch from", None));
    let said = |words: &str| {
        node()
            .styles([flex(), items_center(), px(2.5), py(1.5)])
            .child(line(words, 13, 1.4, Role::Subtle))
    };
    let from = match listed {
        None => from.child(said("Reading the branches…")),
        Some([]) => from.child(said("Not a Git repository, so it has no worktrees.")),
        Some(branches) => from.children(crate::composer::branch_rows(
            branches,
            state.continuing.base.as_ref(),
        )),
    };
    menu.child(from)
        .child(
            node()
                .styles([
                    flex(),
                    justify_end(),
                    border_t(),
                    border_color(Role::Border),
                    mt(1),
                    px(1),
                    py(1.5),
                ])
                .child(
                    Button::new("Continue", CONTINUE_START)
                        .size(Size::Sm)
                        .disabled(!ready)
                        .build(),
                ),
        )
        .over_button(MENU_BODY)
}

/// The chip above the prompt box of the chat `chat` that continues it in a project, with the choice over it while open; switched off while the chat's agent is working, has prompts waiting or is writing the summary, and left out when no project is on the chat's machine.
pub(crate) fn chip(state: &WorkspaceState, chat: &Session) -> Option<Node> {
    if !chat.is_chat() || state.continue_projects(chat).is_empty() {
        return None;
    }
    let open = state.picker == Some(Picker::Continue);
    let label = if chat.handing_over {
        "Continuing in project…"
    } else {
        "Continue in project…"
    };
    let button = Button::new(label, CONTINUE)
        .variant(crate::button::Variant::Ghost)
        .size(Size::Xs)
        .with_icon(IconName::GitFork)
        .selected(open)
        .disabled(!state.can_continue(chat))
        .build();
    Some(anchored(button, open.then(|| menu(state, chat))))
}

#[cfg(test)]
mod tests;
