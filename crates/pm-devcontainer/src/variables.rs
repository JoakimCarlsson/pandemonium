//! The `${…}` substitutions a declaration may use, and where each is defined.

use std::collections::BTreeMap;
use std::path::Path;

/// Which part of the declaration a string is in, which decides what it may reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Place {
    /// `workspaceMount` and `workspaceFolder`, which define the container workspace itself.
    Workspace,
    /// Everything applied before the container exists: build, mounts, container variables and user.
    Creation,
    /// Everything applied to the editor's tools: remote variables and user and lifecycle commands.
    Tools,
}

/// The values substituted into a declaration.
pub struct Scope<'a> {
    /// Reads one environment variable of the desktop.
    pub local_env: &'a dyn Fn(&str) -> Option<String>,
    /// The source folder on the desktop.
    pub local_workspace: &'a Path,
    /// The workspace folder inside the container, once known.
    pub container_workspace: Option<&'a str>,
    /// The environment the container was created with, once it exists.
    pub container_env: Option<&'a BTreeMap<String, String>>,
    /// The identity digits of the environment.
    pub id: &'a str,
}

/// One recognised reference.
enum Reference<'a> {
    /// A desktop environment variable and its default.
    LocalEnv(&'a str, Option<&'a str>),
    /// A container environment variable and its default.
    ContainerEnv(&'a str, Option<&'a str>),
    /// The source folder.
    LocalWorkspace,
    /// The last component of the source folder.
    LocalWorkspaceBasename,
    /// The workspace folder inside the container.
    ContainerWorkspace,
    /// The last component of the workspace folder inside the container.
    ContainerWorkspaceBasename,
    /// The identity digits.
    Id,
}

/// Splits `text` into literal runs and the bodies of its `${…}` references.
fn references(text: &str) -> Result<Vec<Result<&str, &str>>, String> {
    let mut parts = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        if start > 0 {
            parts.push(Ok(&rest[..start]));
        }
        let after = &rest[start + 2..];
        let end = after
            .find('}')
            .ok_or_else(|| format!("the reference starting at ${{{after} is never closed"))?;
        parts.push(Err(&after[..end]));
        rest = &after[end + 1..];
    }
    if !rest.is_empty() {
        parts.push(Ok(rest));
    }
    Ok(parts)
}

/// Names the reference a `${…}` body stands for.
fn recognise(body: &str, place: Place) -> Result<Reference<'_>, String> {
    let (name, rest) = match body.split_once(':') {
        Some((name, rest)) => (name, Some(rest)),
        None => (body, None),
    };
    let reference = match (name, rest) {
        ("localEnv" | "env", Some(rest)) => {
            let (variable, default) = rest
                .split_once(':')
                .map_or((rest, None), |(variable, default)| {
                    (variable, Some(default))
                });
            Reference::LocalEnv(variable, default)
        }
        ("containerEnv", Some(rest)) => {
            let (variable, default) = rest
                .split_once(':')
                .map_or((rest, None), |(variable, default)| {
                    (variable, Some(default))
                });
            if place != Place::Tools {
                return Err(
                    "${containerEnv:…} is only defined where tools are configured, such as remoteEnv and lifecycle commands"
                        .to_owned(),
                );
            }
            Reference::ContainerEnv(variable, default)
        }
        ("localWorkspaceFolder", None) => Reference::LocalWorkspace,
        ("localWorkspaceFolderBasename", None) => Reference::LocalWorkspaceBasename,
        ("containerWorkspaceFolder", None) | ("containerWorkspaceFolderBasename", None)
            if place == Place::Workspace =>
        {
            return Err(format!(
                "${{{name}}} cannot be used in the properties that define the workspace folder"
            ));
        }
        ("containerWorkspaceFolder", None) => Reference::ContainerWorkspace,
        ("containerWorkspaceFolderBasename", None) => Reference::ContainerWorkspaceBasename,
        ("devcontainerId", None) => Reference::Id,
        _ => return Err(format!("the variable ${{{body}}} is not supported")),
    };
    Ok(reference)
}

/// Checks every reference of `text` against `place` without substituting.
pub fn check(text: &str, place: Place) -> Result<(), String> {
    for part in references(text)? {
        if let Err(body) = part {
            recognise(body, place)?;
        }
    }
    Ok(())
}

/// Substitutes every reference of `text`, using the desktop only for `localEnv` and the source folder.
pub fn expand(text: &str, place: Place, scope: &Scope<'_>) -> Result<String, String> {
    let mut expanded = String::with_capacity(text.len());
    for part in references(text)? {
        match part {
            Ok(literal) => expanded.push_str(literal),
            Err(body) => expanded.push_str(&value(recognise(body, place)?, scope)?),
        }
    }
    Ok(expanded)
}

/// The value one reference stands for in `scope`.
fn value(reference: Reference<'_>, scope: &Scope<'_>) -> Result<String, String> {
    Ok(match reference {
        Reference::LocalEnv(name, default) => {
            (scope.local_env)(name).unwrap_or_else(|| default.unwrap_or_default().to_owned())
        }
        Reference::ContainerEnv(name, default) => {
            let environment = scope
                .container_env
                .ok_or("the container's environment is not known yet")?;
            environment
                .get(name)
                .cloned()
                .unwrap_or_else(|| default.unwrap_or_default().to_owned())
        }
        Reference::LocalWorkspace => scope.local_workspace.display().to_string(),
        Reference::LocalWorkspaceBasename => basename(&scope.local_workspace.display().to_string()),
        Reference::ContainerWorkspace => scope
            .container_workspace
            .ok_or("the container workspace folder is not known yet")?
            .to_owned(),
        Reference::ContainerWorkspaceBasename => basename(
            scope
                .container_workspace
                .ok_or("the container workspace folder is not known yet")?,
        ),
        Reference::Id => scope.id.to_owned(),
    })
}

/// The last component of a slash separated path.
fn basename(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned()
}
