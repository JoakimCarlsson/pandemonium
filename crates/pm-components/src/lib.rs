//! The components the app is built from.
//!
//! A component is a plain value: its content plus a [`pm_style::Style`] made of
//! utilities, so the caller changes how it looks with `.styles([...])` and
//! never reaches into it. Each component lives in its own module and is
//! re-exported here.

mod label;

pub use label::{Label, label};
