//! A folder in a tree of changed files: a caret that folds it, a box that
//! stages or unstages everything under it, ticked, empty or partly filled,
//! its icon and name, and how many lines the files under it add and remove.

use std::rc::Rc;

use rustle::prelude::*;

use crate::checkbox::{CheckState, Checkbox};
use crate::file_icon::FileIcon;
use crate::label::Label;
use crate::size::Size;
use crate::tally::{Tally, TallyVariant};
use rustle::color_scheme;

/// How far each level of the tree is indented.
const INDENT: f32 = 12.0;

/// How a changed folder row reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChangeFolderRowVariant {
    /// With a box that stages everything under it.
    #[default]
    Checkable,
    /// Without a box, leaving room where it would be, as for conflicted files.
    Plain,
}

/// A changed folder row with a variant and a size.
#[derive(Clone)]
pub struct ChangeFolderRow {
    /// What the row says: the folder's name, or several joined by slashes.
    name: String,
    /// How many folders hold it.
    depth: usize,
    /// Whether it is unfolded.
    open: bool,
    /// What its box shows.
    state: CheckState,
    /// Lines added and removed under it.
    tally: (usize, usize),
    /// How it reads.
    variant: ChangeFolderRowVariant,
    /// Its height.
    size: Size,
    /// What clicking the row does.
    on_fold: Option<Rc<dyn Fn()>>,
    /// What ticking its box does.
    on_toggle: Option<Rc<dyn Fn()>>,
}

impl ChangeFolderRow {
    /// An unfolded row for the folder `name`, `depth` folders down.
    pub fn new(name: impl Into<String>, depth: usize) -> Self {
        Self {
            name: name.into(),
            depth,
            open: true,
            state: CheckState::Off,
            tally: (0, 0),
            variant: ChangeFolderRowVariant::Checkable,
            size: Size::Sm,
            on_fold: None,
            on_toggle: None,
        }
    }

    /// The row shown unfolded when `open`, folded when not.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// The row with its box showing `state`.
    pub fn state(mut self, state: CheckState) -> Self {
        self.state = state;
        self
    }

    /// The row saying the files under it add `added` lines and remove `removed`.
    pub fn tally(mut self, added: usize, removed: usize) -> Self {
        self.tally = (added, removed);
        self
    }

    /// The row in `variant`.
    pub fn variant(mut self, variant: ChangeFolderRowVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The row at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The row calling `on_fold` when clicked.
    pub fn on_fold(mut self, on_fold: impl Fn() + 'static) -> Self {
        self.on_fold = Some(Rc::new(on_fold));
        self
    }

    /// The row calling `on_toggle` when its box is clicked.
    pub fn on_toggle(mut self, on_toggle: impl Fn() + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }

    /// How far a row `depth` folders down is indented.
    pub fn indent(depth: usize) -> f32 {
        depth as f32 * INDENT
    }

    /// Emits the row.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let hovered = remember(|| false);
        let height = match self.size {
            Size::Xs => 20.0,
            Size::Sm => 24.0,
            Size::Md => 28.0,
            Size::Lg => 32.0,
        };
        let fill = if hovered.get() {
            scheme.surface_container_high
        } else {
            Color::TRANSPARENT
        };
        let mut modifier = Modifier::new()
            .fill_max_width()
            .height(crate::row_height(height))
            .background_rounded(fill, 4.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if let Some(on_fold) = self.on_fold.clone() {
            modifier = modifier
                .cursor(CursorIcon::Pointer)
                .clickable(move || on_fold());
        }
        row_arranged(
            modifier.padding_xy(4.0, 0.0),
            Arrangement::SpacedBy(crate::spacing(6.0)),
            Alignment::Center,
            move || {
                if self.depth > 0 {
                    spacer(Modifier::new().width(Self::indent(self.depth)));
                }
                match self.variant {
                    ChangeFolderRowVariant::Checkable => {
                        let toggle = self.on_toggle.clone();
                        Checkbox::new(self.state)
                            .size(Size::Xs)
                            .on_toggle(move || {
                                if let Some(toggle) = &toggle {
                                    toggle();
                                }
                            })
                            .show();
                    }
                    ChangeFolderRowVariant::Plain => {
                        spacer(Modifier::new().width(crate::row_height(14.0)));
                    }
                }
                icon(
                    if self.open {
                        phosphor::CARET_DOWN
                    } else {
                        phosphor::CARET_RIGHT
                    },
                    10.0,
                    scheme.on_surface_variant,
                );
                FileIcon::new(self.name.clone())
                    .folder(self.open)
                    .size(Size::Xs)
                    .show();
                row_arranged(
                    Modifier::new().weight(1.0).clip(),
                    Arrangement::Start,
                    Alignment::Center,
                    || {
                        Label::new(self.name.clone())
                            .size(Size::Xs)
                            .colour(scheme.on_surface)
                            .show();
                    },
                );
                let (added, removed) = self.tally;
                Tally::new(added, removed)
                    .variant(TallyVariant::Compact)
                    .size(Size::Xs)
                    .show();
            },
        );
    }
}
