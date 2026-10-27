//! How a registry is asked about a dependency, as a language folder declares it, and when the machine's own settings say a dependency must not be asked about.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Where the machine's settings that make a dependency private are read from.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Private {
    /// A program, with its arguments, that prints the value of each of `patterns` and `hosts` on a line of its own when they are put after them, such as `go env`; the variables of the process are read when it cannot be run.
    pub read: Vec<String>,
    /// Variables that hold comma-separated names, or paths whose elements may be globs, of dependencies that are private.
    pub patterns: Vec<String>,
    /// Variables that list the hosts dependencies are fetched from: every dependency is private unless the host of the registry is the only one they list.
    pub hosts: Vec<String>,
    /// Files of `key=value` lines that can name another registry: one that starts with `~/` is in the home folder, one that starts with `$NAME/` in the folder the variable names, and any other is looked for in the folder of the manifest and every folder above it.
    pub ini: Vec<String>,
    /// The keys of that file that name another registry, where `{scope}` stands for the scope a name starts with, such as `@types`.
    pub ini_keys: Vec<String>,
}

/// How the latest version of a dependency is found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Provider {
    /// The address that answers with what the registry knows of a dependency, where `{name}` stands for its name.
    pub url: String,
    /// The places in the answer, a JSON document, that hold the latest version, as keys joined by dots; the first that has one is used.
    pub latest: Vec<String>,
    /// How a name is written in the address: each pair has what is found in the name and what it is written as instead.
    pub escape: Vec<[String; 2]>,
    /// What is written before a capital letter, which is then written in lower case; capitals are left as they are when nothing.
    pub capitals: Option<String>,
    /// The pattern a version or requirement in a manifest is read with: its `version` group holds dot-separated numbers, its `prefix` group what stands before them, and its `pre` group a pre-release tag.
    pub requirement: String,
    /// When a dependency is not asked about.
    pub private: Private,
    /// The least time between two requests to the registry, in milliseconds; the default when nothing.
    pub spacing_ms: Option<u64>,
    /// How long one batch of lookups may take, in seconds; the default when nothing.
    pub budget_seconds: Option<u64>,
    /// How many dependencies of one manifest are looked up; the default when nothing.
    pub most: Option<usize>,
    /// How long an answer is kept, in minutes; the default when nothing.
    pub fresh_minutes: Option<u64>,
}

/// The host of the address `url`.
fn host_of(url: &str) -> &str {
    url.split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split(['/', ':'])
        .next()
        .unwrap_or_default()
}

/// Whether the dependency `name` is, or lies under, what `pattern` names, a path whose elements may be globs such as `*.corp.example.com`.
fn matches_pattern(pattern: &str, name: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    let count = pattern.split('/').count();
    let head: Vec<&str> = name.split('/').take(count).collect();
    head.len() == count
        && pattern
            .split('/')
            .zip(head)
            .all(|(pattern, element)| glob(pattern, element))
}

/// Whether `name` is what `pattern` describes, where `*` stands for any run of characters.
fn glob(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let mut parts: Vec<&str> = parts.collect();
    let Some(last) = parts.pop() else {
        return rest.is_empty();
    };
    for part in parts {
        let Some(at) = rest.find(part) else {
            return false;
        };
        rest = &rest[at + part.len()..];
    }
    rest.ends_with(last)
}

/// The files `file` names: one place for a path in the home folder or under a variable, and for any other path one in each of `folder` and the folders above it.
fn files_of(file: &str, folder: &Path) -> Vec<PathBuf> {
    if let Some(rest) = file.strip_prefix("~/") {
        return std::env::var_os("HOME")
            .map(|home| vec![PathBuf::from(home).join(rest)])
            .unwrap_or_default();
    }
    if let Some(rest) = file.strip_prefix('$') {
        let (name, rest) = rest.split_once('/').unwrap_or((rest, ""));
        return std::env::var_os(name)
            .map(|place| vec![PathBuf::from(place).join(rest)])
            .unwrap_or_default();
    }
    if Path::new(file).is_absolute() {
        return vec![PathBuf::from(file)];
    }
    folder.ancestors().map(|above| above.join(file)).collect()
}

/// What the machine's settings say, read once for a batch of dependencies.
pub(crate) struct Settings {
    /// The value of each variable that was asked for.
    values: HashMap<String, String>,
    /// The lines of the file of settings.
    ini: Vec<(String, String)>,
}

impl Provider {
    /// The settings of this machine that matter to the provider.
    pub(crate) fn settings(&self, folder: &Path) -> Settings {
        let private = &self.private;
        let names: Vec<&String> = private.patterns.iter().chain(&private.hosts).collect();
        let asked = (!private.read.is_empty() && !names.is_empty())
            .then(|| {
                let program = pm_tools::installed(&private.read[0])
                    .unwrap_or_else(|| PathBuf::from(&private.read[0]));
                std::process::Command::new(&program)
                    .env("PATH", pm_tools::path_beside(&program))
                    .args(&private.read[1..])
                    .args(&names)
                    .output()
                    .ok()
                    .filter(|output| output.status.success())
                    .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            })
            .flatten();
        let mut values = HashMap::new();
        match asked {
            Some(text) => {
                for (name, line) in names.iter().zip(text.lines()) {
                    values.insert((*name).clone(), line.trim().to_owned());
                }
            }
            None => {
                for name in &names {
                    values.insert((*name).clone(), std::env::var(name).unwrap_or_default());
                }
            }
        }
        let ini = private
            .ini
            .iter()
            .flat_map(|file| files_of(file, folder))
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .flat_map(|text| {
                text.lines()
                    .filter_map(|line| line.split_once('='))
                    .map(|(key, value)| {
                        let clean = |text: &str| text.trim().trim_matches(['"', '\'']).to_owned();
                        (clean(key), clean(value))
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        Settings { values, ini }
    }

    /// Whether the dependency `name` must not be asked about, because `settings` say it is private or comes from another registry; a name sent to a registry is disclosed to it.
    pub(crate) fn skips(&self, name: &str, settings: &Settings) -> bool {
        let private = &self.private;
        let own = host_of(&self.url);
        let named = private.patterns.iter().any(|variable| {
            settings.values.get(variable).is_some_and(|value| {
                value
                    .split(',')
                    .any(|pattern| matches_pattern(pattern.trim(), name))
            })
        });
        let elsewhere = private.hosts.iter().any(|variable| {
            let value = settings
                .values
                .get(variable)
                .map_or("", |value| value.trim());
            if value.is_empty() {
                return false;
            }
            let hosts: Vec<&str> = value
                .split([',', '|'])
                .map(str::trim)
                .filter(|entry| !entry.is_empty() && *entry != "direct" && *entry != "off")
                .map(host_of)
                .collect();
            hosts.is_empty() || hosts.iter().any(|host| !host.eq_ignore_ascii_case(own))
        });
        let scope = name.split('/').next().filter(|_| name.starts_with('@'));
        let configured = settings.ini.iter().any(|(key, value)| {
            private.ini_keys.iter().any(|wanted| {
                let wanted = match scope {
                    Some(scope) => wanted.replace("{scope}", scope),
                    None if wanted.contains("{scope}") => return false,
                    None => wanted.clone(),
                };
                key.eq_ignore_ascii_case(&wanted) && !host_of(value).eq_ignore_ascii_case(own)
            })
        });
        named || elsewhere || configured
    }

    /// The address that answers with what the registry knows of the dependency `name`.
    pub(crate) fn address(&self, name: &str) -> String {
        let mut written = name.to_owned();
        for [found, instead] in &self.escape {
            written = written.replace(found.as_str(), instead);
        }
        if let Some(mark) = &self.capitals {
            written = written
                .chars()
                .flat_map(|held| {
                    if held.is_uppercase() {
                        mark.chars().chain(held.to_lowercase()).collect()
                    } else {
                        vec![held]
                    }
                })
                .collect();
        }
        self.url.replace("{name}", &written)
    }

    /// The latest version in `answer`, what the registry said at the address of a dependency.
    pub(crate) fn latest_in(&self, answer: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(answer).ok()?;
        self.latest.iter().find_map(|path| {
            let mut at = &value;
            for key in path.split('.') {
                at = at.get(key)?;
            }
            at.as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
        })
    }
}
