//! The bar under a session's header that tells how the pull request of its branch stands, with its checks and review, and offers what can be done next: push, open, update, mark ready, merge, reopen, and delete the branch from its remote. It names the review as the code host its repository is on does, and offers only pushing when that host is not supported.

use crate::kit::sp;
use crate::kit::{FocusId, IconName, Radius, Role, Side, Styled, Tooltip};
use pm_git::{CheckState, CodeHost, PullRequestStatus, ReviewDecision};
use rustle::Modifier;
use rustle::{Alignment, Arrangement, row_arranged};

use crate::Button;
use crate::badge::{Badge, Variant as BadgeVariant};
use crate::button::Variant;
use crate::parts::{glyph, line, spacer};
use crate::sample::Session;
use crate::size::Size;
use crate::state::{Outgoing, Page, WorkspaceState};

/// Opens the pull request on its code host.
pub const PULL_OPEN: FocusId = FocusId(7_414_000);

/// Pushes the branch.
pub const PULL_PUSH: FocusId = FocusId(7_414_001);

/// Pushes the branch and opens a pull request of it.
pub const PULL_CREATE: FocusId = FocusId(7_414_002);

/// Pushes the branch and opens a draft pull request of it.
pub const PULL_CREATE_DRAFT: FocusId = FocusId(7_414_003);

/// Pushes the branch and redrafts the body of its open pull request.
pub const PULL_UPDATE: FocusId = FocusId(7_414_004);

/// Marks the draft pull request ready for review.
pub const PULL_READY: FocusId = FocusId(7_414_005);

/// Squashes and merges the open pull request.
pub const PULL_MERGE: FocusId = FocusId(7_414_006);

/// Reopens the closed pull request.
pub const PULL_REOPEN: FocusId = FocusId(7_414_007);

/// Deletes the branch from its remote.
pub const PULL_DELETE_BRANCH: FocusId = FocusId(7_414_008);

/// Opens the page of the pull request's checks.
pub const PULL_CHECKS: FocusId = FocusId(7_414_009);

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
#[track_caller]
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
fn tally(icon: IconName, tone: Role, count: usize) {
    if count == 0 {
        return;
    }
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(sp(1)),
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
        PULL_CHECKS,
    )
    .variant(Variant::Ghost)
    .size(Size::Xs)
    .mark(move || {
        row_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(sp(2)),
            Alignment::Center,
            || {
                tally(IconName::CheckCircle, Role::Success, counts.0);
                tally(IconName::Close, Role::Danger, counts.1);
                tally(IconName::Progress, Role::Warning, counts.2);
            },
        );
    })
    .modifier(Modifier::new().tooltip(format!("{summary}: Open the Checks")))
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
fn action(label: &str, id: FocusId, main: bool, tooltip: Option<String>) {
    let mut modifier = Modifier::new();
    if let Some(words) = tooltip {
        modifier = modifier.tooltip(words);
    }
    Button::new(label, id)
        .variant(if main {
            Variant::Default
        } else {
            Variant::Outline
        })
        .size(Size::Xs)
        .modifier(modifier)
        .build();
}

/// Emits the buttons for what can be done to the branch of `session` and its pull request, as that stands.
#[track_caller]
fn actions(session: &Session) {
    let ahead = session.tracking.upstream.is_none() || session.tracking.ahead > 0;
    if unsupported(session).is_some() {
        action("Push", PULL_PUSH, true, None);
        return;
    }
    let noun = title_case(noun(session));
    match &session.pull_request {
        None => {
            action("Create Draft", PULL_CREATE_DRAFT, false, None);
            action(
                &format!("Create {noun}"),
                PULL_CREATE,
                true,
                Some(format!("Push and Open a {noun}")),
            );
        }
        Some(request) if request.is_open() => {
            if ahead {
                action("Push", PULL_PUSH, false, None);
            }
            action(
                "Update",
                PULL_UPDATE,
                false,
                Some("Push and Redraft the Description".to_owned()),
            );
            if request.draft {
                action(
                    "Ready",
                    PULL_READY,
                    true,
                    Some("Ready for Review".to_owned()),
                );
            } else {
                action(
                    "Merge",
                    PULL_MERGE,
                    true,
                    Some("Squash and Merge".to_owned()),
                );
            }
        }
        Some(request) if request.is_closed() => {
            action("Delete Branch", PULL_DELETE_BRANCH, false, None);
            action("Reopen", PULL_REOPEN, true, None);
        }
        Some(_) => action("Delete Branch", PULL_DELETE_BRANCH, false, None),
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
        .h(10)
        .clip_to(Radius::None)
        .stroke_side(Side::Bottom, 1.0, Role::Border)
        .pad_x(5);
    row_arranged(
        modifier,
        Arrangement::SpacedBy(sp(3)),
        Alignment::Center,
        || {
            match (&session.pull_request, unsupported(session)) {
                (_, Some(host)) => {
                    let said = if host.name.is_empty() {
                        "This code host is not supported yet".to_owned()
                    } else {
                        format!("{} is not supported yet", host.name)
                    };
                    glyph(IconName::GitCompare, 3.5, Role::Muted);
                    line(said, 12, 1.4, Role::Muted);
                    spacer();
                }
                (Some(request), None) => {
                    let (said, tone) = standing(request);
                    let number = request.number;
                    Button::new("", PULL_OPEN)
                        .variant(Variant::Ghost)
                        .size(Size::Xs)
                        .mark(move || state_badge(number, said, tone))
                        .modifier(Modifier::new().tooltip(match host_name {
                            Some(name) => format!("Open on {name}: {}", request.title),
                            None => format!("Open: {}", request.title),
                        }))
                        .build();
                    checks(request);
                    review(request);
                    spacer();
                }
                (None, None) => {
                    glyph(IconName::GitCompare, 3.5, Role::Muted);
                    line(format!("No {}", noun(session)), 12, 1.4, Role::Muted);
                    spacer();
                }
            }
            match session.pull_busy {
                Some(doing) => {
                    glyph(IconName::Loading, 3.5, Role::Muted);
                    line(doing, 12, 1.4, Role::Muted);
                }
                None => actions(session),
            }
        },
    );
}

impl WorkspaceState {
    /// Carries out `action` if it is one of the bar's, on the open session, and says whether it was.
    pub(crate) fn press_pull_request(&mut self, action: FocusId) -> bool {
        let (operation, doing) = match action {
            PULL_OPEN | PULL_CHECKS => {
                let address = self
                    .open()
                    .and_then(|session| session.pull_request.as_ref())
                    .map(|request| match action {
                        PULL_CHECKS if !request.checks_url.is_empty() => request.checks_url.clone(),
                        _ => request.url.clone(),
                    });
                if let Some(address) = address {
                    self.outbox.push(Outgoing::Open { address });
                }
                return true;
            }
            PULL_PUSH => ("push", "Pushing…"),
            PULL_CREATE => ("create", "Opening…"),
            PULL_CREATE_DRAFT => ("create_draft", "Opening the draft…"),
            PULL_UPDATE => ("update", "Updating…"),
            PULL_READY => ("ready", "Marking ready…"),
            PULL_MERGE => ("squash", "Merging…"),
            PULL_REOPEN => ("reopen", "Reopening…"),
            PULL_DELETE_BRANCH => ("delete_branch", "Deleting the branch…"),
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
