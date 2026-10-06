//! The running binary's version and source commit, formatted for sharing.

/// The version label this binary was built with: the workspace version, or the
/// channel name a preview install sets.
pub const VERSION: &str = env!("PANDEMONIUM_VERSION");

/// The short source commit, possibly dirty, or empty outside a Git checkout.
pub const COMMIT: &str = env!("PANDEMONIUM_COMMIT");

/// Formats the application name, version and available commit for display or copying.
pub fn description() -> String {
    match COMMIT.is_empty() {
        true => format!("pandemonium {VERSION}"),
        false => format!("pandemonium {VERSION} ({COMMIT})"),
    }
}
