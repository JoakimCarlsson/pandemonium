//! The files a server asked to hear about, and telling it when they change.
//!
//! A server does not watch the disk itself when the editor offers to: it
//! registers the patterns it cares about — every `*.rs`, a `Cargo.toml`, a
//! `tsconfig.json` — and is sent `workspace/didChangeWatchedFiles` for
//! whatever changes that matches one of them. What changed is the editor's
//! to find out; which of it this server wants is decided here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher};
use serde_json::{Value, json};

use crate::lsp::uri;

/// The method a server registers to be told about files changing on disk.
const METHOD: &str = "workspace/didChangeWatchedFiles";

/// What happened to a file a server may be watching.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Watched {
    /// The file was made.
    Created,
    /// What the file holds changed.
    Changed,
    /// The file was taken away.
    Deleted,
}

impl Watched {
    /// The bit a watcher's kind sets to hear about this, in the protocol's words.
    fn bit(self) -> u64 {
        match self {
            Self::Created => 1,
            Self::Changed => 2,
            Self::Deleted => 4,
        }
    }

    /// The change type the protocol sends this as.
    fn code(self) -> u64 {
        match self {
            Self::Created => 1,
            Self::Changed => 2,
            Self::Deleted => 3,
        }
    }
}

/// One pattern a server registered, and which kinds of change it wants.
struct Pattern {
    /// What a path is matched against.
    glob: GlobMatcher,
    /// The directory the pattern is relative to, when it was given one.
    base: Option<PathBuf>,
    /// The kinds of change wanted, as the protocol's bits.
    kinds: u64,
}

impl Pattern {
    /// The pattern `watcher` describes, if it can be read.
    fn read(watcher: &Value) -> Option<Self> {
        let kinds = watcher["kind"].as_u64().unwrap_or(7);
        let pattern = &watcher["globPattern"];
        let (glob, base) = match pattern.as_str() {
            Some(glob) => (glob, None),
            None => (
                pattern["pattern"].as_str()?,
                Some(base_of(&pattern["baseUri"])?),
            ),
        };
        let glob = GlobBuilder::new(glob)
            .literal_separator(true)
            .build()
            .ok()?
            .compile_matcher();
        Some(Self { glob, base, kinds })
    }

    /// Whether this pattern wants to hear that `path` was `touched`.
    fn wants(&self, path: &Path, touched: Watched) -> bool {
        if self.kinds & touched.bit() == 0 {
            return false;
        }
        match &self.base {
            Some(base) => path
                .strip_prefix(base)
                .is_ok_and(|relative| self.glob.is_match(relative)),
            None => self.glob.is_match(path),
        }
    }
}

/// The directory a relative pattern's base names: a URI, or a workspace folder.
fn base_of(base: &Value) -> Option<PathBuf> {
    let written = base.as_str().or_else(|| base["uri"].as_str())?;
    uri::path(written)
}

/// Every pattern a server has registered, by the registration it came in.
#[derive(Default)]
pub(super) struct Watchers {
    /// The patterns, by the id the server registered them under.
    registered: HashMap<String, Vec<Pattern>>,
}

impl Watchers {
    /// Takes in the registrations of `params` that are about watched files.
    pub(super) fn register(&mut self, params: &Value) {
        let registrations = params["registrations"].as_array().into_iter().flatten();
        for registration in registrations {
            if registration["method"] != METHOD {
                continue;
            }
            let Some(id) = registration["id"].as_str() else {
                continue;
            };
            let patterns = registration["registerOptions"]["watchers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Pattern::read)
                .collect();
            self.registered.insert(id.to_owned(), patterns);
        }
    }

    /// Forgets the registrations `params` takes back.
    ///
    /// The protocol spells the list `unregisterations`, and it is read as spelt.
    pub(super) fn unregister(&mut self, params: &Value) {
        let taken = params["unregisterations"].as_array().into_iter().flatten();
        for unregistration in taken {
            if let Some(id) = unregistration["id"].as_str() {
                self.registered.remove(id);
            }
        }
    }

    /// The notification telling the server about the part of `changes` it
    /// registered for, when it registered for any of it.
    pub(super) fn notification(&self, changes: &[(PathBuf, Watched)]) -> Option<Value> {
        let wanted = changes
            .iter()
            .filter(|(path, touched)| {
                self.registered
                    .values()
                    .flatten()
                    .any(|pattern| pattern.wants(path, *touched))
            })
            .map(|(path, touched)| json!({ "uri": uri::of(path), "type": touched.code() }))
            .collect::<Vec<_>>();
        if wanted.is_empty() {
            return None;
        }
        Some(json!({
            "method": METHOD,
            "params": { "changes": wanted },
        }))
    }
}
