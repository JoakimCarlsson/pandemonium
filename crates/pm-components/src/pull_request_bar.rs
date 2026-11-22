//! The bar under a session's header that tells how the pull request of its branch stands, with its checks and review, and offers what can be done next: push, open, update, mark ready, merge, reopen, and delete the branch from its remote. It names the review as the code host its repository is on does, and offers only pushing when that host is not supported.

use crate::areas::GitEvent;
use crate::target::Target;
use crate::theme;
use crate::theme::Role;
use pm_git::{CheckState, CodeHost, PullRequestStatus, ReviewDecision};
use rustle::Modifier;
use rustle::{Alignment, Arrangement};
use rustle::{Icon, phosphor};

use crate::Button;
use crate::badge::{Badge, Variant as BadgeVariant};
use crate::button::Variant;
use crate::parts::{glyph, line, spacer};
use crate::sample::Session;
use crate::size::Size;
use crate::state::{Outgoing, Page, WorkspaceState};

/// Whether `session` gets the bar: a session with a branch of its own, made from another, or one that has a pull request already.
fn shows(session: &Session) -> bool {
    !session.is_chat()
        && !session.home
        && session.on_branch
        && !session.branch.is_empty()
        && (session.base.is_some() || session.pull_request.is_some())
}

/// What the code host of `session` calls a review, in lower case: `pull request` until its server says otherwise.
fn noun(session: &Session) -> &str {
    session
        .code_host
        .as_ref()
        .map_or("pull request", CodeHost::noun)
}

/// `words` with the first letter of each word in upper case, such as `Pull Request`.
fn title_case(words: &str) -> String {
    words
        .split(' ')
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(letters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The code host of `session` when its server said it is one whose reviews cannot be read or acted on.
fn unsupported(session: &Session) -> Option<&CodeHost> {
    session.code_host.as_ref().filter(|host| !host.supported)
}

/// Emits the badge naming the pull request's number and how it stands, in the colour of that.
fn state_badge(number: u64, said: &str, tone: Role) {
    Badge::new(format!("#{number} {said}"))
        .tone(tone)
        .variant(BadgeVariant::Soft)
        .build();
}

/// What the badge of `request` says of how it stands, and the colour of that.
fn standing(request: &PullRequestStatus) -> (&'static str, Role) {
    if request.is_merged() {
        ("Merged", Role::Accent)
    } else if request.is_closed() {
        ("Closed", Role::Danger)
    } else if request.draft {
        ("Draft", Role::Muted)
    } else {
        ("Open", Role::Success)
    }
}

/// Emits a glyph in `tone` followed by `count`; emits nothing when the count is nothing.
#[track_caller]
fn tally(icon: Icon, tone: Role, count: usize) {
    if count == 0 {
        return;
    }
    rustle::row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            glyph(icon, 3.5, tone);
            line(count.to_string(), 12, 1.4, tone);
        },
    );
}

/// Emits the button counting the checks by how they stand, whose tooltip names the first that failed; it opens the page of the checks.
#[track_caller]
fn checks(request: &PullRequestStatus) {
    let failed = request
        .checks
        .iter()
        .find(|check| check.state == CheckState::Failed);
    let summary = if request.checks.is_empty() {
        "No checks".to_owned()
    } else if let Some(check) = failed {
        format!("{} failed", check.name)
    } else if request.count(CheckState::Pending) > 0 {
        "Checks running".to_owned()
    } else {
        "Checks passed".to_owned()
    };
    let counts = (
        request.count(CheckState::Passed),
        request.count(CheckState::Failed),
        request.count(CheckState::Pending),
    );
    Button::new(
        if request.checks.is_empty() {
            "No checks"
        } else {
            ""
        },
        Target::Git(GitEvent::PullChecks),
    )
    .variant(Variant::Ghost)
    .size(Size::Xs)
    .mark(move || {
        rustle::row_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            || {
                tally(phosphor::CHECK_CIRCLE, Role::Success, counts.0);
                tally(phosphor::X, Role::Danger, counts.1);
                tally(phosphor::TIMER, Role::Warning, counts.2);
            },
        );
    })
    .tooltip(format!("{summary}: Open the Checks"))
    .build();
}

/// Emits what the reviewers decided, in words and the colour of that; emits nothing when nothing is asked of them.
#[track_caller]
fn review(request: &PullRequestStatus) {
    let (said, tone) = match request.review {
        ReviewDecision::None => return,
        ReviewDecision::Required => ("Review required", Role::Warning),
        ReviewDecision::Approved => ("Approved", Role::Success),
        ReviewDecision::ChangesRequested => ("Changes requested", Role::Danger),
    };
    Badge::new(said).tone(tone).build();
}

/// Emits a small button saying `label` that does `id`, filled when it is the `main` thing to do, with `tooltip` when there is one.
#[track_caller]
fn action(label: &str, id: GitEvent, main: bool, tooltip: Option<String>) {
    let button = Button::new(label, Target::Git(id))
        .variant(if main {
            Variant::Default
        } else {
            Variant::Outline
        })
        .size(Size::Xs);
    match tooltip {
        Some(words) => button.tooltip(words).build(),
        None => button.build(),
    }
}

/// Emits the buttons for what can be done to the branch of `session` and its pull request, as that stands.
#[track_caller]
fn actions(session: &Session) {
    let ahead = session.tracking.upstream.is_none() || session.tracking.ahead > 0;
    if unsupported(session).is_some() {
        action("Push", GitEvent::PullPush, true, None);
        return;
    }
    let noun = title_case(noun(session));
    match &session.pull_request {
        None => {
            action("Create Draft", GitEvent::PullCreateDraft, false, None);
            action(
                &format!("Create {noun}"),
                GitEvent::PullCreate,
                true,
                Some(format!("Push and Open a {noun}")),
            );
        }
        Some(request) if request.is_open() => {
            if ahead {
                action("Push", GitEvent::PullPush, false, None);
            }
            let redraft = "Push and Redraft the Description";
            action(
                "Update",
                GitEvent::PullUpdate,
                false,
                Some(redraft.to_owned()),
            );
            let (label, id, tip) = if request.draft {
                ("Ready", GitEvent::PullReady, "Ready for Review")
            } else {
                ("Merge", GitEvent::PullMerge, "Squash and Merge")
            };
            action(label, id, true, Some(tip.to_owned()));
        }
        Some(request) => {
            action("Delete Branch", GitEvent::PullDeleteBranch, false, None);
            if request.is_closed() {
                action("Reopen", GitEvent::PullReopen, true, None);
            }
        }
    }
}

/// Emits the bar for `session`; emits nothing when it has no branch of its own to open a pull request of.
#[track_caller]
pub fn bar(session: &Session) {
    if !shows(session) {
        return;
    }
    let host_name = session
        .code_host
        .as_ref()
        .map(|host| host.name.as_str())
        .filter(|name| !name.is_empty());
    let modifier = Modifier::new()
        .height(theme::sp(10))
        .clip()
        .border_bottom(1.0, theme::color(Role::Border))
        .padding_each(0.0, 0.0, 0.0, 1.0)
        .padding_xy(theme::sp(5), 0.0);
    rustle::row_arranged(
        modifier,
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Center,
        || {
            match (&session.pull_request, unsupported(session)) {
                (Some(request), None) => {
                    let (said, tone) = standing(request);
                    let number = request.number;
                    Button::new("", Target::Git(GitEvent::PullOpen))
                        .variant(Variant::Ghost)
                        .size(Size::Xs)
                        .mark(move || state_badge(number, said, tone))
                        .tooltip(match host_name {
                            Some(name) => format!("Open on {name}: {}", request.title),
                            None => format!("Open: {}", request.title),
                        })
                        .build();
                    checks(request);
                    review(request);
                }
                (_, host) => {
                    let said = match host {
                        Some(host) if host.name.is_empty() => {
                            "This code host is not supported yet".to_owned()
                        }
                        Some(host) => format!("{} is not supported yet", host.name),
                        None => format!("No {}", noun(session)),
                    };
                    glyph(phosphor::GIT_DIFF, 3.5, Role::Muted);
                    line(said, 12, 1.4, Role::Muted);
                }
            }
            spacer();
            match session.pull_busy {
                Some(doing) => {
                    glyph(phosphor::CIRCLE_DASHED, 3.5, Role::Muted);
                    line(doing, 12, 1.4, Role::Muted);
                }
                None => actions(session),
            }
        },
    );
}

impl WorkspaceState {
    /// Carries out `action` if it is one of the bar's, on the open session, and says whether it was.
    pub(crate) fn press_pull_request(&mut self, action: Target) -> bool {
        let (operation, doing) = match action {
            Target::Git(GitEvent::PullOpen) | Target::Git(GitEvent::PullChecks) => {
                let address = self
                    .open()
                    .and_then(|session| session.pull_request.as_ref())
                    .map(|request| match action {
                        Target::Git(GitEvent::PullChecks) if !request.checks_url.is_empty() => {
                            request.checks_url.clone()
                        }
                        _ => request.url.clone(),
                    });
                if let Some(address) = address {
                    self.outbox.push(Outgoing::Open { address });
                }
                return true;
            }
            Target::Git(GitEvent::PullPush) => ("push", "Pushing…"),
            Target::Git(GitEvent::PullCreate) => ("create", "Opening…"),
            Target::Git(GitEvent::PullCreateDraft) => ("create_draft", "Opening the draft…"),
            Target::Git(GitEvent::PullUpdate) => ("update", "Updating…"),
            Target::Git(GitEvent::PullReady) => ("ready", "Marking ready…"),
            Target::Git(GitEvent::PullMerge) => ("squash", "Merging…"),
            Target::Git(GitEvent::PullReopen) => ("reopen", "Reopening…"),
            Target::Git(GitEvent::PullDeleteBranch) => ("delete_branch", "Deleting the branch…"),
            _ => return false,
        };
        let Page::Session(index) = self.page else {
            return true;
        };
        let Some(session) = self
            .sessions
            .get_mut(index)
            .filter(|session| session.pull_busy.is_none())
        else {
            return true;
        };
        session.pull_busy = Some(doing);
        let title = session.title.clone();
        let plan = crate::activity::latest_plan(session)
            .map(|steps| steps.iter().map(|step| step.text.clone()).collect())
            .unwrap_or_default();
        self.outbox.push(Outgoing::PullRequestAction {
            session: index,
            action: operation,
            title,
            plan,
        });
        true
    }

    /// The state after the server said the branch of the session at `session` has `request` as its newest pull request, or none, on the code host `host`, when it said which.
    pub fn with_pull_request(
        mut self,
        session: usize,
        request: Option<PullRequestStatus>,
        host: Option<CodeHost>,
    ) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            held.pull_request = request;
            if host.is_some() {
                held.code_host = host;
            }
        }
        self
    }

    /// The state after the server acted on the pull request of the session at `session`: it stands as `request` afterwards, unless the action failed for `failure`, which is said in the git panel.
    pub fn pull_request_done(
        mut self,
        session: usize,
        request: Option<PullRequestStatus>,
        failure: Option<String>,
    ) -> Self {
        let mut said = "pull request".to_owned();
        if let Some(held) = self.sessions.get_mut(session) {
            held.pull_busy = None;
            said = noun(held).to_owned();
            if failure.is_none() {
                held.pull_request = request;
            }
        }
        match failure {
            Some(message) => {
                self.git_failed(&format!("Could not do that to the {said}: {message}"))
            }
            None => self,
        }
    }
}
