//! The agent's work shown inline in its reply: one row per tool it used,
//! opening onto what the tool printed, and its thinking folded to a line.

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::label::{Label, LabelVariant};
use crate::markdown::{Markdown, MarkdownVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a step stands, which picks the mark at its end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepVariant {
    /// Still running.
    #[default]
    Running,
    /// Finished.
    Done,
    /// Failed.
    Failed,
}

/// One tool call of the agent, as a row that opens onto its output.
#[derive(Clone, Debug)]
pub struct Step {
    /// What the call does.
    title: String,
    /// The glyph naming the kind of tool.
    glyph: Icon,
    /// How it stands.
    variant: StepVariant,
    /// What it printed or returned.
    output: Option<String>,
    /// Lines added and removed, for an edit.
    lines: Option<(usize, usize)>,
    /// Its text size.
    size: Size,
}

impl Step {
    /// A step titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            glyph: phosphor::WRENCH,
            variant: StepVariant::Running,
            output: None,
            lines: None,
            size: Size::Xs,
        }
    }

    /// The step with `glyph` naming its kind of tool.
    pub fn icon(mut self, glyph: Icon) -> Self {
        self.glyph = glyph;
        self
    }

    /// The step with `variant`.
    pub fn variant(mut self, variant: StepVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The step opening onto `output`.
    pub fn output(mut self, output: Option<String>) -> Self {
        self.output = output.filter(|text| !text.trim().is_empty());
        self
    }

    /// The step counting `added` and `removed` lines.
    pub fn lines(mut self, lines: Option<(usize, usize)>) -> Self {
        self.lines = lines;
        self
    }

    /// The step at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the step.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let open = remember(|| self.variant == StepVariant::Failed);
        let toggle = open.clone();
        let has_output = self.output.is_some();
        column_with(Modifier::new().fill_max_width(), move || {
            let mut header = Modifier::new()
                .fill_max_width()
                .height(24.0)
                .padding_xy(4.0, 0.0);
            if has_output {
                header = header
                    .cursor(CursorIcon::Pointer)
                    .clickable(move || toggle.set(!toggle.get()));
            }
            row_arranged(
                header,
                Arrangement::SpacedBy(8.0),
                Alignment::Center,
                || {
                    icon(self.glyph.clone(), 13.0, scheme.on_surface_variant);
                    Label::new(self.title.clone())
                        .variant(LabelVariant::Muted)
                        .size(self.size)
                        .modifier(Modifier::new().weight(1.0).clip())
                        .show();
                    if let Some((added, removed)) = self.lines {
                        Label::new(format!("+{added}"))
                            .colour(extended.success)
                            .size(self.size)
                            .show();
                        Label::new(format!("-{removed}"))
                            .colour(scheme.error)
                            .size(self.size)
                            .show();
                    }
                    let (mark, tint) = match self.variant {
                        StepVariant::Running => (phosphor::CIRCLE_NOTCH, scheme.primary),
                        StepVariant::Done => (phosphor::CHECK, scheme.on_surface_variant),
                        StepVariant::Failed => (phosphor::X, scheme.error),
                    };
                    icon(mark, 12.0, tint);
                    if has_output {
                        icon(
                            if open.get() {
                                phosphor::CARET_DOWN
                            } else {
                                phosphor::CARET_RIGHT
                            },
                            11.0,
                            scheme.on_surface_variant,
                        );
                    }
                },
            );
            if let (true, Some(output)) = (open.get(), &self.output) {
                let output = output.clone();
                column_with(
                    Modifier::new()
                        .fill_max_width()
                        .padding_each(25.0, 2.0, 4.0, 4.0),
                    move || {
                        column_with(
                            Modifier::new()
                                .fill_max_width()
                                .background_rounded(scheme.surface_container, 6.0)
                                .padding_xy(10.0, 8.0),
                            move || {
                                Label::new(output)
                                    .variant(LabelVariant::Code)
                                    .size(Size::Xs)
                                    .wrap()
                                    .show();
                            },
                        );
                    },
                );
            }
        });
    }
}

/// What the agent thought, folded to one line until opened.
#[derive(Clone, Debug)]
pub struct Thought {
    /// The thinking, as markdown source.
    text: String,
    /// Its text size.
    size: Size,
}

impl Thought {
    /// The thought `text`.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            size: Size::Xs,
        }
    }

    /// The thought at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the thought.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let open = remember(|| false);
        let toggle = open.clone();
        let first = self.text.lines().next().unwrap_or_default().to_owned();
        column_with(Modifier::new().fill_max_width(), move || {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(24.0)
                    .padding_xy(4.0, 0.0)
                    .cursor(CursorIcon::Pointer)
                    .clickable(move || toggle.set(!toggle.get())),
                Arrangement::SpacedBy(8.0),
                Alignment::Center,
                || {
                    icon(phosphor::BRAIN, 13.0, scheme.on_surface_variant);
                    Label::new("Thought")
                        .variant(LabelVariant::Muted)
                        .size(self.size)
                        .show();
                    if !open.get() {
                        Label::new(first)
                            .variant(LabelVariant::Subtle)
                            .size(self.size)
                            .modifier(Modifier::new().weight(1.0).clip())
                            .show();
                    } else {
                        spacer(Modifier::new().weight(1.0));
                    }
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
                let parsed = pm_markdown::parse(&self.text);
                row_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .height_intrinsic(IntrinsicSize::Min)
                        .padding_each(10.0, 2.0, 0.0, 4.0),
                    Arrangement::SpacedBy(12.0),
                    Alignment::Start,
                    move || {
                        spacer(
                            Modifier::new()
                                .width(2.0)
                                .fill_max_height()
                                .background(scheme.outline_variant),
                        );
                        column_with(Modifier::new().weight(1.0), move || {
                            Markdown::new(parsed)
                                .variant(MarkdownVariant::Muted)
                                .size(self.size)
                                .show()
                        });
                    },
                );
            }
        });
    }
}
