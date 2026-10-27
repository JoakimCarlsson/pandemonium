//! Asking an extension's module about a language it carries.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use pm_plugin_api::{EXPORT_LANGUAGE, LanguageCall, LanguageReply};
use pm_policy::Grants;
use pm_wasm::{Limits, Sandbox};

use crate::gate::Gate;
use crate::package::Package;

/// A compiled module and when its file was last changed.
type Compiled = (SystemTime, Arc<Sandbox>);

/// The modules compiled so far, by file, so that a call that comes often does not compile one each time.
fn compiled() -> &'static Mutex<HashMap<PathBuf, Compiled>> {
    static COMPILED: OnceLock<Mutex<HashMap<PathBuf, Compiled>>> = OnceLock::new();
    COMPILED.get_or_init(Mutex::default)
}

/// The module of `package`, compiled, or nothing when it has none.
fn sandbox_of(package: &Package) -> Result<Option<Arc<Sandbox>>, String> {
    let Some(wasm) = package.manifest.wasm.as_deref() else {
        return Ok(None);
    };
    let file = package.file(wasm)?;
    let changed = std::fs::metadata(&file)
        .and_then(|held| held.modified())
        .map_err(|error| format!("{wasm}: {error}"))?;
    if let Some((when, held)) = compiled()
        .lock()
        .ok()
        .and_then(|all| all.get(&file).cloned())
        && when == changed
    {
        return Ok(Some(held));
    }
    let bytes = std::fs::read(&file).map_err(|error| format!("{wasm}: {error}"))?;
    let sandbox = Arc::new(Sandbox::new(&bytes, Limits::default())?);
    if let Ok(mut all) = compiled().lock() {
        all.insert(file, (changed, sandbox.clone()));
    }
    Ok(Some(sandbox))
}

/// What the module of `package` answers to `call`, which is about a language `package` carries, in `project` when it runs in one; the module is allowed only what `grants` say. A package with no module, or a module that does not answer language calls, says [`LanguageReply::Unsupported`].
pub fn run_language(
    package: &Package,
    call: &LanguageCall,
    project: Option<&Path>,
    grants: &Grants,
) -> Result<LanguageReply, String> {
    let Some(sandbox) = sandbox_of(package)? else {
        return Ok(LanguageReply::Unsupported);
    };
    if !sandbox.exports_language() {
        return Ok(LanguageReply::Unsupported);
    }
    let gate = Arc::new(Gate::new(grants.clone(), project.map(Path::to_path_buf)));
    let reply = sandbox.call(
        EXPORT_LANGUAGE,
        &serde_json::to_vec(call).map_err(|error| error.to_string())?,
        gate,
    )?;
    serde_json::from_slice(&reply).map_err(|error| {
        format!(
            "{} answered a language call badly: {error}",
            package.manifest.name
        )
    })
}
