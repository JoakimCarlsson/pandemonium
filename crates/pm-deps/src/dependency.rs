//! What a manifest says of one dependency, and what is known of its newest version.

use std::ops::Range;

/// One dependency a manifest names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependency {
    /// What the registry knows it by.
    pub name: String,
    /// The version or range the manifest asks for, as written.
    pub requirement: String,
    /// The bytes of the manifest the requirement stands in.
    pub span: Range<usize>,
}

/// A dependency whose version is behind the latest the registry has.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outdated {
    /// The dependency.
    pub dependency: Dependency,
    /// The latest version the registry has.
    pub latest: String,
    /// What the requirement is written as to ask for that version, keeping the kind of range it was.
    pub replacement: String,
}
