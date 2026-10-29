//! The language servers of the projects a server's clients have files open in, and the text of each open file as the clients edit it.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use pm_buffer::{Buffer, Lines, Position, Span};
use serde_json::Value;

use crate::candidates::{self, Candidate};
use crate::choice::ServerChoice;
use crate::install::{installed_manifest, language_of};
use crate::manifest::ServerSpec;
use crate::preferences::ServerOverride;
use crate::provider::{Provider, SymbolLabel};
use crate::server::{
    DiagnosticsSink, Found, LanguageServer, LinkAnswer, Pressed, action_of, file_changes,
};
use crate::types::{
    CodeAction, CodeLens, ColorPresentation, ColorSwatch, Completion, Diagnostic, DocumentLink,
    FileEdit, FoldRange, Hover, InlayHint, InlineValue, LinkTarget, Location, Problem,
    RenameTarget, Seek, SemanticToken, Signature, TextChange, Triggers,
};
use crate::uri::{file_uri, path_of};

/// How long a question about a file waits for the edits it was asked after to arrive.
const CATCHING_UP: Duration = Duration::from_secs(2);

/// The most bytes of a file outside a project that are sent to be shown read only.
const MOST_SHOWN: u64 = 2 * 1024 * 1024;

/// The most symbols handed back for one search of a project.
const MOST_SYMBOLS: usize = 200;

/// The most symbols of folders attached to a project that are handed back for one search, each with the text of its file to be shown read only.
const MOST_ATTACHED_SYMBOLS: usize = 25;

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

/// What a server is started with besides the machine's own environment, for the project at a folder, the language whose file asked for it and the server of a spec: the variables to set, among them the `PATH` to run it with.
pub type Environment =
    Arc<dyn Fn(&Path, &str, &ServerSpec) -> BTreeMap<String, String> + Send + Sync>;

/// Edits by the version they make.
type Edits = BTreeMap<i32, Vec<TextChange>>;

/// The latest diagnostics a server published for a file: the version they are about, when it says, and the diagnostics.
type Latest = (Option<i32>, Vec<Diagnostic>);

/// The changes a server last offered for a file, as it wrote them, with that server.
type Offered = (Arc<LanguageServer>, Vec<Value>);

/// The folders besides a project's own that its language servers work on, as the project at a folder has them.
pub type Neighbours = Arc<dyn Fn(&Path) -> Vec<PathBuf> + Send + Sync>;

/// A language server of one project, started once however many files ask for it at the same time.
type Slot = Arc<OnceLock<Result<Arc<LanguageServer>, String>>>;

/// A companion server a file is open on, besides its language's own.
struct Attached {
    /// The server's spec.
    spec: ServerSpec,
    /// The scopes of the language its completions are offered in; everywhere when none.
    scopes: Vec<String>,
    /// The server, running.
    server: Arc<LanguageServer>,
}

/// The servers that read one file: the one that is its language's and the others that run beside it.
#[derive(Default)]
struct Plan {
    /// The language's server, when one reads the file.
    primary: Option<ServerSpec>,
    /// The others, each with the scopes of the language its completions are offered in; everywhere when none.
    beside: Vec<(ServerSpec, Vec<String>)>,
}

/// A file open in a project.
struct Document {
    /// The installed language it is written in.
    language: String,
    /// Its text and version.
    buffer: Buffer,
    /// Its language's server, when the language has one.
    spec: Option<ServerSpec>,
    /// Edits that arrived ahead of the ones before them, by the version they make.
    early: Edits,
    /// The server it was opened on, once that is running.
    server: Option<Arc<LanguageServer>>,
    /// The companion servers it was opened on, once those are running.
    companions: Vec<Attached>,
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
    /// The changes last offered for each open file, as its server wrote them, with that server, by project and path.
    actions: Mutex<HashMap<(PathBuf, String), Offered>>,
    /// The lenses last offered for each open file, as its server wrote them, with that server, by project and path.
    lenses: Mutex<HashMap<(PathBuf, String), Offered>>,
    /// The language servers the reader chose other than the ones languages come with, by language.
    choices: Mutex<HashMap<String, ServerChoice>>,
    /// What the reader set for each language server, by the server's id.
    overrides: Mutex<HashMap<String, ServerOverride>>,
    /// Whoever asks the extensions that brought languages about their language servers.
    provider: Mutex<Option<Arc<dyn Provider>>>,
    /// Where diagnostics of open files go.
    sink: Sink,
    /// What servers are started with.
    environment: Environment,
    /// The folders besides a project's own its servers work on.
    neighbours: Mutex<Neighbours>,
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
        let text = document.buffer.text();
        document.companions.retain(|held| {
            held.server.change(uri, reached, &next, text).is_ok() && held.server.is_alive()
        });
    }
    lost
}

impl Hub {
    /// A hub with no files open, handing what servers say is wrong with open files to `sink`.
    pub fn new(sink: Sink, environment: Environment) -> Arc<Self> {
        Arc::new(Self {
            documents: Mutex::new(HashMap::new()),
            changed: Condvar::new(),
            servers: Mutex::new(HashMap::new()),
            published: Mutex::new(HashMap::new()),
            addresses: Mutex::new(HashMap::new()),
            waiting: Mutex::new(HashMap::new()),
            actions: Mutex::new(HashMap::new()),
            lenses: Mutex::new(HashMap::new()),
            choices: Mutex::new(HashMap::new()),
            overrides: Mutex::new(HashMap::new()),
            provider: Mutex::new(None),
            sink,
            environment,
            neighbours: Mutex::new(Arc::new(|_| Vec::new())),
        })
    }

    /// The folders `neighbours` names for each project are attached to the workspace of its language servers from now on, and the servers running are told which came and went.
    pub fn attach_folders(&self, neighbours: Neighbours) {
        if let Ok(mut held) = self.neighbours.lock() {
            *held = neighbours;
        }
        self.refresh_folders();
    }

    /// The folders besides the project at `root`'s own that its servers work on.
    fn neighbours_of(&self, root: &Path) -> Vec<PathBuf> {
        let resolve = self.neighbours.lock().map(|held| held.clone()).ok();
        resolve
            .map(|resolve| resolve(root))
            .unwrap_or_default()
            .into_iter()
            .filter(|folder| folder != root && folder.is_dir())
            .collect()
    }

    /// Tells every running server which folders the workspace of its project holds now.
    pub fn refresh_folders(&self) {
        let running: Vec<(PathBuf, Arc<LanguageServer>)> = self
            .servers
            .lock()
            .map(|servers| {
                servers
                    .iter()
                    .filter_map(|((root, _), slot)| {
                        Some((root.clone(), slot.get()?.as_ref().ok()?.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (root, server) in running {
            let _ = server.set_extra_folders(&self.neighbours_of(&root));
        }
    }

    /// Whether `file` is inside a folder attached to the workspace of a server running for the project at `root`, besides the project's own.
    fn attached(&self, root: &Path, file: &Path) -> bool {
        self.running(root).iter().any(|server| {
            server
                .workspace_folders()
                .iter()
                .skip(1)
                .any(|folder| file.starts_with(folder))
        })
    }

    /// The server the files of `language`, whose own is `own`, are read by, as the reader chose.
    fn chosen(&self, language: &str, own: Option<ServerSpec>) -> Option<ServerSpec> {
        let choice = self
            .choices
            .lock()
            .ok()
            .and_then(|choices| choices.get(language).cloned());
        match choice {
            Some(choice) => choice.applied(language, own),
            None => own,
        }
    }

    /// Has `provider` say what the language servers of the languages extensions brought are started with and told, and what their completions and symbols are called.
    pub fn set_provider(&self, provider: Arc<dyn Provider>) {
        if let Ok(mut held) = self.provider.lock() {
            *held = Some(provider);
        }
    }

    /// The provider, when there is one.
    fn provider(&self) -> Option<Arc<dyn Provider>> {
        self.provider.lock().ok().and_then(|held| held.clone())
    }

    /// `spec`, a server of `language` that an extension provides, with what the extension says put under what the reader set, ready to start for the project at `root`; `spec` itself when it is not provided.
    fn provided(
        &self,
        language: &str,
        spec: &ServerSpec,
        root: &Path,
    ) -> Result<ServerSpec, String> {
        let Some(provider) = self.provider().filter(|_| spec.provided) else {
            return Ok(spec.clone());
        };
        let set = self
            .overrides
            .lock()
            .ok()
            .and_then(|held| held.get(&spec.id).cloned())
            .unwrap_or_default();
        let said = provider.provide(language, spec, root, set.binary.trim().is_empty())?;
        let mut merged = spec.clone();
        if let Some(command) = said.command {
            merged.command = command.command;
            merged.args = command.args;
            merged.env.extend(command.env);
            merged.install = None;
            merged.check = Vec::new();
        }
        if let Some(init) = said.init {
            merged.init = init;
        }
        if let Some(settings) = said.settings {
            merged.settings = settings;
        }
        Ok(set.applied(&merged))
    }

    /// `spec` with what the reader set for the server it is put over it.
    fn overridden(&self, spec: ServerSpec) -> ServerSpec {
        match self
            .overrides
            .lock()
            .ok()
            .and_then(|held| held.get(&spec.id).cloned())
        {
            Some(set) => set.applied(&spec),
            None => spec,
        }
    }

    /// The servers that read the file at `path` of the project at `root`, written in `language`: the one that is the language's server and the others that run beside it, as the reader chose, each with what the reader set for it.
    fn plan(&self, language: &str, root: &Path, path: &str) -> Plan {
        let own = installed_manifest(language).and_then(|manifest| manifest.server);
        let choice = self
            .choices
            .lock()
            .ok()
            .and_then(|choices| choices.get(language).cloned());
        if matches!(choice, Some(ServerChoice::Refused(_))) {
            return Plan::default();
        }
        let refused: Vec<String> = self
            .choices
            .lock()
            .map(|choices| {
                choices
                    .iter()
                    .filter(|(_, choice)| matches!(choice, ServerChoice::Refused(_)))
                    .map(|(name, _)| name.clone())
                    .collect()
            })
            .unwrap_or_default();
        let offers: Vec<Candidate> = candidates::candidates_of(language, own.clone())
            .into_iter()
            .filter(|offer| !refused.contains(&offer.origin))
            .collect();
        let (primary, beside): (Option<ServerSpec>, Vec<Candidate>) = match &choice {
            Some(ServerChoice::Servers(names)) => {
                let mut list = candidates::named(&offers, names, language, root, path);
                if list.is_empty() {
                    (None, Vec::new())
                } else {
                    let first = list.remove(0);
                    (Some(first.spec), list)
                }
            }
            _ => {
                let mut primary = self.chosen(language, own);
                let mut beside: Vec<Candidate> =
                    candidates::by_default(&offers, language, root, path)
                        .into_iter()
                        .filter(|offer| !offer.primary)
                        .collect();
                let off = matches!(choice, Some(ServerChoice::Off));
                if primary.is_none() && !off && !beside.is_empty() {
                    primary = Some(beside.remove(0).spec);
                }
                (primary, beside)
            }
        };
        Plan {
            primary: primary.map(|spec| self.overridden(spec)),
            beside: beside
                .into_iter()
                .map(|offer| (self.overridden(offer.spec), offer.scopes))
                .collect(),
        }
    }

    /// Reads the files of each language by the server `choices` names for it, or by the one it comes with when it names none, moving the open files whose server that changes onto the new one at once.
    pub fn choose_servers(self: &Arc<Self>, choices: HashMap<String, ServerChoice>) {
        let before = match self.choices.lock() {
            Ok(mut held) => std::mem::replace(&mut *held, choices.clone()),
            Err(_) => return,
        };
        let moved: Vec<(PathBuf, String)> = self
            .documents
            .lock()
            .map(|documents| {
                documents
                    .iter()
                    .filter(|(_, document)| {
                        before.get(&document.language) != choices.get(&document.language)
                    })
                    .map(|(key, _)| key.clone())
                    .collect()
            })
            .unwrap_or_default();
        for key in moved {
            self.move_server(key);
        }
    }

    /// Takes the open file `key` off the servers it is on and onto the ones its language is read by now.
    fn move_server(self: &Arc<Self>, key: (PathBuf, String)) {
        let uri = file_uri(&key.0.join(&key.1));
        let (old, dropped, spec) = {
            let Ok(mut documents) = self.documents.lock() else {
                return;
            };
            let Some(document) = documents.get_mut(&key) else {
                return;
            };
            let plan = self.plan(&document.language, &key.0, &key.1);
            let old = document.server.take().zip(document.spec.take());
            document.spec = plan.primary.clone();
            let (kept, dropped): (Vec<Attached>, Vec<Attached>) =
                std::mem::take(&mut document.companions)
                    .into_iter()
                    .partition(|held| {
                        plan.beside.iter().any(|(spec, _)| spec.id == held.spec.id)
                            && plan
                                .primary
                                .as_ref()
                                .is_none_or(|primary| primary.id != held.spec.id)
                    });
            document.companions = kept;
            (old, dropped, plan.primary)
        };
        if let Some((server, old_spec)) = old {
            let _ = server.close(&uri);
            self.stop_unused(&key.0, &old_spec, server);
        }
        for held in dropped {
            let _ = held.server.close(&uri);
            self.stop_unused(&key.0, &held.spec, held.server);
        }
        match spec {
            Some(spec) => {
                let hub = self.clone();
                std::thread::spawn(move || {
                    let _ = hub.attach(&key, &spec);
                });
            }
            None => {
                let hub = self.clone();
                std::thread::spawn(move || hub.attach_companions(&key));
            }
        }
    }

    /// What the reader set for each language server, by the server's id; a server whose settings changed is stopped and started again with them, and the open files it reads move onto the new one at once.
    pub fn set_overrides(self: &Arc<Self>, overrides: HashMap<String, ServerOverride>) {
        let before = match self.overrides.lock() {
            Ok(mut held) => std::mem::replace(&mut *held, overrides.clone()),
            Err(_) => return,
        };
        let changed: Vec<String> = before
            .keys()
            .chain(overrides.keys())
            .filter(|id| before.get(*id) != overrides.get(*id))
            .cloned()
            .collect();
        if changed.is_empty() {
            return;
        }
        let stopping: Vec<Arc<LanguageServer>> = match self.servers.lock() {
            Ok(mut servers) => {
                let keys: Vec<(PathBuf, String)> = servers
                    .keys()
                    .filter(|(_, id)| changed.contains(id))
                    .cloned()
                    .collect();
                keys.iter()
                    .filter_map(|key| servers.remove(key))
                    .filter_map(|slot| slot.get().and_then(|started| started.clone().ok()))
                    .collect()
            }
            Err(_) => return,
        };
        let moved: Vec<(PathBuf, String)> = self
            .documents
            .lock()
            .map(|mut documents| {
                documents
                    .iter_mut()
                    .filter(|(_, document)| {
                        document
                            .spec
                            .iter()
                            .chain(document.companions.iter().map(|held| &held.spec))
                            .any(|spec| changed.contains(&spec.id))
                    })
                    .map(|(key, document)| {
                        document.server = None;
                        document
                            .companions
                            .retain(|held| !changed.contains(&held.spec.id));
                        key.clone()
                    })
                    .collect()
            })
            .unwrap_or_default();
        for server in stopping {
            std::thread::spawn(move || server.stop());
        }
        for key in moved {
            self.move_server(key);
        }
    }

    /// Stops `server`, the one of `spec` for the project at `root`, when no open file of the project is on it any more.
    fn stop_unused(&self, root: &Path, spec: &ServerSpec, server: Arc<LanguageServer>) {
        let still_used = self.documents.lock().is_ok_and(|documents| {
            documents.iter().any(|((held_root, _), held)| {
                held_root == root
                    && (held.spec.as_ref().is_some_and(|other| other.id == spec.id)
                        || held.companions.iter().any(|other| other.spec.id == spec.id))
            })
        });
        if still_used {
            return;
        }
        if let Ok(mut servers) = self.servers.lock() {
            servers.remove(&(root.to_path_buf(), spec.id.clone()));
        }
        std::thread::spawn(move || server.stop());
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
        language: &str,
        spec: &ServerSpec,
        sink: DiagnosticsSink,
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
                    let ready = self.provided(language, spec, root)?;
                    LanguageServer::start(
                        &ready,
                        root,
                        &self.neighbours_of(root),
                        &(self.environment)(root, language, &ready),
                        sink.clone(),
                    )
                    .map(Arc::new)
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

    /// Opens the file at `path` of the project at `root`, written in the installed language `language` and holding `text` at `version`, on its language's server, starting that server first; returns the characters after which the server says something without being asked.
    pub fn open(
        self: &Arc<Self>,
        root: &Path,
        path: &str,
        language: &str,
        version: i32,
        text: &str,
    ) -> Result<Triggers, String> {
        installed_manifest(language)
            .ok_or_else(|| format!("{language} is not installed on this machine"))?;
        let plan = self.plan(language, root, path);
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
                language: language.to_owned(),
                buffer: Buffer::new(text, version),
                spec: plan.primary.clone(),
                early,
                server: None,
                companions: Vec::new(),
            };
            catch_up(&mut document, &uri);
            documents.insert(key.clone(), document)
        };
        if let Some(document) = replaced {
            for server in document
                .server
                .into_iter()
                .chain(document.companions.into_iter().map(|held| held.server))
            {
                let _ = server.close(&uri);
            }
        }
        if let Ok(mut addresses) = self.addresses.lock() {
            addresses.insert(uri.clone(), key.clone());
        }
        if let Some(ServerChoice::Refused(reason)) = self
            .choices
            .lock()
            .ok()
            .and_then(|choices| choices.get(language).cloned())
        {
            return Err(reason);
        }
        let Some(spec) = plan.primary else {
            return Ok(Triggers::default());
        };
        self.attach(&key, &spec)
    }

    /// Opens the open file `key` on the server of `spec`, starting it first, with the text the file holds by then; returns the characters after which the server says something without being asked.
    fn attach(
        self: &Arc<Self>,
        key: &(PathBuf, String),
        spec: &ServerSpec,
    ) -> Result<Triggers, String> {
        let language = self
            .documents
            .lock()
            .ok()
            .and_then(|documents| documents.get(key).map(|document| document.language.clone()))
            .unwrap_or_default();
        let server = self.server(&key.0, &language, spec, self.diagnostics_sink())?;
        let uri = file_uri(&key.0.join(&key.1));
        {
            let mut documents = self
                .documents
                .lock()
                .map_err(|_| "the server is shutting down".to_owned())?;
            let Some(document) = documents.get_mut(key) else {
                return Ok(Triggers::default());
            };
            if document.server.is_none() {
                server.open(
                    &uri,
                    spec.language_id_for(&document.language, &key.1),
                    document.buffer.version(),
                    document.buffer.text(),
                )?;
                document.server = Some(server.clone());
                if server.pulls_diagnostics() {
                    LanguageServer::pull_diagnostics_soon(&server, &uri, document.buffer.version());
                }
            }
        }
        let hub = self.clone();
        let companions_of = key.clone();
        std::thread::spawn(move || hub.attach_companions(&companions_of));
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
        Ok(Triggers {
            completion: server.triggers().to_vec(),
            signature: server.signature_triggers(),
            format: server.format_triggers(),
            formats_ranges: server.formats_ranges(),
            folds: server.folds(),
            links: server.links(),
            semantic_tokens: server.has_semantic_tokens(),
            semantic_ranges: server.has_semantic_ranges(),
            inlay_hints: server.has_inlay_hints(),
            code_lens: server.has_code_lens(),
            inline_values: server.has_inline_values(),
            document_links: server.has_document_links(),
            colors: server.has_colors(),
        })
    }

    /// Opens the open file `key` on each server that runs beside its language's own, starting those first; a server that cannot be started is left out, for the language's own server still reads the file.
    fn attach_companions(self: &Arc<Self>, key: &(PathBuf, String)) {
        let Some(language) = self
            .documents
            .lock()
            .ok()
            .and_then(|documents| documents.get(key).map(|document| document.language.clone()))
        else {
            return;
        };
        let uri = file_uri(&key.0.join(&key.1));
        for (spec, scopes) in self.plan(&language, &key.0, &key.1).beside {
            let quiet: DiagnosticsSink = Arc::new(|_, _, _| {});
            let Ok(server) = self.server(&key.0, &language, &spec, quiet) else {
                continue;
            };
            let Ok(mut documents) = self.documents.lock() else {
                return;
            };
            let Some(document) = documents.get_mut(key) else {
                return;
            };
            if document
                .companions
                .iter()
                .any(|held| held.spec.id == spec.id)
            {
                continue;
            }
            let opened = server.open(
                &uri,
                spec.language_id_for(&language, &key.1),
                document.buffer.version(),
                document.buffer.text(),
            );
            if opened.is_ok() {
                document.companions.push(Attached {
                    spec,
                    scopes,
                    server,
                });
            }
        }
    }

    /// The language of the open file `key` and the spec of the server it is on, when it is on one.
    fn language_and_spec(&self, key: &(PathBuf, String)) -> (String, Option<ServerSpec>) {
        self.documents
            .lock()
            .ok()
            .and_then(|documents| {
                documents
                    .get(key)
                    .map(|document| (document.language.clone(), document.spec.clone()))
            })
            .unwrap_or_default()
    }

    /// Renames `items`, completions of the server `spec` for files of `language`, as the extension that provides the server says.
    fn label_completions(&self, language: &str, spec: &ServerSpec, items: &mut [Completion]) {
        let Some(provider) = self.provider().filter(|_| spec.provided) else {
            return;
        };
        let labels = provider.label_completions(language, spec, items);
        for (item, label) in items.iter_mut().zip(labels) {
            if let Some(label) = label {
                item.label = label;
            }
        }
    }

    /// The companion servers the open file `key` is on, each with the scopes of its language their completions are offered in, which are none when they are offered everywhere.
    fn companions_of(
        &self,
        key: &(PathBuf, String),
    ) -> Vec<(Vec<String>, Arc<LanguageServer>, ServerSpec)> {
        let Ok(documents) = self.documents.lock() else {
            return Vec::new();
        };
        documents.get(key).map_or_else(Vec::new, |document| {
            document
                .companions
                .iter()
                .map(|held| (held.scopes.clone(), held.server.clone(), held.spec.clone()))
                .collect()
        })
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
            if let Some(server) = document
                .server
                .as_ref()
                .filter(|server| server.pulls_diagnostics())
            {
                LanguageServer::pull_diagnostics_soon(server, &uri, document.buffer.version());
            }
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
        if let Ok(mut actions) = self.actions.lock() {
            actions.remove(&key);
        }
        if let Ok(mut lenses) = self.lenses.lock() {
            lenses.remove(&key);
        }
        for held in document.companions {
            let _ = held.server.close(&uri);
            self.stop_unused(root, &held.spec, held.server);
        }
        let (Some(server), Some(spec)) = (document.server, document.spec) else {
            return;
        };
        let _ = server.close(&uri);
        self.stop_unused(root, &spec, server);
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

    /// What `look` makes of the language and the text of the open file at `path` of the project at `root`, once the file reaches `version`; it is told the text as the language server has it, whether or not the language has a server.
    pub fn read_open<T>(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        look: impl FnOnce(&str, &Buffer) -> T,
    ) -> Result<T, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let deadline = Instant::now() + CATCHING_UP;
        let mut documents = self
            .documents
            .lock()
            .map_err(|_| "the server is shutting down".to_owned())?;
        loop {
            let document = documents
                .get(&key)
                .ok_or_else(|| format!("{path} is not open"))?;
            let now = Instant::now();
            if document.buffer.version() >= version || now >= deadline {
                return Ok(look(&document.language, &document.buffer));
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
        scope: Option<&str>,
    ) -> Result<Vec<Completion>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, typed)) = self.caught_up(&key, version, |buffer| {
            word_before(buffer.text(), buffer.offset_of(position)).to_owned()
        })?
        else {
            return Ok(Vec::new());
        };
        let address = file_uri(&root.join(path));
        let (language, own) = self.language_and_spec(&key);
        let mut items = server.completion(&address, position, trigger)?;
        if let Some(own) = &own {
            self.label_completions(&language, own, &mut items);
        }
        for (scopes, companion, spec) in self.companions_of(&key) {
            let here = scopes.is_empty()
                || scope.is_some_and(|scope| scopes.iter().any(|name| name == scope));
            if here && let Ok(mut more) = companion.completion(&address, position, trigger) {
                self.label_completions(&language, &spec, &mut more);
                more.iter_mut().for_each(|item| item.resolve = None);
                items.extend(more);
            }
        }
        Ok(rank(items, &typed, MOST_COMPLETIONS))
    }

    /// The completion `raw`, as the language server of the open file at `path` of the project at `root` wrote it in its list, with what that server adds when asked: its documentation and detail, and the edits choosing it makes besides writing it.
    pub fn resolve_completion(
        &self,
        root: &Path,
        path: &str,
        raw: &str,
    ) -> Result<Completion, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let server = self
            .documents
            .lock()
            .map_err(|_| "the server is shutting down".to_owned())?
            .get(&key)
            .and_then(|document| document.server.clone())
            .ok_or_else(|| format!("{path} has no language server"))?;
        server.resolve_completion(raw)
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
        let address = file_uri(&root.join(path));
        let mut hover = server.hover(&address, position)?;
        for (_, companion, _) in self.companions_of(&key) {
            let Ok(Some(more)) = companion.hover(&address, position) else {
                continue;
            };
            hover = Some(match hover {
                Some(held) => Hover {
                    text: format!("{}\n\n---\n\n{}", held.text, more.text),
                    span: held.span.or(more.span),
                },
                None => more,
            });
        }
        Ok(hover)
    }

    /// How a level of indentation is written in the file at `path`: the columns it is and whether it is a tab, as its language says.
    fn indentation_of(path: &str) -> (usize, bool) {
        language_of(path)
            .and_then(|id| installed_manifest(&id))
            .map_or((4, false), |manifest| (manifest.indent, manifest.tabs))
    }

    /// The edits that format the open file at `path` of the project at `root`, or only the stretch `range` of it, as its language says, once it reaches `version`, each a stretch of the text at that version and what replaces it; none when its language server cannot format.
    pub fn format(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        range: Option<Span>,
    ) -> Result<Vec<TextChange>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        let (indent, tabs) = Self::indentation_of(path);
        server.formatting(&file_uri(&root.join(path)), range, indent, tabs)
    }

    /// The edits that format what was just typed, the character `typed` at `position` of the open file at `path` of the project at `root`, once it reaches `version`; none when its language server does not format as one types.
    pub fn format_on_type(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
        typed: &str,
    ) -> Result<Vec<TextChange>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        let (indent, tabs) = Self::indentation_of(path);
        server.formatting_on_type(&file_uri(&root.join(path)), position, typed, indent, tabs)
    }

    /// The blocks of lines of the open file at `path` of the project at `root` that can be folded, once it reaches `version`; none when its language server does not say.
    pub fn folding_ranges(
        &self,
        root: &Path,
        path: &str,
        version: i32,
    ) -> Result<Vec<FoldRange>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        server.folding_ranges(&file_uri(&root.join(path)))
    }

    /// The stretches of the open file at `path` of the project at `root` that are edited together with the one at `position`, once it reaches `version`; none when its language server knows of none.
    pub fn linked_ranges(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
    ) -> Result<Vec<Span>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        server.linked_ranges(&file_uri(&root.join(path)), position)
    }

    /// The places `seek` looks for from `position` of the open file at `path` of the project at `root`, once the file reaches `version`: files inside the project by their path from its root, and files outside it with what they hold, to be shown read only; each with the line it starts on.
    pub fn locate(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
        seek: Seek,
    ) -> Result<Vec<Location>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        let found = server.locate(&file_uri(&root.join(path)), position, seek)?;
        let mut texts: HashMap<PathBuf, Option<String>> = HashMap::new();
        Ok(found
            .into_iter()
            .filter_map(|place| self.location(root, place, seek, &mut texts))
            .collect())
    }

    /// What a client is told of the place `found` a server named for the project at `root`, reading the files it is in once each into `texts`; the whole of a file outside the project is sent only for a definition, which is opened at once.
    fn location(
        &self,
        root: &Path,
        (address, span, name): Found,
        seek: Seek,
        texts: &mut HashMap<PathBuf, Option<String>>,
    ) -> Option<Location> {
        let file = path_of(&address)?;
        let relative = relative_to(root, &file);
        let text = texts
            .entry(file.clone())
            .or_insert_with(|| {
                let open = relative.as_ref().and_then(|relative| {
                    self.documents.lock().ok().and_then(|documents| {
                        documents
                            .get(&(root.to_path_buf(), relative.clone()))
                            .map(|document| document.buffer.text().to_owned())
                    })
                });
                open.or_else(|| {
                    std::fs::metadata(&file)
                        .ok()
                        .filter(|data| data.is_file() && data.len() <= MOST_SHOWN)
                        .and_then(|_| std::fs::read_to_string(&file).ok())
                })
            })
            .clone();
        let line = text.as_deref().and_then(|text| {
            text.lines()
                .nth(span.start.line as usize)
                .map(|line| line.trim_end().to_owned())
        });
        match relative {
            Some(relative) => Some(Location {
                path: relative,
                span,
                inside: true,
                text: None,
                language: None,
                name,
                line,
            }),
            None => {
                let shown = file.to_string_lossy().into_owned();
                let whole = matches!(
                    seek,
                    Seek::Definition
                        | Seek::Declaration
                        | Seek::TypeDefinition
                        | Seek::Implementation
                );
                Some(Location {
                    language: language_of(&shown),
                    path: shown,
                    span,
                    inside: false,
                    text: if whole { Some(text?) } else { None },
                    name,
                    line,
                })
            }
        }
    }

    /// What the language servers last said is wrong with the files of the project at `root`, open or not, the worst first and then by file and place.
    pub fn problems(&self, root: &Path) -> Vec<Problem> {
        let Ok(published) = self.published.lock() else {
            return Vec::new();
        };
        let mut found: Vec<Problem> = published
            .iter()
            .filter_map(|(address, (_, diagnostics))| {
                let path = relative_to(root, &path_of(address)?)?;
                Some(diagnostics.iter().map(move |diagnostic| Problem {
                    path: path.clone(),
                    diagnostic: diagnostic.clone(),
                }))
            })
            .flatten()
            .collect();
        found.sort_by(|left, right| {
            left.diagnostic
                .severity
                .cmp(&right.diagnostic.severity)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.diagnostic.span.start.cmp(&right.diagnostic.span.start))
        });
        found
    }

    /// The symbols of the project at `root` whose names match `query`, as the language server of the open file at `path` finds them, defined inside the project or in a folder attached to its workspace, at most a limit of them; those in attached folders come with their files, to be shown read only.
    pub fn workspace_symbols(
        &self,
        root: &Path,
        path: &str,
        query: &str,
    ) -> Result<Vec<Location>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let (language, spec) = self.language_and_spec(&key);
        let server = self
            .documents
            .lock()
            .map_err(|_| "the server is shutting down".to_owned())?
            .get(&key)
            .and_then(|document| document.server.clone());
        let Some(server) = server else {
            return Ok(Vec::new());
        };
        let (mut found, symbols): (Vec<_>, Vec<SymbolLabel>) =
            server.workspace_symbols(query)?.into_iter().unzip();
        if let (Some(provider), Some(spec)) = (self.provider(), spec.filter(|spec| spec.provided)) {
            for (place, label) in found
                .iter_mut()
                .zip(provider.label_symbols(&language, &spec, &symbols))
            {
                if label.is_some() {
                    place.2 = label;
                }
            }
        }
        let mut texts: HashMap<PathBuf, Option<String>> = HashMap::new();
        let mut inside = Vec::new();
        let mut beside = Vec::new();
        for place in found.into_iter().take(MOST_SYMBOLS) {
            let in_attached = path_of(&place.0).is_some_and(|file| {
                relative_to(root, &file).is_none() && self.attached(root, &file)
            });
            if in_attached {
                if beside.len() < MOST_ATTACHED_SYMBOLS
                    && let Some(location) = self.location(root, place, Seek::Definition, &mut texts)
                {
                    beside.push(location);
                }
            } else if let Some(location) = self.location(root, place, Seek::References, &mut texts)
                && location.inside
            {
                inside.push(location);
            }
        }
        inside.extend(beside);
        Ok(inside)
    }

    /// The stretches of the open file at `path` of the project at `root` where the thing at `position` is used, once the file reaches `version`.
    pub fn highlights(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
    ) -> Result<Vec<Span>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        server.highlights(&file_uri(&root.join(path)), position)
    }

    /// What each name of the open file at `path` of the project at `root` is, once the file reaches `version`, only those within `range` when it is given and its language server says the names of a stretch, which is told alongside; none when its language server does not say.
    pub fn semantic_tokens(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        user: &[crate::semantic::TokenRule],
        range: Option<Span>,
    ) -> Result<(Vec<SemanticToken>, bool), String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok((Vec::new(), false));
        };
        let language = self
            .documents
            .lock()
            .ok()
            .and_then(|documents| {
                documents
                    .get(&key)
                    .map(|document| document.language.clone())
            })
            .unwrap_or_default();
        let rules = crate::semantic::rules_for(&language, user);
        server.semantic_tokens(&file_uri(&root.join(path)), &rules, range)
    }

    /// Which stretches of the open file at `path` of the project at `root` within `range` to show the values of while the program is paused at `stopped` in the debugger's frame `frame`, once the file reaches `version`; none when its language server does not say.
    pub fn inline_values(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        range: Span,
        frame: i64,
        stopped: Span,
    ) -> Result<Vec<InlineValue>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        server.inline_values(&file_uri(&root.join(path)), range, frame, stopped)
    }

    /// What to show inside the lines of the open file at `path` of the project at `root` within `span`, once the file reaches `version`; none when its language server does not say.
    pub fn inlay_hints(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        span: Span,
    ) -> Result<Vec<InlayHint>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        server.inlay_hints(&file_uri(&root.join(path)), span)
    }

    /// The lines of text to show with stretches of the open file at `path` of the project at `root`, once the file reaches `version`; they are kept to be pressed by their place in the list.
    pub fn code_lenses(
        &self,
        root: &Path,
        path: &str,
        version: i32,
    ) -> Result<Vec<CodeLens>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        let (lenses, raw) = server.code_lenses(&file_uri(&root.join(path)))?;
        if let Ok(mut held) = self.lenses.lock() {
            held.insert(key, (server, raw));
        }
        Ok(lenses)
    }

    /// The stretches of the open file at `path` of the project at `root` that lead somewhere when pressed, once the file reaches `version`: web addresses, and files inside the project; links to anywhere else are left out.
    pub fn document_links(
        &self,
        root: &Path,
        path: &str,
        version: i32,
    ) -> Result<Vec<DocumentLink>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        let found = server.document_links(&file_uri(&root.join(path)))?;
        Ok(found
            .into_iter()
            .filter_map(|(span, address)| {
                let target = match address {
                    LinkAnswer::Address(address) => link_target(root, &address)?,
                    LinkAnswer::Raw(link) => LinkTarget::Unresolved { link },
                };
                Some(DocumentLink { span, target })
            })
            .collect())
    }

    /// Where the link `raw`, as the language server of the open file at `path` of the project at `root` wrote it, leads; nothing when the server does not say or it leads outside the project and the web.
    pub fn resolve_link(
        &self,
        root: &Path,
        path: &str,
        raw: &str,
    ) -> Result<Option<LinkTarget>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let server = self
            .documents
            .lock()
            .map_err(|_| "the server is shutting down".to_owned())?
            .get(&key)
            .and_then(|document| document.server.clone())
            .ok_or_else(|| format!("{path} has no language server"))?;
        Ok(server
            .resolve_link(raw)?
            .and_then(|address| link_target(root, &address)))
    }

    /// The ways the language server of the open file at `path` of the project at `root` offers to write `color` in place of `span`, once the file reaches `version`.
    pub fn color_presentations(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        span: Span,
        color: [u8; 4],
    ) -> Result<Vec<ColorPresentation>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        server.color_presentations(&file_uri(&root.join(path)), color, span)
    }

    /// The stretches of the open file at `path` of the project at `root` that spell a colour, once the file reaches `version`; none when its language server does not say.
    pub fn colors(
        &self,
        root: &Path,
        path: &str,
        version: i32,
    ) -> Result<Vec<ColorSwatch>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        server.colors(&file_uri(&root.join(path)))
    }

    /// Presses the `index`-th lens last offered for the open file at `path` of the project at `root`: the places it points to, as to be shown, or the changes the press made to the open files, files no client has open being changed on disk here.
    pub fn press_lens(
        &self,
        root: &Path,
        path: &str,
        index: usize,
    ) -> Result<(Vec<Location>, Vec<FileEdit>), String> {
        let key = (root.to_path_buf(), path.to_owned());
        let (server, lens) = self
            .lenses
            .lock()
            .ok()
            .and_then(|held| {
                let (server, offered) = held.get(&key)?;
                Some((server.clone(), offered.get(index)?.clone()))
            })
            .ok_or_else(|| "that lens is no longer offered".to_owned())?;
        match server.press_lens(&lens)? {
            Pressed::Places(found) => {
                let mut texts: HashMap<PathBuf, Option<String>> = HashMap::new();
                let places = found
                    .into_iter()
                    .filter_map(|place| self.location(root, place, Seek::References, &mut texts))
                    .collect();
                Ok((places, Vec::new()))
            }
            Pressed::Edits(edits) => Ok((Vec::new(), self.share_edits(root, &edits)?)),
        }
    }

    /// The signature of the call at `position` of the open file at `path` of the project at `root`, once the file reaches `version`.
    pub fn signature(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
    ) -> Result<Option<Signature>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(None);
        };
        server.signature(&file_uri(&root.join(path)), position)
    }

    /// What can be renamed at `position` of the open file at `path` of the project at `root`, once the file reaches `version`; nothing when nothing there can be.
    pub fn prepare_rename(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
    ) -> Result<Option<RenameTarget>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Err("no language server reads this file".to_owned());
        };
        server.prepare_rename(&file_uri(&root.join(path)), position)
    }

    /// Renames the thing at `position` of the open file at `path` of the project at `root` to `name`, once the file reaches `version`: files of the project no client has open are changed on disk here, and the changes to the open ones are returned for the clients to make.
    pub fn rename(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        position: Position,
        name: &str,
    ) -> Result<Vec<FileEdit>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Err("no language server reads this file".to_owned());
        };
        let edit = server.rename(&file_uri(&root.join(path)), position, name)?;
        if edit.is_null() {
            return Err(format!("nothing at this place can be renamed to {name}"));
        }
        self.share_edits(root, &[edit])
    }

    /// The servers running for the project at `root`.
    fn running(&self, root: &Path) -> Vec<Arc<LanguageServer>> {
        let slots: Vec<Slot> = self
            .servers
            .lock()
            .map(|servers| {
                servers
                    .iter()
                    .filter(|((held, _), _)| held == root)
                    .map(|(_, slot)| slot.clone())
                    .collect()
            })
            .unwrap_or_default();
        slots
            .iter()
            .filter_map(|slot| slot.get()?.as_ref().ok().cloned())
            .collect()
    }

    /// Asks the servers running for the project at `root` what has to change before the file or folder at `from` is renamed to `to`, both from the root of the project: files no client has open are changed on disk here, and the changes to the open ones are returned for the clients to make.
    pub fn will_rename(&self, root: &Path, from: &str, to: &str) -> Result<Vec<FileEdit>, String> {
        let (old, new) = (file_uri(&root.join(from)), file_uri(&root.join(to)));
        let edits: Vec<Value> = self
            .running(root)
            .iter()
            .filter_map(|server| server.will_rename(&old, &new).ok())
            .filter(|edit| !edit.is_null())
            .collect();
        self.share_edits(root, &edits)
    }

    /// Tells the servers running for the project at `root` that the file or folder at `from` was renamed to `to`, both from the root of the project.
    pub fn did_rename(&self, root: &Path, from: &str, to: &str) {
        let (old, new) = (file_uri(&root.join(from)), file_uri(&root.join(to)));
        for server in self.running(root) {
            let _ = server.did_rename(&old, &new);
        }
    }

    /// The changes the language server of the open file at `path` of the project at `root` offers for the stretch `span`, once the file reaches `version`, only those of the kinds in `only` when it names any; they are kept to be taken by their place in the list.
    pub fn code_actions(
        &self,
        root: &Path,
        path: &str,
        version: i32,
        span: Span,
        only: &[String],
    ) -> Result<Vec<CodeAction>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let Some((server, ())) = self.caught_up(&key, version, |_| ())? else {
            return Ok(Vec::new());
        };
        let offered: Vec<Value> = server
            .code_actions(&file_uri(&root.join(path)), span, only)?
            .into_iter()
            .filter(|value| action_of(value).is_some())
            .collect();
        let actions = offered.iter().filter_map(action_of).collect();
        if let Ok(mut held) = self.actions.lock() {
            held.insert(key, (server, offered));
        }
        Ok(actions)
    }

    /// Takes the `index`-th change last offered for the open file at `path` of the project at `root`: files of the project no client has open are changed on disk here, and the changes to the open ones are returned for the clients to make.
    pub fn take_action(
        &self,
        root: &Path,
        path: &str,
        index: usize,
    ) -> Result<Vec<FileEdit>, String> {
        let key = (root.to_path_buf(), path.to_owned());
        let (server, action) = self
            .actions
            .lock()
            .ok()
            .and_then(|held| {
                let (server, offered) = held.get(&key)?;
                Some((server.clone(), offered.get(index)?.clone()))
            })
            .ok_or_else(|| "that change is no longer offered".to_owned())?;
        let edits = server.take_action(&action)?;
        self.share_edits(root, &edits)
    }

    /// Splits the edits across the project at `root` that a server wrote as `edits`: files of the project no client has open are changed on disk at once, and the changes to the open ones are returned; files outside the project are left alone.
    fn share_edits(&self, root: &Path, edits: &[Value]) -> Result<Vec<FileEdit>, String> {
        let mut open = Vec::new();
        for (address, changes) in edits.iter().flat_map(file_changes) {
            let Some(relative) = path_of(&address).and_then(|file| relative_to(root, &file)) else {
                continue;
            };
            let is_open = self.documents.lock().is_ok_and(|documents| {
                documents.contains_key(&(root.to_path_buf(), relative.clone()))
            });
            if is_open {
                match open
                    .iter_mut()
                    .find(|held: &&mut FileEdit| held.path == relative)
                {
                    Some(held) => held.changes.extend(changes),
                    None => open.push(FileEdit {
                        path: relative,
                        changes,
                    }),
                }
            } else {
                change_on_disk(&inside_root(root, &relative)?, &changes)?;
            }
        }
        Ok(open)
    }
}

/// Where the address `address` a language server gave a link leads: a web address as it is, a file inside the project at `root` with the place its fragment names, `L12,5` or `12,5` counted from 1; nothing for any other address.
fn link_target(root: &Path, address: &str) -> Option<LinkTarget> {
    if ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| address.starts_with(scheme))
    {
        return Some(LinkTarget::Url {
            address: address.to_owned(),
        });
    }
    let (file, fragment) = address.split_once('#').unwrap_or((address, ""));
    let path = relative_to(root, &path_of(file)?)?;
    let mut numbers = fragment
        .trim_start_matches('L')
        .split([',', ':', 'C'])
        .filter_map(|number| number.trim_start_matches('L').parse::<u32>().ok());
    let position = numbers.next().map(|line| {
        pm_buffer::Position::new(
            line.saturating_sub(1),
            numbers.next().map_or(0, |column| column.saturating_sub(1)),
        )
    });
    Some(LinkTarget::File { path, position })
}

/// The path of `file` from the root of the project at `root`, when it is inside it.
fn relative_to(root: &Path, file: &Path) -> Option<String> {
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    file.strip_prefix(root)
        .or_else(|_| file.strip_prefix(&canonical))
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

/// The file at `relative` under the project at `root`, refused when the path steps out of the project, by `..` or through a link that leads elsewhere.
fn inside_root(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let stays = Path::new(relative)
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)));
    let refused = || format!("{relative} is not a file inside the project");
    if !stays {
        return Err(refused());
    }
    let canonical_root = root.canonicalize().map_err(|_| refused())?;
    let file = root.join(relative).canonicalize().map_err(|_| refused())?;
    if file.starts_with(&canonical_root) {
        Ok(file)
    } else {
        Err(refused())
    }
}

/// Makes `changes`, each a stretch of the file as it is before any of them and what replaces it, to the file at `file` on disk.
fn change_on_disk(file: &Path, changes: &[TextChange]) -> Result<(), String> {
    let mut text = std::fs::read_to_string(file)
        .map_err(|error| format!("could not read {}: {error}", file.display()))?;
    let lines = Lines::new(&text);
    let mut ranges: Vec<(std::ops::Range<usize>, &str)> = changes
        .iter()
        .filter_map(|change| Some((lines.range_of(change.span?, &text), change.text.as_str())))
        .collect();
    ranges.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, replacement) in ranges {
        text.replace_range(range, replacement);
    }
    std::fs::write(file, text)
        .map_err(|error| format!("could not write {}: {error}", file.display()))
}
