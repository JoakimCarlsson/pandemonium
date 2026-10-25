//! An embedded Chromium that paints web pages offscreen.
//!
//! An [`Engine`] starts the browser process; a [`View`] is one page drawn
//! into frames the renderer can show as a picture. The crate knows nothing of
//! windows or GPUs: it hands back [`Frame`]s and takes sizes and input.

mod app;
mod devtools;
mod engine;
mod error;
mod frame;
mod handler;
mod input;
mod view;

pub use devtools::find_tools;
pub use engine::{Engine, Options, run_helper};
pub use error::Error;
pub use frame::{Frame, PaintStats};
pub use input::{Button, Cursor, Key, Modifiers};
pub use view::View;
