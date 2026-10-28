//! The tasks of a folder: those the project writes down and those the languages' templates make of its manifests.

use std::collections::BTreeMap;
use std::path::Path;

use crate::file;
use crate::task::Task;
use crate::template::Template;
use crate::variables::{Variables, needs_place};

/// What reading a folder's tasks found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Found {
    /// The tasks, those the project writes first.
    pub tasks: Vec<Task>,
    /// Why the project's own file could not be read, when it could not.
    pub problem: Option<String>,
}

/// What reads the manifest at the path it is given and answers, for each place carrying one of the tags, what the place's captures matched; the folder's own language does the reading.
pub type Reader<'a> = &'a dyn Fn(&Path, &[String]) -> Vec<BTreeMap<String, String>>;

/// The tasks `templates` make of the project at `root`, those with the lowest `order` first; `reader` finds the places of its manifests.
pub fn detect(root: &Path, templates: &[Template], reader: Reader) -> Vec<Task> {
    let mut listed: Vec<&Template> = templates
        .iter()
        .filter(|template| template.is_listed() && template.applies(root))
        .collect();
    listed.sort_by_key(|template| template.position());
    let mut found: Vec<Task> = Vec::new();
    for template in listed {
        let base = Variables::of_project(root);
        let made: Vec<Task> = if template.from.is_empty() {
            vec![template.expand(root, &base, false)]
        } else {
            let Some(manifest) = template.from.iter().find(|name| root.join(name).is_file()) else {
                continue;
            };
            reader(&root.join(manifest), &template.tags)
                .iter()
                .map(|custom| {
                    let variables =
                        Variables::of_file(root, manifest, &template.language, custom, None);
                    template.expand(root, &variables, false)
                })
                .collect()
        };
        for task in made {
            let task = Task {
                tags: Vec::new(),
                ..task
            };
            if !found.iter().any(|held| held.name == task.name) {
                found.push(task);
            }
        }
    }
    found
}

/// Every task of `root`: those written in its own file that do not need a place in a file, then those the templates make of its manifests, leaving out one a written task has the name of.
pub fn tasks(root: &Path, templates: &[Template], reader: Reader) -> Found {
    let (written, problem) = file::read(root);
    let mut tasks: Vec<Task> = written
        .into_iter()
        .filter(|task| !needs_place(&task.command) && !needs_place(&task.name))
        .collect();
    let own = tasks.len();
    for detected in detect(root, templates, reader) {
        if !tasks[..own]
            .iter()
            .any(|task| task.is_named(&detected.name))
        {
            tasks.push(detected);
        }
    }
    Found { tasks, problem }
}

/// The tasks that can be run from a place tagged with any of `tags`, in the file at `path` of the project at `root`, whose captures are `custom`: the project's own tasks with such a tag when it has any, which replace the templates', and otherwise the templates' in the order they are written.
pub fn candidates(
    root: &Path,
    templates: &[Template],
    tags: &[String],
    variables: &Variables,
) -> Vec<Task> {
    let (written, _) = file::read(root);
    let own: Vec<Task> = written
        .into_iter()
        .filter(|task| task.tags.iter().any(|tag| tags.contains(tag)))
        .map(|task| Task {
            name: variables.put_in(&task.name, false),
            command: variables.put_in(&task.command, true),
            env: task
                .env
                .iter()
                .map(|(name, value)| (name.clone(), variables.put_in(value, false)))
                .chain(variables.all().clone())
                .collect(),
            ..task
        })
        .collect();
    if !own.is_empty() {
        return own;
    }
    templates
        .iter()
        .filter(|template| {
            template.is_tagged(tags, variables.get("PM_FILENAME").unwrap_or_default())
                && template.applies(root)
        })
        .map(|template| template.expand(root, variables, true))
        .collect()
}
