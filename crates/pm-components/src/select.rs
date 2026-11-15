//! A box showing the choice picked from a list, which opens the list.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::Label;
use crate::size::Size;
use rustle::color_scheme;

/// How a select box is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectVariant {
    /// The window background with a hairline border.
    #[default]
    Outlined,
    /// No fill until hovered.
    Ghost,
}

/// A select box with a variant and a size.
#[derive(Clone)]
pub struct Select {
    /// The choice picked, as it reads.
    shown: String,
    /// Whether its list is open.
    open: bool,
    /// How it is drawn.
    variant: SelectVariant,
    /// Its size.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
}

impl Select {
    /// A box showing `shown`.
    pub fn new(shown: impl Into<String>) -> Self {
        Self {
            shown: shown.into(),
            open: false,
            variant: SelectVariant::Outlined,
            size: Size::Sm,
            modifier: Modifier::new(),
            on_click: None,
        }
    }

    /// The box marked open when `open`.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// The box in `variant`.
    pub fn variant(mut self, variant: SelectVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The box at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The box laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The box calling `on_click` when clicked.
    pub fn on_click(mut self, on_click: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// Emits the box.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let hovered = remember(|| false);
        let height = match self.size {
            Size::Xs => 22.0,
            Size::Sm => 28.0,
            Size::Md => 32.0,
            Size::Lg => 38.0,
        };
        let height = crate::row_height(height);
        let fill = match (self.variant, hovered.get() || self.open) {
            (_, true) => scheme.surface_container_high,
            (SelectVariant::Outlined, false) => scheme.background,
            (SelectVariant::Ghost, false) => Color::TRANSPARENT,
        };
        let ring = crate::focus_ring::remember_focus_ring();
        let outline = if ring.shown() {
            scheme.primary
        } else if self.open {
            scheme.outline
        } else {
            scheme.outline_variant
        };
        let mut modifier = self
            .modifier
            .height(height)
            .background_rounded(fill, 6.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if self.variant == SelectVariant::Outlined || self.open || ring.shown() {
            modifier = modifier.border(1.0, outline, 6.0);
        }
        if let Some(on_click) = self.on_click {
            let pressed = on_click.clone();
            let clicked = ring.clone();
            modifier = modifier.cursor(CursorIcon::Pointer).clickable(move || {
                clicked.clicked();
                on_click()
            });
            modifier = modifier.focusable(&ring.focus, move |event| {
                crate::focus_ring::activates(event, &*pressed)
                    || crate::focus_ring::opens(event, &*pressed)
            });
        }
        let shown = self.shown;
        row_arranged(
            modifier.padding_xy(10.0, 0.0),
            Arrangement::SpacedBy(crate::spacing(6.0)),
            Alignment::Center,
            move || {
                Label::new(shown)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                icon(phosphor::CARET_UP_DOWN, 12.0, scheme.on_surface_variant);
            },
        );
    }
}
