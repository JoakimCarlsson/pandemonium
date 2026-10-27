//! `package.json`: the dependencies it lists and the npm registry that knows their versions.

use crate::dependency::Dependency;
use crate::ecosystem::{Ecosystem, file_name};

/// The sections of a `package.json` that list dependencies.
const SECTIONS: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];

/// The npm ecosystem.
pub(crate) struct Npm;

/// A string of a JSON text: what is between its quotes, the bytes that stand in, and where it ends.
struct Word<'a> {
    /// The characters between the quotes, as written.
    text: &'a str,
    /// The bytes they stand in.
    span: std::ops::Range<usize>,
}

/// The string that starts at the quote at `open` of `text`, or nothing when it never ends.
fn word(text: &str, open: usize) -> Option<Word<'_>> {
    let bytes = text.as_bytes();
    let mut at = open + 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'"' => {
                return Some(Word {
                    text: &text[open + 1..at],
                    span: open + 1..at,
                });
            }
            _ => at += 1,
        }
    }
    None
}

impl Ecosystem for Npm {
    /// Its name.
    fn name(&self) -> &'static str {
        "npm"
    }

    /// Whether the file is a `package.json`.
    fn reads(&self, path: &str) -> bool {
        file_name(path) == "package.json"
    }

    /// The entries of the sections that list dependencies, found by following the braces and strings of the JSON.
    fn dependencies(&self, text: &str) -> Vec<Dependency> {
        let bytes = text.as_bytes();
        let mut found = Vec::new();
        let mut depth = 0usize;
        let mut section_depth = None;
        let mut key: Option<Word<'_>> = None;
        let mut at = 0;
        while at < bytes.len() {
            match bytes[at] {
                b'"' => {
                    let Some(held) = word(text, at) else {
                        break;
                    };
                    at = held.span.end + 1;
                    let after = text[at..].trim_start();
                    if after.starts_with(':') {
                        key = Some(held);
                    } else if let (Some(name), Some(_)) =
                        (&key, section_depth.filter(|d| *d == depth))
                    {
                        found.push(Dependency {
                            name: name.text.to_owned(),
                            requirement: held.text.to_owned(),
                            span: held.span.clone(),
                        });
                        key = None;
                    }
                    continue;
                }
                b'{' => {
                    depth += 1;
                    if depth == 2
                        && key
                            .as_ref()
                            .is_some_and(|held| SECTIONS.contains(&held.text))
                    {
                        section_depth = Some(depth);
                    }
                    key = None;
                }
                b'}' => {
                    if section_depth == Some(depth) {
                        section_depth = None;
                    }
                    depth = depth.saturating_sub(1);
                    key = None;
                }
                _ => {}
            }
            at += 1;
        }
        found
    }

    /// Whether the package's scope has a registry of its own in the user's `.npmrc`, or the default registry is not npm's, so that the public registry is not asked about it.
    fn skips(&self, name: &str) -> bool {
        let Some(config) = std::env::var_os("HOME")
            .map(|home| std::path::Path::new(&home).join(".npmrc"))
            .and_then(|path| std::fs::read_to_string(path).ok())
        else {
            return false;
        };
        let scope = name.split('/').next().filter(|_| name.starts_with('@'));
        config.lines().map(str::trim).any(|line| {
            let own_default = line.starts_with("registry=") && !line.contains("registry.npmjs.org");
            let scoped = scope.is_some_and(|scope| line.starts_with(&format!("{scope}:registry=")));
            own_default || scoped
        })
    }

    /// The registry's answer about the latest version, which is `/latest` after the package; a scoped name has its slash written as `%2f`.
    fn address(&self, name: &str) -> String {
        format!(
            "https://registry.npmjs.org/{}/latest",
            name.replace('/', "%2f")
        )
    }

    /// The `version` of the answer.
    fn latest_in(&self, answer: &str) -> Option<String> {
        serde_json::from_str::<serde_json::Value>(answer)
            .ok()?
            .get("version")?
            .as_str()
            .map(str::to_owned)
    }
}
