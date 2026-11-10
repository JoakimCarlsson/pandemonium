//! The frame of a window beside the main one: the title bar that closes it over what it holds, with the border the window draws itself when it has no frame from the desktop.

use std::sync::atomic::{AtomicBool, Ordering};

use pm_style::decoration::ShadowLayer;
use pm_style::{
    Role, bg, border, border_color, flex, flex_1, flex_col, h_full, min_h, rounded, shadow_layers,
    w_full,
};
use pm_ui::{Node, node};

use crate::TitleBar;
use crate::title_bar::FRAMELESS;

/// How wide the transparent band around a floating window is, in logical pixels, which is where its shadow is cast and where the window is grabbed to resize it.
pub const SHADOW_MARGIN: f32 = 32.0;

/// The radius of the corners of a floating window, in logical pixels.
pub const FRAME_RADIUS: f32 = 10.0;

/// The layers of the shadow a floating window casts: a wide soft one that falls away below it and a tight one that keeps its edge defined.
pub const WINDOW_SHADOW: &[ShadowLayer] = &[
    ShadowLayer::new(0.0, 8.0, 22.0, 0.0, 0.5, false),
    ShadowLayer::new(0.0, 2.0, 8.0, 0.0, 0.32, false),
    ShadowLayer::new(0.0, 0.0, 2.0, 0.0, 0.28, false),
];

/// Whether the windows are floating, which is when they are neither maximized nor full screen and so have room around them for a shadow.
static FLOATING: AtomicBool = AtomicBool::new(false);

/// Whether the desktop shows the shadow of a floating window outside the window, so that the window leaves no band for it.
static SHADOW_OUTSIDE: AtomicBool = AtomicBool::new(false);

/// Says whether the windows built from now on float, with room around them for their shadow.
pub fn set_floating(floating: bool) {
    FLOATING.store(floating, Ordering::Relaxed);
}

/// Says whether the shadow of a floating window is shown outside it, by the desktop, rather than in a band inside the window.
pub fn set_shadow_outside(outside: bool) {
    SHADOW_OUTSIDE.store(outside, Ordering::Relaxed);
}

/// Whether the shadow of a floating window is shown outside it, as [`set_shadow_outside`] last said.
pub fn shadow_outside() -> bool {
    SHADOW_OUTSIDE.load(Ordering::Relaxed)
}

/// Whether the windows draw their own frame and float, and so have rounded corners, a border and a shadow.
pub fn framed() -> bool {
    FRAMELESS && FLOATING.load(Ordering::Relaxed)
}

/// How wide the band around a window is that holds its shadow, in logical pixels: none when the desktop frames the window, it is maximized, or its shadow is shown outside it.
pub fn shadow_margin() -> f32 {
    if framed() && !shadow_outside() {
        SHADOW_MARGIN
    } else {
        0.0
    }
}

/// `frame`, which fills the window, with rounded corners that hold everything inside it, a border and, when there is a band of [`shadow_margin`] around the window, the shadow it casts into it; `frame` itself when the window is not [`framed`].
pub fn shadowed(frame: Node) -> Node {
    if !framed() {
        return frame;
    }
    let frame = frame.styles([border(), border_color(Role::Border), rounded(FRAME_RADIUS)]);
    let frame = if shadow_margin() > 0.0 {
        frame.styles([shadow_layers(WINDOW_SHADOW)])
    } else {
        frame
    };
    frame.masked()
}

/// A window's frame being described: what its body holds, and what is laid over the body.
///
/// Build it with [`Window::new`], fill it with [`Window::body`], lay menus
/// over it with [`Window::over`], and turn it into a node with
/// [`Window::build`]. The node fills the window.
#[derive(Clone, Debug)]
pub struct Window {
    /// What fills the window under the title bar.
    body: Node,
    /// What is laid over the window, such as menus.
    over: Vec<Node>,
}

impl Window {
    /// A frame with nothing in it.
    pub fn new() -> Self {
        Self {
            body: node().styles([flex(), flex_col(), flex_1(), min_h(0), w_full()]),
            over: Vec::new(),
        }
    }

    /// The frame with `child` filling the window under the title bar.
    pub fn body(mut self, child: Node) -> Self {
        self.body = self.body.child(child);
        self
    }

    /// The frame with every node of `nodes` laid over the window.
    pub fn over(mut self, nodes: impl IntoIterator<Item = Node>) -> Self {
        self.over.extend(nodes);
        self
    }

    /// The node that draws the frame, with the shadow it casts while the window floats.
    pub fn build(self) -> Node {
        shadowed(self.frame())
    }

    /// The node that draws the frame itself.
    fn frame(self) -> Node {
        node()
            .styles([flex(), flex_col(), w_full(), h_full(), bg(Role::Surface)])
            .styles(
                FRAMELESS
                    .then(|| [border(), border_color(Role::Border)])
                    .into_iter()
                    .flatten(),
            )
            .child(TitleBar::closing().build())
            .child(self.body)
            .children(self.over)
    }
}

impl Default for Window {
    /// A frame with nothing in it.
    fn default() -> Self {
        Self::new()
    }
}
