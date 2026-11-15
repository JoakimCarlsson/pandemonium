//! A number with buttons making it smaller and bigger.

use std::rc::Rc;

use rustle::prelude::*;

use crate::icon_button::IconButton;
use crate::label::Label;
use crate::size::Size;
use rustle::color_scheme;

/// How a stepper is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepperVariant {
    /// The window background with a hairline border.
    #[default]
    Outlined,
    /// No border.
    Plain,
}

/// A stepper with a variant and a size.
#[derive(Clone)]
pub struct Stepper {
    /// The number, as it is written, such as `14 px`.
    shown: String,
    /// How it is drawn.
    variant: StepperVariant,
    /// Its size.
    size: Size,
    /// What a button does: given -1 for smaller and 1 for bigger.
    on_step: Option<Rc<dyn Fn(i32)>>,
}

impl Stepper {
    /// A stepper showing `shown`.
    pub fn new(shown: impl Into<String>) -> Self {
        Self {
            shown: shown.into(),
            variant: StepperVariant::Outlined,
            size: Size::Sm,
            on_step: None,
        }
    }

    /// The stepper in `variant`.
    pub fn variant(mut self, variant: StepperVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The stepper at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The stepper calling `on_step` with -1 when made smaller and 1 when made bigger.
    pub fn on_step(mut self, on_step: impl Fn(i32) + 'static) -> Self {
        self.on_step = Some(Rc::new(on_step));
        self
    }

    /// The width a stepper takes.
    pub const WIDTH: f32 = 136.0;

    /// Emits the stepper.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let height = match self.size {
            Size::Xs => 22.0,
            Size::Sm => 28.0,
            Size::Md => 32.0,
            Size::Lg => 38.0,
        };
        let height = crate::row_height(height);
        let mut modifier = Modifier::new()
            .width(Self::WIDTH)
            .height(height)
            .background_rounded(scheme.background, 6.0)
            .padding_xy(2.0, 0.0);
        if self.variant == StepperVariant::Outlined {
            modifier = modifier.border(1.0, scheme.outline_variant, 6.0);
        }
        let smaller = self.on_step.clone();
        let bigger = self.on_step;
        let shown = self.shown;
        row_arranged(
            modifier,
            Arrangement::SpaceBetween,
            Alignment::Center,
            || {
                IconButton::new(phosphor::MINUS)
                    .size(Size::Xs)
                    .on_click(move || {
                        if let Some(step) = &smaller {
                            step(-1);
                        }
                    })
                    .show();
                Label::new(shown).show();
                IconButton::new(phosphor::PLUS)
                    .size(Size::Xs)
                    .on_click(move || {
                        if let Some(step) = &bigger {
                            step(1);
                        }
                    })
                    .show();
            },
        );
    }
}
