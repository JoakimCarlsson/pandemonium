//! Turning parsed JSON into a [`Declaration`], refusing what is not supported.

use super::command::Command;
use super::mount::Mount;
use super::problem::Problems;
use super::resolve::Hooks;
use super::{BuildSpec, Declaration, Origin};
use crate::jsonc;
use crate::variables::{self, Place};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// Properties that do not change what runs, or that other tools own.
const INERT: [&str; 7] = [
    "$schema",
    "name",
    "hostRequirements",
    "portsAttributes",
    "otherPortsAttributes",
    "shutdownAction",
    "updateRemoteUserUID",
];

/// Properties that change what runs and are not implemented, with what to do instead.
const UNSUPPORTED: [(&str, &str); 7] = [
    (
        "dockerComposeFile",
        "Docker Compose is not supported; declare an image or a Dockerfile",
    ),
    (
        "service",
        "Docker Compose is not supported; declare an image or a Dockerfile",
    ),
    (
        "runServices",
        "Docker Compose is not supported; declare an image or a Dockerfile",
    ),
    (
        "overrideFeatureInstallOrder",
        "Dev Container Features are not supported",
    ),
    (
        "appPort",
        "publishing ports to the host is not supported; use forwardPorts",
    ),
    (
        "userEnvProbe",
        "probing the user's shell environment is not supported; set remoteEnv instead",
    ),
    ("gpuRequirements", "GPU requirements are not supported"),
];

/// Reads one declaration, collecting every problem before giving up.
pub(super) fn declaration(path: &Path, text: &str) -> Result<Declaration, Problems> {
    let mut problems = Problems::default();
    let value = match jsonc::parse(text) {
        Ok(value) => value,
        Err(error) => {
            problems.add(
                "devcontainer.json",
                format!("is not valid JSON with comments: {error}"),
            );
            return Err(problems);
        }
    };
    let Value::Object(root) = value else {
        problems.add("devcontainer.json", "must hold a JSON object");
        return Err(problems);
    };
    let reader = Reader { root: &root };
    reader.unknown(&mut problems);
    let origin = reader.origin(&mut problems);
    let hooks = reader.hooks(&mut problems);
    let declaration = Declaration {
        path: path.to_path_buf(),
        digest: crate::identity::digest(text.as_bytes()),
        name: reader.string("name", &mut problems),
        origin: origin.unwrap_or(Origin::Image(String::new())),
        workspace_mount: reader.mount("workspaceMount", &mut problems),
        workspace_folder: reader.string("workspaceFolder", &mut problems),
        mounts: reader.mounts(&mut problems),
        container_env: reader.map("containerEnv", &mut problems),
        remote_env: reader.map("remoteEnv", &mut problems),
        container_user: reader.string("containerUser", &mut problems),
        remote_user: reader.string("remoteUser", &mut problems),
        forward_ports: reader.ports(&mut problems),
        run_args: reader.list("runArgs", &mut problems),
        init: reader.flag("init", false, &mut problems),
        privileged: reader.flag("privileged", false, &mut problems),
        cap_add: reader.list("capAdd", &mut problems),
        security_opt: reader.list("securityOpt", &mut problems),
        override_command: reader.flag("overrideCommand", true, &mut problems),
        hooks,
        extensions: reader.extensions(&mut problems),
    };
    check_variables(&declaration, &mut problems);
    problems.into_result(declaration)
}

/// Checks every string that may hold a variable against the place it appears in.
fn check_variables(declaration: &Declaration, problems: &mut Problems) {
    let mut check = |field: &str, text: &str, place: Place| {
        if let Err(message) = variables::check(text, place) {
            problems.add(field, message);
        }
    };
    let creation = |check: &mut dyn FnMut(&str, &str, Place), field: &str, text: &str| {
        check(field, text, Place::Creation)
    };
    match &declaration.origin {
        Origin::Image(image) => creation(&mut check, "image", image),
        Origin::Build(build) => {
            creation(&mut check, "build.dockerfile", &build.dockerfile);
            creation(&mut check, "build.context", &build.context);
            for value in build.args.values() {
                creation(&mut check, "build.args", value);
            }
        }
    }
    for text in declaration.workspace_mount.iter().flat_map(Mount::strings) {
        check("workspaceMount", text, Place::Workspace);
    }
    if let Some(text) = &declaration.workspace_folder {
        check("workspaceFolder", text, Place::Workspace);
    }
    for mount in &declaration.mounts {
        for text in mount.strings() {
            creation(&mut check, "mounts", text);
        }
    }
    for value in declaration.container_env.values() {
        creation(&mut check, "containerEnv", value);
    }
    for text in declaration
        .container_user
        .iter()
        .chain(&declaration.run_args)
    {
        creation(&mut check, "containerUser or runArgs", text);
    }
    for value in declaration.remote_env.values() {
        check("remoteEnv", value, Place::Tools);
    }
    if let Some(text) = &declaration.remote_user {
        check("remoteUser", text, Place::Tools);
    }
    let hooks = &declaration.hooks;
    for text in hooks.initialize.iter().flat_map(Command::strings) {
        creation(&mut check, "initializeCommand", text);
    }
    for (field, command) in hooks.tools() {
        for text in command.iter().flat_map(|command| command.strings()) {
            check(field, text, Place::Tools);
        }
    }
}

/// Typed access to the members of the declaration's top-level object.
struct Reader<'a> {
    /// The declaration's members.
    root: &'a Map<String, Value>,
}

impl Reader<'_> {
    /// Reports every property that is neither supported, inert nor explicitly refused.
    fn unknown(&self, problems: &mut Problems) {
        for key in self.root.keys() {
            if let Some((_, reason)) = UNSUPPORTED.iter().find(|(name, _)| name == key) {
                problems.add(key, *reason);
            } else if !KNOWN.contains(&key.as_str()) && !INERT.contains(&key.as_str()) {
                problems.add(
                    key,
                    "is not supported; remove it or the environment would differ from the one declared",
                );
            }
        }
        match self.root.get("features") {
            Some(Value::Object(features)) if features.is_empty() => {}
            Some(Value::Null) | None => {}
            Some(_) => problems.add("features", "Dev Container Features are not supported"),
        }
    }

    /// A string property.
    fn string(&self, key: &str, problems: &mut Problems) -> Option<String> {
        match self.root.get(key)? {
            Value::String(text) => Some(text.clone()),
            Value::Null => None,
            _ => {
                problems.add(key, "must be a string");
                None
            }
        }
    }

    /// A boolean property with a default.
    fn flag(&self, key: &str, default: bool, problems: &mut Problems) -> bool {
        match self.root.get(key) {
            Some(Value::Bool(flag)) => *flag,
            Some(Value::Null) | None => default,
            Some(_) => {
                problems.add(key, "must be true or false");
                default
            }
        }
    }

    /// An array of strings.
    fn list(&self, key: &str, problems: &mut Problems) -> Vec<String> {
        match self.root.get(key) {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| match item.as_str() {
                    Some(text) => Some(text.to_owned()),
                    None => {
                        problems.add(key, "must hold only strings");
                        None
                    }
                })
                .collect(),
            Some(Value::Null) | None => Vec::new(),
            Some(_) => {
                problems.add(key, "must be an array of strings");
                Vec::new()
            }
        }
    }

    /// An object of strings; a `null` value, which would unset a variable, is refused.
    fn map(&self, key: &str, problems: &mut Problems) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        match self.root.get(key) {
            Some(Value::Object(members)) => {
                for (name, value) in members {
                    match value {
                        Value::String(text) => {
                            map.insert(name.clone(), text.clone());
                        }
                        Value::Null => problems.add(
                            format!("{key}.{name}"),
                            "unsetting a variable with null is not supported",
                        ),
                        _ => problems.add(format!("{key}.{name}"), "must be a string"),
                    }
                }
            }
            Some(Value::Null) | None => {}
            Some(_) => problems.add(key, "must be an object of strings"),
        }
        map
    }

    /// One mount property.
    fn mount(&self, key: &str, problems: &mut Problems) -> Option<Mount> {
        let value = self.root.get(key).filter(|value| !value.is_null())?;
        match Mount::parse(value) {
            Ok(mount) => Some(mount),
            Err(message) => {
                problems.add(key, message);
                None
            }
        }
    }

    /// The `mounts` array.
    fn mounts(&self, problems: &mut Problems) -> Vec<Mount> {
        match self.root.get("mounts") {
            Some(Value::Array(items)) => items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| match Mount::parse(item) {
                    Ok(mount) => Some(mount),
                    Err(message) => {
                        problems.add(format!("mounts[{index}]"), message);
                        None
                    }
                })
                .collect(),
            Some(Value::Null) | None => Vec::new(),
            Some(_) => {
                problems.add("mounts", "must be an array");
                Vec::new()
            }
        }
    }

    /// The `forwardPorts` array, refusing `service:port` entries that only Docker Compose gives meaning.
    fn ports(&self, problems: &mut Problems) -> Vec<u16> {
        let Some(Value::Array(items)) = self.root.get("forwardPorts") else {
            if !matches!(self.root.get("forwardPorts"), Some(Value::Null) | None) {
                problems.add("forwardPorts", "must be an array");
            }
            return Vec::new();
        };
        let mut ports = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let field = format!("forwardPorts[{index}]");
            let port = match item {
                Value::Number(number) => number.as_u64().and_then(|port| u16::try_from(port).ok()),
                Value::String(text) if text.chars().all(|c| c.is_ascii_digit()) => {
                    text.parse().ok()
                }
                Value::String(text) => {
                    problems.add(
                        field,
                        format!("\"{text}\" names a Compose service or host; only container ports are supported"),
                    );
                    continue;
                }
                _ => None,
            };
            match port.filter(|port| *port > 0) {
                Some(port) if !ports.contains(&port) => ports.push(port),
                Some(_) => {}
                None => problems.add(field, "must be a port number from 1 to 65535"),
            }
        }
        ports
    }

    /// The image or the build section.
    fn origin(&self, problems: &mut Problems) -> Option<Origin> {
        let image = self.string("image", problems);
        let build = match self.root.get("build") {
            Some(Value::Object(build)) => Some(build.clone()),
            Some(Value::Null) | None => None,
            Some(_) => {
                problems.add("build", "must be an object");
                None
            }
        };
        let legacy = self.string("dockerFile", problems);
        match (image, build, legacy) {
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
                problems.add("image", "cannot be combined with build or dockerFile");
                None
            }
            (Some(image), None, None) => Some(Origin::Image(image)),
            (None, build, legacy) => self.build(build, legacy, problems),
        }
    }

    /// The build section, or the legacy top-level `dockerFile` and `context`.
    fn build(
        &self,
        build: Option<Map<String, Value>>,
        legacy: Option<String>,
        problems: &mut Problems,
    ) -> Option<Origin> {
        let members = build.unwrap_or_default();
        let nested = Reader { root: &members };
        let dockerfile = nested
            .string("dockerfile", problems)
            .or_else(|| nested.string("dockerFile", problems))
            .or(legacy);
        let Some(dockerfile) = dockerfile else {
            problems.add("image", "declare an image, or a build with a dockerfile");
            return None;
        };
        for key in members.keys() {
            if ![
                "dockerfile",
                "dockerFile",
                "context",
                "args",
                "target",
                "cacheFrom",
                "options",
            ]
            .contains(&key.as_str())
            {
                problems.add(format!("build.{key}"), "is not supported");
            }
        }
        let context = nested
            .string("context", problems)
            .or_else(|| self.string("context", problems))
            .unwrap_or_else(|| ".".to_owned());
        let cache_from = match members.get("cacheFrom") {
            Some(Value::String(text)) => vec![text.clone()],
            Some(Value::Array(_)) => nested.list("cacheFrom", problems),
            _ => Vec::new(),
        };
        Some(Origin::Build(BuildSpec {
            dockerfile,
            context,
            args: nested.map("args", problems),
            target: nested.string("target", problems),
            cache_from,
            options: nested.list("options", problems),
        }))
    }

    /// The lifecycle commands.
    fn hooks(&self, problems: &mut Problems) -> Hooks {
        let mut command = |key: &str| {
            let value = self.root.get(key)?;
            match Command::parse(value) {
                Ok(command) => command,
                Err(message) => {
                    problems.add(key, message);
                    None
                }
            }
        };
        Hooks {
            initialize: command("initializeCommand"),
            on_create: command("onCreateCommand"),
            update_content: command("updateContentCommand"),
            post_create: command("postCreateCommand"),
            post_start: command("postStartCommand"),
            post_attach: command("postAttachCommand"),
        }
    }

    /// The editor extensions this project asks for; other tools' namespaces are not read.
    fn extensions(&self, problems: &mut Problems) -> Vec<String> {
        let Some(Value::Object(customizations)) = self.root.get("customizations") else {
            return Vec::new();
        };
        let Some(Value::Object(ours)) = customizations.get("pandemonium") else {
            return Vec::new();
        };
        match ours.get("extensions") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| match item.as_str() {
                    Some(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
                    _ => {
                        problems.add(
                            "customizations.pandemonium.extensions",
                            "must hold only extension identifiers",
                        );
                        None
                    }
                })
                .collect(),
            Some(Value::Null) | None => Vec::new(),
            Some(_) => {
                problems.add(
                    "customizations.pandemonium.extensions",
                    "must be an array of extension identifiers",
                );
                Vec::new()
            }
        }
    }
}

/// Properties this implementation reads.
const KNOWN: [&str; 27] = [
    "image",
    "build",
    "dockerFile",
    "context",
    "features",
    "workspaceMount",
    "workspaceFolder",
    "mounts",
    "containerEnv",
    "remoteEnv",
    "containerUser",
    "remoteUser",
    "forwardPorts",
    "runArgs",
    "init",
    "privileged",
    "capAdd",
    "securityOpt",
    "overrideCommand",
    "initializeCommand",
    "onCreateCommand",
    "updateContentCommand",
    "postCreateCommand",
    "postStartCommand",
    "postAttachCommand",
    "waitFor",
    "customizations",
];
