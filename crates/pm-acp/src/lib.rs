//! The agents the window conducts, over the Agent Client Protocol.
//!
//! An agent is another program: it is started in a worktree, told what the
//! reader wants and left to work, and it reads files, writes them and asks
//! permission through the editor that started it. This crate is that seam —
//! the process, the protocol on its pipes, and what it says turned into
//! [`Event`]s a window can draw. It knows nothing of panes, projects or
//! sessions in the editor's sense; it speaks to one agent at a time.
//!
//! [`Agent`] is which agents there are and how each is run; [`Session`] is
//! one of them running, and everything else here is what a session says.

mod agent;
mod session;
mod transport;
mod update;

pub use agent::{AGENTS, Agent, Source};
pub use session::{Notify, Session};
pub use update::{
    Ask, Choice, Command, Event, Kind, Location, Method, Mode, Output, Status, Step, Stop,
    ToolCall, Voice, Weight,
};
