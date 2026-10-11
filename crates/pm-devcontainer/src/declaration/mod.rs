//! Reading `devcontainer.json`: discovery, the supported subset, and substitution.
//!
//! The format is the Development Containers Specification. This module reads
//! the subset Pandemonium implements and says, before anything runs, which
//! other execution-affecting fields it refuses rather than ignoring.

mod command;
mod inherit;
mod mount;
mod problem;
mod read;
mod resolve;

pub use command::{Command, Single};
pub use inherit::inherit;
pub use mount::{Kind, Mount};
pub use problem::{Problem, Problems};
pub use resolve::{Build, Hooks, Resolved, ResolvedOrigin, Tools, default_folder};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where the container's image comes from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Origin {
    /// A named image, pulled when it is not already present.
    Image(String),
    /// A Dockerfile built into an image of the environment's own.
    Build(BuildSpec),
}

/// The `build` section, before its variables are expanded.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BuildSpec {
    /// The Dockerfile, relative to the declaration unless absolute.
    pub dockerfile: String,
    /// The build context, relative to the declaration unless absolute.
    pub context: String,
    /// Build arguments.
    pub args: BTreeMap<String, String>,
    /// The multi-stage target.
    pub target: Option<String>,
    /// Images whose layers may be reused.
    pub cache_from: Vec<String>,
    /// Further arguments for `docker build`.
    pub options: Vec<String>,
}

/// A parsed declaration, with variables still unexpanded.
#[derive(Clone, Debug)]
pub struct Declaration {
    /// The file this was read from.
    pub path: PathBuf,
    /// A digest of the file, to notice a declaration that changed since a container was created.
    pub digest: String,
    /// The display name.
    pub name: Option<String>,
    /// Where the image comes from.
    pub origin: Origin,
    /// `workspaceMount`, replacing the default bind mount of the source folder.
    pub workspace_mount: Option<Mount>,
    /// `workspaceFolder`, where the source appears in the container.
    pub workspace_folder: Option<String>,
    /// `mounts`.
    pub mounts: Vec<Mount>,
    /// `containerEnv`, applied to the container and so to everything in it.
    pub container_env: BTreeMap<String, String>,
    /// `remoteEnv`, applied to the editor's tools only.
    pub remote_env: BTreeMap<String, String>,
    /// `containerUser`, the user the container runs as.
    pub container_user: Option<String>,
    /// `remoteUser`, the user the editor's tools run as.
    pub remote_user: Option<String>,
    /// `forwardPorts`, container loopback ports to reach from the desktop.
    pub forward_ports: Vec<u16>,
    /// `runArgs`, passed to the engine's create command unchanged.
    pub run_args: Vec<String>,
    /// `init`.
    pub init: bool,
    /// `privileged`.
    pub privileged: bool,
    /// `capAdd`.
    pub cap_add: Vec<String>,
    /// `securityOpt`.
    pub security_opt: Vec<String>,
    /// `overrideCommand`: whether the container idles instead of running the image's command.
    pub override_command: bool,
    /// The lifecycle commands.
    pub hooks: Hooks,
    /// `customizations.pandemonium.extensions`.
    pub extensions: Vec<String>,
}

/// The conventional places a declaration lives, in the order they are tried.
const LOCATIONS: [&str; 2] = [".devcontainer/devcontainer.json", ".devcontainer.json"];

/// The declaration of the project or worktree rooted at `root`, if it has one.
pub fn discover(root: &Path) -> Option<PathBuf> {
    LOCATIONS
        .iter()
        .map(|location| root.join(location))
        .find(|path| path.is_file())
}

impl Declaration {
    /// Reads and checks the declaration at `path`.
    pub fn load(path: &Path) -> Result<Self, Problems> {
        let text = std::fs::read_to_string(path).map_err(|error| {
            Problems(vec![Problem::new(
                path.display().to_string(),
                format!("cannot be read: {error}"),
            )])
        })?;
        Self::parse(path, &text)
    }

    /// Parses `text` as the declaration at `path`, reporting every unsupported field at once.
    pub fn parse(path: &Path, text: &str) -> Result<Self, Problems> {
        read::declaration(path, text)
    }

    /// The folder holding the declaration, against which its relative paths resolve.
    pub fn directory(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("."))
    }
}
