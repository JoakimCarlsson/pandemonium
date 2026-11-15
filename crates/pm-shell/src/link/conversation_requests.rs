//! What the reader does to the open session's conversation, sent to the
//! server: answering the agent's questions, trusting the project, acting on
//! the pull request, the conversation commands and continuing a chat in a
//! project.

use pm_git::{CheckState, PullRequestStatus, ReviewDecision};
use pm_protocol::Request;

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

    /// Asks the server for the pull request of the open session's branch.
    pub(super) fn ask_pull_request(&mut self) {
        let Some((directory, branch)) = self.open_branch() else {
            return;
        };
        self.ask(Request::PullRequest { directory, branch }, Ask::PullRequest);
    }

    /// Pushes the open session's branch or acts on its pull request with
    /// `action`, such as `create` or `merge`.
    pub fn pull_request_action(&mut self, action: String) {
        let Some((directory, branch)) = self.open_branch() else {
            return;
        };
        let plan = self
            .workspace
            .extras
            .plan
            .iter()
            .map(|item| item.text.clone())
            .collect();
        self.ask(
            Request::PullRequestAction {
                directory,
                branch,
                base: None,
                action,
                title: String::new(),
                plan,
            },
            Ask::PullRequest,
        );
    }

    /// Shows the pull request the server reported for the open session's branch.
    pub(super) fn show_pull_request(&mut self, status: Option<PullRequestStatus>) {
        self.workspace.extras.pull_request =
            Some(status.map(pull_request_info).unwrap_or_default());
    }

    /// The folder and branch of the open session, when it has a branch.
    fn open_branch(&self) -> Option<(String, String)> {
        let info = self
            .sessions
            .iter()
            .find(|info| Some(info.session) == self.watched)?;
        Some((info.directory.clone(), info.branch.clone()?))
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
                self.workspace.extras.prefill = Some(format!("{} ", pm_harness::HANDOFF));
            }
            ConversationCommand::SideQuestion => {
                self.workspace.extras.prefill = Some(format!("{} ", pm_harness::BTW));
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

    /// Continues the open chat as a session in the project at index path `at`.
    pub fn continue_chat(&mut self, at: Vec<usize>) {
        let Some(session) = self.watched else {
            return;
        };
        let Some(project) = project_at(&self.workspace.projects, &at) else {
            return;
        };
        let repository = project.path.clone();
        let title = self.workspace.title.clone();
        self.workspace.extras.chat = false;
        self.ask(
            Request::ContinueChat {
                session,
                repository,
                base: "HEAD".to_owned(),
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
    }
}
