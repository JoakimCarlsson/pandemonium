//! The bar above the conversation naming the pull request of the session's
//! branch, how its checks and review stand, and what can be done with it.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::badge::{Badge, BadgeVariant};
use crate::button::{Button, ButtonVariant};
use crate::icon_button::IconButton;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How the pull request stands, which picks its colour and its actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PullRequestVariant {
    /// The branch has no pull request yet.
    #[default]
    None,
    /// Open and ready for review.
    Open,
    /// Open as a draft.
    Draft,
    /// Merged.
    Merged,
    /// Closed without merging.
    Closed,
}

/// The pull request bar.
#[derive(Clone)]
pub struct PullRequestBar {
    /// Its number, or nought for none.
    number: u64,
    /// Its title.
    title: String,
    /// How it stands.
    variant: PullRequestVariant,
    /// The review decision, or empty.
    review: String,
    /// Checks passed, failed and pending.
    checks: (usize, usize, usize),
    /// What each action does, given its name such as `merge`.
    on_action: Option<Rc<dyn Fn(String)>>,
    /// Its text size.
    size: Size,
    /// What the code host calls a review, in lower case.
    noun: String,
    /// Whether the code host's reviews can be acted on; only pushing is offered when not.
    supported: bool,
    /// Whether the branch has commits its remote has not.
    ahead: bool,
    /// What is under way, such as `Pushing…`, which replaces the buttons.
    busy: Option<String>,
    /// Why the last action failed.
    error: Option<String>,
}

impl PullRequestBar {
    /// A bar for the pull request numbered `number` titled `title`.
    pub fn new(number: u64, title: impl Into<String>) -> Self {
        Self {
            number,
            title: title.into(),
            variant: PullRequestVariant::None,
            review: String::new(),
            checks: (0, 0, 0),
            on_action: None,
            size: Size::Xs,
            noun: "pull request".to_owned(),
            supported: true,
            ahead: false,
            busy: None,
            error: None,
        }
    }

    /// The bar calling a review `noun`, in lower case, as the code host does.
    pub fn noun(mut self, noun: impl Into<String>) -> Self {
        let noun = noun.into();
        if !noun.is_empty() {
            self.noun = noun;
        }
        self
    }

    /// The bar offering only to push while the code host's reviews are not `supported`.
    pub fn supported(mut self, supported: bool) -> Self {
        self.supported = supported;
        self
    }

    /// The bar offering to push while the branch is `ahead` of its remote.
    pub fn ahead(mut self, ahead: bool) -> Self {
        self.ahead = ahead;
        self
    }

    /// The bar saying `busy` in place of its buttons while something is under way.
    pub fn busy(mut self, busy: Option<String>) -> Self {
        self.busy = busy;
        self
    }

    /// The bar saying why the last action failed.
    pub fn error(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
    }

    /// The bar with `variant`.
    pub fn variant(mut self, variant: PullRequestVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The bar naming the review decision `review`.
    pub fn review(mut self, review: impl Into<String>) -> Self {
        self.review = review.into();
        self
    }

    /// The bar counting checks `passed`, `failed` and `pending`.
    pub fn checks(mut self, passed: usize, failed: usize, pending: usize) -> Self {
        self.checks = (passed, failed, pending);
        self
    }

    /// The bar running `on_action` with the name of the action pressed:
    /// `push`, `create`, `create_draft`, `ready`, `merge`, `squash`,
    /// `reopen`, `delete_branch` or `open`.
    pub fn on_action(mut self, on_action: impl Fn(String) + 'static) -> Self {
        self.on_action = Some(Rc::new(on_action));
        self
    }

    /// The bar at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the bar.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let (tint, state) = match self.variant {
            PullRequestVariant::None => (scheme.on_surface_variant, ""),
            PullRequestVariant::Open => (extended.success, "Open"),
            PullRequestVariant::Draft => (scheme.on_surface_variant, "Draft"),
            PullRequestVariant::Merged => (scheme.primary, "Merged"),
            PullRequestVariant::Closed => (scheme.error, "Closed"),
        };
        let mut actions: Vec<(String, &str, &str)> = Vec::new();
        let open = matches!(
            self.variant,
            PullRequestVariant::Open | PullRequestVariant::Draft
        );
        if !self.supported || (open && self.ahead) {
            actions.push(("Push".to_owned(), "push", "Push the branch"));
        }
        if self.supported {
            match self.variant {
                PullRequestVariant::None => {
                    actions.push((
                        "Create draft".to_owned(),
                        "create_draft",
                        "Push and open a draft",
                    ));
                    actions.push((
                        format!("Create {}", self.noun),
                        "create",
                        "Push and open it for review",
                    ));
                }
                PullRequestVariant::Open | PullRequestVariant::Draft => {
                    actions.push((
                        "Update".to_owned(),
                        "update",
                        "Push and redraft the description",
                    ));
                    if self.variant == PullRequestVariant::Draft {
                        actions.push(("Ready".to_owned(), "ready", "Ready for review"));
                    } else {
                        actions.push(("Merge".to_owned(), "merge", "Squash and merge"));
                    }
                }
                PullRequestVariant::Closed => {
                    actions.push((
                        "Delete branch".to_owned(),
                        "delete_branch",
                        "Delete the branch from its remote",
                    ));
                    actions.push(("Reopen".to_owned(), "reopen", "Reopen it"));
                }
                PullRequestVariant::Merged => {
                    actions.push((
                        "Delete branch".to_owned(),
                        "delete_branch",
                        "Delete the branch from its remote",
                    ));
                }
            }
        }
        row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(34.0)
                .border_bottom_hairline(scheme.outline_variant)
                .padding_xy(12.0, 0.0),
            Arrangement::SpacedBy(crate::spacing(8.0)),
            Alignment::Center,
            move || {
                icon(phosphor::GIT_PULL_REQUEST, 14.0, tint);
                if self.variant == PullRequestVariant::None {
                    Label::new(format!("No {} for this branch yet", self.noun))
                        .variant(LabelVariant::Muted)
                        .size(self.size)
                        .modifier(Modifier::new().weight(1.0))
                        .show();
                } else {
                    Label::new(format!("#{}", self.number))
                        .variant(LabelVariant::Muted)
                        .size(self.size)
                        .show();
                    Label::new(self.title.clone())
                        .size(self.size)
                        .modifier(Modifier::new().weight(1.0).clip())
                        .show();
                    Badge::new(state)
                        .variant(match self.variant {
                            PullRequestVariant::Open => BadgeVariant::Success,
                            PullRequestVariant::Merged => BadgeVariant::Accent,
                            PullRequestVariant::Closed => BadgeVariant::Danger,
                            _ => BadgeVariant::Neutral,
                        })
                        .size(Size::Xs)
                        .show();
                    checks(self.checks, self.size);
                    if !self.review.is_empty() {
                        Label::new(self.review.clone())
                            .variant(LabelVariant::Subtle)
                            .size(self.size)
                            .show();
                    }
                }
                if let Some(error) = &self.error {
                    crate::tip::tip(error, || {
                        icon(phosphor::WARNING, 14.0, scheme.error);
                    });
                }
                if let Some(busy) = &self.busy {
                    circular_progress_indicator_indeterminate(10.0);
                    Label::new(busy.clone())
                        .variant(LabelVariant::Muted)
                        .size(self.size)
                        .show();
                    return;
                }
                let count = actions.len();
                for (index, (label, action, tip)) in actions.into_iter().enumerate() {
                    let on_action = self.on_action.clone();
                    let action = action.to_owned();
                    let button = Button::new(label)
                        .variant(if index + 1 == count {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(Size::Xs)
                        .on_click(move || {
                            if let Some(on_action) = &on_action {
                                on_action(action.clone());
                            }
                        });
                    crate::tip::tip(tip, move || button.show());
                }
                if self.variant != PullRequestVariant::None {
                    let on_action = self.on_action.clone();
                    IconButton::new(phosphor::ARROW_SQUARE_OUT)
                        .size(Size::Xs)
                        .tooltip("Open in browser")
                        .on_click(move || {
                            if let Some(on_action) = &on_action {
                                on_action("open".to_owned());
                            }
                        })
                        .show();
                }
            },
        );
    }
}

/// Emits the counts of checks passed, failed and pending, leaving out none.
fn checks((passed, failed, pending): (usize, usize, usize), size: Size) {
    let scheme = color_scheme();
    let extended = extended_colors();
    for (count, glyph, tint) in [
        (passed, phosphor::CHECK_CIRCLE, extended.success),
        (failed, phosphor::X_CIRCLE, scheme.error),
        (pending, phosphor::CIRCLE_DASHED, extended.warning),
    ] {
        if count == 0 {
            continue;
        }
        row_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(crate::spacing(3.0)),
            Alignment::Center,
            || {
                icon(glyph, 12.0, tint);
                Label::new(count.to_string())
                    .variant(LabelVariant::Muted)
                    .size(size)
                    .show();
            },
        );
    }
}
