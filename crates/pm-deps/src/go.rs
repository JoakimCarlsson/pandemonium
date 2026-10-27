//! `go.mod`: the modules it requires and the module proxy that knows their versions.

use crate::dependency::Dependency;
use crate::ecosystem::{Ecosystem, file_name};

/// The Go ecosystem.
pub(crate) struct Go;

/// The module and the version `line`, an entry of a `require` block or what follows `require`, names, with the bytes of the version relative to the line; nothing for a line marked `// indirect`.
fn entry(line: &str) -> Option<(String, String, std::ops::Range<usize>)> {
    let (code, comment) = line.split_once("//").unwrap_or((line, ""));
    if comment.trim().starts_with("indirect") {
        return None;
    }
    let mut words = code.split_whitespace();
    let module = words.next()?;
    let version = words.next()?;
    let start = code.find(version)?;
    version.starts_with('v').then(|| {
        (
            module.trim_matches('"').to_owned(),
            version.to_owned(),
            start..start + version.len(),
        )
    })
}

impl Ecosystem for Go {
    /// Its name.
    fn name(&self) -> &'static str {
        "go"
    }

    /// Whether the file is a `go.mod`.
    fn reads(&self, path: &str) -> bool {
        file_name(path) == "go.mod"
    }

    /// The modules of the `require` lines and blocks that are not marked indirect.
    fn dependencies(&self, text: &str) -> Vec<Dependency> {
        let mut found = Vec::new();
        let mut inside = false;
        let mut offset = 0;
        for line in text.split_inclusive('\n') {
            let start = offset;
            offset += line.len();
            let trimmed = line.trim();
            let (body, shift) = if inside {
                if trimmed.starts_with(')') {
                    inside = false;
                    continue;
                }
                (line, 0)
            } else if trimmed.starts_with("require") {
                let after = line.find("require").map_or(0, |at| at + "require".len());
                if line[after..].trim_start().starts_with('(') {
                    inside = true;
                    continue;
                }
                (&line[after..], after)
            } else {
                continue;
            };
            if let Some((module, version, span)) = entry(body) {
                found.push(Dependency {
                    name: module,
                    requirement: version,
                    span: start + shift + span.start..start + shift + span.end,
                });
            }
        }
        found
    }

    /// The module proxy's address for the latest version, with capital letters written as `!` and the lower case letter, as the proxy asks.
    fn address(&self, name: &str) -> String {
        let escaped: String = name
            .chars()
            .flat_map(|held| {
                if held.is_ascii_uppercase() {
                    vec!['!', held.to_ascii_lowercase()]
                } else {
                    vec![held]
                }
            })
            .collect();
        format!("https://proxy.golang.org/{escaped}/@latest")
    }

    /// The `Version` of the answer.
    fn latest_in(&self, answer: &str) -> Option<String> {
        serde_json::from_str::<serde_json::Value>(answer)
            .ok()?
            .get("Version")?
            .as_str()
            .map(str::to_owned)
    }
}
