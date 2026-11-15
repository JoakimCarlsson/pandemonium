//! A strip between two panels that resizes one of them when dragged.

use rustle::prelude::*;

use crate::size::Size;
use rustle::color_scheme;

/// Which way a sash runs, and so which way it resizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SashVariant {
    /// A vertical strip between panels side by side, resizing their width.
    #[default]
    Vertical,
    /// A horizontal strip between panels one over the other, resizing their height.
    Horizontal,
}

/// A strip between two panels that changes the size of one of them while dragged, within a range.
pub struct Sash {
    /// Which way it runs.
    variant: SashVariant,
    /// How thick it is.
    size: Size,
    /// The panel's size now, in logical pixels.
    value: f32,
    /// The smallest and largest size the panel may take.
    range: (f32, f32),
    /// Whether the panel lies after the sash, so dragging towards it makes it smaller.
    after: bool,
    /// Called with the panel's new size while the sash is dragged.
    on_resize: Option<std::rc::Rc<dyn Fn(f32)>>,
}

impl Sash {
    /// A vertical sash for a panel `value` pixels across, before it.
    pub fn new(value: f32) -> Self {
        Self {
            variant: SashVariant::Vertical,
            size: Size::Sm,
            value,
            range: (0.0, f32::MAX),
            after: false,
            on_resize: None,
        }
    }

    /// The sash in `variant`.
    pub fn variant(mut self, variant: SashVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The sash in `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The panel kept between `smallest` and `largest`.
    pub fn range(mut self, smallest: f32, largest: f32) -> Self {
        self.range = (smallest, largest.max(smallest));
        self
    }

    /// The panel lying after the sash rather than before it.
    pub fn after(mut self) -> Self {
        self.after = true;
        self
    }

    /// Calls `on_resize` with the panel's new size while the sash is dragged.
    pub fn on_resize(mut self, on_resize: impl Fn(f32) + 'static) -> Self {
        self.on_resize = Some(std::rc::Rc::new(on_resize));
        self
    }

    /// How thick the strip is.
    fn thickness(&self) -> f32 {
        match self.size {
            Size::Xs => 2.0,
            Size::Sm => 4.0,
            Size::Md => 6.0,
            Size::Lg => 8.0,
        }
    }

    /// Emits the sash.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let hovered = remember(|| false);
        let grab = remember(|| None::<(f32, f32)>);
        let thickness = self.thickness();
        let vertical = self.variant == SashVariant::Vertical;
        let (value, (smallest, largest), after) = (self.value, self.range, self.after);
        let on_resize = self.on_resize;
        let modifier = if vertical {
            Modifier::new().width(thickness).fill_max_height()
        } else {
            Modifier::new().height(thickness).fill_max_width()
        };
        let line = if hovered.get() || grab.get().is_some() {
            scheme.primary
        } else {
            scheme.outline_variant
        };
        let cursor = if vertical {
            CursorIcon::ColResize
        } else {
            CursorIcon::RowResize
        };
        let ending = grab.clone();
        let hovering = hovered.clone();
        stack_aligned(
            modifier
                .cursor(cursor)
                .on_hover(move |inside| hovering.set(inside))
                .on_drag_end(move || ending.set(None))
                .on_drag(move |at| {
                    let along = if vertical { at.x } else { at.y };
                    let (start, from) = grab.get().unwrap_or((value, along));
                    if grab.get().is_none() {
                        grab.set(Some((start, from)));
                    }
                    let moved = if after { from - along } else { along - from };
                    if let Some(on_resize) = &on_resize {
                        on_resize((start + moved).clamp(smallest, largest));
                    }
                }),
            BoxAlignment::Center,
            move || {
                let hairline = if vertical {
                    Modifier::new().width(1.0).fill_max_height()
                } else {
                    Modifier::new().height(1.0).fill_max_width()
                };
                spacer(hairline.background(line));
            },
        );
    }
}
