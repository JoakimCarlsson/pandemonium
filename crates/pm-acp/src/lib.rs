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
//! one of them running; a [`Request`] is what it asks the editor to do for
//! it, and everything else here is what a session says.

mod agent;
mod attachment;
mod elicitation;
mod limits;
mod mcp;
mod process;
mod registry;
mod request;
mod session;
mod transport;
mod update;

pub use agent::{AGENTS, Agent, Source, agents, install};
pub use attachment::Attachment;
pub use elicitation::{Alternative, Elicitation, Field, Given, Input, Inquiry, Link, Reply};
pub use limits::{Limits, Window};
pub use mcp::{McpServer, Offered, Reach, install_mcp};
pub use registry::{Listing, search as search_registry};
pub use request::{Answer, Exit, Request, Run};
pub use session::{Notify, Session};
pub use update::{
    About, Ask, Choice, Command, Cost, Event, History, Kind, Knob, Location, Method, Mode, Output,
    Pick, Setting, Status, Step, Stop, ToolCall, Usage, Voice, Way, Weight,
};
