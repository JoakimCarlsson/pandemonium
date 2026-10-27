//! `Cargo.toml`: the dependencies it lists and crates.io, which knows their versions.

use crate::dependency::Dependency;
use crate::ecosystem::{Ecosystem, file_name, quoted};

/// The Cargo ecosystem.
pub(crate) struct Cargo;

/// What the header of a table of a manifest says about the dependencies in it.
enum Table {
    /// Not a table of dependencies.
    Other,
    /// A table whose entries are dependencies.
    Entries,
    /// A table that is one dependency, by its name.
    One(String),
}

/// What the table whose header is `header`, without its brackets, holds.
fn table_of(header: &str) -> Table {
    let header = header.trim();
    let tables = ["dependencies", "dev-dependencies", "build-dependencies"];
    if tables.iter().any(|table| {
        header == *table
            || header == format!("workspace.{table}")
            || (header.starts_with("target.") && header.ends_with(&format!(".{table}")))
    }) {
        return Table::Entries;
    }
    for table in tables {
        let marker = format!("{table}.");
        if let Some(at) = header.find(&marker)
            && (at == 0 || header[..at].ends_with('.'))
        {
            return Table::One(header[at + marker.len()..].trim_matches('"').to_owned());
        }
    }
    Table::Other
}

/// The value of the quoted string after `key =` in `text`, which starts at byte `base` of the manifest, with the bytes it stands in.
fn string_after(text: &str, key: &str, base: usize) -> Option<(String, std::ops::Range<usize>)> {
    let mut from = 0;
    while let Some(found) = text[from..].find(key) {
        let at = from + found;
        let before_ok = text[..at]
            .chars()
            .next_back()
            .is_none_or(|held| !(held.is_alphanumeric() || held == '_' || held == '-'));
        let rest = text[at + key.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let start = text.len() - rest.len() + 1;
            let (value, span) = quoted(text, start)?;
            if text[start..span.start - 1].trim().is_empty() {
                return Some((value.to_owned(), base + span.start..base + span.end));
            }
        }
        from = at + key.len();
    }
    None
}

impl Ecosystem for Cargo {
    /// Its name.
    fn name(&self) -> &'static str {
        "cargo"
    }

    /// Whether the file is a `Cargo.toml`.
    fn reads(&self, path: &str) -> bool {
        file_name(path) == "Cargo.toml"
    }

    /// The dependencies written as `name = "1.0"`, `name = { version = "1.0" }`, `name.version = "1.0"` or in a table of their own; those with no version, such as a path or a workspace member, are left out.
    fn dependencies(&self, text: &str) -> Vec<Dependency> {
        let mut found = Vec::new();
        let mut table = Table::Other;
        let mut renamed: Option<String> = None;
        let mut elsewhere = false;
        let mut pending: Option<(String, Dependency)> = None;
        let mut offset = 0;
        let finish = |pending: &mut Option<(String, Dependency)>,
                      renamed: &mut Option<String>,
                      found: &mut Vec<Dependency>| {
            if let Some((_, mut dependency)) = pending.take() {
                if let Some(real) = renamed.take() {
                    dependency.name = real;
                }
                found.push(dependency);
            }
            *renamed = None;
        };
        for line in text.split_inclusive('\n') {
            let start = offset;
            offset += line.len();
            let trimmed = line.trim();
            if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
                finish(&mut pending, &mut renamed, &mut found);
                let header = trimmed
                    .trim_start_matches('[')
                    .split(']')
                    .next()
                    .unwrap_or_default();
                table = table_of(header);
                elsewhere = false;
                continue;
            }
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            match &table {
                Table::Other => {}
                Table::One(name) => {
                    if string_after(line, "registry", start).is_some()
                        || string_after(line, "git", start).is_some()
                    {
                        elsewhere = true;
                        pending = None;
                    } else if !elsewhere
                        && let Some((version, span)) = string_after(line, "version", start)
                    {
                        pending = Some((
                            name.clone(),
                            Dependency {
                                name: name.clone(),
                                requirement: version,
                                span,
                            },
                        ));
                    } else if let Some((real, _)) = string_after(line, "package", start) {
                        renamed = Some(real);
                    }
                }
                Table::Entries => {
                    let Some((key, value)) = trimmed.split_once('=') else {
                        continue;
                    };
                    let key = key.trim();
                    if let Some(name) = key.strip_suffix(".version") {
                        if let Some((version, span)) = quoted(line, 0).map(|(held, span)| {
                            (held.to_owned(), start + span.start..start + span.end)
                        }) {
                            found.push(Dependency {
                                name: name.trim().trim_matches('"').to_owned(),
                                requirement: version,
                                span,
                            });
                        }
                        continue;
                    }
                    let name = key.trim_matches('"').to_owned();
                    let value = value.trim();
                    if value.starts_with('"') {
                        if let Some((version, span)) = quoted(line, line.find('=').unwrap_or(0)) {
                            found.push(Dependency {
                                name,
                                requirement: version.to_owned(),
                                span: start + span.start..start + span.end,
                            });
                        }
                    } else if value.starts_with('{')
                        && string_after(line, "registry", start).is_none()
                        && string_after(line, "git", start).is_none()
                        && let Some((version, span)) = string_after(
                            &line[line.find('{').unwrap_or(0)..],
                            "version",
                            start + line.find('{').unwrap_or(0),
                        )
                    {
                        let real = string_after(line, "package", start)
                            .map(|(real, _)| real)
                            .unwrap_or(name);
                        found.push(Dependency {
                            name: real,
                            requirement: version,
                            span,
                        });
                    }
                }
            }
        }
        finish(&mut pending, &mut renamed, &mut found);
        found
    }

    /// The crates.io API address of the crate.
    fn address(&self, name: &str) -> String {
        format!("https://crates.io/api/v1/crates/{name}")
    }

    /// The newest version that is not a pre-release, or the newest at all when there is none.
    fn latest_in(&self, answer: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(answer).ok()?;
        let held = value.get("crate")?;
        ["max_stable_version", "max_version"]
            .iter()
            .find_map(|key| held.get(*key)?.as_str().filter(|text| !text.is_empty()))
            .map(str::to_owned)
    }
}
