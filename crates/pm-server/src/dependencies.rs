//! The dependencies a manifest lists, looked up in their registry for a newer version, and the lens that updates one: the language says which files list dependencies, how to find them and where the registry answers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use pm_buffer::{Buffer, Span};
use pm_lsp::{CodeLens, DependencySpec, FileEdit, TextChange};
use pm_treesitter::Syntax;

/// How long a version found stays good before it is looked up again.
const FRESH: Duration = Duration::from_secs(3600);

/// How long a failed lookup is not tried again.
const FAILED: Duration = Duration::from_secs(300);

/// How long one lookup may take.
const PATIENCE: Duration = Duration::from_secs(5);

/// How many lookups run at once.
const AT_ONCE: usize = 6;

/// The most dependencies looked up for one file.
const MOST: usize = 80;

/// One dependency of an open manifest, before the registry is asked about it.
pub(crate) struct Listed {
    /// What it is called.
    name: String,
    /// The version the manifest asks for, as written.
    current: String,
    /// Where that version is written.
    span: Span,
    /// The line of the file the dependency is written on.
    line: String,
}

/// A newer version of one dependency that a lens offers to write in its place.
#[derive(Clone)]
struct Offer {
    /// Where the version it replaces is written.
    span: Span,
    /// What the version it replaces says now, so that a changed file is left alone.
    current: String,
    /// What to write there.
    replacement: String,
}

/// What a lookup found and when: the newest version, or nothing when the lookup failed.
type Found = (Instant, Option<String>);

/// The offers made for one open manifest: how many lenses of the language server come first, and the offers that follow.
type Offers = (usize, Vec<Offer>);

/// The newest versions found, by the address that said them, and when.
fn cache() -> &'static Mutex<HashMap<String, Found>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Found>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// The offers last made for each open manifest, in the order of the lenses that follow the language server's own, with how many of those there are.
fn held() -> &'static Mutex<HashMap<(PathBuf, String), Offers>> {
    static HELD: OnceLock<Mutex<HashMap<(PathBuf, String), Offers>>> = OnceLock::new();
    HELD.get_or_init(Mutex::default)
}

/// Where the newest versions of the dependencies of the file at `path`, written in the installed language `language`, are looked up; nothing when the language does not look them up for that file.
fn spec_of(language: &str, path: &str) -> Option<DependencySpec> {
    let spec = pm_lsp::installed_manifest(language)?.dependencies?;
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    (spec.files.is_empty() || spec.files.iter().any(|file| file == name)).then_some(spec)
}

/// Whether the file at `path`, written in `language`, lists dependencies whose newest versions are looked up.
pub(crate) fn applies(language: &str, path: &str) -> bool {
    spec_of(language, path).is_some()
}

/// The dependencies `buffer` lists, the text of the file at `path` written in `language`.
pub(crate) fn listed(language: &str, path: &str, buffer: &Buffer) -> Vec<Listed> {
    if !applies(language, path) {
        return Vec::new();
    }
    let Some(grammar) = crate::scan::grammar_of(language) else {
        return Vec::new();
    };
    let text = buffer.text();
    Syntax::parse(grammar, text)
        .map(|syntax| {
            syntax
                .dependencies(text)
                .into_iter()
                .map(|found| {
                    let span = buffer.span_of(found.range);
                    Listed {
                        name: found.name,
                        current: found.version,
                        line: text
                            .lines()
                            .nth(span.start.line as usize)
                            .unwrap_or_default()
                            .to_owned(),
                        span,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Whether `name` is one of the private names the environment variables `variables` list, each a pattern with `*` for any run of characters that names a module or the folder of modules it is inside.
fn is_private(name: &str, variables: &[String]) -> bool {
    variables
        .iter()
        .filter_map(|variable| std::env::var(variable).ok())
        .flat_map(|value| {
            value
                .split(',')
                .map(|pattern| pattern.trim().to_owned())
                .filter(|pattern| !pattern.is_empty())
                .collect::<Vec<_>>()
        })
        .any(|pattern| {
            let parts = pattern.split('/').count();
            let head = name.split('/').take(parts).collect::<Vec<_>>().join("/");
            pm_lsp::glob(&pattern, &head)
        })
}

/// `name` written for use inside an address: letters, digits and `-._~@` stay and everything else is percent-encoded.
fn encoded(name: &str) -> String {
    name.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'@' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// `name` written as the Go module proxy writes it: each capital letter as `!` and its small letter.
fn escaped(name: &str) -> String {
    name.chars()
        .map(|letter| {
            if letter.is_ascii_uppercase() {
                format!("!{}", letter.to_ascii_lowercase())
            } else {
                letter.to_string()
            }
        })
        .collect()
}

/// The newest version of `name` the registry of `spec` says, from the cache when it is fresh; nothing when it cannot be looked up.
fn newest(spec: &DependencySpec, name: &str) -> Option<String> {
    let address = spec
        .registry
        .replace("{escaped}", &encoded(&escaped(name)))
        .replace("{name}", &encoded(name));
    if !address.starts_with("https://") {
        return None;
    }
    if let Some((when, found)) = cache().lock().ok()?.get(&address)
        && when.elapsed() < if found.is_some() { FRESH } else { FAILED }
    {
        return found.clone();
    }
    let found = fetch(&address, &spec.pointer);
    if let Ok(mut held) = cache().lock() {
        held.insert(address, (Instant::now(), found.clone()));
    }
    found
}

/// The text at `pointer` of the JSON the address `address` answers.
fn fetch(address: &str, pointer: &str) -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(PATIENCE))
        .build()
        .into();
    let mut response = agent
        .get(address)
        .header(
            "User-Agent",
            concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .ok()?;
    let text = response.body_mut().read_to_string().ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.pointer(pointer)?.as_str().map(str::to_owned)
}

/// The numbers of the release `version` names, or nothing when it is not one, such as a range, a tag or a pre-release.
fn release(version: &str) -> Option<Vec<u64>> {
    let plain = version.strip_prefix('v').unwrap_or(version);
    if plain.contains(['-', '+']) {
        return None;
    }
    plain.split('.').map(|part| part.parse().ok()).collect()
}

/// The operator the manifest writes in front of `version` and keeps, and the rest of it.
fn split_operator(version: &str) -> (&str, &str) {
    let rest = version.trim_start_matches(['^', '~', '=']);
    (&version[..version.len() - rest.len()], rest)
}

/// What to write in place of `current` to ask for `latest`, when `latest` is newer.
fn replacement(current: &str, latest: &str) -> Option<String> {
    let (operator, bare) = split_operator(current.trim());
    let (have, newer) = (release(bare)?, release(latest)?);
    let width = have.len().max(newer.len());
    let pad = |mut numbers: Vec<u64>| {
        numbers.resize(width, 0);
        numbers
    };
    if pad(newer.clone()) <= pad(have) {
        return None;
    }
    let version = if bare.starts_with('v') || !latest.starts_with('v') {
        latest.to_owned()
    } else {
        latest.trim_start_matches('v').to_owned()
    };
    Some(format!("{operator}{version}"))
}

/// The lenses that offer to update the dependencies `found` of the open file at `path` of the project at `root`, looked up in the registry the language names; `after` is how many lenses the language server gave, which these follow.
pub(crate) fn lenses(
    root: &Path,
    path: &str,
    language: &str,
    found: Vec<Listed>,
    after: usize,
) -> Vec<CodeLens> {
    let Some(spec) = spec_of(language, path) else {
        return Vec::new();
    };
    let found: Vec<Listed> = found
        .into_iter()
        .filter(|listed| !is_private(&listed.name, &spec.private_env))
        .filter(|listed| {
            !spec
                .private_markers
                .iter()
                .any(|marker| listed.line.contains(marker.as_str()))
        })
        .collect();
    let mut names: Vec<&str> = found.iter().map(|listed| listed.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names.truncate(MOST);
    let mut newest_of: HashMap<&str, String> = HashMap::new();
    for chunk in names.chunks(AT_ONCE) {
        let answers: Vec<Option<String>> = std::thread::scope(|scope| {
            chunk
                .iter()
                .map(|name| scope.spawn(|| newest(&spec, name)))
                .map(|lookup| lookup.join().ok().flatten())
                .collect()
        });
        newest_of.extend(
            chunk
                .iter()
                .zip(answers)
                .filter_map(|(name, answer)| Some((*name, answer?))),
        );
    }
    let mut offers = Vec::new();
    let mut lenses = Vec::new();
    for listed in &found {
        let Some(latest) = newest_of.get(listed.name.as_str()) else {
            continue;
        };
        let Some(replacement) = replacement(&listed.current, latest) else {
            continue;
        };
        let title = spec
            .title
            .as_deref()
            .unwrap_or("Update to {latest}")
            .replace("{latest}", latest)
            .replace("{current}", &listed.current);
        lenses.push(CodeLens {
            span: listed.span,
            title,
            pressable: true,
        });
        offers.push(Offer {
            span: listed.span,
            current: listed.current.clone(),
            replacement,
        });
    }
    if let Ok(mut all) = held().lock() {
        all.insert((root.to_path_buf(), path.to_owned()), (after, offers));
    }
    lenses
}

/// Forgets the offers made for the file at `path` of the project at `root`.
pub(crate) fn forget(root: &Path, path: &str) {
    if let Ok(mut all) = held().lock() {
        all.remove(&(root.to_path_buf(), path.to_owned()));
    }
}

/// The change that presses the `index`th lens offered for the open file at `path` of the project at `root`, when that lens is one of these and not the language server's: the version written again; `text` is what the file's text holds at the stretch now.
pub(crate) fn press(
    root: &Path,
    path: &str,
    index: usize,
    now: impl FnOnce(Span) -> Option<String>,
) -> Option<Result<FileEdit, String>> {
    let offer = {
        let all = held().lock().ok()?;
        let (after, offers) = all.get(&(root.to_path_buf(), path.to_owned()))?;
        offers.get(index.checked_sub(*after)?)?.clone()
    };
    if now(offer.span).as_deref() != Some(offer.current.as_str()) {
        return Some(Err("the file changed; the lens is out of date".to_owned()));
    }
    Some(Ok(FileEdit {
        path: path.to_owned(),
        changes: vec![TextChange {
            span: Some(offer.span),
            text: offer.replacement,
        }],
    }))
}
