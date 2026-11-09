//! Answering the harness's requests to change code and to look after the servers from the app's language servers: formatting a stretch of a file, the code actions offered for a stretch and the edits applying one makes, the edits moving a file makes, telling the servers a file moved, and how the servers stand and restarting them. Every edit is handed back for the harness to write; nothing is written here.

use std::path::Path;
use std::time::{Duration, Instant};

use omc_wire::{ActionInfo, LanguageAnswer, ServerInfo, Stretch};
use pm_buffer::Span;

use super::navigation::{Texts, file_edits, position_of};
use super::{HubLanguages, LOOK_AGAIN};

/// How long a restarted server is waited for to be running again before how it stands is told as it is.
const RESTARTING: Duration = Duration::from_secs(30);

/// The span of the protocol `range` names in `text`, the text of its file.
fn span_of(text: &str, range: &Stretch) -> Span {
    Span::new(
        position_of(text, range.line, range.column),
        position_of(text, range.end_line, range.end_column),
    )
}

/// `state`, a server as the hub tells how it stands, as the harness describes it.
fn info(state: pm_lsp::ServerState) -> ServerInfo {
    ServerInfo {
        name: state.id,
        languages: state.languages,
        state: state.state,
        detail: state.detail,
    }
}

impl HubLanguages {
    /// The path of `path` from the root of the project, or the answer refusing a file outside it.
    fn relative(&self, path: &Path) -> Result<String, LanguageAnswer> {
        path.strip_prefix(&self.root)
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map_err(|_| LanguageAnswer::Failed {
                reason: format!(
                    "{} is outside the project the language servers run for",
                    path.display()
                ),
            })
    }

    /// The whole text of the file `range` names with only that stretch formatted by its server; nothing when that changes nothing.
    pub(super) fn format_range(&self, range: &Stretch) -> LanguageAnswer {
        let ready = match self.ready(
            &range.path,
            "documentRangeFormattingProvider",
            "formatting a stretch of a file",
        ) {
            Ok(ready) => ready,
            Err(answer) => return answer,
        };
        if ready.version == 0 {
            return LanguageAnswer::Failed {
                reason: format!(
                    "{} is open in the editor, whose text may differ from the file on disk; format the whole file instead",
                    ready.relative
                ),
            };
        }
        let span = span_of(&ready.text, range);
        match self
            .hub
            .format(&self.root, &ready.relative, ready.version, Some(span))
        {
            Ok(edits) => {
                let formatted = super::apply(&ready.text, ready.version, edits)
                    .filter(|formatted| *formatted != ready.text);
                LanguageAnswer::Formatted { text: formatted }
            }
            Err(reason) => LanguageAnswer::Failed { reason },
        }
    }

    /// The code actions the server of the file `range` names offers there, only those of the kinds `only` when it names any.
    pub(super) fn code_actions(&self, range: &Stretch, only: &[String]) -> LanguageAnswer {
        let ready = match self.ready(&range.path, "codeActionProvider", "code actions") {
            Ok(ready) => ready,
            Err(answer) => return answer,
        };
        let span = span_of(&ready.text, range);
        match self
            .hub
            .code_actions(&self.root, &ready.relative, ready.version, span, only)
        {
            Ok(offered) => LanguageAnswer::Actions {
                actions: offered
                    .into_iter()
                    .map(|action| ActionInfo {
                        title: action.title,
                        kind: action.kind,
                        preferred: action.preferred,
                        disabled: action.disabled,
                    })
                    .collect(),
            },
            Err(reason) => LanguageAnswer::Failed { reason },
        }
    }

    /// The edits applying the code action titled `title`, or the `index`-th counted from one, among those offered for `range` and `only`, makes, its command's among them; nothing is written.
    pub(super) fn apply_code_action(
        &self,
        range: &Stretch,
        only: &[String],
        index: u32,
        title: Option<&str>,
    ) -> LanguageAnswer {
        let ready = match self.ready(&range.path, "codeActionProvider", "code actions") {
            Ok(ready) => ready,
            Err(answer) => return answer,
        };
        let span = span_of(&ready.text, range);
        let applied = self.hub.action_edits(
            &self.root,
            &ready.relative,
            ready.version,
            span,
            only,
            usize::try_from(index).unwrap_or(usize::MAX),
            title,
        );
        match applied {
            Ok((title, changed)) => {
                let mut texts = Texts::default();
                texts.0.insert(range.path.clone(), ready.text);
                LanguageAnswer::Applied {
                    title,
                    files: file_edits(&mut texts, changed),
                }
            }
            Err(reason) => LanguageAnswer::Failed { reason },
        }
    }

    /// The edits the servers of the project ask for before the file at `from` is moved to `to`, having opened the file so that its language's server runs and waited for it to load the project the first time; none when no installed language reads the file.
    pub(super) fn will_rename_file(&self, from: &Path, to: &Path) -> LanguageAnswer {
        let (old, new) = match (self.relative(from), self.relative(to)) {
            (Ok(old), Ok(new)) => (old, new),
            (Err(answer), _) | (_, Err(answer)) => return answer,
        };
        let Some(synced) = self.sync(from) else {
            return LanguageAnswer::Edits { files: Vec::new() };
        };
        self.settle(&synced.path);
        match self.hub.rename_file_edits(&self.root, &old, &new) {
            Ok(changed) => LanguageAnswer::Edits {
                files: file_edits(&mut Texts::default(), changed),
            },
            Err(reason) => LanguageAnswer::Failed { reason },
        }
    }

    /// Tells the servers of the project that the file at `from` is now at `to`, opens it where it is now, and closes it where it was when the conversation opened it there, so that its server keeps running.
    pub(super) fn file_renamed(&self, from: &Path, to: &Path) -> LanguageAnswer {
        let (old, new) = match (self.relative(from), self.relative(to)) {
            (Ok(old), Ok(new)) => (old, new),
            (Err(answer), _) | (_, Err(answer)) => return answer,
        };
        let _ = self.sync(to);
        let held = self
            .opened
            .lock()
            .ok()
            .and_then(|mut opened| opened.remove(&old));
        if let Some(held) = held {
            self.hub.close_opening(&self.root, &old, held.opening);
        }
        self.hub.did_rename(&self.root, &old, &new);
        LanguageAnswer::Done {}
    }

    /// The language servers running for the project and how each stands.
    pub(super) fn status(&self) -> LanguageAnswer {
        LanguageAnswer::Servers {
            servers: self
                .hub
                .server_states(&self.root)
                .into_iter()
                .map(info)
                .collect(),
        }
    }

    /// Restarts the server named `server`, or the one that reads the file at `near`, or every server of the project, waiting up to [`RESTARTING`] for them to run again, and tells how they stand.
    pub(super) fn reload(&self, server: Option<&str>, near: Option<&Path>) -> LanguageAnswer {
        let ids: Vec<String> = match (server, near) {
            (Some(name), _) => vec![name.to_owned()],
            (None, Some(path)) => self
                .sync(path)
                .and_then(|synced| self.hub.server_id(&self.root, &synced.path))
                .into_iter()
                .collect(),
            (None, None) => self
                .hub
                .server_states(&self.root)
                .into_iter()
                .map(|state| state.id)
                .collect(),
        };
        let restarted = self.hub.restart(&self.root, &ids);
        if restarted.is_empty() {
            let running: Vec<String> = self
                .hub
                .server_states(&self.root)
                .into_iter()
                .map(|state| state.id)
                .collect();
            return LanguageAnswer::NoServer {
                reason: match (server, near) {
                    (Some(name), _) => format!(
                        "no language server named {name} runs for this project; running: {}",
                        if running.is_empty() {
                            "none".to_owned()
                        } else {
                            running.join(", ")
                        }
                    ),
                    (None, Some(path)) => {
                        format!("no language server runs for {}", path.display())
                    }
                    (None, None) => "no language server runs for this project".to_owned(),
                },
            };
        }
        if let Ok(mut loaded) = self.loaded.lock() {
            loaded.clear();
        }
        let deadline = Instant::now() + RESTARTING;
        loop {
            let states: Vec<pm_lsp::ServerState> = self
                .hub
                .server_states(&self.root)
                .into_iter()
                .filter(|state| restarted.contains(&state.id))
                .collect();
            let settled = states.len() == restarted.len()
                && states.iter().all(|state| state.state != "starting");
            if settled || Instant::now() >= deadline {
                return LanguageAnswer::Servers {
                    servers: states.into_iter().map(info).collect(),
                };
            }
            std::thread::sleep(LOOK_AGAIN);
        }
    }
}

#[cfg(test)]
/// Tests for answering requests to change code and look after the servers from the app's real language servers.
mod tests {
    use std::path::PathBuf;
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

    /// A Cargo project under the workspace's git-ignored `build/` whose `src/util.rs` uses `HashMap` without importing it and is declared in `src/lib.rs`, and its folder.
    fn project() -> PathBuf {
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/tests/pm-harness/actions");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"actions\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            "pub mod util;\n\npub fn count() -> usize {\n    util::names().len()\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/util.rs"),
            "pub fn names() -> HashMap<u32, u32> {\n    HashMap::new()\n}\n",
        )
        .unwrap();
        dir.canonicalize().unwrap()
    }

    /// The app's own Rust language server offers to import `HashMap` and hands back the edit doing it without writing it, answers the move of a file with the edit to its module declaration, is told the file moved, is listed running, and runs again once restarted. Skipped when Rust is not installed in the app.
    #[test]
    fn answers_code_actions_moves_and_restarts_from_the_apps_language_servers() {
        if pm_lsp::language_of("src/lib.rs").is_none() {
            eprintln!("skipped: Rust is not installed in the app");
            return;
        }
        let dir = project();
        let (lib, util) = (dir.join("src/lib.rs"), dir.join("src/util.rs"));
        let languages = HubLanguages::new(hub(), dir.clone());
        let range = Stretch {
            path: util.clone(),
            line: 2,
            column: 5,
            end_line: 2,
            end_column: 12,
        };
        let until = Instant::now() + Duration::from_secs(120);
        let (index, title) = loop {
            let LanguageAnswer::Actions { actions } = languages.answer(&LanguageAsk::CodeActions {
                range: range.clone(),
                only: Vec::new(),
            }) else {
                panic!("not an actions answer");
            };
            if let Some(found) = actions
                .iter()
                .position(|action| action.title.contains("std::collections::HashMap"))
            {
                break (found + 1, actions[found].title.clone());
            }
            assert!(Instant::now() < until, "no import offered: {actions:?}");
            std::thread::sleep(Duration::from_millis(500));
        };
        let LanguageAnswer::Applied {
            title: applied,
            files,
        } = languages.answer(&LanguageAsk::ApplyCodeAction {
            range,
            only: Vec::new(),
            index: u32::try_from(index).unwrap(),
            title: Some(title.clone()),
        })
        else {
            panic!("not an applied answer");
        };
        assert_eq!(applied, title);
        let edits = &files.iter().find(|file| file.path == util).unwrap().edits;
        assert!(
            edits
                .iter()
                .any(|edit| edit.text.contains("use std::collections::HashMap;")),
            "{edits:?}"
        );
        assert!(
            std::fs::read_to_string(&util)
                .unwrap()
                .starts_with("pub fn names()")
        );
        let helpers = dir.join("src/helpers.rs");
        let LanguageAnswer::Edits { files } = languages.answer(&LanguageAsk::WillRenameFile {
            from: util.clone(),
            to: helpers.clone(),
        }) else {
            panic!("not an edits answer");
        };
        let edits = &files
            .iter()
            .find(|file| file.path == lib)
            .expect("the move changes the module declaration")
            .edits;
        assert!(edits.iter().any(|edit| edit.text == "helpers"), "{edits:?}");
        std::fs::rename(&util, &helpers).unwrap();
        assert_eq!(
            languages.answer(&LanguageAsk::FileRenamed {
                from: util.clone(),
                to: helpers.clone()
            }),
            LanguageAnswer::Done {}
        );
        let LanguageAnswer::Servers { servers } = languages.answer(&LanguageAsk::Status {}) else {
            panic!("not a servers answer");
        };
        assert_eq!(servers.len(), 1, "{servers:?}");
        let name = servers[0].name.clone();
        assert!(
            ["running", "loading"].contains(&servers[0].state.as_str()),
            "{servers:?}"
        );
        let LanguageAnswer::Servers { servers } = languages.answer(&LanguageAsk::Reload {
            server: Some(name.clone()),
            near: None,
        }) else {
            panic!("not a servers answer");
        };
        assert_eq!(servers.len(), 1, "{servers:?}");
        assert_eq!(servers[0].name, name);
        assert!(
            ["running", "loading"].contains(&servers[0].state.as_str()),
            "{servers:?}"
        );
        assert!(matches!(
            languages.answer(&LanguageAsk::Reload {
                server: Some("no-such-server".to_owned()),
                near: None
            }),
            LanguageAnswer::NoServer { .. }
        ));
    }
}
