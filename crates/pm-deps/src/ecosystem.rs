//! What one kind of manifest and its registry have to say about dependencies.

use crate::dependency::Dependency;

/// A kind of manifest and the registry its dependencies are published in.
pub trait Ecosystem: Sync {
    /// What it is called, which keeps what is learnt of it apart from what is learnt of the others.
    fn name(&self) -> &'static str;

    /// Whether the file at `path` is a manifest of this ecosystem, by its name.
    fn reads(&self, path: &str) -> bool;

    /// The dependencies `text`, a manifest, names with a version of a registry, in the order they are written.
    fn dependencies(&self, text: &str) -> Vec<Dependency>;

    /// The address that answers with what the registry knows of the dependency `name`.
    fn address(&self, name: &str) -> String;

    /// The latest version in `answer`, what the registry said at the address of a dependency.
    fn latest_in(&self, answer: &str) -> Option<String>;
}

/// The ecosystem that reads the manifest at `path`, when there is one.
pub fn ecosystem_for(path: &str) -> Option<&'static dyn Ecosystem> {
    static ECOSYSTEMS: [&dyn Ecosystem; 3] =
        [&crate::npm::Npm, &crate::cargo::Cargo, &crate::go::Go];
    ECOSYSTEMS
        .iter()
        .copied()
        .find(|ecosystem| ecosystem.reads(path))
}

/// The name of the file at `path`, without the folders before it.
pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The text between the quotes of the first quoted string in `text` that starts at or after `from`, with the bytes it stands in.
pub(crate) fn quoted(text: &str, from: usize) -> Option<(&str, std::ops::Range<usize>)> {
    let open = text[from..].find('"')? + from + 1;
    let close = text[open..].find('"')? + open;
    Some((&text[open..close], open..close))
}
