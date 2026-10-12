//! What an environment reports while it changes: progress, logs and forwards.

use std::sync::Arc;

/// The step an environment is working on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// Looking for the container engine and the declaration.
    Checking,
    /// Running the declaration's `initializeCommand` on the desktop.
    Initializing,
    /// Pulling the declared image.
    Pulling,
    /// Building the declared Dockerfile.
    Building,
    /// Creating the container and its volumes.
    Creating,
    /// Starting the container.
    Starting,
    /// Running the lifecycle commands.
    SettingUp,
    /// Installing and connecting the endpoint.
    Connecting,
    /// Opening desktop listeners for declared ports.
    Forwarding,
    /// Running the attach commands.
    Attaching,
    /// Releasing processes and forwards, then stopping the container.
    Stopping,
    /// Removing what the environment owns before it is created again.
    Removing,
}

impl Phase {
    /// A sentence fragment for a progress line.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Checking => "Checking the container engine and declaration",
            Self::Initializing => "Running initializeCommand",
            Self::Pulling => "Pulling the image",
            Self::Building => "Building the image",
            Self::Creating => "Creating the container",
            Self::Starting => "Starting the container",
            Self::SettingUp => "Running lifecycle commands",
            Self::Connecting => "Connecting the endpoint",
            Self::Forwarding => "Forwarding ports",
            Self::Attaching => "Running attach commands",
            Self::Stopping => "Stopping the environment",
            Self::Removing => "Removing the previous container",
        }
    }
}

/// Where an environment is in its life.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum State {
    /// Nothing exists yet; opening creates it.
    #[default]
    Absent,
    /// The container exists and is not running.
    Stopped,
    /// A lifecycle step is in progress.
    Working(Phase),
    /// Setup finished and tools may run.
    Ready,
    /// The last attempt failed, with the reason.
    Failed(String),
}

/// A desktop listener for one container port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Forward {
    /// The port in the container, on its loopback.
    pub container: u16,
    /// The desktop port the declaration asked for.
    pub requested: u16,
    /// The desktop loopback port actually listening, or the reason there is none.
    pub local: Result<u16, String>,
}

impl Forward {
    /// Whether the listener is on another port than requested.
    pub fn remapped(&self) -> bool {
        self.local
            .as_ref()
            .is_ok_and(|local| *local != self.requested)
    }

    /// One line saying where the port can be reached.
    pub fn describe(&self) -> String {
        match &self.local {
            Ok(local) if *local == self.requested => {
                format!("container port {} is at 127.0.0.1:{local}", self.container)
            }
            Ok(local) => format!(
                "desktop port {} is busy, so container port {} is at 127.0.0.1:{local}",
                self.requested, self.container
            ),
            Err(error) => format!(
                "container port {} is not forwarded: {error}",
                self.container
            ),
        }
    }
}

/// One report from a running operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    /// The environment changed state.
    State(State),
    /// One line of output from the engine or a command, named by its source.
    Log {
        /// What produced the line, such as `build` or `postCreateCommand`.
        source: String,
        /// The line without its newline.
        line: String,
    },
    /// Something the author should know that is not a failure.
    Notice(String),
    /// The desktop listeners changed.
    Forwards(Vec<Forward>),
}

/// Receives events from whichever thread produced them.
pub type Sink = Arc<dyn Fn(Event) + Send + Sync>;
