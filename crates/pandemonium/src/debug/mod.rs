//! Debugging: breakpoints in the gutter, and the program they stop, in the
//! bottom panel.
//!
//! A program is debugged in a worktree, over the Debug Adapter Protocol that
//! [`pm_dap`] speaks. [`Debuggers`] is the model — the breakpoints each
//! worktree keeps and the program each is debugging, the one seam either is
//! changed through — and [`debug_view`] is the screen it is read in, the
//! bottom panel's debug view, under the files it stops in.

mod store;
mod view;

pub use store::{Debugger, Debuggers};
pub use view::debug_view;
