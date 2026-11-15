//! What the reader does to the open session's conversation, sent to the
//! server: answering the agent's questions, trusting the project, acting on
//! the pull request, the conversation commands and continuing a chat in a
//! project.

use pm_git::{CheckState, PullRequestStatus, ReviewDecision};
use pm_protocol::{Request, Response};

use super::{Ask, Link};
use crate::conversation::{InquiryReply, PullRequestInfo, TrustVerdict};
use crate::model::Project;

/// A command that takes the conversation back, starts it over or asks aside.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConversationCommand {
    /// Answer the last prompt again.
    Retry,
    /// Start the conversation afresh, keeping its settings.
    Afresh,
    /// Hand the work over to a new conversation; the composer is filled for
    /// the reader to say what it should work toward.
    HandOver,
    /// Ask a question on the side; the composer is filled for the reader to write it.
    SideQuestion,
    /// Take the conversation back to before the reader's turn given.
    Rewind(u64),
    /// Start a session branching off after the reader's turn given.
    Fork(u64),
    /// Put the files back as they were before the reader's turn given.
    Restore(u64),
    /// Take what the reader's turn given changed back out of the files.
    UndoTurn(u64),
    /// Show in the Diff tab what the reader's turn given changed, with a button that undoes it.
    TurnChanges(u64),
    /// Stop the subagent with this id, and those it started.
    StopSubagent(String),
    /// Sign in again as the session's account.
    SignIn,
}

impl Link {
    /// Answers the permission numbered `ticket` with the choice `option`.
    pub fn choose(&mut self, ticket: u64, option: String) {
        let Some(session) = self.watched else {
            return;
        };
        self.workspace.extras.approval = None;
        self.ask(
            Request::Choose {
                session,
                ticket,
                option,
            },
            Ask::Nothing,
        );
    }

    /// Answers the form numbered `ticket` with `replies`, or abandons it when nothing.
    pub fn answer_inquiry(&mut self, ticket: u64, replies: Option<Vec<InquiryReply>>) {
        let Some(session) = self.watched else {
            return;
        };
        self.workspace.extras.inquiry = None;
        let replies = replies.map(|replies| {
            replies
                .into_iter()
                .map(|reply| pm_harness::Reply {
                    picks: reply.picks,
                    other: reply.other,
                })
                .collect()
        });
        self.ask(
            Request::Answer {
                session,
                ticket,
                replies,
            },
            Ask::Nothing,
        );
    }

    /// Answers the trust request numbered `ticket` with `verdict`.
    pub fn trust(&mut self, ticket: u64, verdict: TrustVerdict) {
        let Some(session) = self.watched else {
            return;
        };
        self.workspace.extras.trust = None;
        let verdict = match verdict {
            TrustVerdict::Once => pm_harness::Verdict::Allow,
            TrustVerdict::Always => pm_harness::Verdict::Always,
            TrustVerdict::Reject => pm_harness::Verdict::Reject,
        };
        self.ask(
            Request::Trust {
                session,
                ticket,
                verdict,
            },
            Ask::Nothing,
        );
    }

    /// Asks the server for the pull request of the open session's branch, when it has a branch of its own.
    pub(super) fn ask_pull_request(&mut self) {
        let Some((session, directory, branch)) = self.open_branch() else {
            self.workspace.extras.pull_request = None;
            return;
        };
        self.ask(
            Request::PullRequest { directory, branch },
            Ask::PullRequest(session),
        );
    }

    /// Pushes the open session's branch or acts on its pull request with
    /// `action`, such as `create` or `merge`, showing that it is under way.
    pub fn pull_request_action(&mut self, action: String) {
        let Some((session, directory, branch)) = self.open_branch() else {
            return;
        };
        if let Some(bar) = &mut self.workspace.extras.pull_request {
            bar.busy = Some(busy_words(&action).to_owned());
            bar.error = None;
        }
        let plan = self
            .workspace
            .extras
            .plan
            .iter()
            .map(|item| item.text.clone())
            .collect();
        let base = self.workspace.git.base.clone();
        self.ask(
            Request::PullRequestAction {
                directory,
                branch,
                base,
                action,
                title: String::new(),
                plan,
            },
            Ask::PullRequest(session),
        );
    }

    /// Takes the server's answer about the pull request of `session`'s branch, shown while that session is open: how it stands, why acting on it failed, or, after a pull request was drafted the code host's program could not open, its page in the reader's browser.
    pub(super) fn answer_pull_request(&mut self, session: u64, response: Response) {
        if self.watched != Some(session) {
            return;
        }
        let known = self.workspace.extras.pull_request.clone();
        match response {
            Response::PullRequest { request, host } => {
                let has_base = self.workspace.git.base.is_some();
                if request.is_none() && !has_base {
                    self.workspace.extras.pull_request = None;
                    return;
                }
                let mut bar = request.map(pull_request_info).unwrap_or_default();
                bar.noun = host
                    .as_ref()
                    .map_or("pull request", pm_git::CodeHost::noun)
                    .to_owned();
                bar.supported = host.is_none_or(|host| host.supported);
                bar.ahead = self.branch_ahead();
                self.workspace.extras.pull_request = Some(bar);
            }
            Response::PullRequestActed { request, compare } => {
                let mut bar = request.map(pull_request_info).unwrap_or_default();
                if let Some(known) = known {
                    bar.noun = known.noun;
                    bar.supported = known.supported;
                }
                bar.ahead = self.branch_ahead();
                self.workspace.extras.pull_request = Some(bar);
                if let Some(url) = compare {
                    self.open_address(&url);
                }
            }
            Response::Failed { message } => {
                if let Some(bar) = &mut self.workspace.extras.pull_request {
                    bar.busy = None;
                    bar.error = Some(message);
                }
            }
            _ => {}
        }
    }

    /// Whether the open session's branch has commits its remote has not, or no remote branch yet.
    fn branch_ahead(&self) -> bool {
        let git = &self.workspace.git;
        git.upstream.is_none() || git.ahead > 0
    }

    /// The session, folder and branch of the open session, when it is no chat and has a branch, the folder as an address on its machine.
    fn open_branch(&self) -> Option<(u64, String, String)> {
        let session = self.watched?;
        let info = self.session_info(session).filter(|info| !info.chat)?;
        Some((session, info.directory.clone(), info.branch.clone()?))
    }

    /// Carries out `command` on the open session.
    pub fn conversation_command(&mut self, command: ConversationCommand) {
        let Some(session) = self.watched else {
            return;
        };
        match command {
            ConversationCommand::Retry => self.send_prompt(pm_harness::RETRY.to_owned()),
            ConversationCommand::Afresh => self.send_prompt(pm_harness::FRESH.to_owned()),
            ConversationCommand::HandOver => {
                self.prefill(format!("{} ", pm_harness::HANDOFF));
            }
            ConversationCommand::SideQuestion => {
                self.prefill(format!("{} ", pm_harness::BTW));
            }
            ConversationCommand::Rewind(turn) => {
                self.ask(Request::Rewind { session, turn }, Ask::Nothing)
            }
            ConversationCommand::Fork(turn) => {
                self.ask(Request::ForkSession { session, turn }, Ask::Fork)
            }
            ConversationCommand::Restore(turn) => {
                let ask = self.files_changed_ask();
                self.ask(Request::Restore { session, turn }, ask)
            }
            ConversationCommand::UndoTurn(turn) => self.ask(
                Request::UndoTurn {
                    session,
                    turn,
                    paths: Vec::new(),
                },
                self.files_changed_ask(),
            ),
            ConversationCommand::TurnChanges(turn) => self.show_turn(turn),
            ConversationCommand::StopSubagent(subagent) => {
                self.ask(Request::StopSubagent { session, subagent }, Ask::Nothing)
            }
            ConversationCommand::SignIn => {
                self.workspace.extras.signed_out = None;
                self.ask(Request::SignInSession { session }, Ask::Nothing)
            }
        }
    }

    /// Picks the project at index path `at` to continue the open chat in, and asks its machine for the branches the session's worktree can be made from.
    pub fn choose_continue_project(&mut self, at: Vec<usize>) {
        let Some(project) = project_at(&self.workspace.projects, &at) else {
            return;
        };
        let directory = super::expand_home(&project.path);
        self.workspace.continuing = crate::model::ContinueChoice {
            project: Some(at),
            branches: None,
        };
        self.ask(
            Request::Branches {
                directory: directory.clone(),
            },
            Ask::ContinueBranches(directory),
        );
    }

    /// Takes the server's answer to the branches of the project at `directory`, asked to continue the open chat in it.
    pub(super) fn answer_continue_branches(&mut self, directory: String, response: Response) {
        let picked = self
            .workspace
            .continuing
            .project
            .as_ref()
            .and_then(|at| project_at(&self.workspace.projects, at))
            .map(|project| super::expand_home(&project.path));
        if picked.as_deref() != Some(directory.as_str()) {
            return;
        }
        self.workspace.continuing.branches = Some(match response {
            Response::Branches { names } => names,
            _ => Vec::new(),
        });
    }

    /// Continues the open chat as a session in the project picked, its worktree made from `base`, unless its agent is working or prompts wait for it.
    pub fn continue_chat(&mut self, base: String) {
        let Some(session) = self.watched else {
            return;
        };
        let composer = &self.workspace.extras.composer;
        if composer.working || !composer.queued.is_empty() {
            return;
        }
        let Some(project) = self
            .workspace
            .continuing
            .project
            .as_ref()
            .and_then(|at| project_at(&self.workspace.projects, at))
        else {
            return;
        };
        let repository = super::expand_home(&project.path);
        let title = self.workspace.title.clone();
        self.workspace.extras.chat = false;
        self.workspace.continuing = crate::model::ContinueChoice::default();
        self.ask(
            Request::ContinueChat {
                session,
                repository,
                base,
                title,
            },
            Ask::Nothing,
        );
    }
}

/// The project at index path `at` among `projects`.
fn project_at<'a>(projects: &'a [Project], at: &[usize]) -> Option<&'a Project> {
    let (first, nested) = at.split_first()?;
    let mut project = projects.get(*first)?;
    for index in nested {
        project = project.children.get(*index)?;
    }
    Some(project)
}

/// What the bar says while `action` is under way.
fn busy_words(action: &str) -> &'static str {
    match action {
        "push" => "Pushing…",
        "create" | "create_draft" => "Creating…",
        "update" => "Updating…",
        "ready" => "Marking ready…",
        "merge" | "squash" | "rebase" => "Merging…",
        "reopen" => "Reopening…",
        "delete_branch" => "Deleting the branch…",
        _ => "Working…",
    }
}

/// The bar `status` shows.
fn pull_request_info(status: PullRequestStatus) -> PullRequestInfo {
    let count = |state: CheckState| {
        status
            .checks
            .iter()
            .filter(|check| check.state == state)
            .count()
    };
    PullRequestInfo {
        number: status.number,
        title: status.title.clone(),
        state: status.state.clone(),
        draft: status.draft,
        review: match status.review {
            ReviewDecision::Approved => "Approved".to_owned(),
            ReviewDecision::Required => "Review required".to_owned(),
            ReviewDecision::None => String::new(),
            ReviewDecision::ChangesRequested => "Changes requested".to_owned(),
        },
        checks: (
            count(CheckState::Passed),
            count(CheckState::Failed),
            count(CheckState::Pending),
        ),
        url: status.url.clone(),
        ..PullRequestInfo::default()
    }
}
