//! A task a language ships in its `tasks.json`: a command with variables in it, tied to the places a language marks by tag or to the manifests a project holds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::condition::Condition;
use crate::task::Task;
use crate::variables::Variables;

/// The `order` a template without one has.
const MIDDLE: i32 = 100;

/// How a capture's text decides the group of a task: the capture, and for each group the texts that put a task in it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct Groups {
    /// The capture whose text is looked up.
    pub capture: String,
    /// The texts of each group, by group name.
    #[serde(flatten)]
    pub members: BTreeMap<String, Vec<String>>,
}

/// One entry of a `tasks.json`.
#[derive(Clone, Debug, Deserialize)]
pub struct Template {
    /// What the task is called, with variables.
    pub label: String,
    /// What is typed into the shell, with variables; their values are quoted for the shell.
    pub command: String,
    /// The folder it runs in, from the root of the project, with variables; empty for the root.
    #[serde(default)]
    pub folder: String,
    /// The file that marks the folder it runs in, such as `Cargo.toml`: the nearest folder above the file the task is run from that holds it; `folder` when none does.
    #[serde(default)]
    pub marker: Option<String>,
    /// What the environment holds besides what the shell starts with, with variables.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// What kind of task it is: `build`, `test`, `run`, or nothing.
    #[serde(default)]
    pub group: String,
    /// Puts a task in a group by what one of its captures matched, in place of `group`.
    #[serde(default)]
    pub groups: Option<Groups>,
    /// The language whose toolchain it runs with, by folder name.
    #[serde(default)]
    pub language: String,
    /// Where the task is said to come from, with variables.
    #[serde(default)]
    pub source: String,
    /// The tags of the places it is run from; a place tagged with any of them runs it.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The names of the files the task is run from; a place in a file with another name does not run it, and any file name does when none are written.
    #[serde(default)]
    pub files: Vec<String>,
    /// What the root of a project holds when the task is listed there; with `from`, it is also what the manifest is looked for under.
    #[serde(default)]
    pub when: Option<Condition>,
    /// The manifests at the root of a project whose places tagged like the task each give a listed task; the first that exists is read.
    #[serde(default)]
    pub from: Vec<String>,
    /// Where the task comes among those listed, the lowest first; a hundred when nothing.
    #[serde(default)]
    pub order: Option<i32>,
}

impl Template {
    /// Whether the task is listed for a project: it names a manifest to read or a condition to meet.
    pub fn is_listed(&self) -> bool {
        self.when.is_some() || !self.from.is_empty()
    }

    /// Where it comes among those listed.
    pub fn position(&self) -> i32 {
        self.order.unwrap_or(MIDDLE)
    }

    /// Whether the project at `root` meets its condition, which a template without one always does.
    pub fn applies(&self, root: &Path) -> bool {
        self.when.as_ref().is_none_or(|when| when.holds(root))
    }

    /// Whether any of its tags is among `tags`, and the file named `file` is one it is run from.
    pub fn is_tagged(&self, tags: &[String], file: &str) -> bool {
        self.tags.iter().any(|tag| tags.contains(tag))
            && (self.files.is_empty() || self.files.iter().any(|name| name == file))
    }

    /// The task it makes of `variables` in the project at `root`; the variables are also what the task's environment holds when `export`.
    pub fn expand(&self, root: &Path, variables: &Variables, export: bool) -> Task {
        let plain = |text: &str| variables.put_in(text, false);
        let mut env: BTreeMap<String, String> = self
            .env
            .iter()
            .map(|(name, value)| (name.clone(), plain(value)))
            .collect();
        if export {
            env.extend(variables.all().clone());
        }
        let group = self
            .groups
            .as_ref()
            .and_then(|groups| {
                let text =
                    variables.get(&format!("{}{}", crate::variables::CUSTOM, groups.capture))?;
                groups
                    .members
                    .iter()
                    .find(|(_, members)| members.iter().any(|member| member == text))
                    .map(|(group, _)| group.clone())
            })
            .unwrap_or_else(|| self.group.clone());
        Task {
            name: plain(&self.label),
            command: variables.put_in(&self.command, true),
            folder: self.folder_of(root, variables),
            env,
            group,
            language: self.language.clone(),
            source: plain(&self.source),
            tags: self.tags.clone(),
        }
    }

    /// The folder the task runs in, from the root: the nearest one above the file of `variables` that holds the marker, or the folder the template names.
    fn folder_of(&self, root: &Path, variables: &Variables) -> String {
        let fallback = variables.put_in(&self.folder, false);
        let (Some(marker), Some(file)) = (&self.marker, variables.get("PM_RELATIVE_FILE")) else {
            return fallback;
        };
        let mut folder = PathBuf::from(file);
        while folder.pop() {
            if root.join(&folder).join(marker).is_file() {
                return folder.to_string_lossy().replace('\\', "/");
            }
        }
        fallback
    }
}

/// The templates written in the text of a `tasks.json`, or why it cannot be read.
pub fn parse_templates(text: &str) -> Result<Vec<Template>, String> {
    serde_json::from_str(text).map_err(|error| format!("tasks.json: {error}"))
}
