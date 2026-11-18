//! What the host tells the views about the frame being drawn: which control
//! holds the keyboard, the clock, the scrollbar held, and the stretch of
//! selectable text selected.
//!
//! The host owns these and sets them with [`set`] before it composes or
//! paints. They are read when painting where they change often (the caret's
//! blink, the focus ring, the spinner), so a change only needs a repaint
//! (`Embedded::repaint`), and when composing where they decide what is built.
//! Drawing that depends on the clock asks for the next frame with
//! [`wake_at`] or [`animate`]; the host reads [`take_wake`] after painting,
//! and [`take_compose`] to know whether that frame needs composing or only
//! painting.

use std::cell::{Cell, RefCell};

use crate::kit::focus::FocusId;
use crate::kit::select::TextSelection;

/// What the host tells the views about the frame being drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// The control holding keyboard focus.
    pub focused: Option<FocusId>,
    /// Milliseconds since the app started, which animations and the caret's blink run on.
    pub clock_ms: f32,
    /// The scrolling box whose bar the pointer holds.
    pub held_bar: Option<FocusId>,
    /// The stretch of selectable text that is selected.
    pub selection: Option<TextSelection>,
}

thread_local! {
    /// The frame being drawn on this thread.
    static FRAME: RefCell<Frame> = RefCell::new(Frame::default());
    /// The control holding keyboard focus, readable without borrowing the frame.
    static FOCUSED: Cell<Option<FocusId>> = const { Cell::new(None) };
    /// The clock of the frame being drawn, in milliseconds.
    static CLOCK: Cell<f32> = const { Cell::new(0.0) };
    /// The earliest clock time something drawn asked to be drawn again at.
    static WAKE: Cell<Option<f32>> = const { Cell::new(None) };
    /// Whether something asked for the screen to be composed again, not only painted.
    static COMPOSE: Cell<bool> = const { Cell::new(false) };
}

/// Tells the views about the frame about to be composed or painted.
pub fn set(frame: Frame) {
    FOCUSED.with(|focused| focused.set(frame.focused));
    CLOCK.with(|clock| clock.set(frame.clock_ms));
    FRAME.with(|held| *held.borrow_mut() = frame);
}

/// The frame being drawn.
pub fn current() -> Frame {
    FRAME.with(|frame| frame.borrow().clone())
}

/// The control holding keyboard focus.
pub fn focused() -> Option<FocusId> {
    FOCUSED.with(Cell::get)
}

/// Whether `id` holds keyboard focus.
pub fn is_focused(id: FocusId) -> bool {
    focused() == Some(id)
}

/// The clock of the frame being drawn, in milliseconds since the app started.
pub fn clock_ms() -> f32 {
    CLOCK.with(Cell::get)
}

/// Asks for the frame to be drawn again once the clock reaches `at_ms`.
pub fn wake_at(at_ms: f32) {
    WAKE.with(|wake| {
        let sooner = wake.get().map_or(at_ms, |held| held.min(at_ms));
        wake.set(Some(sooner));
    });
}

/// Asks for the next frame as soon as it can be drawn, for something moving.
pub fn animate() {
    wake_at(clock_ms());
}

/// Takes the earliest clock time something drawn asked to be drawn again at.
pub fn take_wake() -> Option<f32> {
    WAKE.with(Cell::take)
}

/// Asks for the screen to be composed again at the next frame, for what
/// changed outside the model, such as a tooltip coming or going or an icon
/// turning; [`take_wake`] says when.
pub fn request_compose() {
    COMPOSE.with(|compose| compose.set(true));
}

/// Takes whether something asked for the screen to be composed again: the
/// host then invalidates the composition instead of only repainting.
pub fn take_compose() -> bool {
    COMPOSE.with(Cell::take)
}
