//! Gathering what the pickers offer without holding a frame up.
//!
//! A worktree can hold a hundred thousand files, a search reads every one
//! of them, and a branch list is a subprocess: none of that can run between
//! a keystroke and the frame that shows it. So the files of a worktree are
//! listed once per opening of a picker on a thread of their own and handed
//! over in batches as they come; a search runs on another, over that same
//! listing, and is dropped the moment the query changes; and git is asked
//! for branches and remotes away from the window too. Each wakes the window
//! with what it found, and what belongs to a picker no longer open is let go.

use pm_host::Location;

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pm_core::Scope;
use pm_text::{Finder, Position, Query};

use crate::app::picker::{file_row, relative};
use crate::app::{App, Wake};
use crate::picker::{Choice, Kind, Row};

/// Most results a project-wide search gathers before it stops looking.
pub(super) const SEARCH_LIMIT: usize = 500;

/// Longest a line of context beside a search result is drawn.
const CONTEXT: usize = 120;

/// Largest file a search reads; anything bigger is data, not source.
const SEARCHED_BYTES: u64 = 1 << 20;

/// How much of the start of a file is looked at to tell whether it is binary.
const SNIFFED_BYTES: usize = 8 << 10;

/// How long a search waits after a keystroke before it starts reading, so
/// that a word typed quickly is searched for once rather than once a letter.
pub(super) const DEBOUNCE: Duration = Duration::from_millis(80);

/// How long a search that has caught up with the listing waits for more.
const CAUGHT_UP: Duration = Duration::from_millis(5);

/// Most files, or results, held back before they are handed to the window.
const BATCH: usize = 256;

/// One file's matches returned by a background worktree search.
pub(super) struct FileMatches {
    /// The file in the worktree.
    pub(super) path: PathBuf,
    /// Every shown match in the file.
    pub(super) ranges: Vec<Range<Position>>,
    /// Fingerprint of the text that produced those ranges.
    pub(super) fingerprint: u64,
}

/// A batch of worktree search results.
pub(super) struct SearchBatch {
    /// The query generation it belongs to.
    pub(super) generation: u64,
    /// Files completed in this batch.
    pub(super) files: Vec<FileMatches>,
    /// Whether the walk has finished.
    pub(super) done: bool,
    /// Whether the search found more matches than it can show.
    pub(super) limited: bool,
}

/// Longest something found is held back before it is handed to the window.
const FLUSH: Duration = Duration::from_millis(40);

/// The files of one worktree, as they are listed away from the window.
struct Files {
    /// The worktree they belong to.
    scope: Scope,
    /// Where the worktree sits on disk.
    root: Location,
    /// Every file listed so far, in the order the walk came upon them.
    paths: Mutex<Vec<PathBuf>>,
    /// Whether the walk has finished, written under the lock on `paths` so
    /// that one look at both says whether any more are coming.
    done: AtomicBool,
    /// Whether nobody wants the listing any more, which stops the walk and
    /// every search over it.
    dropped: AtomicBool,
}

/// Something gathered away from the window for the picker.
enum Found {
    /// Results of the search of this generation.
    Matches(u64, Vec<Row>),
    /// The search of this generation has read everything it was going to.
    Searched(u64),
    /// Rows asked of git for a picker of this kind, in this opening.
    Rows(u64, Kind, Vec<Row>, bool),
}

/// What the pickers have running away from the window, and what came back.
#[derive(Default)]
pub(super) struct Listings {
    /// What has come back and not been taken in yet.
    found: Arc<Mutex<Vec<Found>>>,
    /// The files of the worktree in front, for the picker open now.
    files: Option<Arc<Files>>,
    /// How many of those files the picker has been given as rows.
    taken: usize,
    /// How many pickers have been opened, which names the one rows from git
    /// belong to.
    opening: u64,
    /// The generation of the search wanted now; a search of any other stops.
    search: Arc<AtomicU64>,
    /// The generation of the search whose results the picker shows.
    shown: u64,
    /// The query last searched for, so that moving the caret along the field
    /// does not search again.
    searched: Option<String>,
}

impl App {
    /// Lets go of everything gathered for the picker that was open: the
    /// listing stops walking and any search over it stops reading.
    pub(super) fn leave_listings(&mut self) {
        if let Some(files) = self.listings.files.take() {
            files.dropped.store(true, Ordering::Release);
        }
        self.listings.taken = 0;
        self.listings.searched = None;
        self.listings.search.fetch_add(1, Ordering::AcqRel);
    }

    /// Counts one more picker opened, so that rows asked for by any before
    /// it are not taken in.
    pub(super) fn begin_opening(&mut self) {
        self.listings.opening += 1;
    }

    /// The files of the worktree in front listed so far, as rows, starting
    /// the listing if this opening has none yet.
    pub(super) fn listed_file_rows(&mut self) -> Vec<Row> {
        let Some(files) = self.files_listing() else {
            return Vec::new();
        };
        let paths = files
            .paths
            .lock()
            .map(|paths| paths.clone())
            .unwrap_or_default();
        self.listings.taken = paths.len();
        paths
            .into_iter()
            .map(|path| file_row(files.scope, &files.root, path))
            .collect()
    }

    /// The listing of this opening, started on a thread of its own the first
    /// time it is asked for.
    fn files_listing(&mut self) -> Option<Arc<Files>> {
        if let Some(files) = &self.listings.files {
            return Some(files.clone());
        }
        let (scope, root) = self.here_on_disk()?;
        let files = Arc::new(Files {
            scope,
            root,
            paths: Mutex::new(Vec::new()),
            done: AtomicBool::new(false),
            dropped: AtomicBool::new(false),
        });
        self.listings.files = Some(files.clone());
        self.listings.taken = 0;
        let listing = files.clone();
        let wake = self.waker(Wake::Listing);
        std::thread::spawn(move || list(&listing, &*wake));
        Some(files)
    }

    /// Searches the worktree in front for `query` away from the window,
    /// stopping whichever search was running.
    pub(super) fn search_later(&mut self, query: &str) {
        if self.listings.searched.as_deref() == Some(query) {
            return;
        }
        self.listings.searched = Some(query.to_owned());
        let generation = self.listings.search.fetch_add(1, Ordering::AcqRel) + 1;
        if query.chars().count() < 2 {
            self.show_matches(generation, Vec::new());
            return;
        }
        let Some(files) = self.files_listing() else {
            return;
        };
        let Ok(finder) = Finder::new(&Query {
            text: query.to_owned(),
            ..Query::default()
        }) else {
            return;
        };
        let search = Search {
            snapshots: self.editor.search_snapshots(files.scope),
            files,
            finder,
            generation,
            wanted: self.listings.search.clone(),
        };
        let found = self.listings.found.clone();
        let wake = self.waker(Wake::Listing);
        std::thread::spawn(move || search.run(&found, &*wake));
    }

    /// Asks git for the rows of a picker of `kind` away from the window, and
    /// fills the picker with them if it is still the one open when git has
    /// answered.
    pub(super) fn ask_git_later(
        &self,
        kind: Kind,
        ask: impl FnOnce() -> Vec<Row> + Send + 'static,
    ) {
        self.ask_git_updates_later(kind, move |publish| publish(ask(), true));
    }

    /// Publishes successive background Git results for the current picker,
    /// marking the final update when the background operation has finished.
    pub(super) fn ask_git_updates_later(
        &self,
        kind: Kind,
        ask: impl FnOnce(&dyn Fn(Vec<Row>, bool)) + Send + 'static,
    ) {
        let opening = self.listings.opening;
        let found = self.listings.found.clone();
        let wake = self.waker(Wake::Listing);
        std::thread::spawn(move || {
            ask(&|rows, finished| {
                if let Ok(mut found) = found.lock() {
                    found.push(Found::Rows(opening, kind, rows, finished));
                }
                wake();
            });
        });
    }

    /// Takes in everything gathered that has come back, answering whether
    /// any of it changed what the picker shows.
    pub(super) fn take_listings(&mut self) -> bool {
        let found = self
            .listings
            .found
            .lock()
            .map(|mut found| std::mem::take(&mut *found))
            .unwrap_or_default();
        let mut changed = self.take_listed_files();
        for found in found {
            changed |= match found {
                Found::Matches(generation, rows) => self.show_matches(generation, rows),
                Found::Searched(generation) => self.show_matches(generation, Vec::new()),
                Found::Rows(opening, kind, rows, finished) => {
                    self.show_asked(opening, kind, rows, finished)
                }
            };
        }
        changed | self.take_project_searches()
    }

    /// Hands the picker the files listed since it was last given any, or
    /// lets the listing go when no picker that wants it is open.
    fn take_listed_files(&mut self) -> bool {
        let Some(files) = self.listings.files.clone() else {
            return false;
        };
        let kind = self.picker.as_ref().map(crate::picker::Picker::kind);
        if !matches!(
            kind,
            Some(Kind::Files | Kind::WorkspaceSymbols | Kind::Commands | Kind::Search)
        ) {
            self.leave_listings();
            return false;
        }
        if !matches!(kind, Some(Kind::Files | Kind::WorkspaceSymbols)) {
            return false;
        }
        let fresh = files
            .paths
            .lock()
            .map(|paths| paths[self.listings.taken.min(paths.len())..].to_vec())
            .unwrap_or_default();
        if fresh.is_empty() {
            return false;
        }
        self.listings.taken += fresh.len();
        let rows = fresh
            .into_iter()
            .map(|path| file_row(files.scope, &files.root, path))
            .collect::<Vec<_>>();
        if kind == Some(Kind::WorkspaceSymbols) {
            self.workspace_files.extend(rows.iter().cloned());
        }
        if let Some(picker) = self.picker.as_mut() {
            picker.extend(rows);
        }
        true
    }

    /// Puts the results of the search of `generation` into the search
    /// picker, in place of an earlier search's or after its own before.
    fn show_matches(&mut self, generation: u64, rows: Vec<Row>) -> bool {
        if generation != self.listings.search.load(Ordering::Acquire) {
            return false;
        }
        let Some(picker) = self
            .picker
            .as_mut()
            .filter(|picker| picker.kind() == Kind::Search)
        else {
            return false;
        };
        if self.listings.shown == generation {
            if rows.is_empty() {
                return false;
            }
            picker.extend(rows);
        } else {
            self.listings.shown = generation;
            picker.refill(rows);
        }
        true
    }

    /// Fills the picker with the rows git answered for a picker of `kind`,
    /// when that picker is still the one open.
    fn show_asked(&mut self, opening: u64, kind: Kind, rows: Vec<Row>, finished: bool) -> bool {
        if opening != self.listings.opening {
            return false;
        }
        let Some(picker) = self.picker.as_mut().filter(|picker| picker.kind() == kind) else {
            return false;
        };
        picker.refill_preserving_selection(rows);
        if kind == Kind::Branches && finished {
            self.branch_refresh_at = Some(Instant::now() + super::picker::BRANCH_REFRESH);
        }
        true
    }
}

/// Walks `files`' worktree into it, waking the window with each batch.
fn list(files: &Files, wake: &(dyn Fn() + Send + Sync)) {
    let mut batch = Vec::new();
    let mut flushed = Instant::now();
    pm_core::walk_each(&files.root, |path| {
        if files.dropped.load(Ordering::Acquire) {
            return false;
        }
        batch.push(path);
        if batch.len() >= BATCH || flushed.elapsed() >= FLUSH {
            if let Ok(mut paths) = files.paths.lock() {
                paths.append(&mut batch);
            }
            flushed = Instant::now();
            wake();
        }
        true
    });
    if let Ok(mut paths) = files.paths.lock() {
        paths.append(&mut batch);
        files.done.store(true, Ordering::Release);
    }
    wake();
}

/// One search of a worktree's files for a query, away from the window.
pub(super) struct Search {
    /// The files searched, as they are listed.
    files: Arc<Files>,
    /// The compiled query.
    finder: Finder,
    /// Open documents as they stood when the search began.
    snapshots: HashMap<PathBuf, String>,
    /// Which search this is.
    generation: u64,
    /// Which search is wanted now.
    wanted: Arc<AtomicU64>,
}

impl Search {
    /// Searches a worktree for a results pane on a background thread.
    pub(super) fn run_project(
        root: Location,
        snapshots: HashMap<PathBuf, String>,
        finder: Finder,
        generation: u64,
        wanted: Arc<AtomicU64>,
        pending: Arc<Mutex<Vec<SearchBatch>>>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) {
        std::thread::spawn(move || {
            std::thread::sleep(DEBOUNCE);
            let mut files = Vec::new();
            let mut count = 0;
            let mut limited = false;
            let mut flushed = Instant::now();
            pm_core::walk_each(&root, |path| {
                if wanted.load(Ordering::Acquire) != generation {
                    return false;
                }
                let Some(content) = search_text(&root.at(&path), &snapshots) else {
                    return true;
                };
                let mut ranges = match_ranges(&content, &finder, SEARCH_LIMIT - count + 1);
                if ranges.len() > SEARCH_LIMIT - count {
                    ranges.truncate(SEARCH_LIMIT - count);
                    limited = true;
                }
                count += ranges.len();
                if !ranges.is_empty() {
                    files.push(FileMatches {
                        path,
                        ranges,
                        fingerprint: fingerprint(&content),
                    });
                }
                if !files.is_empty() && (files.len() >= 32 || flushed.elapsed() >= FLUSH || limited)
                {
                    if let Ok(mut pending) = pending.lock() {
                        pending.push(SearchBatch {
                            generation,
                            files: std::mem::take(&mut files),
                            done: false,
                            limited,
                        });
                    }
                    wake();
                    flushed = Instant::now();
                }
                !limited
            });
            if wanted.load(Ordering::Acquire) == generation {
                if let Ok(mut pending) = pending.lock() {
                    pending.push(SearchBatch {
                        generation,
                        files,
                        done: true,
                        limited,
                    });
                }
                wake();
            }
        });
    }
    /// Whether this search is still wanted.
    fn live(&self) -> bool {
        self.wanted.load(Ordering::Acquire) == self.generation
            && !self.files.dropped.load(Ordering::Acquire)
    }

    /// Reads the listed files one at a time for the needle, handing what it
    /// finds to `found` in batches, until it has read them all, found
    /// enough, or is no longer wanted.
    fn run(self, found: &Mutex<Vec<Found>>, wake: &(dyn Fn() + Send + Sync)) {
        std::thread::sleep(DEBOUNCE);
        let mut next = 0;
        let mut given = 0;
        let mut batch = Vec::new();
        let mut flushed = Instant::now();
        while self.live() && given + batch.len() < SEARCH_LIMIT {
            let (path, done) = match self.files.paths.lock() {
                Ok(paths) => (
                    paths.get(next).cloned(),
                    self.files.done.load(Ordering::Acquire),
                ),
                Err(_) => break,
            };
            let Some(path) = path else {
                if done {
                    break;
                }
                std::thread::sleep(CAUGHT_UP);
                continue;
            };
            next += 1;
            self.search_file(&path, &mut batch, SEARCH_LIMIT - given);
            if !batch.is_empty() && (batch.len() >= BATCH || flushed.elapsed() >= FLUSH) {
                given += batch.len();
                self.hand(
                    found,
                    Found::Matches(self.generation, std::mem::take(&mut batch)),
                );
                flushed = Instant::now();
                wake();
            }
        }
        if !self.live() {
            return;
        }
        if !batch.is_empty() {
            self.hand(found, Found::Matches(self.generation, batch));
        }
        self.hand(found, Found::Searched(self.generation));
        wake();
    }

    /// Puts `what` where the window takes it in from.
    fn hand(&self, found: &Mutex<Vec<Found>>, what: Found) {
        if let Ok(mut found) = found.lock() {
            found.push(what);
        }
    }

    /// Adds a row to `rows` for every line of the file at `path` the needle
    /// is on, until `rows` holds `room` of them.
    ///
    /// A file too big to be source, one that is not text, or one that cannot
    /// be read is passed over.
    fn search_file(&self, path: &Path, rows: &mut Vec<Row>, room: usize) {
        let Some(text) = search_text(&self.files.root.at(path), &self.snapshots) else {
            return;
        };
        let lines = search_lines(&text);
        for found in match_ranges(&text, &self.finder, room.saturating_sub(rows.len())) {
            let line = found.start.line;
            let content = lines.get(line).copied().unwrap_or_default();
            rows.push(Row {
                section: None,
                label: content.trim().chars().take(CONTEXT).collect(),
                detail: format!("{}:{}", relative(&self.files.root, path), line + 1),
                choice: Choice::OpenAt(self.files.scope, path.to_path_buf(), found.start),
                enabled: true,
            });
        }
    }
}

/// Reads a file's current open text or its disk copy for background search.
pub(super) fn search_text(
    location: &Location,
    snapshots: &HashMap<PathBuf, String>,
) -> Option<String> {
    let path = &location.path;
    if location
        .host
        .fs()
        .symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return None;
    }
    if let Some(snapshot) = snapshots.get(path) {
        return Some(snapshot.clone());
    }
    if !location
        .host
        .fs()
        .metadata(path)
        .is_ok_and(|meta| meta.len() <= SEARCHED_BYTES)
    {
        return None;
    }
    let bytes = location.host.fs().read(path).ok()?;
    if bytes[..bytes.len().min(SNIFFED_BYTES)].contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Returns at most `room` match ranges in one file, measured in character columns.
pub(super) fn match_ranges(text: &str, finder: &Finder, room: usize) -> Vec<Range<Position>> {
    let mut found = Vec::new();
    for (line, content) in search_lines(text).into_iter().enumerate() {
        for range in finder.line(content) {
            if found.len() == room {
                return found;
            }
            found.push(Position::new(line, range.start)..Position::new(line, range.end));
        }
    }
    found
}

/// Splits a file into editor lines, retaining its empty final line.
fn search_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'\r' || bytes[at] == b'\n' {
            lines.push(&text[start..at]);
            if bytes[at] == b'\r' && bytes.get(at + 1) == Some(&b'\n') {
                at += 1;
            }
            start = at + 1;
        }
        at += 1;
    }
    lines.push(&text[start..]);
    lines
}

/// Fingerprints a file snapshot so stale results are discarded after an edit.
pub(super) fn fingerprint(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
