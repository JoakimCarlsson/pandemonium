//! Work a server says it is doing: indexing a checkout, loading a workspace.
//!
//! A server that has work to report begins it under a token of its own,
//! reports on it as it goes, and ends it. What is in flight is kept here, in
//! the order it began, for the window to show while it lasts.

use lsp_types::{NumberOrString, ProgressParams, ProgressParamsValue, WorkDoneProgress};

/// One piece of work a server is doing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Progress {
    /// What the work is, as the server names it: "Indexing", "Loading".
    pub title: String,
    /// What it is on at the moment, when the server says.
    pub message: Option<String>,
    /// How far along it is, out of a hundred, when the server says.
    pub percentage: Option<u32>,
}

/// Everything a server has begun and not yet ended.
#[derive(Default)]
pub(super) struct Works {
    /// The work in flight, by the token it was begun under, oldest first.
    running: Vec<(NumberOrString, Progress)>,
}

impl Works {
    /// Takes in one report of progress, saying whether anything changed.
    pub(super) fn report(&mut self, params: ProgressParams) -> bool {
        let ProgressParamsValue::WorkDone(work) = params.value;
        let at = self
            .running
            .iter()
            .position(|(token, _)| *token == params.token);
        match (work, at) {
            (WorkDoneProgress::Begin(begun), at) => {
                let progress = Progress {
                    title: begun.title,
                    message: begun.message,
                    percentage: begun.percentage,
                };
                match at {
                    Some(at) => self.running[at].1 = progress,
                    None => self.running.push((params.token, progress)),
                }
                true
            }
            (WorkDoneProgress::Report(report), Some(at)) => {
                let progress = &mut self.running[at].1;
                if report.message.is_some() {
                    progress.message = report.message;
                }
                if report.percentage.is_some() {
                    progress.percentage = report.percentage;
                }
                true
            }
            (WorkDoneProgress::End(_), Some(at)) => {
                self.running.remove(at);
                true
            }
            (_, None) => false,
        }
    }

    /// The work in flight, oldest first.
    pub(super) fn running(&self) -> Vec<Progress> {
        self.running
            .iter()
            .map(|(_, progress)| progress.clone())
            .collect()
    }

    /// Forgets everything, for a server that has gone.
    pub(super) fn clear(&mut self) {
        self.running.clear();
    }
}
