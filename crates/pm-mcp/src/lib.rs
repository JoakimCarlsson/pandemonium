//! The MCP client: servers, and the tools, resources and prompts they offer.
//!
//! A tool server is started as a program spoken to over its standard streams,
//! or reached at a web address; either way it is asked what tools it offers.

mod http;
mod listing;
mod stdio;

pub use listing::{Server, Tool, tools};
