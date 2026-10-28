//! Where the toolchains of a language are kept, as its folder describes it, and finding the ones a machine has.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::toolchain::{SYSTEM, Toolchain};

/// What a source of toolchains looks like on disk.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// The one the machine's own path finds: the first of `programs` that runs and answers `argument`.
    #[default]
    System,
    /// One folder that holds a toolchain, called `label`.
    Folder,
    /// Every folder inside `folder` is a toolchain, called by its own name.
    Children,
    /// The folders `names` inside the project, which are never run to ask for a version.
    Project,
    /// What a program prints, one toolchain a line: the name, then what it is.
    Listed,
}

/// One place a language keeps its toolchains, written in its `config.toml`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolchainSource {
    /// What it looks like on disk.
    #[serde(default)]
    pub kind: SourceKind,
    /// The programs a toolchain is made of: the ones asked for a version, and the one a folder must hold to count.
    #[serde(default)]
    pub programs: Vec<String>,
    /// What a `system` toolchain is asked to get its version.
    #[serde(default)]
    pub argument: Option<String>,
    /// What a toolchain is called, where `{name}` stands for the folder's own name.
    #[serde(default)]
    pub label: Option<String>,
    /// The folder, for `folder` and `children`; a leading `~/` stands for the reader's home.
    #[serde(default)]
    pub folder: Option<String>,
    /// The folder inside a toolchain that holds its programs; `bin` when nothing.
    #[serde(default)]
    pub bin: Option<String>,
    /// The same, on Windows, when it differs.
    #[serde(default)]
    pub bin_windows: Option<String>,
    /// The folders of a `project` source, inside the project.
    #[serde(default)]
    pub names: Vec<String>,
    /// The program and arguments of a `listed` source.
    #[serde(default)]
    pub command: Vec<String>,
    /// What the environment holds for a toolchain, where `{id}` and `{path}` stand for its id and its folder.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// The file of a `project` folder the version is read from, without running anything.
    #[serde(default)]
    pub version_file: Option<String>,
    /// The keys of that file the version is written under.
    #[serde(default)]
    pub version_keys: Vec<String>,
    /// How that version is shown, where `{version}` stands for it.
    #[serde(default)]
    pub version_label: Option<String>,
    /// Whether it counts only when no other source of the language found anything.
    #[serde(default)]
    pub unless_found: bool,
}

/// The reader's home folder.
fn home() -> Option<PathBuf> {
    std::env::home_dir()
}

/// `path` with a leading `~/` made the reader's home.
fn expand(path: &str) -> Option<PathBuf> {
    match path.strip_prefix("~/") {
        Some(rest) => home().map(|home| home.join(rest)),
        None => Some(PathBuf::from(path)),
    }
}

/// The first line `program` prints for `argument`, when it runs and succeeds.
fn first_line(program: impl AsRef<std::ffi::OsStr>, argument: &str) -> Option<String> {
    let output = Command::new(program).arg(argument).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = if output.stdout.is_empty() {
        output.stderr
    } else {
        output.stdout
    };
    String::from_utf8_lossy(&text)
        .lines()
        .next()
        .map(|line| line.trim().to_owned())
}

/// The folders inside `folder`, by name, in order; none when it cannot be read.
fn children(folder: &Path) -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, PathBuf)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                entry.path(),
            )
        })
        .collect();
    found.sort();
    found
}

/// The name of `program` on disk on this platform.
fn on_disk(program: &str) -> String {
    if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_owned()
    }
}

/// The version `folder` says it was made with in the file and keys the source names.
fn version_of(source: &ToolchainSource, folder: &Path) -> Option<String> {
    let text = std::fs::read_to_string(folder.join(source.version_file.as_deref()?)).ok()?;
    let version = text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        source
            .version_keys
            .iter()
            .any(|held| held == key.trim())
            .then(|| value.trim().to_owned())
    })?;
    Some(match &source.version_label {
        Some(label) => label.replace("{version}", &version),
        None => version,
    })
}

impl ToolchainSource {
    /// Where the programs of a toolchain kept in `folder` are.
    fn programs_in(&self, folder: &Path) -> PathBuf {
        let bin = match (cfg!(windows), &self.bin_windows) {
            (true, Some(bin)) => bin,
            _ => self.bin.as_deref().unwrap_or("bin"),
        };
        folder.join(bin)
    }

    /// The toolchain of `family` kept in `folder`, called `name`, when it holds the program; it is run to ask for a version only when `trusted`.
    fn in_folder(
        &self,
        family: &str,
        name: String,
        folder: &Path,
        trusted: bool,
    ) -> Option<Toolchain> {
        let bin = self.programs_in(folder);
        let binary = self
            .programs
            .iter()
            .map(|program| bin.join(on_disk(program)))
            .find(|binary| binary.is_file())?;
        let detail = trusted
            .then(|| first_line(&binary, "--version").or_else(|| first_line(&binary, "version")))
            .flatten()
            .or_else(|| version_of(self, folder))
            .unwrap_or_else(|| bin.display().to_string());
        let env = crate::safe_env(
            self.env
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        value
                            .replace("{id}", &name)
                            .replace("{path}", &folder.to_string_lossy()),
                    )
                })
                .collect(),
        );
        Some(Toolchain {
            language: family.to_owned(),
            id: name.clone(),
            name,
            detail,
            path: vec![bin],
            env,
        })
    }

    /// The name a toolchain whose folder is called `name` is shown under.
    fn named(&self, name: &str) -> String {
        self.label
            .as_deref()
            .unwrap_or("{name}")
            .replace("{name}", name)
    }

    /// The toolchains of `family` this source finds on the machine, for the project in `root`.
    pub fn find(&self, family: &str, root: &Path) -> Vec<Toolchain> {
        match self.kind {
            SourceKind::System => {
                let argument = self.argument.as_deref().unwrap_or("--version");
                self.programs
                    .iter()
                    .find_map(|program| {
                        first_line(program, argument).map(|detail| Toolchain {
                            language: family.to_owned(),
                            id: SYSTEM.to_owned(),
                            name: "System".to_owned(),
                            detail,
                            ..Toolchain::default()
                        })
                    })
                    .into_iter()
                    .collect()
            }
            SourceKind::Folder => self
                .folder
                .as_deref()
                .and_then(expand)
                .and_then(|folder| {
                    let name = self.named(&folder.to_string_lossy());
                    self.in_folder(family, name, &folder, true)
                })
                .into_iter()
                .collect(),
            SourceKind::Children => self
                .folder
                .as_deref()
                .and_then(expand)
                .into_iter()
                .flat_map(|parent| children(&parent))
                .filter_map(|(name, folder)| {
                    self.in_folder(family, self.named(&name), &folder, true)
                })
                .collect(),
            SourceKind::Project => self
                .names
                .iter()
                .filter_map(|name| {
                    self.in_folder(family, self.named(name), &root.join(name), false)
                })
                .collect(),
            SourceKind::Listed => self.listed(family),
        }
    }

    /// The toolchains of `family` a `listed` source's command prints, or none when it cannot be run.
    fn listed(&self, family: &str) -> Vec<Toolchain> {
        let Some((program, arguments)) = self.command.split_first() else {
            return Vec::new();
        };
        let printed = Command::new(program)
            .args(arguments)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned());
        printed
            .iter()
            .flat_map(|printed| printed.lines())
            .filter_map(|line| {
                let (id, detail) = line.split_once(' ').unwrap_or((line, ""));
                (!id.is_empty()).then(|| Toolchain {
                    language: family.to_owned(),
                    id: id.to_owned(),
                    name: id.to_owned(),
                    detail: detail.trim().trim_matches(['(', ')']).to_owned(),
                    env: crate::safe_env(
                        self.env
                            .iter()
                            .map(|(key, value)| (key.clone(), value.replace("{id}", id)))
                            .collect(),
                    ),
                    ..Toolchain::default()
                })
            })
            .collect()
    }
}

/// The toolchains installed on this machine, and those the project in `root` keeps, found in the places each family `sources` is written for.
pub fn toolchains(root: &Path, sources: &[(String, ToolchainSource)]) -> Vec<Toolchain> {
    let mut found: Vec<Toolchain> = Vec::new();
    for (family, source) in sources {
        if source.unless_found && found.iter().any(|held| held.language == *family) {
            continue;
        }
        found.extend(source.find(family, root));
    }
    found
}
