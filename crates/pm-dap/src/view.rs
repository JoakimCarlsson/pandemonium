//! A debugging session as it looks at one moment, in a form a client is sent.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::state::{Frame, Line, Placed, Scope, Standing, Thread, Variable, Watched};

/// How many of the console's last lines a view holds.
pub const SHOWN_LINES: usize = 500;

/// Everything a window draws of a debugging session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// What the program is being debugged as.
    pub label: String,
    /// Where the session has got to.
    pub standing: Standing,
    /// Why the program last paused, while it is paused.
    pub reason: Option<String>,
    /// The program's threads.
    pub threads: Vec<Thread>,
    /// The thread the session is looking at.
    pub thread: Option<i64>,
    /// That thread's stack, innermost frame first.
    pub frames: Vec<Frame>,
    /// The frame the session is looking at.
    pub frame: Option<i64>,
    /// That frame's scopes.
    pub scopes: Vec<Scope>,
    /// The variables behind every reference asked about since the last pause.
    pub variables: Vec<(i64, Vec<Variable>)>,
    /// The watch expressions.
    pub watches: Vec<String>,
    /// The last result of each watch expression.
    pub watched: Vec<Watched>,
    /// The last lines of the console, oldest first.
    pub lines: Vec<Line>,
    /// How many lines the console held in all.
    pub line_count: usize,
    /// Where the adapter put the breakpoints of each file.
    pub placed: Vec<(PathBuf, Vec<Placed>)>,
}

impl View {
    /// The variables behind `reference`, once they have been asked for.
    pub fn variables_of(&self, reference: i64) -> Option<&[Variable]> {
        self.variables
            .iter()
            .find(|(held, _)| *held == reference)
            .map(|(_, variables)| variables.as_slice())
    }

    /// The frame the session is looking at.
    pub fn selected_frame(&self) -> Option<&Frame> {
        let id = self.frame?;
        self.frames.iter().find(|frame| frame.id == id)
    }

    /// Where the adapter put the breakpoints of `path`.
    pub fn placed_in(&self, path: &std::path::Path) -> &[Placed] {
        self.placed
            .iter()
            .find(|(held, _)| held == path)
            .map_or(&[], |(_, placed)| placed.as_slice())
    }
}
