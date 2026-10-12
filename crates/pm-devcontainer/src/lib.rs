//! Dev container environments for a project or worktree, owned in Rust.
//!
//! The format is the Development Containers Specification; this crate is our
//! implementation of the subset it documents, using a local Docker engine
//! and the `pm-host` transport, with no reference CLI, Node or SSH daemon.
//!
//! [`environment`] is the provider-independent contract the rest of the
//! editor resolves tools through; [`Docker`] is the first provider.

pub mod declaration;
mod docker;
mod engine;
pub mod environment;
mod event;
mod failure;
mod forward;
mod identity;
mod jsonc;
mod lifecycle;
mod plan;
mod record;
pub mod variables;
mod workspace;

pub use declaration::{Declaration, Problem, Problems};
pub use docker::{Docker, Settings};
pub use engine::Info;
pub use environment::{
    Capabilities, Environment, ExecutionContext, Intent, Provided, Provider, Request,
};
pub use event::{Event, Forward, Phase, Sink, State};
pub use failure::{Failure, Stage};
pub use identity::Identity;
pub use record::Record;
