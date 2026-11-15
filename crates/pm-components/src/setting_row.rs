//! A row of a settings page: what the setting is called and does, its
//! control, a button putting it back as it comes, and anything shown under it.

use std::rc::Rc;

use rustle::prelude::*;

use crate::icon_button::IconButton;
use crate::label::{Label, LabelVariant};
use crate::size::Size;

/// How a setting row is laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingRowVariant {
    /// The words on the left and the control on the right.
    #[default]
    Wide,
    /// The words over the control, for a narrow window.
    Stacked,
}

/// A setting row with a variant and a size.
pub struct SettingRow {
    /// What the setting is called.
    title: String,
    /// What it does.
    about: String,
    /// How it is laid out.
    variant: SettingRowVariant,
    /// Its size.
    size: Size,
    /// What the reset button does, shown only while the setting differs from how it comes.
    on_reset: Option<Rc<dyn Fn()>>,
    /// The control, beside or under the words.
    control: Option<Box<dyn FnOnce()>>,
    /// What shows under the row, such as an open list or the lines of a list.
    below: Option<Box<dyn FnOnce()>>,
}

impl SettingRow {
    /// A row for the setting called `title` that does `about`.
    pub fn new(title: impl Into<String>, about: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            about: about.into(),
            variant: SettingRowVariant::Wide,
            size: Size::Sm,
            on_reset: None,
            control: None,
            below: None,
        }
    }

    /// The row in `variant`.
    pub fn variant(mut self, variant: SettingRowVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The row at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The row with a reset button calling `on_reset`, for a setting that differs from how it comes.
    pub fn on_reset(mut self, on_reset: impl Fn() + 'static) -> Self {
        self.on_reset = Some(Rc::new(on_reset));
        self
    }

    /// The row with `control` beside or under its words.
    pub fn control(mut self, control: impl FnOnce() + 'static) -> Self {
        self.control = Some(Box::new(control));
        self
    }

    /// The row with `below` under it.
    pub fn below(mut self, below: impl FnOnce() + 'static) -> Self {
        self.below = Some(Box::new(below));
        self
    }

    /// Emits the row.
    #[track_caller]
    pub fn show(self) {
        let padding = match self.size {
            Size::Xs => 6.0,
            Size::Sm => 10.0,
            Size::Md => 12.0,
            Size::Lg => 16.0,
        };
        let padding = crate::spacing(padding);
        let title = self.title;
        let about = self.about;
        let on_reset = self.on_reset;
        let control = self.control;
        let below = self.below;
        let words = move || {
            Label::new(title).show();
            if !about.is_empty() {
                Label::new(about)
                    .variant(LabelVariant::Muted)
                    .size(Size::Xs)
                    .wrap()
                    .show();
            }
        };
        let trailing = move |control: Option<Box<dyn FnOnce()>>| {
            row_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(crate::spacing(6.0)),
                Alignment::Center,
                move || {
                    if let Some(control) = control {
                        control();
                    }
                    if let Some(on_reset) = on_reset {
                        IconButton::new(phosphor::ARROW_COUNTER_CLOCKWISE)
                            .size(Size::Xs)
                            .tooltip("Reset to default")
                            .on_click(move || on_reset())
                            .show();
                    }
                },
            );
        };
        column_arranged(
            Modifier::new().fill_max_width().padding_xy(0.0, padding),
            Arrangement::SpacedBy(crate::spacing(8.0)),
            Alignment::Start,
            move || {
                match self.variant {
                    SettingRowVariant::Wide => row_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(crate::spacing(24.0)),
                        Alignment::Center,
                        move || {
                            column_arranged(
                                Modifier::new().weight(1.0),
                                Arrangement::SpacedBy(crate::spacing(2.0)),
                                Alignment::Start,
                                words,
                            );
                            trailing(control);
                        },
                    ),
                    SettingRowVariant::Stacked => {
                        column_arranged(
                            Modifier::new().fill_max_width(),
                            Arrangement::SpacedBy(crate::spacing(2.0)),
                            Alignment::Start,
                            words,
                        );
                        trailing(control);
                    }
                }
                if let Some(below) = below {
                    column_with(Modifier::new().fill_max_width(), below);
                }
            },
        );
    }
}
