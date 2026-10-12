//! Why an environment operation did not complete, and what to do about it.

use crate::declaration::Problems;
use std::path::PathBuf;

/// The step that failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    /// The container engine is missing, stopped or unusable.
    Engine,
    /// The declaration is invalid or uses unsupported fields.
    Declaration,
    /// `initializeCommand` failed.
    Initialize,
    /// The image could not be pulled or built.
    Image,
    /// The container could not be created.
    Create,
    /// The container could not be started.
    Start,
    /// A lifecycle command failed.
    Setup,
    /// The endpoint could not be installed or reached.
    Endpoint,
    /// The container is not the one this environment owns.
    Ownership,
    /// Stopping or removing failed.
    Stop,
    /// Another operation holds the environment.
    Busy,
}

/// A failed operation, with the log that explains it.
#[derive(Clone, Debug)]
pub struct Failure {
    /// The step that failed.
    pub stage: Stage,
    /// The engine's or command's own complaint.
    pub message: String,
    /// Every unsupported or invalid declaration field, for a declaration failure.
    pub problems: Problems,
    /// The retained log of the attempt.
    pub log: Option<PathBuf>,
}

impl Failure {
    /// A failure of `stage` with `message`.
    pub fn new(stage: Stage, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
            problems: Problems::default(),
            log: None,
        }
    }

    /// A failure of the declaration itself.
    pub fn declaration(problems: Problems) -> Self {
        Self {
            stage: Stage::Declaration,
            message: "The declaration cannot be used as written".to_owned(),
            problems,
            log: None,
        }
    }

    /// Points at the log kept for the attempt.
    pub fn with_log(mut self, log: PathBuf) -> Self {
        self.log = Some(log);
        self
    }

    /// What the author can do next.
    pub fn advice(&self) -> &'static str {
        match self.stage {
            Stage::Engine => {
                "Install and start Docker, or choose another engine in the preferences, then try again."
            }
            Stage::Declaration => {
                "Edit devcontainer.json to remove or correct these fields, then open the environment again."
            }
            Stage::Initialize => {
                "Fix initializeCommand in devcontainer.json, then open the environment again."
            }
            Stage::Image => "Fix the image reference or Dockerfile, then rebuild the environment.",
            Stage::Create => {
                "Fix the mounts, runArgs or user in devcontainer.json, then rebuild the environment."
            }
            Stage::Start => "Inspect the log, then reopen or rebuild the environment.",
            Stage::Setup => {
                "Fix the failing lifecycle command and open the environment again to retry; the container is kept for inspection."
            }
            Stage::Endpoint => {
                "Make sure the container has a POSIX shell and a supported Linux system, then reopen it."
            }
            Stage::Ownership => {
                "Remove or rename the other container; Pandemonium never touches containers it does not own."
            }
            Stage::Stop => {
                "Inspect the log; the container can be stopped with the engine's own tools."
            }
            Stage::Busy => "Wait for the running operation to finish.",
        }
    }
}

impl std::fmt::Display for Failure {
    /// The complaint, any declaration problems, the advice and the log path.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if !self.problems.is_empty() {
            write!(f, "\n{}", self.problems)?;
        }
        write!(f, "\n{}", self.advice())?;
        if let Some(log) = &self.log {
            write!(f, " Log: {}", log.display())?;
        }
        Ok(())
    }
}

impl std::error::Error for Failure {}
