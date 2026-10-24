//! The client side of the server connection: requests out, answers and events in.
//!
//! A [`Client`] keeps its connection for as long as it lives. When the
//! connection drops it dials again, and asks for the events of every session
//! it watches that came after the last one it has.

mod local;
mod session;

pub use local::local;
pub use session::{Client, Incoming};
