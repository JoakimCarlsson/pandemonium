//! The frame of a window beside the main one: the title bar that closes it over what it holds, with the border the window draws itself when it has no frame from the desktop.

use crate::theme;
use crate::theme::Frame;
use rustle::{Alignment, Arrangement};

use rustle::{Modifier, Shadow};

use crate::theme::ShadowLayers;

use crate::TitleBar;
use crate::theme::{Radius, Role, color};
use crate::title_bar::FRAMELESS;

/// How wide the transparent band around a floating window is, in logical pixels, which is where its shadow is cast and where the window is grabbed to resize it.
pub const SHADOW_MARGIN: f32 = 32.0;

/// The radius of the corners of a floating window, in logical pixels.
pub const FRAME_RADIUS: f32 = 10.0;

/// The layers of the shadow a floating window casts: a wide soft one that falls away below it and a tight one that keeps its edge defined.
///
/// Each layer is a CSS box shadow of its offset, how far it blurs, how far it grows past the window and how dark it is.
pub const WINDOW_SHADOW: &[Shadow] = &[
    crate::theme::box_shadow(0.0, 8.0, 22.0, 0.0, 0.5),
    crate::theme::box_shadow(0.0, 2.0, 8.0, 0.0, 0.32),
    crate::theme::box_shadow(0.0, 0.0, 2.0, 0.0, 0.28),
];

thread_local! {
    /// Whether the windows of this thread are floating, which is when they are neither maximized nor full screen and so have room around them for a shadow, as snapshot state the frames read.
    static FLOATING: rustle::State<bool> = rustle::mutable_state(false);

    /// Whether the desktop shows the shadow of a floating window of this thread outside the window, so that the window leaves no band for it, as snapshot state the frames read.
    static SHADOW_OUTSIDE: rustle::State<bool> = rustle::mutable_state(false);
}

/// Writes `value` to the flag `flag` holds when it differs, so its readers are composed again only for a change.
fn set_flag(flag: &'static std::thread::LocalKey<rustle::State<bool>>, value: bool) {
    flag.with(|held| {
        if held.get() != value {
            held.set(value);
        }
    });
}

/// Says whether the windows of this thread float, with room around them for their shadow; the frames that read it are composed again when it changes.
pub fn set_floating(floating: bool) {
    set_flag(&FLOATING, floating);
}

/// Says whether the shadow of a floating window is shown outside it, by the desktop, rather than in a band inside the window; the frames that read it are composed again when it changes.
pub fn set_shadow_outside(outside: bool) {
    set_flag(&SHADOW_OUTSIDE, outside);
}

/// Whether the shadow of a floating window is shown outside it, as [`set_shadow_outside`] last said.
pub fn shadow_outside() -> bool {
    SHADOW_OUTSIDE.with(rustle::State::get)
}

/// Whether the windows draw their own frame and float, and so have rounded corners, a border and a shadow.
pub fn framed() -> bool {
    FRAMELESS && FLOATING.with(rustle::State::get)
}

/// How wide the band around a window is that holds its shadow, in logical pixels: none when the desktop frames the window, it is maximized, or its shadow is shown outside it.
pub fn shadow_margin() -> f32 {
    if framed() && !shadow_outside() {
        SHADOW_MARGIN
    } else {
        0.0
    }
}

/// How far in from the frame of a window a press resizes it, in logical pixels; so does a
/// press anywhere in the band around the frame that holds its shadow.
pub const RESIZE_EDGE: f32 = 4.0;

/// Emits, over everything in a window that draws its own frame and floats, the strips along
/// its edges and the squares at its corners that resize it: the band that holds its shadow
/// and [`RESIZE_EDGE`] into the frame. Each shows the pointer shape of its edge and hands a
/// press of the primary button to the app, which asks the desktop to resize the window.
/// Emit it last in a stack that fills the window.
#[track_caller]
pub fn resize_edges() {
    use rustle::{BoxAlignment, CursorIcon, PointerButton, PointerPhase, ResizeEdge};
    if !framed() {
        return;
    }
    let reach = shadow_margin() + RESIZE_EDGE;
    let operations = crate::events::operations();
    let across = (
        Modifier::new().fill_max_width().height(reach),
        CursorIcon::NsResize,
    );
    let down = (
        Modifier::new().fill_max_height().width(reach),
        CursorIcon::EwResize,
    );
    let corner = Modifier::new().width(reach).height(reach);
    let falling = (corner.clone(), CursorIcon::NwseResize);
    let rising = (corner, CursorIcon::NeswResize);
    let edges = [
        (ResizeEdge::North, BoxAlignment::TopCenter, across.clone()),
        (ResizeEdge::South, BoxAlignment::BottomCenter, across),
        (ResizeEdge::West, BoxAlignment::CenterStart, down.clone()),
        (ResizeEdge::East, BoxAlignment::CenterEnd, down),
        (
            ResizeEdge::NorthWest,
            BoxAlignment::TopStart,
            falling.clone(),
        ),
        (ResizeEdge::NorthEast, BoxAlignment::TopEnd, rising.clone()),
        (ResizeEdge::SouthWest, BoxAlignment::BottomStart, rising),
        (ResizeEdge::SouthEast, BoxAlignment::BottomEnd, falling),
    ];
    rustle::stack_with(
        Modifier::new()
            .fill_max_size()
            .z_index(rustle::POPUP_Z_INDEX + 1),
        || {
            for (edge, alignment, (size, cursor)) in edges {
                let pressed = std::rc::Rc::clone(&operations);
                rustle::spacer(size.align(alignment).cursor(cursor).on_pointer_button(
                    PointerButton::Primary,
                    move |event| {
                        if event.phase == PointerPhase::Press {
                            pressed.edge_pressed(edge);
                        }
                    },
                ));
            }
        },
    );
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
        own.shadow_layers(WINDOW_SHADOW, FRAME_RADIUS)
    } else {
        own
    };
    let own = own
        .clip_rounded(FRAME_RADIUS)
        .border(1.0, color(Role::Border), FRAME_RADIUS);
    rustle::column_arranged(own, Arrangement::Start, Alignment::Start, frame);
}

/// A window's frame being described: what its body holds, and what is laid over the body.
///
/// Build it with [`Window::new`], fill it with [`Window::body`], lay menus
/// over it with [`Window::over`], and emit it with [`Window::build`]. It
/// fills the window.
#[derive(Default)]
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
        Self::default()
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
        let own = Modifier::new()
            .fill_max_size()
            .background(theme::color(Role::Surface));
        let own = if FRAMELESS {
            own.outlined(Radius::None)
        } else {
            own
        };
        let Self {
            body,
            over,
            modifier,
        } = self;
        rustle::column_arranged(
            modifier.chain(own),
            Arrangement::Start,
            Alignment::Start,
            move || {
                TitleBar::closing().build();
                rustle::column_arranged(
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    /// A window that stops floating composes again what reads the band around it, and only
    /// that, with no one asking for the whole tree to be composed again.
    #[test]
    fn floating_composes_again_only_what_reads_the_band() {
        set_floating(true);
        let banded = Rc::new(Cell::new((0, 0.0)));
        let bystander = Rc::new(Cell::new(0));
        let (band, other) = (Rc::clone(&banded), Rc::clone(&bystander));
        let mut ui = rustle::Embedded::without_renderer(
            move || {
                let band = Rc::clone(&band);
                rustle::skippable((), move || band.set((band.get().0 + 1, shadow_margin())));
                let other = Rc::clone(&other);
                rustle::skippable((), move || other.set(other.get() + 1));
            },
            || {},
        );
        ui.resize((100, 100), 1.0);
        ui.update();
        let (runs, margin) = banded.get();
        assert_eq!(margin, if FRAMELESS { SHADOW_MARGIN } else { 0.0 });
        set_floating(false);
        ui.update();
        let expected = if FRAMELESS { runs + 1 } else { runs };
        assert_eq!(banded.get(), (expected, 0.0));
        assert_eq!(
            bystander.get(),
            1,
            "a block reading no band was composed again"
        );
    }
}
