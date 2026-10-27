//! The lenses the server adds to those of a language server: the ones a language folder declares with a `lenses.scm` and `[[lens]]` entries, and the upgrades of the dependencies a manifest names.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pm_buffer::Buffer;
use pm_lsp::{CodeLens, FileEdit, LensSpec, TextChange};
use pm_task::Task;
use pm_treesitter::{Editing, Grammar, Queries, Syntax};

/// What pressing a lens the server added comes to.
#[derive(Clone)]
enum Held {
    /// Running a command in a shell.
    Run(Task),
    /// Making a change to the file.
    Upgrade(FileEdit),
}

/// A language's grammar with its lenses query compiled, and the lenses its folder declares.
struct Evaluator {
    /// The grammar.
    grammar: Arc<Grammar>,
    /// The lenses the folder declares.
    specs: Vec<LensSpec>,
}

/// The lenses of the open files, as last offered, and what is needed to work them out.
#[derive(Default)]
pub(crate) struct Lenses {
    /// What each lens last offered for a file does, by project and path and then by its place in the list the client was given.
    held: Mutex<HashMap<(PathBuf, String), HashMap<usize, Held>>>,
    /// The evaluators made so far, by language and version, nothing for a language that declares no lenses.
    evaluators: Mutex<HashMap<String, Option<Arc<Evaluator>>>>,
    /// What registries said of dependencies.
    registry: pm_deps::Registry,
}

/// Whether `word` is safe to put into a command typed into a shell: names of services, scripts and targets, and nothing a shell reads as more.
fn is_plain(word: &str) -> bool {
    !word.is_empty()
        && !word.starts_with('-')
        && word
            .chars()
            .all(|held| held.is_alphanumeric() || "_-.:/@+".contains(held))
}

/// The folder, from `root`, a lens of the file `path` runs in: the nearest one above the file that holds `marker`, or the folder of the file when nothing is marked or no folder holds it.
fn folder_of(root: &Path, path: &str, marker: Option<&str>) -> String {
    let mut folder = PathBuf::from(path);
    folder.pop();
    let own = folder.to_string_lossy().replace('\\', "/");
    let Some(marker) = marker else {
        return own;
    };
    while !root.join(&folder).join(marker).is_file() {
        if !folder.pop() {
            return own;
        }
    }
    folder.to_string_lossy().replace('\\', "/")
}

/// The evaluator of the installed language `language`, or nothing when it declares no lenses or its grammar does not load.
fn evaluator_of(language: &str) -> Option<Arc<Evaluator>> {
    let (id, manifest) = crate::languages::installed_named(language)?;
    if manifest.lenses.is_empty() {
        return None;
    }
    let folder = pm_lsp::language_folder(&id).ok()?;
    let lenses = std::fs::read_to_string(folder.join("lenses.scm")).ok()?;
    let wasm = std::fs::read(pm_lsp::grammar_file(&id).ok()?).ok()?;
    let grammar = Grammar::load(
        &id,
        &manifest.grammar,
        &wasm,
        &Queries {
            lenses,
            ..Queries::default()
        },
        Editing {
            indent: manifest.indent,
            tabs: manifest.tabs,
            line_comment: None,
            pairs: Vec::new(),
            scopes: Vec::new(),
            tags: None,
        },
    )
    .ok()?;
    Some(Arc::new(Evaluator {
        grammar,
        specs: manifest.lenses,
    }))
}

impl Lenses {
    /// The evaluator of `language`, made the first time it is asked for.
    fn evaluator(&self, language: &str) -> Option<Arc<Evaluator>> {
        let version = crate::languages::installed_named(language)
            .map(|(id, manifest)| format!("{id}@{}", manifest.version))?;
        let mut evaluators = self.evaluators.lock().ok()?;
        evaluators
            .entry(version)
            .or_insert_with(|| evaluator_of(language))
            .clone()
    }

    /// Whether a language folder declares lenses for the language `language`, which makes its files worth asking for lenses even when no language server reads them.
    pub(crate) fn declares(language: &str) -> bool {
        crate::languages::installed_named(language)
            .is_some_and(|(_, manifest)| !manifest.lenses.is_empty())
    }

    /// The lenses of the file `path` of the project at `root`, written in `language`, whose text is `text`: those its language folder declares, and upgrades for the dependencies it names when `registry` allows asking for them. Each is numbered from `first`, and what pressing it does is kept for [`Lenses::run`] and [`Lenses::upgrade`].
    pub(crate) fn offered(
        &self,
        root: &Path,
        path: &str,
        language: &str,
        text: &str,
        first: usize,
        registry: bool,
    ) -> Vec<CodeLens> {
        let buffer = Buffer::new(text, 0);
        let mut offered = Vec::new();
        let mut held = HashMap::new();
        let mut number = first;
        let mut add = |lens: CodeLens, what: Held| {
            offered.push(lens);
            held.insert(number, what);
            number += 1;
        };
        let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
        if let Some(evaluator) = self
            .evaluator(language)
            .filter(|evaluator| evaluator.specs.iter().any(|spec| spec.is_for(path)))
            && let Some(syntax) = Syntax::parse(evaluator.grammar.clone(), text)
        {
            let mut shown: Vec<&str> = Vec::new();
            for place in syntax.lens_places(text) {
                let Some(spec) = evaluator
                    .specs
                    .iter()
                    .filter(|spec| spec.kind == place.kind && spec.is_for(path))
                    .find(|spec| !(spec.once && shown.contains(&spec.title.as_str())))
                else {
                    continue;
                };
                let wants_name = spec.title.contains("{name}") || spec.command.contains("{name}");
                let name = place.name.as_deref().unwrap_or_default();
                if (wants_name && !is_plain(name)) || !is_plain(file) {
                    continue;
                }
                let fill =
                    |template: &str| template.replace("{name}", name).replace("{file}", file);
                let title = fill(&spec.title);
                shown.push(&spec.title);
                let task = Task {
                    name: title.clone(),
                    command: fill(&spec.command),
                    folder: folder_of(root, path, spec.marker.as_deref()),
                    env: Default::default(),
                    group: "run".to_owned(),
                    language: spec
                        .toolchain
                        .clone()
                        .map(|family| pm_task::family(&family).to_owned())
                        .unwrap_or_default(),
                    source: path.to_owned(),
                };
                add(
                    CodeLens {
                        span: buffer.span_of(place.range.start..place.range.start),
                        title,
                        pressable: true,
                        runs: true,
                    },
                    Held::Run(task),
                );
            }
        }
        if registry {
            for outdated in self.registry.outdated(path, text) {
                let span = outdated.dependency.span.clone();
                add(
                    CodeLens {
                        span: buffer.span_of(span.start..span.start),
                        title: format!("Latest {} · Upgrade", outdated.latest),
                        pressable: true,
                        runs: false,
                    },
                    Held::Upgrade(FileEdit {
                        path: path.to_owned(),
                        changes: vec![TextChange {
                            span: Some(buffer.span_of(span)),
                            text: outdated.replacement,
                        }],
                    }),
                );
            }
        }
        if let Ok(mut all) = self.held.lock() {
            let key = (root.to_path_buf(), path.to_owned());
            if held.is_empty() {
                all.remove(&key);
            } else {
                all.insert(key, held);
            }
        }
        offered
    }

    /// What the lens numbered `index` last offered for the file `path` of the project at `root` does.
    fn found(&self, root: &Path, path: &str, index: usize) -> Option<Held> {
        self.held
            .lock()
            .ok()?
            .get(&(root.to_path_buf(), path.to_owned()))?
            .get(&index)
            .cloned()
    }

    /// The task the lens numbered `index` last offered for the file `path` of the project at `root` runs, when it runs one.
    pub(crate) fn run(&self, root: &Path, path: &str, index: usize) -> Result<Task, String> {
        match self.found(root, path, index) {
            Some(Held::Run(task)) => Ok(task),
            _ => Err("that lens is no longer offered".to_owned()),
        }
    }

    /// The change the lens numbered `index` last offered for the file `path` of the project at `root` makes, when it makes one.
    pub(crate) fn upgrade(&self, root: &Path, path: &str, index: usize) -> Option<FileEdit> {
        match self.found(root, path, index)? {
            Held::Upgrade(edit) => Some(edit),
            Held::Run(_) => None,
        }
    }
}
