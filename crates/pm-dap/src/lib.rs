//! Debugging, over the Debug Adapter Protocol.
//!
//! A debug adapter is another program: gdb, lldb-dap, debugpy, delve. It is
//! started beside a worktree, told which program to run and where to stop,
//! and it answers with where that program is paused and what its variables
//! hold. This crate is that seam — the process, the protocol on its pipe or
//! its socket, and what it says kept as a [`Session`] a window can draw. It
//! knows nothing of panes, projects or gutters; it debugs one program at a
//! time.
//!
//! [`Adapter`] is which adapters there are and how each is run; [`Scenario`]
//! is what a worktree says to debug, read from the files editors already
//! write it in; [`Session`] is one of them running.

mod adapter;
mod attach;
mod scenario;
mod session;
mod state;
mod wire;

pub use adapter::{ADAPTERS, Adapter, Connect};
pub use attach::{Process, processes};
pub use scenario::{Request, Scenario, scenarios};
pub use session::{Notify, Session};
pub use state::{
    Breakpoint, Category, Event, Frame, Line, Placed, Scope, Standing, Thread, Variable, Watched,
};
