//! A language that builds on another: `inherits = "yaml"` in its `config.toml` gives it the other language's grammar, queries, editing rules and language servers wherever its own folder says nothing.

use std::path::{Path, PathBuf};

use toml::{Table, Value};

use crate::paths::language_folder;

/// The keys that say what a language is and which files are written in it; these are never taken from the language it builds on.
const OWN: [&str; 10] = [
    "name",
    "version",
    "description",
    "icon",
    "color",
    "suffixes",
    "names",
    "patterns",
    "aliases",
    "hidden",
];

/// The keys whose tables a language that writes them gives in place of the other language's, whole, since a language server is not made of parts that fit another's.
const REPLACED: [&str; 2] = ["server", "companion"];

/// The most languages deep a chain of `inherits` is followed, which stops one that names itself.
const DEPTH: usize = 4;

/// The language `table` builds on, when it does.
pub(crate) fn parent_of(table: &Table) -> Option<&str> {
    table.get("inherits").and_then(Value::as_str)
}

/// `child` laid over `parent`: what the child writes replaces what the parent has, tables are laid over one another key by key, and the keys of [`OWN`] come from the child alone.
fn laid_over(child: Table, parent: Table) -> Table {
    let mut merged: Table = parent
        .into_iter()
        .filter(|(key, _)| !OWN.contains(&key.as_str()) && key != "requires")
        .collect();
    for (key, value) in child {
        let value = match (merged.remove(&key), value) {
            (Some(Value::Table(under)), Value::Table(over))
                if !REPLACED.contains(&key.as_str()) =>
            {
                Value::Table(laid_over(over, under))
            }
            (_, value) => value,
        };
        merged.insert(key, value);
    }
    merged
}

/// What the `config.toml` in `folder` says once everything it builds on is laid under it, and the folder of the language its language servers come from when that is not `folder`.
pub(crate) fn read_table(folder: &Path, depth: usize) -> Result<(Table, Option<PathBuf>), String> {
    let text = std::fs::read_to_string(folder.join("config.toml"))
        .map_err(|error| format!("{} has no config.toml: {error}", folder.display()))?;
    let table: Table = text
        .parse()
        .map_err(|error| format!("config.toml: {error}"))?;
    let Some(parent) = parent_of(&table).map(str::to_owned) else {
        return Ok((table, None));
    };
    if depth >= DEPTH {
        return Err(format!(
            "{} builds on languages without end",
            folder.display()
        ));
    }
    let parent_folder = language_folder(&parent)?;
    let (under, under_servers) = read_table(&parent_folder, depth + 1)?;
    let own_server = table.contains_key("server") || table.contains_key("companion");
    let servers = (!own_server).then(|| under_servers.unwrap_or_else(|| parent_folder.clone()));
    Ok((laid_over(table, under), servers))
}

/// The file `name` of the language `id`: the one in its folder, or else the one in the folder of the language it builds on.
pub fn language_file(id: &str, name: &str) -> Option<PathBuf> {
    let mut folder = language_folder(id).ok()?;
    for _ in 0..DEPTH {
        let own = folder.join(name);
        if own.is_file() {
            return Some(own);
        }
        let text = std::fs::read_to_string(folder.join("config.toml")).ok()?;
        let table: Table = text.parse().ok()?;
        folder = language_folder(parent_of(&table)?).ok()?;
    }
    None
}
