//! The tasks of a folder: those its manifests imply and those it writes down.

use std::path::Path;

use crate::task::Task;
use crate::{cargo, file, go, make, npm, python};

/// What reading a folder's tasks found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Found {
    /// The tasks, those the project writes first.
    pub tasks: Vec<Task>,
    /// Why the project's own file could not be read, when it could not.
    pub problem: Option<String>,
}

/// The tasks the manifests in `root` imply: Cargo, npm scripts, a Makefile, Go modules and Python projects.
pub fn detect(root: &Path) -> Vec<Task> {
    [
        cargo::detect(root),
        npm::detect(root),
        make::detect(root),
        go::detect(root),
        python::detect(root),
    ]
    .concat()
}

/// Every task of `root`: those written in its own file, then those its manifests imply, leaving out a detected task a written one has the name of.
pub fn tasks(root: &Path) -> Found {
    let (mut tasks, problem) = file::read(root);
    let written = tasks.len();
    for detected in detect(root) {
        if !tasks[..written]
            .iter()
            .any(|own| own.is_named(&detected.name))
        {
            tasks.push(detected);
        }
    }
    Found { tasks, problem }
}
