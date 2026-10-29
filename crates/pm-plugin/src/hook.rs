//! Asking an extension's module a question while the host runs a language server or a debug adapter.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use pm_plugin_api::{HookCall, HookReply};
use pm_policy::Grants;
use pm_wasm::{Limits, Sandbox};

use crate::gate::{Gate, Status};
use crate::package::Package;

/// A compiled module and the time its file was last changed.
type Compiled = (SystemTime, Arc<Sandbox>);

/// The modules compiled so far, by file, so that a question does not compile the module again.
static MODULES: LazyLock<Mutex<HashMap<PathBuf, Compiled>>> = LazyLock::new(Default::default);

/// The compiled module in `file`, compiled again only when the file changed since.
fn compiled(file: &Path) -> Result<Arc<Sandbox>, String> {
    let changed = std::fs::metadata(file)
        .and_then(|meta| meta.modified())
        .map_err(|error| format!("{}: {error}", file.display()))?;
    if let Some((held_at, held)) = MODULES
        .lock()
        .map_err(|_| "the modules are locked".to_owned())?
        .get(file)
        && *held_at == changed
    {
        return Ok(held.clone());
    }
    let bytes = std::fs::read(file).map_err(|error| format!("{}: {error}", file.display()))?;
    let sandbox = Arc::new(Sandbox::new(&bytes, Limits::default())?);
    MODULES
        .lock()
        .map_err(|_| "the modules are locked".to_owned())?
        .insert(file.to_path_buf(), (changed, sandbox.clone()));
    Ok(sandbox)
}

/// Whether `package` has a module that answers hooks.
pub fn answers_hooks(package: &Package) -> bool {
    package
        .manifest
        .wasm
        .as_deref()
        .and_then(|wasm| package.file(wasm).ok())
        .and_then(|file| compiled(&file).ok())
        .is_some_and(|sandbox| sandbox.exports_hooks())
}

/// Asks the module of `package` the question `call`, in `project` when it is for one, the module allowed only what `grants` say and telling `status` what it is doing; returns its answer, or why it could not answer.
pub fn run_hook(
    package: &Package,
    call: &HookCall,
    project: Option<&Path>,
    grants: &Grants,
    status: Option<Status>,
) -> Result<HookReply, String> {
    let wasm = package
        .manifest
        .wasm
        .as_deref()
        .ok_or_else(|| format!("{} has no module", package.manifest.name))?;
    let sandbox = compiled(&package.file(wasm)?)?;
    if !sandbox.exports_hooks() {
        return Err(format!("{wasm} does not answer hooks"));
    }
    let gate = Arc::new(Gate::new(
        grants.clone(),
        project.map(Path::to_path_buf),
        crate::package::storage(&package.id)?,
        status,
    ));
    let reply = sandbox.hook(
        &serde_json::to_vec(call).map_err(|error| error.to_string())?,
        gate,
    )?;
    match serde_json::from_slice(&reply) {
        Ok(HookReply::Error(reason)) => Err(reason),
        Ok(answer) => Ok(answer),
        Err(error) => Err(format!("{wasm} answered badly: {error}")),
    }
}
