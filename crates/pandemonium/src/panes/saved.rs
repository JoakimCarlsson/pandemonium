//! The pane tree as it is written down between launches.
//!
//! A launch should find the window as the last one left it: the same files
//! open in the same panes, in the same order, each looking at the same line.
//! What is written down is this description rather than the tree itself — a
//! pane's id means nothing to the next launch, and a file is named by where
//! it lives rather than by the number this run gave it.

use std::path::PathBuf;

use pm_ui::Axis;
use serde::{Deserialize, Serialize};

/// The whole division of the window, and which pane had the keyboard.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Saved {
    /// Which pane had the keyboard, counted in the order they are drawn.
    pub focus: usize,
    /// The division itself.
    pub root: SavedNode,
}

/// One node of the written-down tree: a pane, or a division of nodes.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedNode {
    /// A pane and what was open in it.
    Pane {
        /// The files open in it, in the order their tabs were drawn.
        #[serde(default)]
        tabs: Vec<SavedTab>,
    },
    /// A division of the space between further nodes.
    Split {
        /// The axis they were divided along.
        axis: SavedAxis,
        /// Each child's share of the division.
        #[serde(default)]
        shares: Vec<f32>,
        /// The children themselves, in order along that axis.
        #[serde(default)]
        children: Vec<SavedNode>,
    },
}

impl Default for SavedNode {
    /// One pane with nothing open in it.
    fn default() -> Self {
        Self::Pane { tabs: Vec::new() }
    }
}

/// Which kind of thing a written-down tab held.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedKind {
    /// A file of the worktree.
    #[default]
    File,
    /// The worktree's changes, gathered for review.
    Review,
    /// The diff of one file of the worktree.
    Change,
}

/// The axis of a division, named as the file names it.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedAxis {
    /// Panes side by side.
    Row,
    /// Panes above one another.
    Column,
}

impl From<Axis> for SavedAxis {
    /// The written name of the axis the tree divides along.
    fn from(axis: Axis) -> Self {
        match axis {
            Axis::Horizontal => Self::Row,
            Axis::Vertical => Self::Column,
        }
    }
}

impl From<SavedAxis> for Axis {
    /// The axis the written name stands for.
    fn from(axis: SavedAxis) -> Self {
        match axis {
            SavedAxis::Row => Self::Horizontal,
            SavedAxis::Column => Self::Vertical,
        }
    }
}

/// What one tab held, in the shape it is written down in.
///
/// A tab is named by where what it held lives rather than by the id this run
/// gave it, because an id means nothing to the next launch.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SavedTab {
    /// Which kind of thing it held.
    pub kind: SavedKind,
    /// The worktree it was opened from.
    pub project: PathBuf,
    /// The file itself, for a tab that held one.
    pub path: PathBuf,
    /// Whether it was only being previewed.
    pub preview: bool,
    /// Whether it was drawn whichever project the window was showing.
    pub pinned: bool,
    /// Whether it was the tab in front in the project it belongs to.
    pub front: bool,
    /// The first line the pane was showing.
    pub scroll: usize,
    /// The line the cursor was on.
    pub line: usize,
    /// The column the cursor was at.
    pub column: usize,
}
