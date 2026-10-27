//! The toolchains installed on a machine, as the language folders say they are found, and what starting a program with one changes.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::spec::ToolchainSpec;

/// One installed toolchain of a language: a version of its compiler or interpreter, or a project's own virtual environment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Toolchain {
    /// The language it is for, by family, as its language folder names it.
    pub language: String,
    /// What it is chosen by; `system` is the one the machine's own path finds.
    pub id: String,
    /// What a reader calls it.
    pub name: String,
    /// Where it is, or which version it is.
    pub detail: String,
    /// The folders put in front of the path programs are found on.
    pub path: Vec<PathBuf>,
    /// What the environment holds besides, which names it.
    pub env: BTreeMap<String, String>,
}

/// The id of the toolchain the machine's own path finds.
const SYSTEM: &str = "system";

impl Toolchain {
    /// The environment a program is started with when this toolchain is chosen: its own variables, and the path with its folders in front of `path`.
    pub fn environment(&self, path: Option<OsString>) -> BTreeMap<String, String> {
        let mut env = self.env.clone();
        if !self.path.is_empty() {
            let rest = path.as_deref().map(std::env::split_paths);
            let joined =
                std::env::join_paths(self.path.iter().cloned().chain(rest.into_iter().flatten()));
            if let Ok(joined) = joined {
                env.insert("PATH".to_owned(), joined.to_string_lossy().into_owned());
            }
        }
        env
    }
}

/// A command that starts `program`, found on the path and in the places tools install themselves, with the path such a program finds its own tools on, since a server started from a desktop session has a short one.
fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let name = program.as_ref();
    let found = name
        .to_str()
        .and_then(pm_tools::installed)
        .unwrap_or_else(|| PathBuf::from(name));
    let mut command = Command::new(&found);
    command.env("PATH", pm_tools::path_beside(&found));
    command
}

/// The first line `program` prints for `argument`, when it runs and succeeds.
fn first_line(program: impl AsRef<std::ffi::OsStr>, argument: &str) -> Option<String> {
    let output = command(program).arg(argument).output().ok()?;
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

/// The reader's home folder.
fn home() -> Option<PathBuf> {
    std::env::home_dir()
}

/// `template` with `{id}`, `{name}` and `{folder}` replaced.
fn filled(template: &str, id: &str, name: &str, folder: &str) -> String {
    template
        .replace("{id}", id)
        .replace("{name}", name)
        .replace("{folder}", folder)
}

/// The path `place`, which may start with `~/`.
fn expanded(place: &str) -> Option<PathBuf> {
    match place.strip_prefix("~/") {
        Some(rest) => home().map(|home| home.join(rest)),
        None => Some(PathBuf::from(place)),
    }
}

/// The name of `program` on this platform.
fn program_name(program: &str) -> String {
    if cfg!(windows) && !program.contains('.') {
        format!("{program}.exe")
    } else {
        program.to_owned()
    }
}

/// The toolchain of `family` the machine's own path finds, when `program` runs with `argument`.
fn system(family: &str, program: &str, argument: &str) -> Option<Toolchain> {
    let version = first_line(program, argument)?;
    Some(Toolchain {
        language: family.to_owned(),
        id: SYSTEM.to_owned(),
        name: "System".to_owned(),
        detail: version,
        ..Toolchain::default()
    })
}

/// The toolchains `spec` lists by running a program that prints one on each line, or nothing when it cannot be run.
fn listed(family: &str, spec: &ToolchainSpec) -> Option<Vec<Toolchain>> {
    let program = spec.list.first()?;
    let output = command(program).args(&spec.list[1..]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let (id, detail) = line.split_once(' ').unwrap_or((line, ""));
                (!id.is_empty()).then(|| Toolchain {
                    language: family.to_owned(),
                    id: id.to_owned(),
                    name: id.to_owned(),
                    detail: detail.trim().trim_matches(['(', ')']).to_owned(),
                    env: spec
                        .env
                        .iter()
                        .map(|(key, value)| (key.clone(), filled(value, id, id, "")))
                        .collect(),
                    ..Toolchain::default()
                })
            })
            .collect(),
    )
}

/// The version a project folder's `file` says, from the first of `keys` it has, written after `prefix`.
fn detail_of(folder: &Path, spec: &ToolchainSpec) -> Option<String> {
    let text = std::fs::read_to_string(folder.join(spec.detail_file.as_deref()?)).ok()?;
    text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        spec.detail_keys
            .iter()
            .any(|wanted| wanted == key.trim())
            .then(|| {
                format!(
                    "{}{}",
                    spec.detail_prefix.as_deref().unwrap_or_default(),
                    value.trim()
                )
            })
    })
}

/// The toolchain of `family` in `folder`, called `name`, when one of the folders `spec` says its programs are in holds its program; its program is run to ask for the version only when `trusted`, which a folder inside a project is not.
fn in_folder(
    family: &str,
    spec: &ToolchainSpec,
    name: String,
    folder: &Path,
    trusted: bool,
) -> Option<Toolchain> {
    let program = program_name(&spec.program);
    let bins: Vec<PathBuf> = if spec.bin.is_empty() {
        vec![folder.to_path_buf()]
    } else {
        spec.bin.iter().map(|bin| folder.join(bin)).collect()
    };
    let bin = bins.into_iter().find(|bin| bin.join(&program).is_file())?;
    let binary = bin.join(&program);
    let shown = folder.display().to_string();
    let detail = if trusted {
        first_line(&binary, "--version").or_else(|| first_line(&binary, "version"))
    } else {
        detail_of(folder, spec)
    };
    Some(Toolchain {
        language: family.to_owned(),
        id: name.clone(),
        detail: detail.unwrap_or_else(|| bin.display().to_string()),
        path: vec![bin],
        env: spec
            .env
            .iter()
            .map(|(key, value)| (key.clone(), filled(value, &name, &name, &shown)))
            .collect(),
        name,
    })
}

/// The toolchains of `family` that `spec` finds on this machine, and in the project in `root`.
fn discover(root: &Path, family: &str, spec: &ToolchainSpec) -> Vec<Toolchain> {
    let mut found = listed(family, spec).unwrap_or_else(|| {
        spec.system
            .iter()
            .find_map(|asked| system(family, asked.first()?, asked.get(1)?))
            .into_iter()
            .collect()
    });
    let named = |folder_name: &str| {
        filled(
            spec.name.as_deref().unwrap_or("{name}"),
            folder_name,
            folder_name,
            "",
        )
    };
    for place in &spec.children {
        let Some(place) = expanded(place) else {
            continue;
        };
        found.extend(
            children(&place)
                .into_iter()
                .filter_map(|(name, folder)| in_folder(family, spec, named(&name), &folder, true)),
        );
    }
    for place in &spec.fixed {
        let Some(folder) = expanded(place) else {
            continue;
        };
        found.extend(in_folder(
            family,
            spec,
            named(place.trim_start_matches(['/', '~'])),
            &folder,
            true,
        ));
    }
    for name in &spec.project {
        found.extend(in_folder(
            family,
            spec,
            named(name),
            &root.join(name),
            false,
        ));
    }
    found
}

/// Every toolchain installed on this machine, and those the project in `root` keeps, as the languages' `specs` find them, each with the family it is for.
pub fn toolchains(root: &Path, specs: &[(String, ToolchainSpec)]) -> Vec<Toolchain> {
    specs
        .iter()
        .flat_map(|(family, spec)| discover(root, family, spec))
        .collect()
}
