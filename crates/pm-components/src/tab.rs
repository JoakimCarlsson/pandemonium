//! A tab: an optional icon and a label, filled when chosen.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a tab is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabVariant {
    /// A rounded chip, filled when chosen, as in the tab strip and the dock.
    #[default]
    Chip,
    /// An icon only, as in the right panel's header.
    Icon,
}

/// A tab with a variant and a size.
#[derive(Clone)]
pub struct Tab {
    /// Its text; empty for an icon tab.
    label: String,
    /// Its glyph.
    icon: Option<Icon>,
    /// How it is drawn.
    variant: TabVariant,
    /// Its size.
    size: Size,
    /// Whether it is the chosen tab.
    selected: bool,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
    /// Whether it is a preview, which its label shows leaning.
    preview: bool,
    /// Whether what it shows has changes not saved, which a dot shows.
    dirty: bool,
    /// What its close button does; it has none without.
    on_close: Option<Rc<dyn Fn()>>,
    /// What a secondary click does.
    on_menu: Option<Rc<dyn Fn()>>,
    /// What a double click does.
    on_double_click: Option<Rc<dyn Fn()>>,
}

impl Tab {
    /// A chip tab reading `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            variant: TabVariant::Chip,
            size: Size::Sm,
            selected: false,
            on_click: None,
            preview: false,
            dirty: false,
            on_close: None,
            on_menu: None,
            on_double_click: None,
        }
    }

    /// An icon tab showing `icon`.
    pub fn icon_only(icon: Icon) -> Self {
        Self::new("").icon(icon).variant(TabVariant::Icon)
    }

    /// The tab leading with `icon`.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The tab in `variant`.
    pub fn variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The tab at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The tab highlighted as chosen when `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The tab calling `on_click` when clicked.
    pub fn on_click(mut self, on_click: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// The tab with its label leaning when `preview`, as a preview tab is.
    pub fn preview(mut self, preview: bool) -> Self {
        self.preview = preview;
        self
    }

    /// The tab showing a dot when `dirty`, for changes not saved.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    /// The tab with a close button calling `on_close`.
    pub fn on_close(mut self, on_close: impl Fn() + 'static) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }

    /// The tab calling `on_menu` when clicked with the secondary button.
    pub fn on_menu(mut self, on_menu: impl Fn() + 'static) -> Self {
        self.on_menu = Some(Rc::new(on_menu));
        self
    }

    /// The tab calling `on_double_click` when double-clicked.
    pub fn on_double_click(mut self, on_double_click: impl Fn() + 'static) -> Self {
        self.on_double_click = Some(Rc::new(on_double_click));
        self
    }

    /// Emits the tab.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let hovered = remember(|| false);
        let (height, padding) = match self.size {
            Size::Xs => (22.0, 6.0),
            Size::Sm => (26.0, 8.0),
            Size::Md => (32.0, 10.0),
            Size::Lg => (36.0, 12.0),
        };
        let fill = match (self.variant, self.selected, hovered.get()) {
            (TabVariant::Chip, true, _) => scheme.secondary_container,
            (_, _, true) => scheme.surface_container_high,
            _ => Color::TRANSPARENT,
        };
        let tint = if self.selected || hovered.get() {
            scheme.on_surface
        } else {
            scheme.on_surface_variant
        };
        let width = (self.variant == TabVariant::Icon).then_some(height);
        let mut modifier = Modifier::new()
            .height(height)
            .background_rounded(fill, 6.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            })
            .cursor(CursorIcon::Pointer);
        modifier = match width {
            Some(width) => modifier.width(width),
            None => modifier.padding_xy(padding, 0.0),
        };
        if let Some(on_click) = self.on_click.clone() {
            modifier = modifier.clickable(move || on_click());
        }
        if let Some(on_menu) = self.on_menu.clone() {
            modifier = modifier.on_secondary_click(move |_| on_menu());
        }
        if let Some(on_double_click) = self.on_double_click.clone() {
            modifier = modifier.on_double_click(move || on_double_click());
        }
        let closable = self.on_close.is_some() || self.dirty;
        let arrangement = if width.is_some() {
            Arrangement::Center
        } else {
            Arrangement::SpacedBy(6.0)
        };
        row_arranged(modifier, arrangement, Alignment::Center, move || {
            if let Some(glyph) = &self.icon {
                icon(glyph.clone(), glyph_size(self.size), tint);
            }
            if !self.label.is_empty() {
                let label = Label::new(self.label.clone())
                    .variant(LabelVariant::Body)
                    .colour(tint)
                    .size(Size::Sm);
                let style = TextStyle {
                    italic: self.preview,
                    ..label.style()
                };
                text_styled(Modifier::new(), self.label.clone(), style);
            }
            if closable {
                close_mark(
                    self.on_close.clone(),
                    self.dirty,
                    self.selected || hovered.get(),
                    glyph_size(self.size),
                );
            }
        });
    }
}

/// Emits the end of a closable tab: a dot while `dirty` and not `revealed`,
/// otherwise a cross calling `on_close` that shows while `revealed`, `side` wide.
fn close_mark(on_close: Option<Rc<dyn Fn()>>, dirty: bool, revealed: bool, side: f32) {
    let scheme = color_scheme();
    let hovered = remember(|| false);
    let box_side = side + 4.0;
    let fill = if hovered.get() {
        scheme.surface_container_highest
    } else {
        Color::TRANSPARENT
    };
    let mut modifier = Modifier::new()
        .size(box_side, box_side)
        .background_rounded(fill, 4.0)
        .on_hover({
            let hovered = hovered.clone();
            move |inside| hovered.set(inside)
        });
    if let Some(on_close) = on_close {
        modifier = modifier.clickable(move || on_close());
    }
    stack_aligned(modifier, BoxAlignment::Center, move || {
        if dirty && !hovered.get() {
            spacer(
                Modifier::new()
                    .size(7.0, 7.0)
                    .background_rounded(scheme.on_surface, 3.5),
            );
        } else if revealed || hovered.get() {
            icon(phosphor::X, side - 2.0, scheme.on_surface_variant);
        }
    });
}

/// The side of a tab's icon at `size`.
fn glyph_size(size: Size) -> f32 {
    match size {
        Size::Xs => 12.0,
        Size::Sm => 13.0,
        Size::Md => 15.0,
        Size::Lg => 16.0,
    }
}
