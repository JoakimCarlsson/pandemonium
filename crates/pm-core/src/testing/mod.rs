//! Framework adapters, normalized test results and revision-bound coverage.

mod coverage;
mod python;
mod result;

pub use coverage::{Coverage, CoveredFile, SourceRevision};
pub use python::{Python, Selection};
pub use result::{Case, Location, Status};
