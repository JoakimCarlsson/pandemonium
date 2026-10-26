//! An embedded Chromium that paints web pages offscreen, run in a helper program of its own.
//!
//! The engine is large, so it is not linked into the window: `pandemonium-browser`
//! (the Electron app in `helpers/electron-browser`) holds it, and the reader
//! downloads it when they turn the browser on. An [`Engine`] starts that helper; a [`View`] is
//! one page the helper paints into shared files, handed back as [`Frame`]s the
//! renderer can show as a picture. This crate knows nothing of windows, GPUs or
//! Chromium: it takes sizes and input and hands back pictures.

mod engine;
mod error;
mod frame;
mod input;
pub mod install;
mod link;
pub mod surface;
mod view;
pub mod wire;

pub use engine::Engine;
pub use error::Error;
pub use frame::Frame;
pub use input::{Button, Cursor, Key, Modifiers};
pub use view::View;
pub use wire::{EditAction, PickedElement, ToolResult};
