//! Reading the `devcontainer.json` of a folder: what its container is built from and run with.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Where a folder keeps its `devcontainer.json`, in the order they are looked for.
const LOCATIONS: [&str; 2] = [".devcontainer/devcontainer.json", ".devcontainer.json"];

/// What a container is made from.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Source {
    /// An image that is pulled as it is named.
    Image(String),
    /// An image built from a Dockerfile.
    Build {
        /// The Dockerfile.
        dockerfile: PathBuf,
        /// The folder the build reads from.
        context: PathBuf,
        /// The arguments the build is given, as `name=value`.
        arguments: Vec<String>,
        /// The stage built, when it is not the last.
        target: Option<String>,
    },
}

/// What a folder's `devcontainer.json` asks for.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Config {
    /// What the container is made from.
    pub source: Source,
    /// Where the folder is inside the container.
    pub workspace: String,
    /// The environment the container is given, as `name=value`.
    pub environment: Vec<String>,
    /// The mounts the container is given, as `docker run --mount` reads them.
    pub mounts: Vec<String>,
    /// What `docker run` is given besides.
    pub run_arguments: Vec<String>,
    /// The command run in the container once, when it is made.
    pub after_create: Option<String>,
}

/// `text` without the comments and trailing commas that a `devcontainer.json` may hold beyond plain JSON.
pub(crate) fn plain_json(text: &str) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut plain = String::with_capacity(text.len());
    let mut index = 0;
    let mut in_string = false;
    while index < characters.len() {
        let character = characters[index];
        let next = characters.get(index + 1).copied();
        if in_string {
            plain.push(character);
            if character == '\\' {
                if let Some(escaped) = next {
                    plain.push(escaped);
                    index += 1;
                }
            } else if character == '"' {
                in_string = false;
            }
        } else if character == '"' {
            in_string = true;
            plain.push(character);
        } else if character == '/' && next == Some('/') {
            while index < characters.len() && characters[index] != '\n' {
                index += 1;
            }
            continue;
        } else if character == '/' && next == Some('*') {
            index += 2;
            while index + 1 < characters.len()
                && !(characters[index] == '*' && characters[index + 1] == '/')
            {
                index += 1;
            }
            index += 2;
            continue;
        } else if character == ',' {
            let following = characters[index + 1..]
                .iter()
                .find(|character| !character.is_whitespace());
            if !matches!(following, Some('}' | ']')) {
                plain.push(character);
            }
        } else {
            plain.push(character);
        }
        index += 1;
    }
    plain
}

/// Whether `folder` holds a `devcontainer.json`.
pub(crate) fn exists(folder: &Path) -> bool {
    locate(folder).is_some()
}

/// Where the `devcontainer.json` of `folder` is, when it has one.
fn locate(folder: &Path) -> Option<PathBuf> {
    LOCATIONS
        .iter()
        .map(|location| folder.join(location))
        .find(|path| path.is_file())
}

/// The strings in the array or the single string `value`.
fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(text)) => vec![text.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

/// The `name=value` pairs in the object `value`.
fn pairs(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Object(entries)) = value else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|(name, value)| Some(format!("{name}={}", value.as_str()?)))
        .collect()
}

/// A mount of the file as `docker run --mount` reads it: a string kept as it is, an object written as its `key=value` parts.
fn mount(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Object(parts) => Some(
            parts
                .iter()
                .filter_map(|(key, value)| Some(format!("{key}={}", value.as_str()?)))
                .collect::<Vec<_>>()
                .join(","),
        ),
        _ => None,
    }
}

/// The command to run after creating the container, written as one line for a shell.
fn after_create(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Array(words) => Some(
            words
                .iter()
                .filter_map(Value::as_str)
                .map(|word| format!("'{}'", word.replace('\'', "'\\''")))
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    }
}

/// What `text`, the contents of the `devcontainer.json` that lies in `directory`, asks for for the folder `folder`.
pub(crate) fn parse(text: &str, directory: &Path, folder: &Path) -> Result<Config, String> {
    let file: Value = serde_json::from_str(&plain_json(text))
        .map_err(|error| format!("devcontainer.json is not valid: {error}"))?;
    if file.get("dockerComposeFile").is_some() {
        return Err("a dev container made with docker compose is not supported".to_owned());
    }
    let build = file.get("build");
    let dockerfile = build
        .and_then(|build| build.get("dockerfile"))
        .or_else(|| file.get("dockerFile"))
        .and_then(Value::as_str);
    let source = match (file.get("image").and_then(Value::as_str), dockerfile) {
        (Some(image), _) => Source::Image(image.to_owned()),
        (None, Some(dockerfile)) => {
            let context = build
                .and_then(|build| build.get("context"))
                .or_else(|| file.get("context"))
                .and_then(Value::as_str)
                .unwrap_or(".");
            Source::Build {
                dockerfile: directory.join(dockerfile),
                context: directory.join(context),
                arguments: pairs(build.and_then(|build| build.get("args"))),
                target: build
                    .and_then(|build| build.get("target"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            }
        }
        (None, None) => {
            return Err("devcontainer.json names neither an image nor a Dockerfile".to_owned());
        }
    };
    let name = folder.file_name().map_or_else(
        || "workspace".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    Ok(Config {
        source,
        workspace: file
            .get("workspaceFolder")
            .and_then(Value::as_str)
            .map_or_else(|| format!("/workspaces/{name}"), str::to_owned),
        environment: pairs(file.get("containerEnv")),
        mounts: file
            .get("mounts")
            .and_then(Value::as_array)
            .map(|mounts| mounts.iter().filter_map(mount).collect())
            .unwrap_or_default(),
        run_arguments: strings(file.get("runArgs")),
        after_create: after_create(file.get("postCreateCommand")),
    })
}

/// What the `devcontainer.json` of `folder` asks for, with the folder of the file the paths in it are read from.
pub(crate) fn read(folder: &Path) -> Result<Config, String> {
    let path =
        locate(folder).ok_or_else(|| format!("{} has no devcontainer.json", folder.display()))?;
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let directory = path.parent().unwrap_or(folder);
    parse(&text, directory, folder)
}
