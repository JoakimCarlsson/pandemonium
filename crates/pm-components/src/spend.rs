//! The strip under the conversation saying what it has cost so far, opening
//! onto the cost of each model asked.

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// The visual treatment of the spend strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpendVariant {
    /// A faint line of text.
    #[default]
    Quiet,
    /// On a surface, as a pill.
    Filled,
}

/// What the conversation cost: a total line, opening onto each model.
#[derive(Clone, Debug)]
pub struct SpendStrip {
    /// The total as shown, such as `$1.84`.
    total: String,
    /// The tokens and requests line.
    detail: String,
    /// Each model: its name, its cost as shown and its tokens as shown.
    models: Vec<(String, String, String)>,
    /// Its visual treatment.
    variant: SpendVariant,
    /// Its text size.
    size: Size,
}

impl SpendStrip {
    /// A strip whose total reads `total`.
    pub fn new(total: impl Into<String>) -> Self {
        Self {
            total: total.into(),
            detail: String::new(),
            models: Vec::new(),
            variant: SpendVariant::Quiet,
            size: Size::Xs,
        }
    }

    /// The strip saying `detail` after its total, such as tokens and requests.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// The strip listing the model `name` at `cost` with `tokens`.
    pub fn model(
        mut self,
        name: impl Into<String>,
        cost: impl Into<String>,
        tokens: impl Into<String>,
    ) -> Self {
        self.models.push((name.into(), cost.into(), tokens.into()));
        self
    }

    /// The strip with `variant`.
    pub fn variant(mut self, variant: SpendVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The strip at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the strip.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let open = remember(|| false);
        let toggle = open.clone();
        let mut modifier = Modifier::new()
            .padding_xy(8.0, 3.0)
            .cursor(CursorIcon::Pointer)
            .clickable(move || toggle.set(!toggle.get()));
        if self.variant == SpendVariant::Filled {
            modifier = modifier.background_rounded(scheme.surface_container_high, 10.0);
        }
        let breakdown = self
            .models
            .iter()
            .map(|(name, cost, tokens)| format!("{name}: {cost}, {tokens}"))
            .collect::<Vec<_>>()
            .join("\n");
        column_with(Modifier::new(), move || {
            let row = || {
                row_arranged(
                    modifier,
                    Arrangement::SpacedBy(crate::spacing(6.0)),
                    Alignment::Center,
                    || {
                        icon(phosphor::COINS, 12.0, scheme.on_surface_variant);
                        Label::new(self.total.clone())
                            .variant(LabelVariant::Muted)
                            .size(self.size)
                            .show();
                        if !self.detail.is_empty() {
                            Label::new(self.detail.clone())
                                .variant(LabelVariant::Subtle)
                                .size(self.size)
                                .show();
                        }
                    },
                )
            };
            if breakdown.is_empty() || open.get() {
                row();
            } else {
                crate::tip::tip(&breakdown, row);
            }
            if open.get() {
                for (name, cost, tokens) in &self.models {
                    row_arranged(
                        Modifier::new().padding_each(26.0, 2.0, 8.0, 0.0),
                        Arrangement::SpacedBy(crate::spacing(10.0)),
                        Alignment::Center,
                        || {
                            Label::new(name.clone())
                                .variant(LabelVariant::Muted)
                                .size(self.size)
                                .modifier(Modifier::new().width(110.0))
                                .show();
                            Label::new(cost.clone())
                                .variant(LabelVariant::Muted)
                                .size(self.size)
                                .modifier(Modifier::new().width(54.0))
                                .show();
                            Label::new(tokens.clone())
                                .variant(LabelVariant::Subtle)
                                .size(self.size)
                                .show();
                        },
                    );
                }
            }
        });
    }
}
