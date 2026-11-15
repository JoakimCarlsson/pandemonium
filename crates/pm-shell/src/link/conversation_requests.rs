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
    /// Show the files the reader's turn given changed, with their diffs, in the Diff tab.
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
                self.ask(Request::Restore { session, turn }, Ask::Nothing)
            }
            ConversationCommand::UndoTurn(turn) => self.ask(
                Request::UndoTurn {
                    session,
                    turn,
                    paths: Vec::new(),
                },
                Ask::Nothing,
            ),
            ConversationCommand::TurnChanges(turn) => self.show_turn_changes(session, turn),
            ConversationCommand::StopSubagent(subagent) => {
                self.ask(Request::StopSubagent { session, subagent }, Ask::Nothing)
            }
            ConversationCommand::SignIn => {
                self.workspace.extras.signed_out = None;
                self.ask(Request::SignInSession { session }, Ask::Nothing)
            }
        }
    }

    /// Opens the Diff tab on what the reader's turn `turn` of `session` changed, and asks the server for those files.
    fn show_turn_changes(&mut self, session: u64, turn: u64) {
        let folder = self.workspace.git.folder.clone();
        let git = &mut self.workspace.git;
        git.menu = None;
        git.revision = Some(crate::review::Revision {
            title: format!("Turn {} changes", turn + 1),
            ..crate::review::Revision::default()
        });
        self.workspace.open_files.tab = crate::open_files::SessionTab::Diff;
        self.ask(
            Request::TurnChanges { session, turn },
            Ask::Git(super::git::GitAsk::Revision(folder)),
        );
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
