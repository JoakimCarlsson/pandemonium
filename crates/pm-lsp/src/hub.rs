//! The language servers of the projects a server's clients have files open in, and the text of each open file as the clients edit it.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use pm_buffer::{Buffer, Position};

use crate::install::{installed_manifest, language_of};
use crate::manifest::ServerSpec;
use crate::server::{DiagnosticsSink, LanguageServer};
use crate::types::{Completion, Diagnostic, Hover, Location, TextChange};
use crate::uri::{file_uri, path_of};

/// How long a question about a file waits for the edits it was asked after to arrive.
const CATCHING_UP: Duration = Duration::from_secs(2);

/// The most bytes of a file outside a project that are sent to be shown read only.
const MOST_SHOWN: u64 = 2 * 1024 * 1024;

/// The most completions handed back for one question.
const MOST_COMPLETIONS: usize = 200;

/// The most edits kept for a file that is not open yet, beyond which the earliest are dropped.
const MOST_WAITING: usize = 4096;

/// What a language server says is wrong with an open file.
#[derive(Clone, Debug, PartialEq)]
pub struct Published {
    /// The project the file is in.
    pub root: PathBuf,
    /// The file, from the root of the project.
    pub path: String,
    /// The version of the file the diagnostics are about, when the server says.
    pub version: Option<i32>,
    /// What is wrong.
    pub diagnostics: Vec<Diagnostic>,
}

/// What is done with what servers say is wrong with open files.
pub type Sink = Arc<dyn Fn(Published) + Send + Sync>;

/// Edits by the version they make.
type Edits = BTreeMap<i32, Vec<TextChange>>;

/// The latest diagnostics a server published for a file: the version they are about, when it says, and the diagnostics.
type Latest = (Option<i32>, Vec<Diagnostic>);

/// A language server of one project, started once however many files ask for it at the same time.
type Slot = Arc<OnceLock<Result<Arc<LanguageServer>, String>>>;

/// A file open in a project.
struct Document {
    /// Its text and version.
    buffer: Buffer,
    /// Its language's server, when the language has one.
    spec: Option<ServerSpec>,
    /// Edits that arrived ahead of the ones before them, by the version they make.
    early: Edits,
    /// The server it was opened on, once that is running.
    server: Option<Arc<LanguageServer>>,
}

/// The language servers and the open files.
pub struct Hub {
    /// The open files, by project and path.
    documents: Mutex<HashMap<(PathBuf, String), Document>>,
    /// Woken whenever an open file reaches a new version.
    changed: Condvar,
    /// The servers, by project and server.
    servers: Mutex<HashMap<(PathBuf, String), Slot>>,
    /// The latest diagnostics of each file, by address, kept for when a file is opened after they were published.
    published: Mutex<HashMap<String, Latest>>,
    /// The project and path of each open file, by address.
    addresses: Mutex<HashMap<String, (PathBuf, String)>>,
    /// Edits of files that arrived before the request that opens them, by project and path and then by the version they make.
    waiting: Mutex<HashMap<(PathBuf, String), Edits>>,
    /// Where diagnostics of open files go.
    sink: Sink,
}

/// How well `filter`, what a completion is matched by, matches the word `typed`: lower is better, nothing when it does not match.
pub fn score(filter: &str, typed: &str) -> Option<u8> {
    if typed.is_empty() {
        return Some(3);
    }
    if filter.starts_with(typed) {
        return Some(0);
    }
    let (filter_lower, typed_lower) = (filter.to_lowercase(), typed.to_lowercase());
    if filter_lower.starts_with(&typed_lower) {
        return Some(1);
    }
    let mut rest = filter_lower.chars();
    typed_lower
        .chars()
        .all(|wanted| rest.any(|held| held == wanted))
        .then_some(2)
}

/// Keeps the completions of `items` that match the word `typed`, best first, at most `most` of them.
pub fn rank(items: Vec<Completion>, typed: &str, most: usize) -> Vec<Completion> {
    let mut scored: Vec<(u8, Completion)> = items
        .into_iter()
        .filter_map(|item| score(&item.filter, typed).map(|held| (held, item)))
        .collect();
    scored.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.sort.cmp(&right.1.sort))
            .then_with(|| left.1.label.len().cmp(&right.1.label.len()))
    });
    scored
        .into_iter()
        .take(most)
        .map(|(_, item)| item)
        .collect()
}

/// The word of `text` that ends at the byte `offset`: the letters, digits and underscores before it.
fn word_before(text: &str, offset: usize) -> &str {
    let head = &text[..offset.min(text.len())];
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, character)| character.is_alphanumeric() || *character == '_')
        .last()
        .map_or(head.len(), |(at, _)| at);
    &head[start..]
}

/// Takes in the edits of `document`, the file at `uri`, that follow the version it is at, in order, telling its server of each; returns the server's spec when the server was lost on the way, so it can be started again.
fn catch_up(document: &mut Document, uri: &str) -> Option<ServerSpec> {
    let mut lost = None;
    while let Some(next) = document.early.remove(&(document.buffer.version() + 1)) {
        for change in &next {
            document.buffer.edit(change.span, &change.text);
        }
        let reached = document.buffer.version() + 1;
        document.buffer.set_version(reached);
        if let Some(server) = &document.server {
            let sent = server.change(uri, reached, &next, document.buffer.text());
            if sent.is_err() || !server.is_alive() {
                document.server = None;
                lost = document.spec.clone();
            }
        }
    }
    lost
}

impl Hub {
    /// A hub with no files open, handing what servers say is wrong with open files to `sink`.
    pub fn new(sink: Sink) -> Arc<Self> {
        Arc::new(Self {
            documents: Mutex::new(HashMap::new()),
            changed: Condvar::new(),
            servers: Mutex::new(HashMap::new()),
            published: Mutex::new(HashMap::new()),
            addresses: Mutex::new(HashMap::new()),
            waiting: Mutex::new(HashMap::new()),
            sink,
        })
    }

    /// What is done with the diagnostics the servers publish: kept, and handed on for a file that is open.
    fn diagnostics_sink(self: &Arc<Self>) -> DiagnosticsSink {
        let hub: Weak<Self> = Arc::downgrade(self);
        Arc::new(move |uri, version, diagnostics| {
            let Some(hub) = hub.upgrade() else {
                return;
            };
            if let Ok(mut published) = hub.published.lock() {
                published.insert(uri.clone(), (version, diagnostics.clone()));
            }
            let place = hub
                .addresses
                .lock()
                .ok()
                .and_then(|addresses| addresses.get(&uri).cloned());
            if let Some((root, path)) = place {
                (hub.sink)(Published {
                    root,
                    path,
                    version,
                    diagnostics,
                });
            }
        })
    }

    /// The running server of `spec` for the project at `root`, starting it when it is not running.
    fn server(
        self: &Arc<Self>,
        root: &Path,
        spec: &ServerSpec,
    ) -> Result<Arc<LanguageServer>, String> {
        let key = (root.to_path_buf(), spec.id.clone());
        for _ in 0..2 {
            let slot = self
                .servers
                .lock()
                .map_err(|_| "the server is shutting down".to_owned())?
                .entry(key.clone())
                .or_default()
                .clone();
            let started = slot
                .get_or_init(|| {
                    LanguageServer::start(spec, root, self.diagnostics_sink()).map(Arc::new)
                })
                .clone();
            match started {
                Ok(server) if server.is_alive() => return Ok(server),
                outcome => {
                    if let Ok(mut servers) = self.servers.lock()
                        && servers
                            .get(&key)
                            .is_some_and(|held| Arc::ptr_eq(held, &slot))
                    {
                        servers.remove(&key);
                    }
                    outcome?;
                }
            }
        }
        Err(format!("{} keeps stopping", spec.command))
    }

    /// Opens the file at `path` of the project at `root`, written in the installed language `language` and holding `text` at `version`, on its language's server, starting that server first; returns the characters after which the server offers completions.
    pub fn open(
        self: &Arc<Self>,
        root: &Path,
        path: &str,
        language: &str,
        version: i32,
        text: &str,
    ) -> Result<Vec<String>, String> {
        let manifest = installed_manifest(language)
            .ok_or_else(|| format!("{language} is not installed on this machine"))?;
        let key = (root.to_path_buf(), path.to_owned());
        let uri = file_uri(&root.join(path));
        let replaced = {
            let mut documents = self
                .documents
                .lock()
                .map_err(|_| "the server is shutting down".to_owned())?;
            let mut early = self
                .waiting
                .lock()
                .ok()
                .and_then(|mut waiting| waiting.remove(&key))
                .unwrap_or_default();
            early.retain(|made, _| *made > version);
            let mut document = Document {
                buffer: Buffer::new(text, version),
                spec: manifest.server.clone(),
                early,
                server: None,
            };
            catch_up(&mut document, &uri);
            documents.insert(key.clone(), document)
        };
        if let Some(server) = replaced.and_then(|document| document.server) {
            let _ = server.close(&uri);
        }
        if let Ok(mut addresses) = self.addresses.lock() {
            addresses.insert(uri.clone(), key.clone());
        }
        let Some(spec) = manifest.server else {
            return Ok(Vec::new());
        };
        self.attach(&key, &spec)
    }

    /// Opens the open file `key` on the server of `spec`, starting it first, with the text the file holds by then; returns the server's trigger characters.
    fn attach(
        self: &Arc<Self>,
        key: &(PathBuf, String),
        spec: &ServerSpec,
    ) -> Result<Vec<String>, String> {
        let server = self.server(&key.0, spec)?;
        let uri = file_uri(&key.0.join(&key.1));
        {
            let mut documents = self
                .documents
                .lock()
                .map_err(|_| "the server is shutting down".to_owned())?;
            let Some(document) = documents.get_mut(key) else {
                return Ok(Vec::new());
            };
            if document.server.is_none() {
                server.open(
                    &uri,
                    &spec.language_id,
                    document.buffer.version(),
                    document.buffer.text(),
                )?;
                document.server = Some(server.clone());
            }
        }
        let kept = self
            .published
            .lock()
            .ok()
            .and_then(|published| published.get(&uri).cloned());
        if let Some((version, diagnostics)) = kept {
            (self.sink)(Published {
                root: key.0.clone(),
                path: key.1.clone(),
                version,
                diagnostics,
            });
        }
        Ok(server.triggers().to_vec())
    }

    /// Takes in `changes`, which make the open file at `path` of the project at `root` reach `version`; edits that arrive ahead of the ones before them wait for those, and ones already taken in are ignored.
    pub fn change(
        self: &Arc<Self>,
        root: &Path,
        path: &str,
        version: i32,
        changes: Vec<TextChange>,
    ) -> Result<(), String> {
        let key = (root.to_path_buf(), path.to_owned());
        let uri = file_uri(&root.join(path));
        let lost = {
            let mut documents = self
                .documents
                .lock()
                .map_err(|_| "the server is shutting down".to_owned())?;
            let Some(document) = documents.get_mut(&key) else {
                if let Ok(mut waiting) = self.waiting.lock() {
                    let held = waiting.entry(key).or_default();
                    held.insert(version, changes);
                    while held.len() > MOST_WAITING {
                        held.pop_first();
                    }
                }
                return Ok(());
            };
            if version <= document.buffer.version() {
                return Ok(());
            }
            document.early.insert(version, changes);
            let lost = catch_up(document, &uri);
            self.changed.notify_all();
            lost
        };
        if let Some(spec) = lost {
            let hub = self.clone();
            std::thread::spawn(move || {
                let _ = hub.attach(&key, &spec);
            });
        }
        Ok(())
    }

    /// Closes the open file at `path` of the project at `root`, and stops its server when no other file of the project uses it.
    pub fn close(self: &Arc<Self>, root: &Path, path: &str) {
        let key = (root.to_path_buf(), path.to_owned());
        let uri = file_uri(&root.join(path));
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.remove(&key);
        }
        let Some(document) = self
            .documents
            .lock()
            .ok()
            .and_then(|mut documents| documents.remove(&key))
        else {
            return;
        };
        if let Ok(mut addresses) = self.addresses.lock() {
            addresses.remove(&uri);
        }
        let (Some(server), Some(spec)) = (document.server, document.spec) else {
            return;
        };
        let _ = server.close(&uri);
        let still_used = self.documents.lock().is_ok_and(|documents| {
            documents.iter().any(|((held_root, _), held)| {
                held_root == root && held.spec.as_ref().is_some_and(|other| other.id == spec.id)
            })
        });
        if still_used {
            return;
        }
        if let Ok(mut servers) = self.servers.lock() {
            servers.remove(&(root.to_path_buf(), spec.id));
        }
        std::thread::spawn(move || server.stop());
    }

    /// Waits until the open file at `key` reaches `version`, then returns its server, its address and what `look` makes of its buffer.
    fn caught_up<T>(
        &self,
        key: &(PathBuf, String),
        version: i32,
        look: impl FnOnce(&Buffer) -> T,
    ) -> Result<Option<(Arc<LanguageServer>, T)>, String> {
        let deadline = Instant::now() + CATCHING_UP;
        let mut documents = self
            .documents
            .lock()
            .map_err(|_| "the server is shutting down".to_owned())?;
        loop {
            let document = documents
                .get(key)
                .ok_or_else(|| format!("{} is not open", key.1))?;
            let now = Instant::now();
            if document.buffer.version() >= version || now >= deadline {
                return Ok(document
                    .server
                    .clone()
                    .map(|server| (server, look(&document.buffer))));
            }
            documents = self
                .changed
                .wait_timeout(documents, deadline - now)
                .map_err(|_| "the server is shutting down".to_owned())?
                .0;
        }
    }

    /// What could be written at `position` of the open file at `path` of the project at `root`, once it reaches `version`, the best match for the word typed there first; `trigger` is the character typed that asked for them, if one did.
    pub fn completion(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
        trigger: Option<&str>,
    ) -> Result<Vec<Completion>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, typed)) = self.caught_up(&key, version, |buffer| {
            word_before(buffer.text(), buffer.offset_of(position)).to_owned()
        })?
        else {
            return Ok(Vec::new());
        };
        let items = server.completion(&file_uri(&root.join(path)), position, trigger)?;
        Ok(rank(items, &typed, MOST_COMPLETIONS))
    }

    /// What the server says about the thing at `position` of the open file at `path` of the project at `root`, once it reaches `version`.
    pub fn hover(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
    ) -> Result<Option<Hover>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(None);
        };
        server.hover(&file_uri(&root.join(path)), position)
    }

    /// The edits that format the open file at `path` of the project at `root` as its language says, once it reaches `version`, each a stretch of the text at that version and what replaces it; none when its language server cannot format.
    pub fn format(&self, root: &Path, path: &str, version: i32) -> Result<Vec<TextChange>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        let manifest = language_of(path).and_then(|id| installed_manifest(&id));
        let (indent, tabs) =
            manifest.map_or((4, false), |manifest| (manifest.indent, manifest.tabs));
        server.formatting(&file_uri(&root.join(path)), indent, tabs)
    }

    /// Where the thing at `position` of the open file at `path` of the project at `root` is defined, once the file reaches `version`: files inside the project by their path from its root, and files outside it with what they hold, to be shown read only.
    pub fn definition(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
    ) -> Result<Vec<Location>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        let found = server.definition(&file_uri(&root.join(path)), position)?;
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        Ok(found
            .into_iter()
            .filter_map(|(address, span)| {
                let file = path_of(&address)?;
                match file
                    .strip_prefix(root)
                    .or_else(|_| file.strip_prefix(&canonical))
                {
                    Ok(relative) => Some(Location {
                        path: relative.to_string_lossy().replace('\\', "/"),
                        span,
                        inside: true,
                        text: None,
                        language: None,
                    }),
                    Err(_) => {
                        let shown = file.to_string_lossy().into_owned();
                        let text = std::fs::metadata(&file)
                            .ok()
                            .filter(|data| data.is_file() && data.len() <= MOST_SHOWN)
                            .and_then(|_| std::fs::read_to_string(&file).ok())?;
                        Some(Location {
                            language: language_of(&shown),
                            path: shown,
                            span,
                            inside: false,
                            text: Some(text),
                        })
                    }
                }
            })
            .collect())
    }
}
