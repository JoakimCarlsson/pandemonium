//! The modal: a panel centred over a darkened window, which pressing outside closes.

use rustle::Modifier;
use rustle::{Alignment, Arrangement, row_arranged};

use crate::kit::{FocusId, Interactive, Styled, Tone};
use crate::{Surface, surface};

/// How dark the window behind a modal is made, from 0.0 to 1.0.
const DIM: f32 = 0.55;

/// How far above the rest of the window the modal is drawn.
const LAYER: i32 = 70;

/// A modal being described: how wide its panel is and what the panel holds.
///
/// Build it with [`Modal::new`], fill it with [`Modal::child`] and
/// [`Modal::children`], or pass the content to [`Modal::build_with`], and
/// emit it with [`Modal::build`]. It covers the layer it is emitted in, so it
/// belongs last in the root of a screen.
pub struct Modal<'a> {
    /// What pressing the dimmed window outside the panel does.
    outside: FocusId,
    /// What the panel answers to, which keeps a press on it from counting as outside.
    inside: FocusId,
    /// How wide the panel is, in logical pixels, when fixed.
    width: Option<f32>,
    /// What the panel holds, emitted in order.
    parts: Vec<Box<dyn FnOnce() + 'a>>,
    /// What the caller puts around the dimmed box.
    modifier: Modifier,
}

impl<'a> Modal<'a> {
    /// A modal whose dimmed window answers to `outside` and whose panel answers to `inside`.
    pub fn new(outside: FocusId, inside: FocusId) -> Self {
        Self {
            outside,
            inside,
            width: None,
            parts: Vec::new(),
            modifier: Modifier::new(),
        }
    }

    /// The modal with a panel `pixels` wide.
    pub fn width(mut self, pixels: f32) -> Self {
        self.width = Some(pixels);
        self
    }

    /// The modal with what `child` emits after what its panel holds.
    pub fn child(mut self, child: impl FnOnce() + 'a) -> Self {
        self.parts.push(Box::new(child));
        self
    }

    /// The modal with what `children` emits after what its panel holds.
    pub fn children(self, children: impl FnOnce() + 'a) -> Self {
        self.child(children)
    }

    /// The modal with `modifier` put around its dimmed box, outside its own.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the modal, its panel holding what was given to it and then `content`.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let own = Modifier::new()
            .match_parent_size()
            .z_index_unclipped(LAYER)
            .fill(Tone::Black.alpha(DIM))
            .target(self.outside);
        let mut panel = Modifier::new();
        if let Some(width) = self.width {
            panel = panel.width(width);
        }
        let panel = panel.target(self.inside);
        let parts = self.parts;
        row_arranged(
            self.modifier.chain(own),
            Arrangement::Center,
            Alignment::Center,
            move || {
                Surface::new()
                    .variant(surface::Variant::Dialog)
                    .modifier(panel)
                    .build_with(move || {
                        for part in parts {
                            part();
                        }
                        content();
                    });
            },
        );
    }

    /// Emits the modal.
    #[track_caller]
    pub fn build(self) {
        self.build_with(|| {});
    }
}
