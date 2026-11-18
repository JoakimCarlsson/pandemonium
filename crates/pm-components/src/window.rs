//! The frame of a window beside the main one: the title bar that closes it over what it holds, with the border the window draws itself when it has no frame from the desktop.

use rustle::{Alignment, Arrangement, column_arranged};
use std::cell::Cell;

use rustle::Modifier;

use crate::TitleBar;
use crate::kit::{Radius, Role, Styled, color};
use crate::title_bar::FRAMELESS;

/// How wide the transparent band around a floating window is, in logical pixels, which is where its shadow is cast and where the window is grabbed to resize it.
pub const SHADOW_MARGIN: f32 = 32.0;

/// The radius of the corners of a floating window, in logical pixels.
pub const FRAME_RADIUS: f32 = 10.0;

/// The layers of the shadow a floating window casts: a wide soft one that falls away below it and a tight one that keeps its edge defined.
///
/// Each layer is `(x, y, blur, spread, alpha)`: its offset, how far it blurs, how far it grows past the window and how dark it is.
pub const WINDOW_SHADOW: &[(f32, f32, f32, f32, f32)] = &[
    (0.0, 8.0, 22.0, 0.0, 0.5),
    (0.0, 2.0, 8.0, 0.0, 0.32),
    (0.0, 0.0, 2.0, 0.0, 0.28),
];

thread_local! {
    /// Whether the windows of this thread are floating, which is when they are neither maximized nor full screen and so have room around them for a shadow.
    static FLOATING: Cell<bool> = const { Cell::new(false) };

    /// Whether the desktop shows the shadow of a floating window of this thread outside the window, so that the window leaves no band for it.
    static SHADOW_OUTSIDE: Cell<bool> = const { Cell::new(false) };
}

/// Says whether the windows this thread builds from now on float, with room around them for their shadow.
pub fn set_floating(floating: bool) {
    FLOATING.with(|held| held.set(floating));
}

/// Says whether the shadow of a floating window is shown outside it, by the desktop, rather than in a band inside the window.
pub fn set_shadow_outside(outside: bool) {
    SHADOW_OUTSIDE.with(|held| held.set(outside));
}

/// Whether the shadow of a floating window is shown outside it, as [`set_shadow_outside`] last said.
pub fn shadow_outside() -> bool {
    SHADOW_OUTSIDE.with(Cell::get)
}

/// Whether the windows draw their own frame and float, and so have rounded corners, a border and a shadow.
pub fn framed() -> bool {
    FRAMELESS && FLOATING.with(Cell::get)
}

/// How wide the band around a window is that holds its shadow, in logical pixels: none when the desktop frames the window, it is maximized, or its shadow is shown outside it.
pub fn shadow_margin() -> f32 {
    if framed() && !shadow_outside() {
        SHADOW_MARGIN
    } else {
        0.0
    }
}

/// Emits `frame`, which fills the window, with rounded corners that hold everything inside it, a border and, when there is a band of [`shadow_margin`] around the window, the shadow it casts into it; emits `frame` alone when the window is not [`framed`].
#[track_caller]
pub fn shadowed(frame: impl FnOnce()) {
    if !framed() {
        frame();
        return;
    }
    let own = Modifier::new().fill_max_size();
    let own = if shadow_margin() > 0.0 {
        cast_shadow(own)
    } else {
        own
    };
    let own = own
        .clip_rounded(FRAME_RADIUS)
        .border(1.0, color(Role::Border), FRAME_RADIUS);
    column_arranged(own, Arrangement::Start, Alignment::Start, frame);
}

/// `modifier` with [`WINDOW_SHADOW`] drawn behind its box, its corners rounded by [`FRAME_RADIUS`].
fn cast_shadow(modifier: Modifier) -> Modifier {
    modifier.draw_behind(|canvas| {
        let (width, height) = (canvas.width(), canvas.height());
        for (x, y, blur, spread, alpha) in WINDOW_SHADOW {
            let shade = rustle::Color::rgba(0, 0, 0, (alpha * 255.0).round() as u8);
            canvas.shadow(
                x - spread,
                y - spread,
                width + 2.0 * spread,
                height + 2.0 * spread,
                (FRAME_RADIUS + spread).max(0.0),
                blur * crate::kit::layout::SHADOW_SOFTNESS,
                shade,
            );
        }
    })
}

/// A window's frame being described: what its body holds, and what is laid over the body.
///
/// Build it with [`Window::new`], fill it with [`Window::body`], lay menus
/// over it with [`Window::over`], and emit it with [`Window::build`]. It
/// fills the window.
pub struct Window<'a> {
    /// What fills the window under the title bar, in the order it was given.
    body: Vec<Box<dyn FnOnce() + 'a>>,
    /// What is laid over the window, such as menus.
    over: Vec<Box<dyn FnOnce() + 'a>>,
    /// What the caller puts around the frame's box.
    modifier: Modifier,
}

impl<'a> Window<'a> {
    /// A frame with nothing in it.
    pub fn new() -> Self {
        Self {
            body: Vec::new(),
            over: Vec::new(),
            modifier: Modifier::new(),
        }
    }

    /// The frame with what `child` emits filling the window under the title bar, after what the body already holds.
    pub fn body(mut self, child: impl FnOnce() + 'a) -> Self {
        self.body.push(Box::new(child));
        self
    }

    /// The frame with what `content` emits laid over the window.
    pub fn over(mut self, content: impl FnOnce() + 'a) -> Self {
        self.over.push(Box::new(content));
        self
    }

    /// The frame with `modifier` put around its box, outside its own.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the frame, with the shadow it casts while the window floats.
    #[track_caller]
    pub fn build(self) {
        shadowed(move || self.frame());
    }

    /// Emits the frame itself: the title bar, the body under it and what is laid over both.
    #[track_caller]
    fn frame(self) {
        let own = Modifier::new().fill_max_size().fill(Role::Surface);
        let own = if FRAMELESS {
            own.stroke(Role::Border, Radius::None)
        } else {
            own
        };
        let Self {
            body,
            over,
            modifier,
        } = self;
        column_arranged(
            modifier.chain(own),
            Arrangement::Start,
            Alignment::Start,
            move || {
                TitleBar::closing().build();
                column_arranged(
                    Modifier::new().weight(1.0).fill_max_width(),
                    Arrangement::Start,
                    Alignment::Start,
                    move || {
                        for child in body {
                            child();
                        }
                    },
                );
                for content in over {
                    content();
                }
            },
        );
    }
}

impl Default for Window<'_> {
    /// A frame with nothing in it.
    fn default() -> Self {
        Self::new()
    }
}
