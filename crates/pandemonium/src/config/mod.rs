//! The preferences on disk, and the one seam they pass through.
//!
//! Every preference the editor remembers between launches lives in one file:
//! [`load`] answers what the last launch left behind, [`save`] records what
//! this one decided. Onboarding writes through here on its first run and a
//! settings surface will edit the same file later — neither keeps a store of
//! its own. Nothing fails loudly: a missing, unreadable or outdated file is a
//! first launch, and a write that cannot land leaves the running editor alone.

mod paths;
mod stored;

use std::fs;

use crate::onboarding::Setup;
use stored::Stored;

/// The preferences the last launch left behind, or a first launch's defaults.
pub fn load() -> Setup {
    paths::settings()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_norway::from_str::<Stored>(&text).ok())
        .map(Stored::into_setup)
        .unwrap_or_default()
}

/// Writes `setup` down, ignoring a file system that will not have it.
pub fn save(setup: &Setup) {
    let Some(path) = paths::settings() else {
        return;
    };
    let Ok(text) = serde_norway::to_string(&Stored::from(setup)) else {
        return;
    };
    if let Some(directory) = path.parent()
        && fs::create_dir_all(directory).is_err()
    {
        return;
    }
    let _ = fs::write(path, text);
}
