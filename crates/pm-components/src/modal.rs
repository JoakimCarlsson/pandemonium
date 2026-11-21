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

/// A modal being described: what its parts answer to and how wide its panel is.
///
/// Build it with [`Modal::new`], change it with [`Modal::width`], and emit it
/// with what its panel holds through [`Modal::build_with`]. It covers the layer
/// it is emitted in, so it belongs last in the root of a screen.
pub struct Modal {
    /// What a press on the dimmed window outside the panel hands to the workspace.
    outside: Target,
    /// What the panel answers to.
    inside: Target,
    /// How wide the panel is, in logical pixels, when fixed.
    width: Option<f32>,
}

impl Modal {
    /// A modal whose dimmed window answers to `outside` and whose panel answers to `inside`.
    pub fn new(outside: Target, inside: Target) -> Self {
        Self {
            outside,
            inside,
            width: None,
        }
    }

    /// The modal with a panel `pixels` wide.
    pub fn width(mut self, pixels: f32) -> Self {
        self.width = Some(pixels);
        self
    }

    /// Emits the modal as rustle's dialog in a layer over the window, its panel holding what
    /// `content` emits; a press outside the panel hands `outside` to the workspace.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let layer = Modifier::new().match_parent_size().z_index_unclipped(LAYER);
        let mut panel = Modifier::new();
        if let Some(width) = self.width {
            panel = panel.width(width);
        }
        let panel = panel.control(self.inside, send(Event::pressing(self.inside)));
        let outside = send(Event::pressing(self.outside));
        rustle::stack_with(layer, move || {
            rustle::dialog_with(outside, PROPERTIES, move || {
                Surface::new()
                    .variant(surface::Variant::Dialog)
                    .modifier(panel)
                    .build_with(content);
            });
        });
    }
}
