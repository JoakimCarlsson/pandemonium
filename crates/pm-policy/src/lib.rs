//! Which tool calls are allowed, asked about or denied, plus budgets and the capabilities plugins may hold.
//!
//! A [`Capability`] is something an extension may do beyond describing itself.
//! Every one is denied until the reader grants it: an extension's manifest
//! declares those it asks for, and a [`Grants`] holds those the reader gave.

mod capability;

pub use capability::{Capability, Denied, Grants};
