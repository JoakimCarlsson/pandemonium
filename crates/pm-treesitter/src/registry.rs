//! The grammars loaded so far, by name and by the other names code fences call them, and those asked for that are not.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, LazyLock, Mutex};

use crate::grammar::Grammar;

/// The grammars loaded so far and the names asked for that none answers to.
#[derive(Default)]
struct Held {
    /// Each grammar, under its name and each of its other names.
    grammars: HashMap<String, Arc<Grammar>>,
    /// Names a stretch of text was written in that no grammar here answers to yet.
    wanted: BTreeSet<String>,
}

/// What this process has loaded.
static HELD: LazyLock<Mutex<Held>> = LazyLock::new(|| Mutex::new(Held::default()));

/// Keeps `grammar`, under its own name and every name of `aliases`.
pub fn add(grammar: Arc<Grammar>, aliases: &[String]) {
    let Ok(mut held) = HELD.lock() else {
        return;
    };
    for name in std::iter::once(grammar.name()).chain(aliases.iter().map(String::as_str)) {
        let name = name.to_ascii_lowercase();
        held.wanted.remove(&name);
        held.grammars.insert(name, grammar.clone());
    }
}

/// The grammar that answers to `name`, whatever its case; a name none answers to is noted as wanted.
pub fn find(name: &str) -> Option<Arc<Grammar>> {
    let name = name.trim().to_ascii_lowercase();
    let mut held = HELD.lock().ok()?;
    match held.grammars.get(&name) {
        Some(grammar) => Some(grammar.clone()),
        None => {
            if !name.is_empty() {
                held.wanted.insert(name);
            }
            None
        }
    }
}

/// The grammar that answers to `name`, if one is loaded, without noting it as wanted.
pub fn loaded(name: &str) -> Option<Arc<Grammar>> {
    HELD.lock()
        .ok()?
        .grammars
        .get(&name.to_ascii_lowercase())
        .cloned()
}

/// The names asked for since the last look that no grammar answered to, each told once.
pub fn take_wanted() -> Vec<String> {
    HELD.lock()
        .map(|mut held| std::mem::take(&mut held.wanted).into_iter().collect())
        .unwrap_or_default()
}
