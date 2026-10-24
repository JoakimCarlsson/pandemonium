//! What the reader set up for the window itself, kept by the window between runs: how it opens and looks, how its shells and git marks are drawn, what it does when an agent finishes and the servers it knows.

mod agents;
mod appearance;
mod general;
mod git;
mod servers;
mod settings;
mod terminal;

pub use agents::{AgentSettings, Finish, Notification};
pub use appearance::{AppearanceSettings, Density, ThemeMode};
pub use general::{Channel, GeneralSettings, Launch};
pub use git::{GitMarks, GitSettings};
pub use servers::{KnownServer, Reach, ServerList};
pub use settings::WindowSettings;
pub use terminal::{TerminalCursor, TerminalSettings};
