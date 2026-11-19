//! The app driven headless through the interface and the same handling of
//! each event the windows get: what pressing, typing, scrolling and dragging
//! do to the screen.
//!
//! Every test builds the app on a snapshot's screen with
//! [`Rig`](super::headless::Rig), which hands each event to rustle's embedded
//! interface and then to [`App::after_interface`](super::App), the path the
//! event loop takes for a window's events, and lets the loop come to rest
//! with [`App::come_to_rest`](super::App) after each.
//!
//! What the pointer, the keyboard, the wheel, an input method, the clipboard,
//! menus and gestures do is covered here, file by file. What is left out, and why:
//!
//! - Pages of the web in the browser pane: the engine runs in a helper process
//!   the app downloads, so forwarding presses, keys and the wheel to a page,
//!   and the cursor a page asks for, need it running. The address field and
//!   the browser's menu are covered.
//! - The card about a word of code and the links a language server finds:
//!   asking needs a language server that has the file. The wait before
//!   asking is covered.
//! - The desktop's file dialog for attaching files, opening a folder in the
//!   file manager, and pictures on the system clipboard: they reach outside
//!   the app.
//! - What the servers answer: the work runs offline, so requests are checked
//!   as asked, and answers are not simulated.

mod benchmarks;
mod diff;
mod drags;
mod fields;
mod keyboard;
mod menus;
mod panes;
mod pointer;
mod scrolling;
mod selection;
mod smoke;
mod terminal;
mod windows;
