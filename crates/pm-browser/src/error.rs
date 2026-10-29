//! What can go wrong starting the browser or opening a page.

use std::fmt;

/// A failure to start the browser helper or to open a page in it.
#[derive(Debug)]
pub enum Error {
    /// The browser is not installed on this machine.
    NotInstalled,
    /// The helper could not be started or reached.
    Helper(String),
    /// The helper did not create the page.
    Page,
}

impl fmt::Display for Error {
    /// Describes the failure in one line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => f.write_str("the embedded browser is not installed"),
            Self::Helper(what) => write!(f, "browser helper: {what}"),
            Self::Page => f.write_str("the browser helper did not create the page"),
        }
    }
}

impl std::error::Error for Error {}
