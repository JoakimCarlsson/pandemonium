//! The toolchains installed on a machine, and what starting a program with one changes.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One installed toolchain of a language: a version of its compiler or interpreter, or a project's own virtual environment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Toolchain {
    /// The language it is for, by family: `rust`, `javascript`, `python` or `go`.
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

/// The family of toolchains a file of the language `language`, by its folder name, is built and run with.
pub fn family(language: &str) -> &str {
    match language {
        "typescript" | "tsx" | "javascript" | "svelte" => "javascript",
        other => other,
    }
}

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

/// The reader's home folder.
fn home() -> Option<PathBuf> {
    std::env::home_dir()
}

/// The toolchain the machine's own path finds for `language`, when `program` runs with `argument`.
fn system(language: &str, program: &str, argument: &str) -> Option<Toolchain> {
    let version = first_line(program, argument)?;
    Some(Toolchain {
        language: language.to_owned(),
        id: SYSTEM.to_owned(),
        name: "System".to_owned(),
        detail: version,
        ..Toolchain::default()
    })
}

/// The Rust toolchains rustup keeps, or the system's when there is no rustup.
fn rust() -> Vec<Toolchain> {
    let listed = Command::new("rustup")
        .args(["toolchain", "list"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned());
    let Some(listed) = listed else {
        return system("rust", "rustc", "--version").into_iter().collect();
    };
    listed
        .lines()
        .filter_map(|line| {
            let (id, detail) = line.split_once(' ').unwrap_or((line, ""));
            (!id.is_empty()).then(|| Toolchain {
                language: "rust".to_owned(),
                id: id.to_owned(),
                name: id.to_owned(),
                detail: detail.trim().trim_matches(['(', ')']).to_owned(),
                env: BTreeMap::from([("RUSTUP_TOOLCHAIN".to_owned(), id.to_owned())]),
                ..Toolchain::default()
            })
        })
        .collect()
}

/// The toolchain of `language` whose programs are in the folder `bin`, called `name`, when it holds `program`; its program is run to ask for the version only when `trusted`, which a folder inside a project is not.
fn in_folder(
    language: &str,
    name: String,
    program: &str,
    bin: PathBuf,
    trusted: bool,
) -> Option<Toolchain> {
    let binary = bin.join(program);
    binary.is_file().then(|| Toolchain {
        language: language.to_owned(),
        id: name.clone(),
        detail: trusted
            .then(|| first_line(&binary, "--version").or_else(|| first_line(&binary, "version")))
            .flatten()
            .unwrap_or_else(|| bin.display().to_string()),
        name,
        path: vec![bin],
        ..Toolchain::default()
    })
}

/// The Node.js toolchains: the system's, and those nvm and fnm keep.
fn node() -> Vec<Toolchain> {
    let mut found: Vec<Toolchain> = system("javascript", "node", "--version")
        .into_iter()
        .collect();
    let Some(home) = home() else {
        return found;
    };
    let nvm = home.join(".nvm/versions/node");
    let fnm = home.join(".local/share/fnm/node-versions");
    found.extend(
        children(&nvm)
            .into_iter()
            .filter_map(|(name, folder)| {
                in_folder(
                    "javascript",
                    format!("nvm {name}"),
                    "node",
                    folder.join("bin"),
                    true,
                )
            })
            .chain(children(&fnm).into_iter().filter_map(|(name, folder)| {
                in_folder(
                    "javascript",
                    format!("fnm {name}"),
                    "node",
                    folder.join("installation/bin"),
                    true,
                )
            })),
    );
    found
}

/// The folder a Python environment keeps its programs in.
fn environment_bin(folder: &Path) -> PathBuf {
    if cfg!(windows) {
        folder.join("Scripts")
    } else {
        folder.join("bin")
    }
}

/// The version of Python a virtual environment was made with, read from its `pyvenv.cfg` without running anything inside the project.
fn venv_version(folder: &Path) -> Option<String> {
    let text = std::fs::read_to_string(folder.join("pyvenv.cfg")).ok()?;
    text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        matches!(key.trim(), "version" | "version_info").then(|| format!("Python {}", value.trim()))
    })
}

/// The Python toolchains: the system's, the virtual environments the project in `root` keeps, and those pyenv keeps.
fn python(root: &Path) -> Vec<Toolchain> {
    let mut found: Vec<Toolchain> = system("python", "python3", "--version")
        .or_else(|| system("python", "python", "--version"))
        .into_iter()
        .collect();
    let program = if cfg!(windows) {
        "python.exe"
    } else {
        "python"
    };
    for name in [".venv", "venv", "env"] {
        let folder = root.join(name);
        if let Some(mut toolchain) = in_folder(
            "python",
            format!("venv {name}"),
            program,
            environment_bin(&folder),
            false,
        ) {
            toolchain.detail =
                venv_version(&folder).unwrap_or_else(|| folder.display().to_string());
            toolchain
                .env
                .insert("VIRTUAL_ENV".to_owned(), folder.display().to_string());
            found.push(toolchain);
        }
    }
    if let Some(home) = home() {
        found.extend(
            children(&home.join(".pyenv/versions"))
                .into_iter()
                .filter_map(|(name, folder)| {
                    in_folder(
                        "python",
                        format!("pyenv {name}"),
                        program,
                        folder.join("bin"),
                        true,
                    )
                }),
        );
    }
    found
}

/// The Go toolchains: the system's, the one in `/usr/local/go` and those `go install golang.org/dl` keeps in `~/sdk`.
fn go() -> Vec<Toolchain> {
    let mut found: Vec<Toolchain> = system("go", "go", "version").into_iter().collect();
    let program = if cfg!(windows) { "go.exe" } else { "go" };
    found.extend(in_folder(
        "go",
        "usr/local/go".to_owned(),
        program,
        PathBuf::from("/usr/local/go/bin"),
        true,
    ));
    if let Some(home) = home() {
        found.extend(
            children(&home.join("sdk"))
                .into_iter()
                .filter_map(|(name, folder)| {
                    in_folder("go", name, program, folder.join("bin"), true)
                }),
        );
    }
    found
}

/// Every toolchain installed on this machine, and those the project in `root` keeps, grouped by language.
pub fn toolchains(root: &Path) -> Vec<Toolchain> {
    [rust(), node(), python(root), go()].concat()
}
