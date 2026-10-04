//! Caller-bound local MCP transport for editor-owned session tools.

mod server;
mod tools;

pub(crate) use server::{Call, Server};
