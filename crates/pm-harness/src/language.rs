//! Answering the harness's language requests from the app's own language servers, so that the agent's edits are formatted and checked by the servers already running and no second set is started for it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use omc_wire::{Diagnostic, LanguageAnswer, LanguageAsk, Severity};
use pm_buffer::Buffer;
use pm_lsp::{Hub, Said, TextChange};

/// The app's language servers, handed to an agent so that the files it writes are formatted and checked by them instead of by servers of its own.
#[derive(Clone)]
pub struct LanguageServers(pub Arc<Hub>);

impl std::fmt::Debug for LanguageServers {
    /// Names the servers without showing them.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LanguageServers")
    }
}

/// The most entries of the agent's folder looked at for files to start language servers with.
const MOST_LOOKED_AT: usize = 2000;

/// The most languages whose servers are started before the agent writes anything.
const MOST_WARMED: usize = 3;

/// Folders never looked in for files to start language servers with: build output and dependencies.
const SKIPPED_FOLDERS: [&str; 5] = ["target", "build", "node_modules", "dist", "vendor"];

/// How long to wait between looks at what the servers have said.
const LOOK_AGAIN: Duration = Duration::from_millis(50);

/// A file the agent's conversation opened on the hub, as the server was last told it.
struct Opened {
    /// Which opening on the hub it is, which tells whether someone else, such as the editor, has opened the file since and it is theirs now.
    opening: u64,
    /// The version it was told.
    version: i32,
    /// The text it was told.
    text: String,
}

/// How a file stood with the hub once it was told the file's text from disk.
struct Synced {
    /// The file, from the root of the project.
    path: String,
    /// The text it holds.
    text: String,
    /// The version the hub was told, or nothing when someone else has the file open and keeps its version.
    version: Option<i32>,
    /// Whether the hub was told anything new.
    changed: bool,
    /// How many times servers had said what is wrong with a file before it was told.
    since: u64,
}

/// The app's language servers, seen from one agent's conversation in the folder `root`: the files the agent changes are opened on the hub under that folder, kept up to date as it changes them, and closed when the conversation ends.
pub(crate) struct HubLanguages {
    /// The hub that runs the language servers.
    hub: Arc<Hub>,
    /// The folder the agent works in.
    root: PathBuf,
    /// The files this conversation opened on the hub, by path from `root`.
    opened: Mutex<HashMap<String, Opened>>,
}

impl HubLanguages {
    /// The language servers of `hub`, for an agent working in `root`.
    pub(crate) fn new(hub: Arc<Hub>, root: PathBuf) -> Self {
        Self {
            hub,
            root,
            opened: Mutex::new(HashMap::new()),
        }
    }

    /// Opens on the hub one file of each of the first few languages found in the agent's folder, which starts their servers, so that they have loaded the project by the time the agent first writes a file and its diagnostics come back in time; takes as long as starting the servers does.
    pub(crate) fn warm(&self) {
        for path in self.samples() {
            let _ = self.sync(&path);
        }
    }

    /// One file of each of the first [`MOST_WARMED`] installed languages with a language server found in the agent's folder, looking at no more than [`MOST_LOOKED_AT`] entries and none in hidden folders or ones that hold build output or dependencies.
    fn samples(&self) -> Vec<PathBuf> {
        let mut found: Vec<(String, PathBuf)> = Vec::new();
        let mut folders = vec![self.root.clone()];
        let mut looked = 0;
        while let Some(folder) = folders.pop() {
            let Ok(entries) = std::fs::read_dir(&folder) else {
                continue;
            };
            let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
            entries.sort();
            for path in entries {
                looked += 1;
                if looked > MOST_LOOKED_AT || found.len() >= MOST_WARMED {
                    return found.into_iter().map(|(_, path)| path).collect();
                }
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if name.starts_with('.') {
                    continue;
                }
                if path.is_dir() {
                    if !SKIPPED_FOLDERS.contains(&name.as_str()) {
                        folders.insert(0, path);
                    }
                    continue;
                }
                let Some(language) = pm_lsp::language_of(&name) else {
                    continue;
                };
                let has_server = pm_lsp::installed_manifest(&language).is_some_and(|manifest| {
                    manifest.server.is_some() || manifest.inherits.is_some()
                });
                if has_server && !found.iter().any(|(held, _)| *held == language) {
                    found.push((language, path));
                }
            }
        }
        found.into_iter().map(|(_, path)| path).collect()
    }

    /// Answers `ask`, which can take as long as it says it may wait.
    pub(crate) fn answer(&self, ask: &LanguageAsk) -> LanguageAnswer {
        match ask {
            LanguageAsk::Format { path } => LanguageAnswer::Formatted {
                text: self.format(path),
            },
            LanguageAsk::Diagnostics { paths, wait_ms } => LanguageAnswer::Diagnostics {
                diagnostics: self.diagnostics(paths, Duration::from_millis(*wait_ms)),
            },
        }
    }

    /// The file at `path` formatted by its language's server, when it has one that formats and that changes the text; nothing for a file someone else has open, whose text on the hub may not be what is on disk.
    fn format(&self, path: &Path) -> Option<String> {
        let synced = self.sync(path)?;
        let version = synced.version?;
        let edits = self
            .hub
            .format(&self.root, &synced.path, version, None)
            .ok()?;
        let formatted = apply(&synced.text, version, edits)?;
        (formatted != synced.text).then_some(formatted)
    }

    /// What the servers say is wrong with the files at `paths`, as they are on disk now, waiting up to `wait` for them to say it about the new text once they have finished any work they report, such as loading the project; by then, what they last said about the new text, which leaves out a file they said nothing new about.
    fn diagnostics(&self, paths: &[PathBuf], wait: Duration) -> Vec<Diagnostic> {
        let deadline = Instant::now() + wait;
        let synced: Vec<(PathBuf, Synced)> = paths
            .iter()
            .filter_map(|path| self.sync(path).map(|synced| (path.clone(), synced)))
            .collect();
        let mut found = Vec::new();
        let mut waiting: Vec<&(PathBuf, Synced)> = synced.iter().collect();
        loop {
            let late = Instant::now() >= deadline;
            waiting.retain(|(path, synced)| {
                let Some(said) = self.hub.said(&self.root, &synced.path).filter(|said| {
                    is_fresh(said, synced)
                        && (late
                            || self
                                .hub
                                .is_settled(&self.root, &synced.path, said.publication))
                }) else {
                    return true;
                };
                found.extend(
                    said.diagnostics
                        .iter()
                        .map(|diagnostic| convert(diagnostic, path, &synced.text)),
                );
                false
            });
            if waiting.is_empty() || late {
                return found;
            }
            std::thread::sleep(LOOK_AGAIN.min(deadline - Instant::now()));
        }
    }

    /// Tells the hub that the file at `path` holds what is on disk now: opens it the first time, changes it after that, and leaves it as it is when someone else has it open, since they keep it up to date, which includes a file the conversation opened that someone has opened again since; nothing when the file is outside the folder, cannot be read or is in no language installed.
    fn sync(&self, path: &Path) -> Option<Synced> {
        let relative = path
            .strip_prefix(&self.root)
            .ok()?
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(path).ok()?;
        let since = self.hub.publications();
        let mut opened = self.opened.lock().ok()?;
        let current = self.hub.opening(&self.root, &relative);
        if opened
            .get(&relative)
            .is_some_and(|held| Some(held.opening) != current)
        {
            opened.remove(&relative);
        }
        let (version, changed) = match opened.get_mut(&relative) {
            Some(held) if held.text == text => (held.version, false),
            Some(held) => {
                held.version += 1;
                held.text.clone_from(&text);
                let change = TextChange {
                    span: None,
                    text: text.clone(),
                };
                self.hub
                    .change(&self.root, &relative, held.version, vec![change])
                    .ok()?;
                (held.version, true)
            }
            None if current.is_some() => {
                return Some(Synced {
                    path: relative,
                    text,
                    version: None,
                    changed: true,
                    since,
                });
            }
            None => {
                let language = pm_lsp::language_of_text(&relative, &text)?;
                let _ = self.hub.open(&self.root, &relative, &language, 1, &text);
                let opening = self.hub.opening(&self.root, &relative)?;
                opened.insert(
                    relative.clone(),
                    Opened {
                        opening,
                        version: 1,
                        text: text.clone(),
                    },
                );
                (1, true)
            }
        };
        Some(Synced {
            path: relative,
            text,
            version: Some(version),
            changed,
            since,
        })
    }
}

impl Drop for HubLanguages {
    /// Closes the files the conversation opened that nobody has opened again since, which stops the servers no other file uses.
    fn drop(&mut self) {
        let opened = self
            .opened
            .lock()
            .map(|mut opened| std::mem::take(&mut *opened))
            .unwrap_or_default();
        for (path, held) in &opened {
            self.hub.close_opening(&self.root, path, held.opening);
        }
    }
}

/// Whether `said` is about the text `synced` holds: said after the hub was told it, about its version or a later one, or about a file that did not change since the last time.
fn is_fresh(said: &Said, synced: &Synced) -> bool {
    let after = !synced.changed || said.publication > synced.since;
    let version = match (said.version, synced.version) {
        (Some(said), Some(told)) => said >= told,
        _ => true,
    };
    after && version
}

/// `text`, at `version`, with the formatting `edits` made to it, the last first so that each is placed where it was meant; nothing when two of them overlap.
fn apply(text: &str, version: i32, mut edits: Vec<TextChange>) -> Option<String> {
    edits.sort_by_key(|edit| edit.span.map(|span| (span.start, span.end)));
    let overlap = edits
        .windows(2)
        .any(|pair| match (pair[0].span, pair[1].span) {
            (Some(first), Some(second)) => first.end > second.start,
            _ => true,
        });
    if overlap && edits.len() > 1 {
        return None;
    }
    let mut buffer = Buffer::new(text, version);
    for edit in edits.iter().rev() {
        buffer.edit(edit.span, &edit.text);
    }
    Some(buffer.text().to_owned())
}

/// `diagnostic`, which a server said about the file at `path` holding `text`, in the harness's form: placed by line and character, both counted from one.
fn convert(diagnostic: &pm_lsp::Diagnostic, path: &Path, text: &str) -> Diagnostic {
    let start = diagnostic.span.start;
    let line_text = text.lines().nth(start.line as usize).unwrap_or_default();
    let mut units = 0;
    let mut column = 1;
    for character in line_text.chars() {
        if units >= start.character {
            break;
        }
        units += u32::try_from(character.len_utf16()).unwrap_or(2);
        column += 1;
    }
    Diagnostic {
        path: path.to_path_buf(),
        line: start.line + 1,
        column,
        severity: match diagnostic.severity {
            pm_lsp::Severity::Error => Severity::Error,
            pm_lsp::Severity::Warning => Severity::Warning,
            pm_lsp::Severity::Information => Severity::Information,
            pm_lsp::Severity::Hint => Severity::Hint,
        },
        message: diagnostic.message.clone(),
        source: diagnostic.source.clone(),
        code: None,
    }
}

#[cfg(test)]
/// Tests for answering from the hub.
mod tests {
    use pm_buffer::{Position, Span};

    use super::*;

    /// The stretch from line `line`, character `from` to character `to`.
    fn span(line: u32, from: u32, to: u32) -> Option<Span> {
        Some(Span::new(
            Position::new(line, from),
            Position::new(line, to),
        ))
    }

    /// Formatting edits are placed by where they fall in the original text, and overlapping ones are refused.
    #[test]
    fn applies_formatting_edits() {
        let edits = vec![
            TextChange {
                span: span(0, 2, 4),
                text: " ".to_owned(),
            },
            TextChange {
                span: span(0, 9, 10),
                text: String::new(),
            },
        ];
        assert_eq!(
            apply("fn  main( ) {}\n", 1, edits).as_deref(),
            Some("fn main() {}\n")
        );
        let overlapping = vec![
            TextChange {
                span: span(0, 0, 5),
                text: String::new(),
            },
            TextChange {
                span: span(0, 3, 6),
                text: String::new(),
            },
        ];
        assert_eq!(apply("fn main() {}\n", 1, overlapping), None);
    }

    /// A diagnostic is placed by line and character counted from one, a character outside the basic plane counting once.
    #[test]
    fn converts_a_diagnostic() {
        let diagnostic = pm_lsp::Diagnostic {
            span: Span::new(Position::new(1, 3), Position::new(1, 4)),
            severity: pm_lsp::Severity::Warning,
            message: "unused".to_owned(),
            source: Some("rustc".to_owned()),
        };
        let converted = convert(&diagnostic, Path::new("/w/a.rs"), "fn a() {}\n🦀x\n");
        assert_eq!((converted.line, converted.column), (2, 3));
        assert_eq!(converted.severity, Severity::Warning);
    }

    /// Diagnostics said after the hub was told the text, about that version or a later one, are fresh; older ones are not, unless the text did not change.
    #[test]
    fn tells_fresh_diagnostics_from_old_ones() {
        let synced = Synced {
            path: "a.rs".to_owned(),
            text: String::new(),
            version: Some(3),
            changed: true,
            since: 10,
        };
        let said = |publication, version| Said {
            publication,
            version,
            diagnostics: Vec::new(),
        };
        assert!(is_fresh(&said(11, Some(3)), &synced));
        assert!(is_fresh(&said(11, None), &synced));
        assert!(!is_fresh(&said(10, Some(3)), &synced));
        assert!(!is_fresh(&said(12, Some(2)), &synced));
        let unchanged = Synced {
            changed: false,
            ..synced
        };
        assert!(is_fresh(&said(4, Some(3)), &unchanged));
    }

    /// Creates a small Cargo project of its own under the workspace's git-ignored `build/` and returns its folder.
    fn project() -> PathBuf {
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/tests/pm-harness/probe");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        dir.canonicalize().unwrap()
    }

    /// The app's own Rust language server, started by the first question to a new hub, finds the error in a file the agent wrote in its first answer, having waited for it to load the project, and then formats another version of the file; skipped when Rust is not installed in the app.
    #[test]
    fn answers_from_the_apps_language_servers() {
        if pm_lsp::language_of("src/lib.rs").is_none() {
            eprintln!("skipped: Rust is not installed in the app");
            return;
        }
        let hub = Hub::new(
            Arc::new(|_| {}),
            Arc::new(|_: &Path, _: &str, _: &pm_lsp::ServerSpec| {
                std::env::var("PATH")
                    .map(|path| std::collections::BTreeMap::from([("PATH".to_owned(), path)]))
                    .unwrap_or_default()
            }),
        );
        let dir = project();
        let file = dir.join("src/lib.rs");
        let languages = HubLanguages::new(hub.clone(), dir.clone());
        std::fs::write(&file, "pub fn answer() -> u32 {\n    \"text\"\n}\n").unwrap();
        let LanguageAnswer::Diagnostics { diagnostics } =
            languages.answer(&LanguageAsk::Diagnostics {
                paths: vec![file.clone()],
                wait_ms: 30_000,
            })
        else {
            panic!("not a diagnostics answer");
        };
        let error = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == Severity::Error)
            .expect("the first answer of a server that just started has the error");
        assert_eq!((&error.path, error.line), (&file, 2));
        std::fs::write(&file, "pub fn  answer( )->u32{ 42 }\n").unwrap();
        let LanguageAnswer::Formatted { text } =
            languages.answer(&LanguageAsk::Format { path: file.clone() })
        else {
            panic!("not a formatting answer");
        };
        let text = text.expect("the server formats");
        assert!(text.contains("pub fn answer() -> u32 {"), "{text}");
        drop(languages);
        assert!(!hub.is_open(&dir, "src/lib.rs"));
    }

    /// Warming opens one file of each language with a server found in the folder, leaving out build output, dependencies and hidden folders; skipped when Rust is not installed in the app.
    #[test]
    fn warming_opens_a_file_of_each_language() {
        if pm_lsp::language_of("lib.rs").is_none() {
            eprintln!("skipped: Rust is not installed in the app");
            return;
        }
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/tests/pm-harness/warm");
        let _ = std::fs::remove_dir_all(&dir);
        for folder in ["src", "target/debug", ".hidden"] {
            std::fs::create_dir_all(dir.join(folder)).unwrap();
        }
        std::fs::write(dir.join("target/debug/built.rs"), "").unwrap();
        std::fs::write(dir.join(".hidden/secret.rs"), "").unwrap();
        std::fs::write(dir.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
        std::fs::write(dir.join("src/more.rs"), "pub fn b() {}\n").unwrap();
        let dir = dir.canonicalize().unwrap();
        let hub = Hub::new(
            Arc::new(|_| {}),
            Arc::new(|_: &Path, _: &str, _: &pm_lsp::ServerSpec| std::collections::BTreeMap::new()),
        );
        let languages = HubLanguages::new(hub.clone(), dir.clone());
        assert_eq!(languages.samples(), [dir.join("src/lib.rs")]);
        languages.warm();
        assert!(hub.is_open(&dir, "src/lib.rs"));
    }

    /// A file the conversation opened that the editor opens afterwards is the editor's: the conversation no longer changes it, and leaves it open when it ends; skipped when Rust is not installed in the app.
    #[test]
    fn leaves_a_file_the_editor_opened_since_alone() {
        if pm_lsp::language_of("lib.rs").is_none() {
            eprintln!("skipped: Rust is not installed in the app");
            return;
        }
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/tests/pm-harness/shared");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let dir = dir.canonicalize().unwrap();
        let file = dir.join("src/lib.rs");
        std::fs::write(&file, "pub fn a() {}\n").unwrap();
        let hub = Hub::new(
            Arc::new(|_| {}),
            Arc::new(|_: &Path, _: &str, _: &pm_lsp::ServerSpec| std::collections::BTreeMap::new()),
        );
        let languages = HubLanguages::new(hub.clone(), dir.clone());
        assert_eq!(languages.sync(&file).unwrap().version, Some(1));
        let language = pm_lsp::language_of("lib.rs").unwrap();
        hub.open(&dir, "src/lib.rs", &language, 7, "pub fn editor() {}\n")
            .unwrap();
        std::fs::write(&file, "pub fn b() {}\n").unwrap();
        assert_eq!(languages.sync(&file).unwrap().version, None);
        let held = hub
            .read_open(&dir, "src/lib.rs", 7, |_, buffer| buffer.text().to_owned())
            .unwrap();
        assert_eq!(held, "pub fn editor() {}\n");
        drop(languages);
        assert!(hub.is_open(&dir, "src/lib.rs"));
    }
}
