//! The languages the server installs for its clients, and the grammars it hands them to highlight files with.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use pm_lsp::{Index, Manifest};
use pm_protocol::{ExtensionInfo, GrammarBundle, ServerMessage};

use crate::core::Core;

/// What the server knows of installing languages.
#[derive(Default)]
pub(crate) struct Installs {
    /// The list of languages there are, as last read.
    index: Mutex<Option<Index>>,
    /// What installing each language being installed is doing now, by folder name.
    busy: Mutex<HashMap<String, String>>,
    /// Why installing each language last went wrong, by folder name.
    problems: Mutex<HashMap<String, String>>,
}

/// The installed language whose folder name is `name`, or failing that one of whose other names is, with its manifest.
pub(crate) fn installed_named(name: &str) -> Option<(String, Manifest)> {
    let name = name.trim().to_ascii_lowercase();
    let languages = pm_lsp::installed();
    languages
        .iter()
        .find(|(id, _)| *id == name)
        .or_else(|| {
            languages.iter().find(|(_, manifest)| {
                manifest.grammar.eq_ignore_ascii_case(&name)
                    || manifest
                        .aliases
                        .iter()
                        .any(|alias| alias.eq_ignore_ascii_case(&name))
            })
        })
        .cloned()
}

/// The family of toolchains the files of the installed language `language` are built and run with, which its language folder names, or the language's own folder name when it names none or the language is not installed.
pub(crate) fn family_of(language: &str) -> String {
    installed_named(language)
        .map(|(id, manifest)| manifest.family.unwrap_or(id))
        .unwrap_or_else(|| language.to_owned())
}

/// The tasks the installed languages declare, each with the family of toolchains it runs with.
pub(crate) fn task_specs() -> Vec<(String, pm_task::TaskSpec)> {
    pm_lsp::installed()
        .into_iter()
        .flat_map(|(id, manifest)| {
            let family = manifest.family.unwrap_or(id);
            manifest
                .tasks
                .into_iter()
                .map(move |spec| (family.clone(), spec))
        })
        .collect()
}

/// How the installed languages say toolchains are found, each with the family it is for; finding them runs programs, so a language an extension carries is left out unless the extension may run programs.
pub(crate) fn toolchain_specs() -> Vec<(String, pm_task::ToolchainSpec)> {
    let owners = crate::extensions::language_owners();
    pm_lsp::installed()
        .into_iter()
        .filter(|(id, _)| {
            owners.get(id).is_none_or(|package| {
                crate::extensions::grants_of(&package.id).allows(pm_policy::Capability::Process)
            })
        })
        .flat_map(|(id, manifest)| {
            let family = manifest.family.unwrap_or(id);
            manifest
                .toolchains
                .into_iter()
                .map(move |spec| (family.clone(), spec))
        })
        .collect()
}

/// The grammar and queries of the installed language `name`, found by folder name or another name.
pub(crate) fn grammar(name: &str) -> Result<GrammarBundle, String> {
    let (id, manifest) =
        installed_named(name).ok_or_else(|| format!("no installed language is called {name}"))?;
    let folder = pm_lsp::language_folder(&id)?;
    let query = |file: &str| std::fs::read_to_string(folder.join(file)).unwrap_or_default();
    Ok(GrammarBundle {
        wasm: std::fs::read(pm_lsp::grammar_file(&id)?)
            .map_err(|error| format!("could not read the grammar of {id}: {error}"))?,
        highlights: query("highlights.scm"),
        indents: query("indents.scm"),
        outline: query("outline.scm"),
        injections: query("injections.scm"),
        brackets: query("brackets.scm"),
        runnables: query("runnables.scm"),
        symbol: manifest.grammar,
        aliases: manifest.aliases,
        indent: manifest.indent,
        tabs: manifest.tabs,
        line_comment: manifest.line_comment,
        pairs: Some(manifest.pairs),
        scopes: manifest.scopes,
        tags: manifest.tags,
        id,
    })
}

/// The icon of the installed language `id`, whose manifest is `manifest`, as an SVG document, when it has one.
fn icon_of(id: &str, manifest: &Manifest) -> Option<String> {
    let file = manifest.icon.as_deref()?;
    if file.contains(['/', '\\']) || file.starts_with('.') {
        return None;
    }
    std::fs::read_to_string(pm_lsp::language_folder(id).ok()?.join(file)).ok()
}

impl Core {
    /// Every extension: those the list offers, then those installed that it does not, each with what the server knows of installing it; the list is read again when `fresh`.
    pub(crate) fn extension_infos(&self, fresh: bool) -> Vec<ExtensionInfo> {
        let installs = &self.installs;
        if fresh || installs.index.lock().is_ok_and(|held| held.is_none()) {
            let loaded = Index::load().or_else(|reason| {
                let local = Index::default().with_local();
                if local.languages.is_empty() {
                    Err(reason)
                } else {
                    Ok(local)
                }
            });
            if let (Ok(index), Ok(mut held)) = (loaded, installs.index.lock()) {
                *held = Some(index);
            }
        }
        let busy = installs
            .busy
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default();
        let problems = installs
            .problems
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default();
        let installed: HashMap<String, Manifest> = pm_lsp::installed().into_iter().collect();
        let listed = installs
            .index
            .lock()
            .ok()
            .and_then(|held| held.clone())
            .unwrap_or_default()
            .with_local()
            .languages;
        let mut infos: Vec<ExtensionInfo> = listed
            .iter()
            .map(|listing| ExtensionInfo {
                id: listing.id.clone(),
                name: listing.name.clone(),
                kind: listing.kind.clone(),
                version: listing.version.clone(),
                description: listing.description.clone(),
                icon: None,
                color: None,
                hidden: listing.hidden,
                suffixes: listing.suffixes.clone(),
                names: listing.names.clone(),
                patterns: listing.patterns.clone(),
                installed: false,
                busy: None,
                problem: None,
                server: None,
                local: listing.local,
                ..ExtensionInfo::default()
            })
            .collect();
        for (id, manifest) in &installed {
            if !infos.iter().any(|info| info.id == *id) {
                infos.push(ExtensionInfo {
                    id: id.clone(),
                    name: manifest.name.clone(),
                    kind: manifest.kind.clone(),
                    version: manifest.version.clone(),
                    description: manifest.description.clone(),
                    hidden: manifest.hidden,
                    suffixes: manifest.suffixes.clone(),
                    names: manifest.names.clone(),
                    patterns: manifest.patterns.clone(),
                    ..ExtensionInfo::default()
                });
            }
        }
        let owners = crate::extensions::language_owners();
        for package in pm_plugin::installed() {
            infos.retain(|info| info.id != package.id);
            infos.push(crate::extensions::describe(&package, true));
        }
        for info in &mut infos {
            info.owner = owners.get(&info.id).map(|package| package.id.clone());
            let mut server_ready = false;
            if let Some(manifest) = installed.get(&info.id) {
                info.installed = true;
                info.update = listed
                    .iter()
                    .find(|listing| listing.id == info.id)
                    .and_then(pm_lsp::update_available);
                info.server = manifest.server.as_ref().map(pm_lsp::server_state);
                server_ready = manifest.server.as_ref().is_none_or(pm_lsp::server_ready);
                info.icon = icon_of(&info.id, manifest);
                info.color = manifest.color.clone();
            }
            info.busy = busy.get(&info.id).cloned();
            info.problem = problems
                .get(&info.id)
                .filter(|_| !server_ready || info.update.is_some())
                .cloned();
        }
        infos.sort_by_key(|info| info.name.to_lowercase());
        infos
    }

    /// Tells every client how the extensions stand now.
    pub(crate) fn tell_extensions(&self) {
        let extensions = self.extension_infos(false);
        self.broadcast(&ServerMessage::ExtensionsChanged { extensions }, None);
    }

    /// Notes that installing `id` is doing `step`, or nothing once it is over, and tells every client.
    fn note_busy(&self, id: &str, step: Option<&str>) {
        if let Ok(mut busy) = self.installs.busy.lock() {
            match step {
                Some(step) => busy.insert(id.to_owned(), step.to_owned()),
                None => busy.remove(id),
            };
        }
        self.tell_extensions();
    }

    /// Notes that installing the extension package `id` is doing `step`, or nothing once it is over, and tells every client.
    pub(crate) fn note_busy_package(&self, id: &str, step: Option<&str>) {
        self.note_busy(id, step);
    }

    /// Notes why installing the extension package `id` went wrong, or that it did not.
    pub(crate) fn note_problem_package(&self, id: &str, problem: Option<String>) {
        self.note_problem(id, problem);
    }

    /// Notes why installing `id` went wrong, or that it did not.
    fn note_problem(&self, id: &str, problem: Option<String>) {
        if let Ok(mut problems) = self.installs.problems.lock() {
            match problem {
                Some(problem) => problems.insert(id.to_owned(), problem),
                None => problems.remove(id),
            };
        }
    }

    /// Installs the languages `ids` one after another on a thread of their own, telling every client how it goes; a language being installed already is left to that install.
    pub(crate) fn install_extensions(self: &Arc<Self>, ids: Vec<String>) {
        let wanted: Vec<String> = {
            let Ok(mut busy) = self.installs.busy.lock() else {
                return;
            };
            let fresh: Vec<String> = ids
                .into_iter()
                .filter(|id| !busy.contains_key(id))
                .collect();
            for id in &fresh {
                busy.insert(id.clone(), "Waiting to install".to_owned());
            }
            fresh
        };
        if wanted.is_empty() {
            return;
        }
        self.tell_extensions();
        let core = self.clone();
        std::thread::spawn(move || {
            for id in wanted {
                let progress = |language: &str, step: &str| {
                    if language == id {
                        core.note_busy(&id, Some(step));
                    }
                };
                let outcome = pm_lsp::install(&id, &progress);
                core.note_problem(
                    &id,
                    match outcome {
                        Ok(problems) if problems.is_empty() => None,
                        Ok(problems) => Some(problems.join("; ")),
                        Err(reason) => Some(reason),
                    },
                );
                core.note_busy(&id, None);
            }
        });
    }

    /// Takes the language `id` off this machine and tells every client.
    pub(crate) fn remove_extension(&self, id: &str) -> Result<(), String> {
        pm_lsp::remove(id)?;
        self.note_problem(id, None);
        self.tell_extensions();
        Ok(())
    }
}
