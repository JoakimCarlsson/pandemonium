//! The tasks of a project run on the server's machine: found from its manifests, run in a shell the client watches, or run to their end before a program is debugged, each with the toolchains the reader chose.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use pm_protocol::{Response, Task, ToolchainInfo};
use pm_task::Toolchain;

use crate::core::Core;

/// The file the choices of toolchains are kept in.
const FILE: &str = "toolchains";

/// The toolchain chosen for each language of each project, by project and language.
type Choices = BTreeMap<String, BTreeMap<String, String>>;

/// The choices as they were last saved, or none when none were.
fn choices() -> Choices {
    pm_store::load_text(FILE)
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// What a project is kept under in the choices: its folder, taken to be the one `project` names when it names one, and otherwise the repository the folder `directory` is a worktree of, or the folder itself when it is not in one.
pub(crate) fn project_key(directory: &Path, project: Option<&str>) -> String {
    let root = project
        .filter(|project| !project.is_empty())
        .map(crate::handler::place)
        .or_else(|| {
            pm_git::git_folders(directory)
                .and_then(|(_, common)| common.parent().map(Path::to_path_buf))
        })
        .unwrap_or_else(|| directory.to_path_buf());
    root.canonicalize()
        .unwrap_or(root)
        .to_string_lossy()
        .into_owned()
}

/// The toolchain of `language`, by family, chosen for the project `project` whose root is `root`, when it is installed and not the machine's own.
fn toolchain_of(root: &Path, project: &str, language: &str) -> Option<Toolchain> {
    let id = choices().get(project)?.get(language)?.clone();
    pm_task::toolchains(root)
        .into_iter()
        .find(|toolchain| toolchain.language == language && toolchain.id == id)
        .filter(|toolchain| !toolchain.path.is_empty() || !toolchain.env.is_empty())
}

/// The family of toolchains the files of the installed language `language` are built and run with: what its manifest says, or its own name.
fn family_of(language: &str) -> String {
    pm_lsp::installed_manifest(language)
        .and_then(|manifest| manifest.toolchain)
        .unwrap_or_else(|| language.to_owned())
}

/// What a reader calls the family of toolchains `family`, as the manifest of a language of it says; nothing when none does.
fn family_name(family: &str) -> String {
    pm_lsp::installed()
        .into_iter()
        .filter(|(id, manifest)| manifest.toolchain.as_deref().unwrap_or(id) == family)
        .find_map(|(_, manifest)| manifest.toolchain_name)
        .unwrap_or_default()
}

/// What a language server of the installed language `language` is started with for the project at `root`: the environment of the toolchain chosen for its family.
pub(crate) fn server_environment(root: &Path, language: &str) -> BTreeMap<String, String> {
    toolchain_of(root, &project_key(root, None), &family_of(language))
        .map(|toolchain| toolchain.environment(std::env::var_os("PATH")))
        .unwrap_or_default()
}

/// The answer to asking what the folder `root` can be built, tested and run with.
pub(crate) fn listed(root: &Path) -> Response {
    let found = pm_task::tasks(root, &crate::scan::templates(), &|path, tags| {
        crate::scan::places(path, tags)
    });
    Response::Tasks {
        tasks: found.tasks,
        problem: found.problem,
    }
}

/// The answer to asking which toolchains the project `project`, whose root is `root`, can use, and which it is set to.
pub(crate) fn toolchains(root: &Path, project: Option<&str>) -> Response {
    let chosen = choices()
        .remove(&project_key(root, project))
        .unwrap_or_default();
    let toolchains = pm_task::toolchains(root)
        .into_iter()
        .map(|toolchain| {
            let selected = match chosen.get(&toolchain.language) {
                Some(id) => *id == toolchain.id,
                None => toolchain.id == "system",
            };
            ToolchainInfo {
                language_name: family_name(&toolchain.language),
                language: toolchain.language,
                id: toolchain.id,
                name: toolchain.name,
                detail: toolchain.detail,
                selected,
            }
        })
        .collect();
    Response::Toolchains { toolchains }
}

/// Sets the toolchain of `language` for `project` to `toolchain`, or back to the machine's own path when there is none.
pub(crate) fn select(project: &str, language: &str, toolchain: Option<&str>) -> Result<(), String> {
    let key = project_key(&crate::handler::place(project), Some(project));
    let mut held = choices();
    let entry = held.entry(key.clone()).or_default();
    match toolchain {
        Some(id) => entry.insert(language.to_owned(), id.to_owned()),
        None => entry.remove(language),
    };
    if entry.is_empty() {
        held.remove(&key);
    }
    let text = serde_json::to_string(&held).map_err(|error| error.to_string())?;
    pm_store::save_text(FILE, &text).map_err(|error| error.to_string())
}

/// Runs the task of the project at `root` that is called `name` to its end, with the toolchains chosen for `project`, and says why it did not succeed when it did not.
pub(crate) fn finish_named(root: &Path, project: Option<&str>, name: &str) -> Result<(), String> {
    let task = pm_task::tasks(root, &crate::scan::templates(), &|path, tags| {
        crate::scan::places(path, tags)
    })
    .tasks
    .into_iter()
    .find(|task| task.is_named(name))
    .ok_or_else(|| {
        format!("the scenario needs the task {name}, which this project does not have")
    })?;
    let toolchain = toolchain_of(root, &project_key(root, project), &task.language);
    let launch = pm_task::Launch::new(root, &task, toolchain.as_ref())?;
    launch
        .finish()
        .map_err(|reason| format!("the task {name} failed: {reason}"))
}

/// What a place of a file that its language marks as runnable was sent as: its tags and what its captures matched, and where it is.
pub(crate) struct Place {
    /// What the language tagged it with.
    pub tags: Vec<String>,
    /// What each of its captures matched, by the capture's name.
    pub variables: BTreeMap<String, String>,
    /// Its line and column, from zero.
    pub position: Option<(usize, usize)>,
}

/// The task that runs `place` of the file `path` of the project at `root`: the first task tied to any of the place's tags, or the one called `chosen` among them.
pub(crate) fn runnable(
    root: &Path,
    path: &str,
    place: &Place,
    chosen: Option<&str>,
) -> Result<Task, String> {
    let language = pm_lsp::language_of(path).unwrap_or_default();
    let variables = pm_task::Variables::of_file(
        root,
        path,
        &language,
        &place.variables,
        place.position.map(|(row, column)| (row + 1, column + 1)),
    );
    let mut candidates =
        pm_task::candidates(root, &crate::scan::templates(), &place.tags, &variables);
    if let Some(chosen) = chosen {
        candidates.retain(|task| task.name == chosen);
    }
    if candidates.is_empty() {
        return Err(format!(
            "no task is tied to {} in {path}",
            place.tags.join(", ")
        ));
    }
    Ok(candidates.remove(0))
}

impl Core {
    /// Runs `task` of the project at `root`, which clients know by `directory`, in a shell started for `connection`, and returns the shell's number.
    pub(crate) fn run_task(
        self: &Arc<Self>,
        root: &Path,
        directory: &str,
        project: Option<&str>,
        task: &Task,
        size: (u16, u16),
    ) -> Result<u64, String> {
        let toolchain = toolchain_of(root, &project_key(root, project), &task.language);
        let launch = pm_task::Launch::new(root, task, toolchain.as_ref())?;
        self.open_shell(
            &launch.directory,
            directory,
            size.0,
            size.1,
            &launch.env,
            Some(&launch.command),
        )
    }
}
