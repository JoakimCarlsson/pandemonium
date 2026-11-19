//! Continuing a chat as a session in a project: the chip above a chat's prompt box, the choice of a project and a base branch behind it, and what the server says of how it went.
//!
//! The server asks the chat's agent for a handoff summary and starts the session with it, so the window only says where the work goes on and is told the outcome.

use crate::areas::ChatEvent;
use crate::areas::SidebarEvent;
use crate::button::Variant;
use crate::kit::{self};

use crate::areas::ComposerEvent;

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::Modifier;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, row_arranged};

use crate::Button;
use crate::events::Interactive;
use crate::parts::line;
use crate::popover::{Popover, anchored, heading};
use crate::sample::{Message, Session, Status};
use crate::size::Size;
use crate::state::{Outgoing, Page, Picker, WorkspaceState};
use crate::theme::Role;

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
pub fn is_continue_part(id: Target) -> bool {
    id == Target::Chat(ChatEvent::Continue)
        || id == Target::Chat(ChatEvent::ContinueStart)
        || matches!(id, Target::Chat(ChatEvent::ContinueProject(project)) if project < MOST_PROJECTS)
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
    fn press_continue_branch(&mut self, id: Target) -> bool {
        if self.picker != Some(Picker::Continue) {
            return false;
        }
        let picked = match id {
            Target::Composer(ComposerEvent::Branch(row)) => self
                .continuing
                .branches
                .as_ref()
                .and_then(|branches| branches.get(row))
                .cloned(),
            _ => None,
        };
        match picked {
            Some(branch) => {
                self.continuing.base = Some(branch);
                true
            }
            None => false,
        }
    }

    /// Works the chip, a project row, a branch row or the button of the choice named by `id`, and returns whether `id` was one of them.
    pub(crate) fn press_continue(&mut self, id: Target) -> bool {
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
        if id == Target::Chat(ChatEvent::Continue) {
            self.picker = (self.picker != Some(Picker::Continue)).then_some(Picker::Continue);
            if self.picker.is_some() {
                let chosen = self.continue_project().unwrap_or(offered[0]);
                self.choose_continue_project(chosen);
            }
        } else if id == Target::Chat(ChatEvent::ContinueStart) {
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
        } else if let Target::Chat(ChatEvent::ContinueProject(project)) = id
            && offered.contains(&project)
        {
            self.choose_continue_project(project);
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
                self.handle(Event::Sidebar(SidebarEvent::Session(session)))
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

/// Emits a line of the choice saying `words`, where rows would be.
#[track_caller]
fn said(words: &str) {
    kit::row(
        Modifier::new()
            .padding_xy(theme::sp(2.5), 0.0)
            .padding_xy(0.0, theme::sp(1.5)),
        0,
        || line(words, 13, 1.4, Role::Subtle),
    );
}

/// Emits the choice of where the open chat `chat` is continued: the projects on its machine, the branches of the one chosen to make the worktree from, and the button that starts it.
#[track_caller]
fn menu(state: &WorkspaceState, chat: &Session) {
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
            Target::Chat(ChatEvent::ContinueProject(project)),
        );
    }
    let listed = state.continuing.branches.as_deref();
    let ready = chosen.is_some() && listed.is_some_and(|branches| !branches.is_empty());
    menu.modifier(Modifier::new().control(
        Target::Composer(ComposerEvent::MenuBody),
        send(Event::Composer(ComposerEvent::MenuBody)),
    ))
    .build_with(|| {
        kit::column(crate::composer::ruled(), 0, || {
            heading("Branch from", None);
            match listed {
                None => said("Reading the branches…"),
                Some([]) => said("Not a Git repository, so it has no worktrees."),
                Some(branches) => {
                    crate::composer::branch_rows(branches, state.continuing.base.as_ref())
                }
            }
        });
        row_arranged(
            crate::composer::ruled()
                .padding_xy(theme::sp(1), 0.0)
                .padding_xy(0.0, theme::sp(1.5)),
            Arrangement::End,
            Alignment::Start,
            || {
                kit::small_button(
                    "Continue",
                    Target::Chat(ChatEvent::ContinueStart),
                    Variant::Default,
                )
                .disabled(!ready)
                .build();
            },
        );
    });
}

/// Whether the chip that continues the chat `chat` in a project shows: `chat` is a chat and a project is on its machine.
pub(crate) fn shows_chip(state: &WorkspaceState, chat: &Session) -> bool {
    chat.is_chat() && !state.continue_projects(chat).is_empty()
}

/// Emits the chip above the prompt box of the chat `chat` that continues it in a project, with the choice over it while open; switched off while the chat's agent is working, has prompts waiting or is writing the summary, and left out when no project is on the chat's machine ([`shows_chip`]).
#[track_caller]
pub(crate) fn chip(state: &WorkspaceState, chat: &Session) {
    if !shows_chip(state, chat) {
        return;
    }
    let open = state.picker == Some(Picker::Continue);
    let label = if chat.handing_over {
        "Continuing in project…"
    } else {
        "Continue in project…"
    };
    anchored(
        || {
            Button::new(label, Target::Chat(ChatEvent::Continue))
                .variant(crate::button::Variant::Ghost)
                .size(Size::Xs)
                .with_icon(phosphor::GIT_FORK)
                .selected(open)
                .disabled(!state.can_continue(chat))
                .build();
        },
        open.then_some(|| menu(state, chat)),
    );
}

#[cfg(test)]
mod tests;
