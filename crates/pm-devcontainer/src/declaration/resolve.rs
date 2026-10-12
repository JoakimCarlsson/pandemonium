//! Substituting a declaration's variables for one environment.

use super::command::Command;
use super::mount::Mount;
use super::problem::Problems;
use super::{Declaration, Origin};
use crate::variables::{self, Place, Scope};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The lifecycle commands, in the order the specification runs them.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Hooks {
    /// Runs on the desktop before the environment is created or started.
    pub initialize: Option<Command>,
    /// Runs once, when the container is created.
    pub on_create: Option<Command>,
    /// Runs once after creation, and again whenever content changes.
    pub update_content: Option<Command>,
    /// Runs once after the content is in place.
    pub post_create: Option<Command>,
    /// Runs each time the container starts.
    pub post_start: Option<Command>,
    /// Runs each time the editor attaches.
    pub post_attach: Option<Command>,
}

impl Hooks {
    /// The commands that run inside the container, by property name.
    pub fn tools(&self) -> [(&'static str, &Option<Command>); 5] {
        [
            ("onCreateCommand", &self.on_create),
            ("updateContentCommand", &self.update_content),
            ("postCreateCommand", &self.post_create),
            ("postStartCommand", &self.post_start),
            ("postAttachCommand", &self.post_attach),
        ]
    }

    /// Expands the variables of the commands that run inside the container.
    fn expanded(&self, scope: &Scope<'_>, problems: &mut Problems) -> Self {
        let mut expand = |field: &str, command: &Option<Command>| {
            let command = command.as_ref()?;
            match command.map(&mut |text| variables::expand(text, Place::Tools, scope)) {
                Ok(command) => Some(command),
                Err(message) => {
                    problems.add(field, message);
                    None
                }
            }
        };
        Self {
            initialize: None,
            on_create: expand("onCreateCommand", &self.on_create),
            update_content: expand("updateContentCommand", &self.update_content),
            post_create: expand("postCreateCommand", &self.post_create),
            post_start: expand("postStartCommand", &self.post_start),
            post_attach: expand("postAttachCommand", &self.post_attach),
        }
    }
}

/// A Dockerfile build with its paths absolute and variables expanded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Build {
    /// The Dockerfile on the desktop.
    pub dockerfile: PathBuf,
    /// The build context on the desktop.
    pub context: PathBuf,
    /// Build arguments.
    pub args: BTreeMap<String, String>,
    /// The multi-stage target.
    pub target: Option<String>,
    /// Images whose layers may be reused.
    pub cache_from: Vec<String>,
    /// Further arguments for the engine's build command.
    pub options: Vec<String>,
}

/// Where the image comes from, after substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedOrigin {
    /// A named image.
    Image(String),
    /// A Dockerfile build.
    Build(Build),
}

/// Everything needed to create the container, with desktop variables substituted.
#[derive(Clone, Debug)]
pub struct Resolved {
    /// Where the image comes from.
    pub origin: ResolvedOrigin,
    /// The mount replacing the default workspace bind mount.
    pub workspace_mount: Option<Mount>,
    /// Where the source appears in the container.
    pub workspace_folder: String,
    /// The declared mounts.
    pub mounts: Vec<Mount>,
    /// The variables the container is created with.
    pub container_env: BTreeMap<String, String>,
    /// The user the container runs as.
    pub container_user: Option<String>,
    /// Arguments for the create command.
    pub run_args: Vec<String>,
    /// The desktop command to run before creating or starting.
    pub initialize: Option<Command>,
}

/// What applies to the editor's tools, once the container's own environment is known.
#[derive(Clone, Debug, Default)]
pub struct Tools {
    /// The user tools run as; none leaves the container's own.
    pub user: Option<String>,
    /// Variables added to every tool.
    pub env: Vec<(String, String)>,
    /// The lifecycle commands that run inside the container.
    pub hooks: Hooks,
}

impl Declaration {
    /// Substitutes everything that applies before the container exists.
    ///
    /// `scope.container_env` and `scope.container_workspace` are ignored here:
    /// a variable of the container is never answered from the desktop.
    pub fn resolve(&self, scope: &Scope<'_>) -> Result<Resolved, Problems> {
        let mut problems = Problems::default();
        let workspace_scope = Scope {
            container_workspace: None,
            container_env: None,
            ..*scope
        };
        let mut expand = |field: &str, text: &str, place: Place, scope: &Scope<'_>| {
            variables::expand(text, place, scope).unwrap_or_else(|message| {
                problems.add(field, message);
                String::new()
            })
        };
        let workspace_mount = self.workspace_mount.as_ref().map(|mount| {
            mount.mapped(&mut |text| {
                expand("workspaceMount", text, Place::Workspace, &workspace_scope)
            })
        });
        let workspace_folder = match &self.workspace_folder {
            Some(text) => expand("workspaceFolder", text, Place::Workspace, &workspace_scope),
            None => default_folder(scope.local_workspace),
        };
        let creation = Scope {
            container_workspace: Some(&workspace_folder),
            container_env: None,
            ..*scope
        };
        let mut creation_text =
            |field: &str, text: &str| expand(field, text, Place::Creation, &creation);
        let origin = match &self.origin {
            Origin::Image(image) => ResolvedOrigin::Image(creation_text("image", image)),
            Origin::Build(spec) => ResolvedOrigin::Build(Build {
                dockerfile: absolute(
                    self.directory(),
                    &creation_text("build.dockerfile", &spec.dockerfile),
                ),
                context: absolute(
                    self.directory(),
                    &creation_text("build.context", &spec.context),
                ),
                args: spec
                    .args
                    .iter()
                    .map(|(name, value)| (name.clone(), creation_text("build.args", value)))
                    .collect(),
                target: spec.target.clone(),
                cache_from: spec.cache_from.clone(),
                options: spec
                    .options
                    .iter()
                    .map(|text| creation_text("build.options", text))
                    .collect(),
            }),
        };
        let mounts = self
            .mounts
            .iter()
            .map(|mount| mount.mapped(&mut |text| creation_text("mounts", text)))
            .collect();
        let container_env = self
            .container_env
            .iter()
            .map(|(name, value)| (name.clone(), creation_text("containerEnv", value)))
            .collect();
        let container_user = self
            .container_user
            .as_deref()
            .map(|text| creation_text("containerUser", text));
        let run_args = self
            .run_args
            .iter()
            .map(|text| creation_text("runArgs", text))
            .collect();
        let initialize =
            self.hooks.initialize.as_ref().map(|command| {
                command.mapped(&mut |text| creation_text("initializeCommand", text))
            });
        problems.into_result(Resolved {
            origin,
            workspace_mount,
            workspace_folder,
            mounts,
            container_env,
            container_user,
            run_args,
            initialize,
        })
    }

    /// Substitutes everything that applies to the editor's tools, now that the container's environment is known.
    pub fn resolve_tools(&self, scope: &Scope<'_>) -> Result<Tools, Problems> {
        let mut problems = Problems::default();
        let mut env = Vec::new();
        for (name, value) in &self.remote_env {
            match variables::expand(value, Place::Tools, scope) {
                Ok(value) => env.push((name.clone(), value)),
                Err(message) => problems.add(format!("remoteEnv.{name}"), message),
            }
        }
        let user = self.remote_user.as_deref().and_then(|text| {
            match variables::expand(text, Place::Tools, scope) {
                Ok(user) => Some(user),
                Err(message) => {
                    problems.add("remoteUser", message);
                    None
                }
            }
        });
        let hooks = self.hooks.expanded(scope, &mut problems);
        problems.into_result(Tools { user, env, hooks })
    }
}

/// The workspace folder the specification defaults to.
pub fn default_folder(source: &Path) -> String {
    let name = source.file_name().map_or_else(
        || "workspace".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    format!("/workspaces/{name}")
}

/// Resolves `text` against `directory` unless it is already absolute.
fn absolute(directory: &Path, text: &str) -> PathBuf {
    let path = Path::new(text);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        directory.join(path)
    }
}
