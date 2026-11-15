//! The card asking the reader to trust a project before the agent uses what
//! its folder brings: hooks, instructions, skills and agents.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::button::{Button, ButtonVariant};
use crate::label::{Label, LabelVariant};
use crate::notice::faded;
use crate::size::Size;
use rustle::color_scheme;

/// What the reader answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustAnswer {
    /// Trust it for this conversation.
    Once,
    /// Trust it from now on.
    Always,
    /// Do not trust it.
    Reject,
}

/// The visual treatment of the trust card.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrustVariant {
    /// In caution colours, as a request waiting on the reader.
    #[default]
    Caution,
    /// Neutral, as a reminder.
    Neutral,
}

/// The card asking to trust the project at a folder.
#[derive(Clone)]
pub struct TrustCard {
    /// The folder asked about.
    root: String,
    /// Why it asks.
    reason: String,
    /// What was left out: each kind and name.
    skipped: Vec<(String, String)>,
    /// What answering does.
    on_answer: Option<Rc<dyn Fn(TrustAnswer)>>,
    /// Its visual treatment.
    variant: TrustVariant,
    /// Its text size.
    size: Size,
}

impl TrustCard {
    /// A card asking to trust the folder `root`.
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            reason: String::new(),
            skipped: Vec::new(),
            on_answer: None,
            variant: TrustVariant::Caution,
            size: Size::Sm,
        }
    }

    /// The card saying why it asks.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    /// The card listing an input of `kind` named `name` that was left out.
    pub fn skipped(mut self, kind: impl Into<String>, name: impl Into<String>) -> Self {
        self.skipped.push((kind.into(), name.into()));
        self
    }

    /// The card running `on_answer` with the reader's answer.
    pub fn on_answer(mut self, on_answer: impl Fn(TrustAnswer) + 'static) -> Self {
        self.on_answer = Some(Rc::new(on_answer));
        self
    }

    /// The card with `variant`.
    pub fn variant(mut self, variant: TrustVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The card at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the card.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let tint = match self.variant {
            TrustVariant::Caution => extended.warning,
            TrustVariant::Neutral => scheme.on_surface_variant,
        };
        column_arranged(
            Modifier::new()
                .fill_max_width()
                .background_rounded(faded(tint, 18), 8.0)
                .border(1.0, faded(tint, 90), 8.0)
                .padding_xy(14.0, 12.0),
            Arrangement::SpacedBy(8.0),
            Alignment::Start,
            move || {
                row_arranged(
                    Modifier::new().fill_max_width(),
                    Arrangement::SpacedBy(8.0),
                    Alignment::Center,
                    || {
                        icon(phosphor::SHIELD_WARNING, 15.0, tint);
                        Label::new(format!("Trust {}?", self.root))
                            .variant(LabelVariant::Heading)
                            .size(self.size)
                            .show();
                    },
                );
                if !self.reason.is_empty() {
                    Label::new(format!(
                        "{} Until it is trusted, the agent works without what the project brings:",
                        self.reason
                    ))
                    .variant(LabelVariant::Muted)
                    .size(self.size)
                    .wrap()
                    .show();
                }
                for (kind, name) in &self.skipped {
                    row_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(8.0),
                        Alignment::Center,
                        || {
                            Label::new(kind.clone())
                                .variant(LabelVariant::Subtle)
                                .size(Size::Xs)
                                .modifier(Modifier::new().width(80.0))
                                .show();
                            Label::new(name.clone())
                                .variant(LabelVariant::Code)
                                .size(Size::Xs)
                                .show();
                        },
                    );
                }
                flow_row(Modifier::new().fill_max_width(), 8.0, 8.0, || {
                    for (label, answer, variant) in [
                        ("Trust always", TrustAnswer::Always, ButtonVariant::Primary),
                        (
                            "Trust this time",
                            TrustAnswer::Once,
                            ButtonVariant::Secondary,
                        ),
                        ("Don't trust", TrustAnswer::Reject, ButtonVariant::Ghost),
                    ] {
                        let on_answer = self.on_answer.clone();
                        Button::new(label)
                            .variant(variant)
                            .size(Size::Sm)
                            .on_click(move || {
                                if let Some(on_answer) = &on_answer {
                                    on_answer(answer);
                                }
                            })
                            .show();
                    }
                });
            },
        );
    }
}
