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
use lsp_types::notification::DidChangeWatchedFiles;
use lsp_types::{
    DidChangeWatchedFilesParams, DidChangeWatchedFilesRegistrationOptions, FileChangeType,
    FileEvent, FileSystemWatcher, GlobPattern, OneOf, Registration, WatchKind,
};
use serde_json::Value;

use crate::lsp::{rpc, uri};

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
    /// The kind a watcher sets to hear about this.
    fn kind(self) -> WatchKind {
        match self {
            Self::Created => WatchKind::Create,
            Self::Changed => WatchKind::Change,
            Self::Deleted => WatchKind::Delete,
        }
    }

    /// The change type the protocol sends this as.
    fn change(self) -> FileChangeType {
        match self {
            Self::Created => FileChangeType::CREATED,
            Self::Changed => FileChangeType::CHANGED,
            Self::Deleted => FileChangeType::DELETED,
        }
    }
}

/// One pattern a server registered, and which kinds of change it wants.
struct Pattern {
    /// What a path is matched against.
    glob: GlobMatcher,
    /// The directory the pattern is relative to, when it was given one.
    base: Option<PathBuf>,
    /// The kinds of change wanted.
    kinds: WatchKind,
}

impl Pattern {
    /// The pattern `watcher` describes, if it can be read.
    fn read(watcher: FileSystemWatcher) -> Option<Self> {
        let kinds = watcher.kind.unwrap_or(WatchKind::all());
        let (glob, base) = match watcher.glob_pattern {
            GlobPattern::String(glob) => (glob, None),
            GlobPattern::Relative(relative) => {
                let base = match relative.base_uri {
                    OneOf::Left(folder) => uri::path_of(&folder.uri),
                    OneOf::Right(base) => uri::path_of(&base),
                };
                (relative.pattern, Some(base?))
            }
        };
        let glob = GlobBuilder::new(&glob)
            .literal_separator(true)
            .build()
            .ok()?
            .compile_matcher();
        Some(Self { glob, base, kinds })
    }

    /// Whether this pattern wants to hear that `path` was `touched`.
    fn wants(&self, root: &Path, path: &Path, touched: Watched) -> bool {
        if !self.kinds.contains(touched.kind()) {
            return false;
        }
        match &self.base {
            Some(base) => path
                .strip_prefix(base)
                .is_ok_and(|relative| self.glob.is_match(relative)),
            None => {
                self.glob.is_match(path)
                    || path
                        .strip_prefix(root)
                        .is_ok_and(|relative| self.glob.is_match(relative))
            }
        }
    }
}

/// Every pattern a server has registered, by the registration it came in.
#[derive(Default)]
pub(super) struct Watchers {
    /// The patterns, by the id the server registered them under.
    registered: HashMap<String, Vec<Pattern>>,
}

impl Watchers {
    /// Takes in the registrations among `registrations` that are about
    /// watched files.
    pub(super) fn register(&mut self, registrations: &[Registration]) {
        for registration in registrations {
            if registration.method != METHOD {
                continue;
            }
            let options = registration.register_options.clone().unwrap_or(Value::Null);
            let Ok(options) =
                serde_json::from_value::<DidChangeWatchedFilesRegistrationOptions>(options)
            else {
                continue;
            };
            let patterns = options
                .watchers
                .into_iter()
                .filter_map(Pattern::read)
                .collect();
            self.registered.insert(registration.id.clone(), patterns);
        }
    }

    /// Forgets the registrations whose ids are among `ids`.
    pub(super) fn unregister<'a>(&mut self, ids: impl IntoIterator<Item = &'a String>) {
        for id in ids {
            self.registered.remove(id);
        }
    }

    /// The notification telling the server about the part of `changes` it
    /// registered for, when it registered for any of it.
    pub(super) fn notification(
        &self,
        root: &Path,
        changes: &[(PathBuf, Watched)],
    ) -> Option<Value> {
        let wanted = changes
            .iter()
            .filter(|(path, touched)| {
                self.registered
                    .values()
                    .flatten()
                    .any(|pattern| pattern.wants(root, path, *touched))
            })
            .map(|(path, touched)| FileEvent::new(uri::typed(path), touched.change()))
            .collect::<Vec<_>>();
        if wanted.is_empty() {
            return None;
        }
        Some(rpc::notification::<DidChangeWatchedFiles>(
            DidChangeWatchedFilesParams { changes: wanted },
        ))
    }
}
