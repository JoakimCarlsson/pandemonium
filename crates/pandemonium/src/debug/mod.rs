//! Debugging: breakpoints in the gutter, and the program they stop, in a pane.
//!
//! A program is debugged in a worktree, over the Debug Adapter Protocol that
//! [`pm_dap`] speaks. [`Debuggers`] is the model — the breakpoints each
//! worktree keeps and the program each is debugging, the one seam either is
//! changed through — and [`debug_pane`] is the screen it is read in, a pane
//! like any other, splittable and tabbable beside the files it stops in.

mod pane;
mod store;

pub use pane::debug_pane;
pub use store::{Debugger, Debuggers};
