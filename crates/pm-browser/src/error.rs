//! What can go wrong starting the browser or opening a page.

use std::fmt;

/// A failure to start the browser engine or to open a page in it.
#[derive(Debug)]
pub enum Error {
    /// The process arguments could not be read.
    Arguments,
    /// The cache directory is not valid text.
    CachePath,
    /// The engine refused to start, or started in a process that is not the browser.
    Engine(&'static str),
    /// The engine did not create the page.
    Page,
}

impl fmt::Display for Error {
    /// Describes the failure in one line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arguments => f.write_str("cannot read the process arguments"),
            Self::CachePath => f.write_str("the browser cache path is not valid text"),
            Self::Engine(what) => write!(f, "browser engine: {what}"),
            Self::Page => f.write_str("the browser engine did not create the page"),
        }
    }
}

impl std::error::Error for Error {}
