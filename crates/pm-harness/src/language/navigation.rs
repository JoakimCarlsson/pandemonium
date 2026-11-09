//! Answering the harness's questions about code from the app's language servers: where symbols are defined and used, what they are, which symbols a file or the project has, what is wrong with the whole project, and the edits a rename makes, which are handed back for the harness to write.
//!
//! The harness counts places in lines and characters from one, a character being a Unicode scalar value; the servers count UTF-16 code units from nought. Every place is converted against the text of its file on disk.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use omc_wire::{FileEdits, LanguageAnswer, Place, Stretch, SymbolInfo, TextEdit};
use pm_buffer::{Position, Span};
use pm_lsp::{Load, Outline, Seek};

use super::{HubLanguages, LOOK_AGAIN, column_of, convert};

/// How long a question about code waits, the first time a language's server is asked, for it to finish loading the project.
const LOADING: Duration = Duration::from_secs(20);

/// How long a server that has reported no work yet is given to begin some, such as loading the project, before it is taken to have none.
const BEGIN: Duration = Duration::from_secs(3);

/// How long a server must stay without work, the first time, before it is taken to have loaded the project, since loading may pass from one step to the next with a moment of nothing between them.
const QUIET: Duration = Duration::from_secs(1);

/// A file the servers are ready to be asked about.
pub(super) struct Ready {
    /// The file, from the root of the project.
    pub(super) relative: String,
    /// The version the hub holds it at, or nought when someone else has it open and keeps its version.
    pub(super) version: i32,
    /// What the file holds on disk.
    pub(super) text: String,
}

/// The texts of files on disk, each read once, to place what the servers point at in them.
#[derive(Default)]
pub(super) struct Texts(pub(super) HashMap<PathBuf, String>);

impl Texts {
    /// The text of the file at `file`, read the first time it is asked for; empty when it cannot be read.
    fn of(&mut self, file: &Path) -> &str {
        self.0
            .entry(file.to_path_buf())
            .or_insert_with(|| std::fs::read_to_string(file).unwrap_or_default())
    }

    /// The stretch `span` of the file at `file`, in UTF-16 code units, as the harness places it.
    pub(super) fn stretch(&mut self, file: &Path, span: Span) -> Stretch {
        let text = self.of(file);
        Stretch {
            path: file.to_path_buf(),
            line: span.start.line + 1,
            column: column_of(text, span.start.line, span.start.character),
            end_line: span.end.line + 1,
            end_column: column_of(text, span.end.line, span.end.character),
        }
    }

    /// The symbol `outline` a server named, as the harness knows it.
    fn symbol(&mut self, outline: Outline) -> SymbolInfo {
        SymbolInfo {
            location: self.stretch(&outline.file, outline.span),
            name: outline.name,
            kind: outline.kind,
            detail: outline.detail,
            container: outline.container,
            depth: outline.depth,
        }
    }
}

/// The edits `changed` a server made, by file, each a stretch of the file as it is on disk and what replaces it, placed as the harness counts places against the texts of `texts`.
pub(super) fn file_edits(
    texts: &mut Texts,
    changed: Vec<(PathBuf, Vec<pm_lsp::TextChange>)>,
) -> Vec<FileEdits> {
    changed
        .into_iter()
        .map(|(file, changes)| {
            let edits = changes
                .into_iter()
                .filter_map(|change| {
                    let stretch = texts.stretch(&file, change.span?);
                    Some(TextEdit {
                        line: stretch.line,
                        column: stretch.column,
                        end_line: stretch.end_line,
                        end_column: stretch.end_column,
                        text: change.text,
                    })
                })
                .collect();
            FileEdits { path: file, edits }
        })
        .collect()
}

/// The place in `text` that line `line` and character `column` name, both counted from one, as a server counts it: the line from nought and the UTF-16 code units into it.
pub(super) fn position_of(text: &str, line: u32, column: u32) -> Position {
    let line = line.saturating_sub(1);
    let units: usize = text
        .lines()
        .nth(line as usize)
        .unwrap_or_default()
        .chars()
        .take(column.saturating_sub(1) as usize)
        .map(char::len_utf16)
        .sum();
    Position::new(line, u32::try_from(units).unwrap_or(u32::MAX))
}

impl HubLanguages {
    /// Opens the file at `path` on the hub with what it holds on disk and waits for its server to load the project the first time, so that it can be asked what `provider` answers, which the harness calls `what`.
    ///
    /// # Errors
    ///
    /// Answers [`LanguageAnswer::Failed`] for a file outside the project, [`LanguageAnswer::NoServer`] when no installed language or no running server reads it, and [`LanguageAnswer::Unsupported`] when its server does not answer `provider`.
    pub(super) fn ready(
        &self,
        path: &Path,
        provider: &str,
        what: &str,
    ) -> Result<Ready, LanguageAnswer> {
        if !path.starts_with(&self.root) {
            return Err(LanguageAnswer::Failed {
                reason: format!(
                    "{} is outside the project the language servers run for",
                    path.display()
                ),
            });
        }
        let synced = self.sync(path).ok_or_else(|| LanguageAnswer::NoServer {
            reason: format!("no installed language reads {}", path.display()),
        })?;
        self.settle(&synced.path);
        match self.hub.offers(&self.root, &synced.path, provider) {
            None => Err(LanguageAnswer::NoServer {
                reason: format!(
                    "no language server runs for {}; its language's server may not be installed",
                    synced.path
                ),
            }),
            Some(false) => Err(LanguageAnswer::Unsupported {
                reason: format!(
                    "the language server of {} does not answer {what}",
                    synced.path
                ),
            }),
            Some(true) => Ok(Ready {
                relative: synced.path,
                version: synced.version.unwrap_or(0),
                text: synced.text,
            }),
        }
    }

    /// Waits up to [`LOADING`], the first time a file of its language is asked about, for the server of the open file at `relative` to finish the work it reports, such as loading the project; at once after that.
    pub(super) fn settle(&self, relative: &str) {
        self.settle_until(relative, Instant::now() + LOADING);
    }

    /// Waits until `deadline` at the latest, the first time a file of its language is asked about, for the server of the open file at `relative` to finish the work it reports, such as loading the project, giving a server that has reported none yet [`BEGIN`] to begin some; at once after that.
    pub(super) fn settle_until(&self, relative: &str, deadline: Instant) {
        let language = pm_lsp::language_of(relative).unwrap_or_default();
        let is_loaded = || {
            self.loaded
                .lock()
                .is_ok_and(|loaded| loaded.contains(&language))
        };
        if is_loaded() {
            return;
        }
        let started = Instant::now();
        let mut quiet_since: Option<Instant> = None;
        while Instant::now() < deadline {
            let now = Instant::now();
            match self.hub.load(&self.root, relative) {
                None => return,
                Some(Load::Working) => quiet_since = None,
                Some(Load::NotBegun) if started.elapsed() < BEGIN => {}
                Some(Load::NotBegun | Load::Idle) => {
                    let since = *quiet_since.get_or_insert(now);
                    if now.duration_since(since) >= QUIET {
                        if let Ok(mut loaded) = self.loaded.lock() {
                            loaded.insert(language);
                        }
                        return;
                    }
                }
            }
            std::thread::sleep(LOOK_AGAIN);
        }
    }

    /// The places `seek` finds from `at`, asked of the server of its file when it answers `provider`, which the harness calls `what`.
    pub(super) fn locate(
        &self,
        at: &Place,
        seek: Seek,
        provider: &str,
        what: &str,
    ) -> LanguageAnswer {
        let ready = match self.ready(&at.path, provider, what) {
            Ok(ready) => ready,
            Err(answer) => return answer,
        };
        let position = position_of(&ready.text, at.line, at.column);
        let found =
            match self
                .hub
                .locate(&self.root, &ready.relative, ready.version, position, seek)
            {
                Ok(found) => found,
                Err(reason) => return LanguageAnswer::Failed { reason },
            };
        let mut texts = Texts::default();
        texts.0.insert(at.path.clone(), ready.text);
        let locations = found
            .into_iter()
            .map(|location| {
                let file = if location.inside {
                    self.root.join(&location.path)
                } else {
                    PathBuf::from(&location.path)
                };
                texts.stretch(&file, location.span)
            })
            .collect();
        LanguageAnswer::Locations { locations }
    }

    /// What the server of the file at `at` says the symbol there is.
    pub(super) fn hover(&self, at: &Place) -> LanguageAnswer {
        let ready = match self.ready(&at.path, "hoverProvider", "hovers") {
            Ok(ready) => ready,
            Err(answer) => return answer,
        };
        let position = position_of(&ready.text, at.line, at.column);
        match self
            .hub
            .hover(&self.root, &ready.relative, ready.version, position)
        {
            Ok(hover) => LanguageAnswer::Hover {
                text: hover.map(|hover| hover.text),
            },
            Err(reason) => LanguageAnswer::Failed { reason },
        }
    }

    /// The symbols the server of the file at `path` names in it.
    pub(super) fn document_symbols(&self, path: &Path) -> LanguageAnswer {
        let ready = match self.ready(path, "documentSymbolProvider", "outlines of files") {
            Ok(ready) => ready,
            Err(answer) => return answer,
        };
        match self
            .hub
            .document_symbols(&self.root, &ready.relative, ready.version)
        {
            Ok(found) => {
                let mut texts = Texts::default();
                texts.0.insert(path.to_path_buf(), ready.text);
                LanguageAnswer::Symbols {
                    symbols: found
                        .into_iter()
                        .map(|outline| texts.symbol(outline))
                        .collect(),
                }
            }
            Err(reason) => LanguageAnswer::Failed { reason },
        }
    }

    /// The project's symbols whose names match `query`, from the server of the file at `near` when it is given, otherwise from the server of each language found in the project.
    pub(super) fn workspace_symbols(&self, query: &str, near: Option<&Path>) -> LanguageAnswer {
        let files = match near {
            Some(path) => vec![path.to_path_buf()],
            None => self.samples(),
        };
        if files.is_empty() {
            return LanguageAnswer::NoServer {
                reason: "no language server runs for this project; name a file to ask the server of its language".to_owned(),
            };
        }
        let mut texts = Texts::default();
        let mut symbols = Vec::new();
        let mut refused = None;
        for file in files {
            let ready = match self.ready(
                &file,
                "workspaceSymbolProvider",
                "searches of the project's symbols",
            ) {
                Ok(ready) => ready,
                Err(answer) => {
                    refused.get_or_insert(answer);
                    continue;
                }
            };
            match self.hub.project_symbols(&self.root, &ready.relative, query) {
                Ok(found) => symbols.extend(found.into_iter().map(|outline| texts.symbol(outline))),
                Err(reason) => {
                    refused.get_or_insert(LanguageAnswer::Failed { reason });
                }
            }
        }
        match refused {
            Some(answer) if symbols.is_empty() => answer,
            _ => LanguageAnswer::Symbols { symbols },
        }
    }

    /// Everything the servers have said is wrong with the project's files, having started the servers of the languages found in it and waited up to `wait` for those still working, such as loading or checking it.
    pub(super) fn workspace_diagnostics(&self, wait: Duration) -> LanguageAnswer {
        let deadline = Instant::now() + wait;
        self.warm();
        let opened: Vec<String> = self
            .opened
            .lock()
            .map(|opened| opened.keys().cloned().collect())
            .unwrap_or_default();
        if opened.is_empty() {
            return LanguageAnswer::NoServer {
                reason: "no language server runs for this project".to_owned(),
            };
        }
        while Instant::now() < deadline
            && opened
                .iter()
                .any(|relative| self.hub.load(&self.root, relative) == Some(Load::Working))
        {
            std::thread::sleep(LOOK_AGAIN.min(deadline - Instant::now()));
        }
        let mut texts = Texts::default();
        let diagnostics = self
            .hub
            .problems(&self.root)
            .into_iter()
            .map(|problem| {
                let file = self.root.join(&problem.path);
                let text = texts.of(&file).to_owned();
                convert(&problem.diagnostic, &file, &text)
            })
            .collect();
        LanguageAnswer::Diagnostics { diagnostics }
    }

    /// The edits renaming the symbol at `at` to `new_name`, by file, as the server of its file makes them; nothing is written.
    pub(super) fn rename(&self, at: &Place, new_name: &str) -> LanguageAnswer {
        let ready = match self.ready(&at.path, "renameProvider", "renames") {
            Ok(ready) => ready,
            Err(answer) => return answer,
        };
        let position = position_of(&ready.text, at.line, at.column);
        let changed = match self.hub.rename_edits(
            &self.root,
            &ready.relative,
            ready.version,
            position,
            new_name,
        ) {
            Ok(changed) => changed,
            Err(reason) => return LanguageAnswer::Failed { reason },
        };
        let mut texts = Texts::default();
        texts.0.insert(at.path.clone(), ready.text);
        let files = file_edits(&mut texts, changed);
        LanguageAnswer::Edits { files }
    }
}

#[cfg(test)]
/// Tests for converting places between the harness and the servers, and for answering questions about code from the app's real language servers.
mod tests {
    use std::sync::Arc;

    use omc_wire::LanguageAsk;

    use super::*;

    /// A hub with no files open whose servers are found on this program's `PATH`.
    fn hub() -> Arc<pm_lsp::Hub> {
        pm_lsp::Hub::new(
            Arc::new(|_| {}),
            Arc::new(|_: &Path, _: &str, _: &pm_lsp::ServerSpec| {
                std::env::var("PATH")
                    .map(|path| std::collections::BTreeMap::from([("PATH".to_owned(), path)]))
                    .unwrap_or_default()
            }),
        )
    }

    /// A Cargo project of two files under the workspace's git-ignored `build/`, `src/util.rs` defining `helper` and `src/lib.rs` calling it twice, and its folder.
    fn two_file_project() -> PathBuf {
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/tests/pm-harness/navigate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"navigate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            "pub mod util;\n\n/// Two ones.\npub fn total() -> u32 {\n    util::helper() + util::helper()\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/util.rs"),
            "/// Gives one.\npub fn helper() -> u32 {\n    1\n}\n",
        )
        .unwrap();
        dir.canonicalize().unwrap()
    }

    /// The app's own Rust language server answers the harness's questions about code in a project of two files: every use of a function across both, what it is, the outline of a file, and the edits renaming it, which leave the files on disk as they were; a file no language reads has no server. Skipped when Rust is not installed in the app.
    #[test]
    fn answers_questions_about_code_from_the_apps_language_servers() {
        if pm_lsp::language_of("src/lib.rs").is_none() {
            eprintln!("skipped: Rust is not installed in the app");
            return;
        }
        let dir = two_file_project();
        let (lib, util) = (dir.join("src/lib.rs"), dir.join("src/util.rs"));
        let languages = HubLanguages::new(hub(), dir.clone());
        let at = Place {
            path: util.clone(),
            line: 2,
            column: 8,
        };
        let until = Instant::now() + Duration::from_secs(120);
        let locations = loop {
            let LanguageAnswer::Locations { locations } =
                languages.answer(&LanguageAsk::References { at: at.clone() })
            else {
                panic!("not a references answer");
            };
            if locations.len() >= 3 || Instant::now() > until {
                break locations;
            }
            std::thread::sleep(Duration::from_millis(500));
        };
        let mut places: Vec<(PathBuf, u32, u32)> = locations
            .iter()
            .map(|stretch| (stretch.path.clone(), stretch.line, stretch.column))
            .collect();
        places.sort();
        assert_eq!(
            places,
            [
                (lib.clone(), 5, 11),
                (lib.clone(), 5, 28),
                (util.clone(), 2, 8)
            ]
        );
        let LanguageAnswer::Hover { text: Some(text) } =
            languages.answer(&LanguageAsk::Hover { at: at.clone() })
        else {
            panic!("no hover");
        };
        assert!(text.contains("fn helper() -> u32"), "{text}");
        let LanguageAnswer::Symbols { symbols } =
            languages.answer(&LanguageAsk::DocumentSymbols { path: lib.clone() })
        else {
            panic!("not a symbols answer");
        };
        let total = symbols
            .iter()
            .find(|symbol| symbol.name == "total")
            .unwrap();
        assert_eq!(
            (
                total.kind.as_str(),
                total.location.line,
                total.location.column
            ),
            ("function", 4, 8)
        );
        let LanguageAnswer::Edits { files } = languages.answer(&LanguageAsk::Rename {
            at,
            new_name: "assist".to_owned(),
        }) else {
            panic!("not a rename answer");
        };
        let mut edited: Vec<(PathBuf, usize)> = files
            .iter()
            .map(|file| (file.path.clone(), file.edits.len()))
            .collect();
        edited.sort();
        assert_eq!(edited, [(lib.clone(), 2), (util.clone(), 1)]);
        assert!(
            files
                .iter()
                .flat_map(|file| &file.edits)
                .all(|edit| edit.text == "assist")
        );
        assert!(
            std::fs::read_to_string(&util)
                .unwrap()
                .contains("pub fn helper()")
        );
        std::fs::write(dir.join("notes.txt"), "helper\n").unwrap();
        assert!(matches!(
            languages.answer(&LanguageAsk::References {
                at: Place {
                    path: dir.join("notes.txt"),
                    line: 1,
                    column: 1
                }
            }),
            LanguageAnswer::NoServer { .. }
        ));
    }

    /// A character outside the basic plane is one character to the harness and two code units to a server, both ways.
    #[test]
    fn converts_places_both_ways() {
        let text = "fn a() {}\nlet 🦀 = value;\n";
        assert_eq!(position_of(text, 2, 10), Position::new(1, 10));
        assert_eq!(column_of(text, 1, 10), 10);
        assert_eq!(position_of(text, 1, 4), Position::new(0, 3));
        let mut texts = Texts::default();
        texts.0.insert(PathBuf::from("/w/a.rs"), text.to_owned());
        let stretch = texts.stretch(
            Path::new("/w/a.rs"),
            Span::new(Position::new(1, 9), Position::new(1, 14)),
        );
        assert_eq!(
            (
                stretch.line,
                stretch.column,
                stretch.end_line,
                stretch.end_column
            ),
            (2, 9, 2, 14)
        );
    }
}
