//! Updating the app from a published release: picking the build for this platform, fetching and checking it, and swapping it in for the running install.

mod asset;
mod checksum;
mod download;
mod install;
mod stage;
mod unpack;

pub use asset::{Asset, Chosen, choose, choose_here};
pub use install::{Installed, blocker, forget_previous, install, relaunch};
pub use stage::{Failure, stage};

/// The environment variable a relaunched editor carries so that the server it finds running is kept rather than retired for being another build, which would stop its agents.
pub const KEEP_SERVER: &str = "PANDEMONIUM_KEEP_SERVER";
