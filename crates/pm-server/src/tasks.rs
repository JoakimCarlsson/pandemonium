//! The tasks of a project run on the server's machine: found from its manifests, run in a shell the client watches, or run to their end before a program is debugged, each with the toolchains the reader chose.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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

/// Every task of `root`: those written in its own file, those the installed languages declare and those the modules of the extensions that carry languages say it has.
pub(crate) fn all_tasks(root: &Path) -> pm_task::Found {
    let mut found = pm_task::tasks(root, &crate::languages::task_specs());
    for (language, package) in crate::extensions::language_owners() {
        if !crate::extensions::grants_of(&package.id).allows(pm_policy::Capability::Process) {
            continue;
        }
        let call = pm_plugin_api::LanguageCall::Tasks {
            project: crate::extensions::project_for(&package, root),
            language: language.clone(),
        };
        let Some(Ok(pm_plugin_api::LanguageReply::Tasks(items))) =
            crate::extensions::language_hook(&package, &call, Some(root))
        else {
            continue;
        };
        let family = crate::languages::family_of(&language);
        for item in items {
            let escapes = std::path::Path::new(&item.folder).components().any(|part| {
                !matches!(
                    part,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            });
            if escapes || found.tasks.iter().any(|held| held.is_named(&item.name)) {
                continue;
            }
            found.tasks.push(Task {
                name: item.name,
                command: item.command,
                folder: item.folder,
                env: BTreeMap::new(),
                group: item.group,
                language: family.clone(),
                source: item.source,
            });
        }
    }
    found
}

/// Every toolchain on this machine and in the project at `root`: those the installed languages say are found, and those the modules of the extensions that carry languages say are.
pub(crate) fn all_toolchains(root: &Path) -> Vec<Toolchain> {
    let mut found = pm_task::toolchains(root, &crate::languages::toolchain_specs());
    for (language, package) in crate::extensions::language_owners() {
        if !crate::extensions::grants_of(&package.id).allows(pm_policy::Capability::Process) {
            continue;
        }
        let call = pm_plugin_api::LanguageCall::Toolchains {
            project: crate::extensions::project_for(&package, root),
            language: language.clone(),
        };
        let Some(Ok(pm_plugin_api::LanguageReply::Toolchains(items))) =
            crate::extensions::language_hook(&package, &call, Some(root))
        else {
            continue;
        };
        let family = crate::languages::family_of(&language);
        found.extend(items.into_iter().map(|item| Toolchain {
            language: family.clone(),
            id: item.id,
            name: item.name,
            detail: item.detail,
            path: item.path.into_iter().map(PathBuf::from).collect(),
            env: item.env,
        }));
    }
    found
}

/// The toolchain of `language`, by family, chosen for the project `project` whose root is `root`, when it is installed and not the machine's own.
fn toolchain_of(root: &Path, project: &str, language: &str) -> Option<Toolchain> {
    let id = choices().get(project)?.get(language)?.clone();
    all_toolchains(root)
        .into_iter()
        .find(|toolchain| toolchain.language == language && toolchain.id == id)
        .filter(|toolchain| !toolchain.path.is_empty() || !toolchain.env.is_empty())
}

/// What a language server of `spec` is started with for the project at `root`: the environment of the toolchain chosen for its language.
pub(crate) fn server_environment(
    root: &Path,
    spec: &pm_lsp::ServerSpec,
) -> BTreeMap<String, String> {
    let family = crate::languages::family_of(&spec.language_id);
    toolchain_of(root, &project_key(root, None), &family)
        .map(|toolchain| toolchain.environment(std::env::var_os("PATH")))
        .unwrap_or_default()
}

/// The answer to asking what the folder `root` can be built, tested and run with.
pub(crate) fn listed(root: &Path) -> Response {
    let found = all_tasks(root);
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
    let toolchains = all_toolchains(root)
        .into_iter()
        .map(|toolchain| {
            let selected = match chosen.get(&toolchain.language) {
                Some(id) => *id == toolchain.id,
                None => toolchain.id == "system",
            };
            ToolchainInfo {
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
    let task = all_tasks(root)
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

/// The folder, from `root`, a runnable place in the file `path` runs in: the nearest one above the file that holds `marker`, or the root when nothing is marked or no folder holds it.
fn folder_of(root: &Path, path: &str, marker: Option<&str>) -> String {
    let Some(marker) = marker else {
        return String::new();
    };
    let mut folder = PathBuf::from(path);
    while folder.pop() {
        if root.join(&folder).join(marker).is_file() {
            return folder.to_string_lossy().replace('\\', "/");
        }
    }
    String::new()
}

/// Whether `name` is made of the characters a function or a module is named with, so that it is safe to put in a command.
fn is_plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
}

/// Whether `path` is made of the characters a path is written with, so that it is safe to put in a command.
fn is_plain_path(path: &str) -> bool {
    !path.starts_with('-')
        && path
            .chars()
            .all(|character| character.is_alphanumeric() || "_-./@+".contains(character))
}

/// The task that runs the place called `name` of kind `kind` in the file `path` of the project at `root`, as the language of the file says.
pub(crate) fn runnable(root: &Path, path: &str, kind: &str, name: &str) -> Result<Task, String> {
    if !is_plain(name) {
        return Err(format!("{name} is not a name that can be run"));
    }
    let (language, manifest) = pm_lsp::language_of(path)
        .and_then(|id| crate::languages::installed_named(&id))
        .ok_or_else(|| format!("no installed language runs {path}"))?;
    let spec = manifest
        .runnables
        .iter()
        .find(|spec| spec.kind == kind)
        .ok_or_else(|| format!("{} has no way to run a {kind}", manifest.name))?;
    let folder = folder_of(root, path, spec.marker.as_deref());
    let inside = path
        .strip_prefix(&format!("{folder}/"))
        .filter(|_| !folder.is_empty())
        .unwrap_or(path);
    let uses_path = [&spec.label, &spec.command]
        .iter()
        .any(|template| template.contains("{file}") || template.contains("{path}"));
    if uses_path && !is_plain_path(path) {
        return Err(format!("{path} is not a path that can be run"));
    }
    let file = path.rsplit('/').next().unwrap_or(path);
    let fill = |template: &str| {
        template
            .replace("{name}", name)
            .replace("{file}", file)
            .replace("{path}", inside)
    };
    Ok(Task {
        name: fill(&spec.label),
        command: fill(&spec.command),
        folder,
        env: BTreeMap::new(),
        group: if kind == "main" { "run" } else { "test" }.to_owned(),
        language: crate::languages::family_of(&language),
        source: path.to_owned(),
    })
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
