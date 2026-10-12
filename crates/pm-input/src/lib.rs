//! Input: what the pointer and keyboard did, and which element has focus.
//!
//! The types here are the app's own, so nothing above the window layer
//! depends on a windowing library: the window translates its events into
//! these, and the rest of the app reads only them.

mod event;
mod focus;
mod pointer;

pub use event::{Button, Key, KeyEvent, Modifiers, PointerEvent};
pub use focus::{Focus, FocusId};
pub use pointer::PointerState;
