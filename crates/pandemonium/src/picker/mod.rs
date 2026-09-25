//! The one list the window asks a reader to choose from.
//!
//! Commands, files, projects, symbols, problems and the matches of a search
//! are six lists and one behaviour: type to narrow, move to select, press to
//! take. [`Picker`] is that behaviour and [`picker`] is how it reads; what
//! fills a given list, and what taking one row does, is the window's.

mod state;
mod view;

pub use state::{Choice, Kind, Picker, Row};
pub use view::{TOP, WIDTH, height, picker, width};
