//! The modal: rustle's dialog, a panel centred over a darkened window, which pressing outside closes.

use crate::events::{Event, send};
use crate::target::Target;
use rustle::{DialogProperties, Modifier};

use crate::events::Interactive;
use crate::{Surface, surface};

/// How far above the rest of the window the modal is drawn.
const LAYER: i32 = 70;

/// How the modal behaves as a dialog: a press outside asks to close it, and Escape is the app's own key.
const PROPERTIES: DialogProperties = DialogProperties {
    dismiss_on_back_press: false,
    dismiss_on_click_outside: true,
};

/// A modal being described: how wide its panel is and what the panel holds.
///
/// Build it with [`Modal::new`], fill it with [`Modal::child`] and
/// [`Modal::children`], or pass the content to [`Modal::build_with`], and
/// emit it with [`Modal::build`]. It covers the layer it is emitted in, so it
/// belongs last in the root of a screen.
pub struct Modal<'a> {
    /// What a press on the dimmed window outside the panel hands to the workspace.
    outside: Target,
    /// What the panel answers to.
    inside: Target,
    /// How wide the panel is, in logical pixels, when fixed.
    width: Option<f32>,
    /// What the panel holds, emitted in order.
    parts: Vec<Box<dyn FnOnce() + 'a>>,
    /// What the caller puts around the layer the dialog fills.
    modifier: Modifier,
}

impl<'a> Modal<'a> {
    /// A modal whose dimmed window answers to `outside` and whose panel answers to `inside`.
    pub fn new(outside: Target, inside: Target) -> Self {
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

    /// The modal with `modifier` put around the layer its dialog fills, outside its own.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the modal as rustle's dialog in a layer over the window, its panel holding what
    /// was given to it and then `content`; a press outside the panel hands `outside` to the
    /// workspace.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let layer = self
            .modifier
            .chain(Modifier::new().match_parent_size().z_index_unclipped(LAYER));
        let mut panel = Modifier::new();
        if let Some(width) = self.width {
            panel = panel.width(width);
        }
        let panel = panel.control(self.inside, send(Event::pressing(self.inside)));
        let parts = self.parts;
        let outside = send(Event::pressing(self.outside));
        rustle::stack_with(layer, move || {
            rustle::dialog_with(outside, PROPERTIES, move || {
                Surface::new()
                    .variant(surface::Variant::Dialog)
                    .modifier(panel)
                    .build_with(move || {
                        for part in parts {
                            part();
                        }
                        content();
                    });
            });
        });
    }

    /// Emits the modal.
    #[track_caller]
    pub fn build(self) {
        self.build_with(|| {});
    }
}
