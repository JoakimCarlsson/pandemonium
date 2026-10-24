//! Running an extension's command: its fixed prompt, or its module.

use std::path::Path;
use std::sync::Arc;

use pm_plugin_api::{CommandCall, CommandReply};
use pm_policy::{Capability, Grants};
use pm_wasm::{Limits, Sandbox};

use crate::gate::Gate;
use crate::package::Package;

/// Runs the command `command` of `package` with `argument` typed after it, in `project` when it runs in one, and returns what it comes to; the module it runs in is allowed only what `grants` say.
pub fn run_command(
    package: &Package,
    command: &str,
    argument: &str,
    project: Option<&Path>,
    grants: &Grants,
) -> Result<CommandReply, String> {
    let entry = package
        .manifest
        .commands
        .iter()
        .find(|entry| entry.id == command)
        .ok_or_else(|| format!("{} has no command {command}", package.manifest.name))?;
    if let Some(prompt) = &entry.prompt {
        return Ok(CommandReply {
            prompt: Some(prompt.replace("{argument}", argument)),
            message: None,
        });
    }
    let wasm = package.manifest.wasm.as_deref().ok_or_else(|| {
        format!(
            "{command} has no prompt and {} no module",
            package.manifest.name
        )
    })?;
    let bytes = std::fs::read(package.file(wasm)?).map_err(|error| format!("{wasm}: {error}"))?;
    let sandbox = Sandbox::new(&bytes, Limits::default())?;
    if !sandbox.exports_commands() {
        return Err(format!("{wasm} does not export what a command module must"));
    }
    let call = CommandCall {
        command: command.to_owned(),
        argument: argument.to_owned(),
        project: project
            .filter(|_| grants.allows(Capability::ReadProject))
            .map(|folder| folder.to_string_lossy().into_owned()),
    };
    let gate = Arc::new(Gate::new(grants.clone(), project.map(Path::to_path_buf)));
    let reply = sandbox.command(
        &serde_json::to_vec(&call).map_err(|error| error.to_string())?,
        gate,
    )?;
    serde_json::from_slice(&reply).map_err(|error| format!("{command} answered badly: {error}"))
}
