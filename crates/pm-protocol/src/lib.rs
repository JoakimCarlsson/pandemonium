//! The messages between a window, or any other client, and the server that owns the agents.

mod address;
mod bytes;
mod credential;
mod message;
mod request;
mod response;

pub use address::{SSH_SCHEME, host_of, is_destination};
pub use credential::{Credential, Target};
pub use message::{ClientMessage, ServerMessage};
pub use request::{Attachment, Login, Request};
pub use response::{Response, SessionInfo};

/// The version of the protocol this build speaks; clients and servers refuse each other when they differ.
pub const VERSION: u32 = 1;

/// The port a server listens on when it is not told another.
pub const DEFAULT_PORT: u16 = 6868;
