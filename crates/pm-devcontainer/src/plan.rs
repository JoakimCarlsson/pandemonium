//! From a resolved declaration to the engine's create command, and the labels that mark ownership.

use crate::declaration::{Declaration, Kind, Mount, Problems, Resolved, ResolvedOrigin};
use crate::identity::Identity;
use crate::workspace;
use std::path::Path;

/// Marks every resource this editor created.
pub const OWNER: &str = "dev.pandemonium.owner";
/// Carries the identity digits of the environment owning a resource.
pub const ENVIRONMENT: &str = "dev.pandemonium.environment";
/// Carries the project an environment belongs to.
pub const PROJECT: &str = "dev.pandemonium.project";
/// Carries the checkout an environment belongs to.
pub const CHECKOUT: &str = "dev.pandemonium.checkout";
/// Carries the digest of the declaration a container was created from.
pub const DECLARATION: &str = "dev.pandemonium.declaration";

/// The command line keeping an otherwise idle container running, so tools can be started in it.
const IDLE: &str = "trap \"exit 0\" 15; while sleep 1 & wait $!; do :; done";

/// What creating one environment's container takes.
#[derive(Clone, Debug)]
pub struct Plan {
    /// The container name.
    pub name: String,
    /// The image the container runs.
    pub image: String,
    /// The labels every resource of the environment carries.
    pub labels: Vec<String>,
    /// The volumes to create, labelled, before the container.
    pub volumes: Vec<String>,
    /// The arguments of the engine's create command, after `create`.
    pub create: Vec<String>,
    /// The folder where the source appears in the container.
    pub workspace: String,
}

/// Plans the container of `identity` from its resolved declaration.
pub fn plan(
    identity: &Identity,
    declaration: &Declaration,
    resolved: &Resolved,
) -> Result<Plan, Problems> {
    let mut problems = Problems::default();
    let id = identity.id();
    let name = identity.container_name();
    let labels = vec![
        format!("{OWNER}=pandemonium"),
        format!("{ENVIRONMENT}={id}"),
        format!("{PROJECT}={}", identity.project()),
        format!("{CHECKOUT}={}", identity.checkout().display()),
        format!("{DECLARATION}={}", declaration.digest),
    ];
    let image = match &resolved.origin {
        ResolvedOrigin::Image(image) => image.clone(),
        ResolvedOrigin::Build(_) => format!("pandemonium-devcontainer/{name}:latest"),
    };
    let mut create = vec!["--name".to_owned(), name.clone()];
    for label in &labels {
        create.extend(["--label".to_owned(), label.clone()]);
    }
    let mut volumes = Vec::new();
    for mount in mounts(identity, resolved, &mut problems) {
        match mount_argument(&id, &mount, &mut volumes) {
            Ok(argument) => create.extend(["--mount".to_owned(), argument]),
            Err(message) => problems.add("mounts", message),
        }
    }
    for (variable, value) in &resolved.container_env {
        create.extend(["-e".to_owned(), format!("{variable}={value}")]);
    }
    if let Some(user) = &resolved.container_user {
        create.extend(["-u".to_owned(), user.clone()]);
    }
    if declaration.init {
        create.push("--init".to_owned());
    }
    if declaration.privileged {
        create.push("--privileged".to_owned());
    }
    for capability in &declaration.cap_add {
        create.extend(["--cap-add".to_owned(), capability.clone()]);
    }
    for option in &declaration.security_opt {
        create.extend(["--security-opt".to_owned(), option.clone()]);
    }
    create.extend(resolved.run_args.iter().cloned());
    if declaration.override_command {
        create.extend(["--entrypoint".to_owned(), "/bin/sh".to_owned()]);
    }
    create.push(image.clone());
    if declaration.override_command {
        create.extend(["-c".to_owned(), IDLE.to_owned()]);
    }
    problems.into_result(Plan {
        name,
        image,
        labels,
        volumes,
        create,
        workspace: resolved.workspace_folder.clone(),
    })
}

/// Every mount of the container: the workspace, git's metadata, then the declared ones.
fn mounts(identity: &Identity, resolved: &Resolved, problems: &mut Problems) -> Vec<Mount> {
    let source = identity.checkout();
    let mut mounts = Vec::new();
    match &resolved.workspace_mount {
        Some(mount) => mounts.push(mount.clone()),
        None => mounts.push(Mount {
            kind: Kind::Bind,
            source: Some(source.display().to_string()),
            target: resolved.workspace_folder.clone(),
            read_only: false,
        }),
    }
    for bind in workspace::git_binds(source, &resolved.workspace_folder) {
        mounts.push(Mount {
            kind: Kind::Bind,
            source: Some(bind.source.display().to_string()),
            target: bind.target,
            read_only: false,
        });
    }
    for mount in &resolved.mounts {
        if mount.kind == Kind::Bind
            && !mount
                .source
                .as_deref()
                .is_some_and(|path| Path::new(path).is_absolute())
        {
            problems.add(
                "mounts",
                format!(
                    "the bind mount of {} needs an absolute source",
                    mount.target
                ),
            );
            continue;
        }
        mounts.push(mount.clone());
    }
    mounts
}

/// The engine's `--mount` argument for one mount, recording the volumes it asks the editor to own.
fn mount_argument(id: &str, mount: &Mount, volumes: &mut Vec<String>) -> Result<String, String> {
    for text in mount.strings() {
        if text.contains(',') || text.contains('"') {
            return Err(format!(
                "{text} cannot be mounted: paths with commas or quotes are not supported"
            ));
        }
    }
    let mut argument = match (mount.kind, &mount.source) {
        (Kind::Bind, Some(source)) => format!("type=bind,source={source},target={}", mount.target),
        (Kind::Volume, Some(source)) => {
            let owned = format!("pm-{id}-{source}");
            if !volumes.contains(&owned) {
                volumes.push(owned.clone());
            }
            format!("type=volume,source={owned},target={}", mount.target)
        }
        (Kind::Volume, None) => format!("type=volume,target={}", mount.target),
        (Kind::Tmpfs, _) => format!("type=tmpfs,target={}", mount.target),
        (Kind::Bind, None) => return Err("a bind mount needs a source".to_owned()),
    };
    if mount.read_only && mount.kind != Kind::Tmpfs {
        argument.push_str(",readonly");
    }
    Ok(argument)
}
