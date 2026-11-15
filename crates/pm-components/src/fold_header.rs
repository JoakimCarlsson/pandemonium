//! The header of a section that folds: a caret, a title, a count, and
//! controls at its end, such as the Changes and Graph sections of the Git
//! panel or a group of changed files.

use std::rc::Rc;

use rustle::prelude::*;

use crate::badge::{Badge, BadgeVariant};
use crate::checkbox::{CheckState, Checkbox};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a fold header reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FoldHeaderVariant {
    /// The title in body text, for a panel's sections.
    #[default]
    Section,
    /// The title muted and small, for a group inside a section.
    Group,
}

/// A fold header with a variant and a size.
pub struct FoldHeader {
    /// The title.
    title: String,
    /// The count after the title, when there is one.
    count: Option<usize>,
    /// Whether the section is unfolded.
    open: bool,
    /// The box before the title that acts on the whole group, when there is one.
    check: Option<CheckState>,
    /// How it reads.
    variant: FoldHeaderVariant,
    /// Its height.
    size: Size,
    /// What a click on the title does.
    on_toggle: Option<Rc<dyn Fn()>>,
    /// What ticking its box does.
    on_check: Option<Rc<dyn Fn()>>,
    /// The controls at its end.
    trailing: Option<Box<dyn FnOnce()>>,
}

impl FoldHeader {
    /// A header titled `title`, unfolded when `open`.
    pub fn new(title: impl Into<String>, open: bool) -> Self {
        Self {
            title: title.into(),
            count: None,
            open,
            check: None,
            variant: FoldHeaderVariant::Section,
            size: Size::Sm,
            on_toggle: None,
            on_check: None,
            trailing: None,
        }
    }

    /// The header showing `count` after its title.
    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// The header with a box showing `state` before its title, calling `on_check` when clicked.
    pub fn check(mut self, state: CheckState, on_check: impl Fn() + 'static) -> Self {
        self.check = Some(state);
        self.on_check = Some(Rc::new(on_check));
        self
    }

    /// The header in `variant`.
    pub fn variant(mut self, variant: FoldHeaderVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The header at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The header calling `on_toggle` when its title is clicked.
    pub fn on_toggle(mut self, on_toggle: impl Fn() + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }

    /// The header emitting `trailing` at its end.
    pub fn trailing(mut self, trailing: impl FnOnce() + 'static) -> Self {
        self.trailing = Some(Box::new(trailing));
        self
    }

    /// Emits the header.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let height = match self.size {
            Size::Xs => 22.0,
            Size::Sm => 28.0,
            Size::Md => 32.0,
            Size::Lg => 36.0,
        };
        let toggle = self.on_toggle.clone();
        row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(height)
                .padding_xy(4.0, 0.0),
            Arrangement::SpacedBy(4.0),
            Alignment::Center,
            move || {
                if let (Some(state), Some(on_check)) = (self.check, self.on_check.clone()) {
                    Checkbox::new(state)
                        .size(Size::Xs)
                        .on_toggle(move || on_check())
                        .show();
                    spacer(Modifier::new().width(2.0));
                }
                let mut title = Modifier::new()
                    .weight(1.0)
                    .fill_max_height()
                    .cursor(CursorIcon::Pointer);
                if let Some(toggle) = toggle {
                    title = title.clickable_plain(move || toggle());
                }
                let folds = self.on_toggle.is_some();
                row_arranged(title, Arrangement::SpacedBy(4.0), Alignment::Center, || {
                    if folds {
                        icon(
                            if self.open {
                                phosphor::CARET_DOWN
                            } else {
                                phosphor::CARET_RIGHT
                            },
                            10.0,
                            scheme.on_surface_variant,
                        );
                    }
                    let label = Label::new(self.title.clone()).size(Size::Xs);
                    match self.variant {
                        FoldHeaderVariant::Section => label.show(),
                        FoldHeaderVariant::Group => label.variant(LabelVariant::Muted).show(),
                    }
                    if let Some(count) = self.count {
                        Badge::new(count.to_string())
                            .variant(BadgeVariant::Neutral)
                            .size(Size::Xs)
                            .show();
                    }
                });
                if let Some(trailing) = self.trailing {
                    trailing();
                }
            },
        );
    }
}
