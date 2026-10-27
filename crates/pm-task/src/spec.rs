//! What a language folder declares about the tasks of a project and the toolchains of a machine, which this crate reads without knowing any language.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where the names of a generated task are read from; each name makes a task of its own.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Each {
    /// The file the names are read from; the first of the task's `files` that exists when nothing.
    #[serde(default)]
    pub file: Option<String>,
    /// A JSON file's object, by its key path joined by dots, whose keys are the names.
    #[serde(default)]
    pub json: Option<String>,
    /// A TOML file's tables, each by its key path joined by dots, whose keys are the names.
    #[serde(default)]
    pub toml: Vec<String>,
    /// A pattern whose `name` group is a name, matched against every line of the file.
    #[serde(default)]
    pub regex: Option<String>,
}

/// A task a project has when its files say so.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TaskSpec {
    /// What it is called, with `{name}` standing for a generated task's name.
    pub name: String,
    /// What is typed into the shell to run it, with `{name}` standing for a generated task's name.
    pub command: String,
    /// What kind of task it is: `build`, `test`, `run`, or nothing.
    #[serde(default)]
    pub group: String,
    /// The kind a generated task is by its name, each kind with the names that make a task one of it.
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<String>>,
    /// The manifest shown as where it was found; the file that made it apply when nothing.
    #[serde(default)]
    pub source: Option<String>,
    /// Files of the project's folder of which at least one must exist, for the task to apply; none required when empty.
    #[serde(default)]
    pub files: Vec<String>,
    /// Files and folders that must all exist.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Files and folders none of which may exist.
    #[serde(default)]
    pub absent: Vec<String>,
    /// Pairs of a file and a pattern: each file must exist and the pattern must match its text.
    #[serde(default)]
    pub text: Vec<[String; 2]>,
    /// Pairs of a file and a pattern: the pattern must not match the text of the file.
    #[serde(default)]
    pub no_text: Vec<[String; 2]>,
    /// Where the names of a task for each of a file's entries are read from; one task when nothing.
    #[serde(default)]
    pub each: Option<Each>,
}

/// How the toolchains of a language are found on a machine.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolchainSpec {
    /// What the language is called in a list of toolchains, such as `Node.js` for the family `javascript`; the family's own name when none.
    #[serde(default)]
    pub title: Option<String>,
    /// Programs, each with the argument that makes it print its version, of which the first that runs is the machine's own toolchain; left out when `list` worked.
    #[serde(default)]
    pub system: Vec<Vec<String>>,
    /// A program with its arguments that prints one toolchain on each line, its id and then its description.
    #[serde(default)]
    pub list: Vec<String>,
    /// What the environment holds when the toolchain is chosen, where `{id}`, `{name}` and `{folder}` stand for what it is.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Folders, which may start with `~/`, whose folders inside are each a toolchain.
    #[serde(default)]
    pub children: Vec<String>,
    /// Folders, which may start with `~/`, that are each a toolchain.
    #[serde(default)]
    pub fixed: Vec<String>,
    /// Folders of the project, by name, that are each a toolchain; no program of theirs is run to ask for a version.
    #[serde(default)]
    pub project: Vec<String>,
    /// The folder inside a toolchain's folder that holds its programs, tried in turn; the toolchain's folder itself when none.
    #[serde(default)]
    pub bin: Vec<String>,
    /// The program that must be in that folder for the folder to be a toolchain.
    #[serde(default)]
    pub program: String,
    /// What the toolchain is called, where `{name}` stands for its folder's name.
    #[serde(default)]
    pub name: Option<String>,
    /// A file in a project folder that says which version it is: its lines of `key=value`.
    #[serde(default)]
    pub detail_file: Option<String>,
    /// The keys of that file that hold the version.
    #[serde(default)]
    pub detail_keys: Vec<String>,
    /// What is written before the version it holds.
    #[serde(default)]
    pub detail_prefix: Option<String>,
}
