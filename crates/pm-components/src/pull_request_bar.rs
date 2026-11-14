//! The bar under a session's header that tells how the pull request of its branch stands, with its checks and review, and offers what can be done next: push, open, update, mark ready, merge, reopen, and delete the branch from its remote. It names the review as the code host its repository is on does, and offers only pushing when that host is not supported.

use pm_git::{CheckState, CodeHost, PullRequestStatus, ReviewDecision};
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, border_b, border_color, flex, flex_none, gap, h, items_center, overflow_hidden,
    px, rounded,
};
use pm_ui::{Node, node};

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

/// The badge naming the pull request's number and how it stands, in the colour of that.
fn state_badge(request: &PullRequestStatus) -> Node {
    let (said, tone) = if request.is_merged() {
        ("Merged", Role::Accent)
    } else if request.is_closed() {
        ("Closed", Role::Danger)
    } else if request.draft {
        ("Draft", Role::Muted)
    } else {
        ("Open", Role::Success)
    };
    Badge::new(format!("#{} {said}", request.number))
        .tone(tone)
        .variant(BadgeVariant::Soft)
        .build()
}

/// A glyph in `tone` followed by `count`, or nothing when the count is nothing.
fn tally(icon: IconName, tone: Role, count: usize) -> Option<Node> {
    (count > 0).then(|| {
        node()
            .styles([flex(), items_center(), gap(1), flex_none()])
            .child(glyph(icon, 3.5, tone))
            .child(line(count.to_string(), 12, 1.4, tone))
    })
}

/// The button counting the checks by how they stand, whose tooltip names the first that failed; it opens the page of the checks.
fn checks(request: &PullRequestStatus) -> Node {
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
    .mark(
        node()
            .styles([flex(), items_center(), gap(2), flex_none()])
            .children(tally(
                IconName::CheckCircle,
                Role::Success,
                request.count(CheckState::Passed),
            ))
            .children(tally(
                IconName::Close,
                Role::Danger,
                request.count(CheckState::Failed),
            ))
            .children(tally(
                IconName::Progress,
                Role::Warning,
                request.count(CheckState::Pending),
            )),
    )
    .build()
    .styles([flex_none()])
    .tooltip(format!("{summary}: Open the Checks"))
}

/// What the reviewers decided, in words and the colour of that, or nothing when nothing is asked of them.
fn review(request: &PullRequestStatus) -> Option<Node> {
    let (said, tone) = match request.review {
        ReviewDecision::None => return None,
        ReviewDecision::Required => ("Review required", Role::Warning),
        ReviewDecision::Approved => ("Approved", Role::Success),
        ReviewDecision::ChangesRequested => ("Changes requested", Role::Danger),
    };
    Some(Badge::new(said).tone(tone).build())
}

/// A small button saying `label` that does `id`, filled when it is the `main` thing to do.
fn action(label: &str, id: FocusId, main: bool) -> Node {
    Button::new(label, id)
        .variant(if main {
            Variant::Default
        } else {
            Variant::Outline
        })
        .size(Size::Xs)
        .build()
        .styles([flex_none()])
}

/// The buttons for what can be done to the branch of `session` and its pull request, as that stands.
fn actions(session: &Session) -> Vec<Node> {
    let ahead = session.tracking.upstream.is_none() || session.tracking.ahead > 0;
    if unsupported(session).is_some() {
        return vec![action("Push", PULL_PUSH, true)];
    }
    let noun = title_case(noun(session));
    match &session.pull_request {
        None => vec![
            action("Create Draft", PULL_CREATE_DRAFT, false),
            action(&format!("Create {noun}"), PULL_CREATE, true)
                .tooltip(format!("Push and Open a {noun}")),
        ],
        Some(request) if request.is_open() => {
            let mut buttons = Vec::new();
            if ahead {
                buttons.push(action("Push", PULL_PUSH, false));
            }
            buttons.push(
                action("Update", PULL_UPDATE, false).tooltip("Push and Redraft the Description"),
            );
            if request.draft {
                buttons.push(action("Ready", PULL_READY, true).tooltip("Ready for Review"));
            } else {
                buttons.push(action("Merge", PULL_MERGE, true).tooltip("Squash and Merge"));
            }
            buttons
        }
        Some(request) if request.is_closed() => vec![
            action("Delete Branch", PULL_DELETE_BRANCH, false),
            action("Reopen", PULL_REOPEN, true),
        ],
        Some(_) => vec![action("Delete Branch", PULL_DELETE_BRANCH, false)],
    }
}

/// The bar for `session`, or nothing when it has no branch of its own to open a pull request of.
pub fn bar(session: &Session) -> Option<Node> {
    if !shows(session) {
        return None;
    }
    let mut strip = node().styles([
        flex(),
        overflow_hidden(),
        items_center(),
        gap(3),
        h(10),
        px(5),
        flex_none(),
        border_b(),
        border_color(Role::Border),
    ]);
    let host_name = session
        .code_host
        .as_ref()
        .map(|host| host.name.as_str())
        .filter(|name| !name.is_empty());
    match (&session.pull_request, unsupported(session)) {
        (_, Some(host)) => {
            let said = if host.name.is_empty() {
                "This code host is not supported yet".to_owned()
            } else {
                format!("{} is not supported yet", host.name)
            };
            strip = strip
                .child(glyph(IconName::GitCompare, 3.5, Role::Muted))
                .child(line(said, 12, 1.4, Role::Muted))
                .child(spacer());
        }
        (Some(request), None) => {
            strip = strip
                .child(
                    Button::new("", PULL_OPEN)
                        .variant(Variant::Ghost)
                        .size(Size::Xs)
                        .mark(state_badge(request))
                        .build()
                        .styles([flex_none(), rounded(Radius::Md)])
                        .tooltip(match host_name {
                            Some(name) => format!("Open on {name}: {}", request.title),
                            None => format!("Open: {}", request.title),
                        }),
                )
                .child(checks(request))
                .children(review(request))
                .child(spacer());
        }
        (None, None) => {
            strip = strip
                .child(glyph(IconName::GitCompare, 3.5, Role::Muted))
                .child(line(format!("No {}", noun(session)), 12, 1.4, Role::Muted))
                .child(spacer());
        }
    }
    strip = match session.pull_busy {
        Some(doing) => strip
            .child(glyph(IconName::Loading, 3.5, Role::Muted))
            .child(line(doing, 12, 1.4, Role::Muted)),
        None => strip.children(actions(session)),
    };
    Some(strip)
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
