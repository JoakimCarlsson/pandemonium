//! The agent's plan as a panel of steps with their progress, and the band
//! that asks the reader to approve a plan or a permission.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::button::{Button, ButtonVariant};
use crate::card::{Card, CardVariant};
use crate::label::{Label, LabelVariant};
use crate::markdown::Markdown;
use crate::notice::faded;
use crate::size::Size;
use rustle::color_scheme;

/// How far along a plan step is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlanStepState {
    /// Not started.
    #[default]
    Pending,
    /// Being worked on.
    Doing,
    /// Done.
    Done,
}

/// The visual treatment of the plan panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlanVariant {
    /// On an outlined card.
    #[default]
    Card,
    /// Flat, without a card.
    Flat,
}

/// The agent's plan: a header counting the steps done, then each step.
#[derive(Clone, Debug, Default)]
pub struct PlanPanel {
    /// Each step and how far along it is.
    steps: Vec<(String, PlanStepState)>,
    /// Its visual treatment.
    variant: PlanVariant,
    /// Its text size.
    size: Size,
}

impl PlanPanel {
    /// An empty plan.
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            variant: PlanVariant::Card,
            size: Size::Xs,
        }
    }

    /// The plan with a step saying `text` at `state`.
    pub fn step(mut self, text: impl Into<String>, state: PlanStepState) -> Self {
        self.steps.push((text.into(), state));
        self
    }

    /// The plan with `variant`.
    pub fn variant(mut self, variant: PlanVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The plan at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the plan, folded to its header until opened; nothing when it has no steps.
    #[track_caller]
    pub fn show(self) {
        if self.steps.is_empty() {
            return;
        }
        let scheme = color_scheme();
        let extended = extended_colors();
        let open = remember(|| true);
        let toggle = open.clone();
        let done = self
            .steps
            .iter()
            .filter(|(_, state)| *state == PlanStepState::Done)
            .count();
        let total = self.steps.len();
        let body = move || {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .cursor(CursorIcon::Pointer)
                    .clickable(move || toggle.set(!toggle.get())),
                Arrangement::SpacedBy(crate::spacing(8.0)),
                Alignment::Center,
                || {
                    icon(phosphor::LIST_CHECKS, 14.0, scheme.on_surface_variant);
                    Label::new("Plan").size(self.size).show();
                    Label::new(format!("{done} of {total} done"))
                        .variant(LabelVariant::Subtle)
                        .size(self.size)
                        .modifier(Modifier::new().weight(1.0))
                        .show();
                    icon(
                        if open.get() {
                            phosphor::CARET_DOWN
                        } else {
                            phosphor::CARET_RIGHT
                        },
                        11.0,
                        scheme.on_surface_variant,
                    );
                },
            );
            if open.get() {
                spacer(Modifier::new().height(6.0));
                for (text, state) in &self.steps {
                    row_arranged(
                        Modifier::new().fill_max_width().padding_xy(0.0, 2.0),
                        Arrangement::SpacedBy(crate::spacing(8.0)),
                        Alignment::Center,
                        || {
                            let (mark, tint) = match state {
                                PlanStepState::Pending => {
                                    (phosphor::CIRCLE, scheme.on_surface_variant)
                                }
                                PlanStepState::Doing => (phosphor::CIRCLE_HALF, scheme.primary),
                                PlanStepState::Done => (phosphor::CHECK_CIRCLE, extended.success),
                            };
                            icon(mark, 13.0, tint);
                            let mut label = Label::new(text.clone()).size(self.size).wrap();
                            label = match state {
                                PlanStepState::Done => label.variant(LabelVariant::Subtle),
                                PlanStepState::Doing => label,
                                PlanStepState::Pending => label.variant(LabelVariant::Muted),
                            };
                            label.modifier(Modifier::new().weight(1.0)).show();
                        },
                    );
                }
            }
        };
        match self.variant {
            PlanVariant::Card => Card::new()
                .variant(CardVariant::Outlined)
                .size(Size::Sm)
                .modifier(Modifier::new().fill_max_width())
                .show(body),
            PlanVariant::Flat => column_with(Modifier::new().fill_max_width(), body),
        }
    }
}

/// The visual treatment of the approve band.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ApproveBandVariant {
    /// Approving a plan, in the accent.
    #[default]
    Plan,
    /// Approving a permission such as running a command, in caution.
    Permission,
}

/// One answer the approve band offers.
#[derive(Clone)]
struct BandChoice {
    /// What it says.
    label: String,
    /// Whether it refuses, which sets it apart.
    refuses: bool,
    /// What choosing it does.
    on_click: Rc<dyn Fn()>,
}

/// The band that asks the reader to approve: a title, the plan or command
/// asked about, and a button per answer.
#[derive(Clone)]
pub struct ApproveBand {
    /// What is asked.
    title: String,
    /// The command it would run.
    command: Option<String>,
    /// The plan written out.
    plan: Option<Vec<pm_markdown::Block>>,
    /// The answers.
    choices: Vec<BandChoice>,
    /// Its visual treatment.
    variant: ApproveBandVariant,
    /// Its text size.
    size: Size,
}

impl ApproveBand {
    /// A band asking `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            command: None,
            plan: None,
            choices: Vec::new(),
            variant: ApproveBandVariant::Plan,
            size: Size::Sm,
        }
    }

    /// The band showing the command `command` it asks to run.
    pub fn command(mut self, command: Option<String>) -> Self {
        self.command = command;
        self
    }

    /// The band showing the parsed `plan` it asks to approve.
    pub fn plan(mut self, plan: Option<Vec<pm_markdown::Block>>) -> Self {
        self.plan = plan;
        self
    }

    /// The band with an answer `label`, refusing when `refuses`, that runs `on_click`.
    pub fn choice(
        mut self,
        label: impl Into<String>,
        refuses: bool,
        on_click: impl Fn() + 'static,
    ) -> Self {
        self.choices.push(BandChoice {
            label: label.into(),
            refuses,
            on_click: Rc::new(on_click),
        });
        self
    }

    /// The band with `variant`.
    pub fn variant(mut self, variant: ApproveBandVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The band at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the band.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let (tint, glyph) = match self.variant {
            ApproveBandVariant::Plan => (scheme.primary, phosphor::LIST_CHECKS),
            ApproveBandVariant::Permission => (extended.warning, phosphor::SHIELD_WARNING),
        };
        column_arranged(
            Modifier::new()
                .fill_max_width()
                .background_rounded(faded(tint, 18), 8.0)
                .border(1.0, faded(tint, 90), 8.0)
                .padding_xy(14.0, 12.0),
            Arrangement::SpacedBy(crate::spacing(10.0)),
            Alignment::Start,
            move || {
                row_arranged(
                    Modifier::new().fill_max_width(),
                    Arrangement::SpacedBy(crate::spacing(8.0)),
                    Alignment::Center,
                    || {
                        icon(glyph, 15.0, tint);
                        Label::new(self.title.clone())
                            .variant(LabelVariant::Heading)
                            .size(self.size)
                            .show();
                    },
                );
                if let Some(command) = &self.command {
                    Label::new(command.clone())
                        .variant(LabelVariant::Code)
                        .size(Size::Xs)
                        .wrap()
                        .modifier(
                            Modifier::new()
                                .fill_max_width()
                                .background_rounded(scheme.surface_container, 6.0)
                                .padding_xy(10.0, 6.0),
                        )
                        .show();
                }
                if let Some(plan) = &self.plan {
                    Markdown::new(plan.clone()).size(self.size).show();
                }
                flow_row(Modifier::new().fill_max_width(), 8.0, 8.0, || {
                    for (index, choice) in self.choices.iter().enumerate() {
                        let on_click = choice.on_click.clone();
                        let variant = match (index, choice.refuses) {
                            (_, true) => ButtonVariant::Ghost,
                            (0, false) => ButtonVariant::Primary,
                            _ => ButtonVariant::Secondary,
                        };
                        Button::new(choice.label.clone())
                            .variant(variant)
                            .size(Size::Sm)
                            .on_click(move || on_click())
                            .show();
                    }
                });
            },
        );
    }
}
