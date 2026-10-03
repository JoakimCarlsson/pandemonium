//! Named project groups, stored by root so membership survives a restart.

use std::path::{Path, PathBuf};

use pm_core::{ProjectId, Projects};
use serde::{Deserialize, Serialize};

/// A named, collapsible collection of project roots.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProjectGroup {
    /// The label shown above the group's projects.
    pub name: String,
    /// Roots belonging to this group, including temporarily closed projects.
    pub projects: Vec<PathBuf>,
    /// Whether only the heading is visible.
    pub collapsed: bool,
}

/// One row in the Projects reading, in display order.
#[derive(Clone, Copy)]
pub enum ProjectRow {
    /// A named group heading, or the heading for ungrouped projects.
    Heading(Option<usize>),
    /// A project and the group it belongs to.
    Project(ProjectId, Option<usize>),
}

/// The index of the first group holding `root`.
pub fn membership(groups: &[ProjectGroup], root: &Path) -> Option<usize> {
    groups
        .iter()
        .position(|group| group.projects.iter().any(|path| path == root))
}

/// Moves a root into one group, or out of all groups.
pub fn assign(groups: &mut [ProjectGroup], root: &Path, target: Option<usize>) {
    for (index, group) in groups.iter_mut().enumerate() {
        group.projects.retain(|path| path != root);
        if target == Some(index) {
            group.projects.push(root.to_path_buf());
        }
    }
}

/// Headings and visible projects in their shared drawing and dragging order.
pub fn rows(open: &Projects, groups: &[ProjectGroup]) -> Vec<ProjectRow> {
    let mut rows = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        rows.push(ProjectRow::Heading(Some(index)));
        if !group.collapsed {
            rows.extend(
                open.iter()
                    .filter(|project| membership(groups, project.root()) == Some(index))
                    .map(|project| ProjectRow::Project(project.id(), Some(index))),
            );
        }
    }
    let ungrouped = open
        .iter()
        .filter(|project| membership(groups, project.root()).is_none())
        .map(|project| ProjectRow::Project(project.id(), None))
        .collect::<Vec<_>>();
    if !groups.is_empty() && !ungrouped.is_empty() {
        rows.push(ProjectRow::Heading(None));
    }
    rows.extend(ungrouped);
    rows
}
