//! Lossless notebook documents and worktree-owned Jupyter kernel processes.

mod document;
mod kernel;

pub use document::{Cell, CellId, Notebook, multiline};
pub use kernel::Kernel;
