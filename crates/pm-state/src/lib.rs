//! Shared, observable state.
//!
//! A [`State`] holds one value that several parts of the app can read and
//! change. Every change bumps a version and tells the observers, so anything
//! that draws from the value knows when to draw again. Nothing here knows
//! about styling or rendering.

mod state;

pub use state::{State, Subscription};
