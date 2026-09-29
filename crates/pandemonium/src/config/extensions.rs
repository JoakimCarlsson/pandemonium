//! Reads user extensions and hands languages, themes and keymaps to their owners.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use pm_text::{ExtensionLanguage, Language};
use serde::Deserialize;

use super::paths;
use super::stored::StoredServer;

/// Extension assets and errors from the most recent read.
#[derive(Default)]
struct Installed {
    /// Theme files in extension order.
    themes: Vec<String>,
    /// Keymap files in extension order.
    keymaps: Vec<String>,
    /// One trouble message per failed extension.
    errors: Vec<String>,
}

/// The assets read at launch or the most recent reload.
static INSTALLED: LazyLock<Mutex<Installed>> = LazyLock::new(Mutex::default);

/// The data an extension declares.
#[derive(Deserialize)]
struct Manifest {
    /// Its human name.
    name: String,
    /// Its version.
    version: String,
    /// Languages it adds.
    #[serde(default)]
    languages: Vec<ManifestLanguage>,
    /// Themes it offers.
    #[serde(default)]
    themes: Vec<PathBuf>,
    /// Keymaps it offers.
    #[serde(default)]
    keymaps: Vec<PathBuf>,
}

/// A language declared by an extension.
#[derive(Deserialize)]
struct ManifestLanguage {
    /// Its status bar name.
    name: String,
    /// Its LSP identifier and WASM export name.
    language_id: String,
    /// The WASM grammar.
    grammar: PathBuf,
    /// Queries joined in this order.
    #[serde(default)]
    highlights: Vec<PathBuf>,
    /// File suffixes it claims.
    #[serde(default)]
    extensions: Vec<String>,
    /// Whole file names it claims.
    #[serde(default)]
    file_names: Vec<String>,
    /// The prefix for a line comment.
    line_comment: Option<String>,
    /// Servers run per worktree.
    #[serde(default)]
    servers: Vec<StoredServer>,
    /// Reserved indentation queries.
    #[serde(default, rename = "indents")]
    _indents: Vec<PathBuf>,
    /// Reserved injection queries.
    #[serde(default, rename = "injections")]
    _injections: Vec<PathBuf>,
    /// Reserved fold queries.
    #[serde(default, rename = "folds")]
    _folds: Vec<PathBuf>,
}

/// Reads every extension in sorted id order and installs its languages.
pub(super) fn reload() {
    let mut installed = Installed::default();
    let mut languages = Vec::new();
    let mut claims = HashSet::new();
    let Some(directory) = paths::extensions() else {
        pm_text::install_languages(languages);
        if let Ok(mut current) = INSTALLED.lock() {
            *current = installed;
        }
        return;
    };
    let mut roots = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    roots.sort();
    for root in roots {
        let id = root.file_name().unwrap_or_default().to_string_lossy();
        if let Err(reason) = read(&root, &mut installed, &mut languages, &mut claims) {
            installed
                .errors
                .push(format!("{id}/extension.yaml: {reason}"));
        }
    }
    pm_text::install_languages(languages);
    if let Ok(mut current) = INSTALLED.lock() {
        *current = installed;
    }
}

/// Reads one manifest and its assets, checking every referenced path.
fn read(
    root: &Path,
    installed: &mut Installed,
    languages: &mut Vec<Language>,
    claims: &mut HashSet<String>,
) -> Result<(), String> {
    let manifest_path = root.join("extension.yaml");
    let content = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    let manifest: Manifest = serde_norway::from_str(&content)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    if manifest.name.trim().is_empty() || manifest.version.trim().is_empty() {
        return Err("name and version are required".into());
    }
    semver::Version::parse(&manifest.version)
        .map_err(|error| format!("invalid version {}: {error}", manifest.version))?;
    let mut added = Vec::new();
    let mut pending_claims = HashSet::new();
    let mut theme_texts = Vec::new();
    let mut keymap_texts = Vec::new();
    for language in manifest.languages {
        if pm_text::Language::builtins().iter().any(|builtin| {
            builtin.name().eq_ignore_ascii_case(&language.name)
                || builtin
                    .language_id()
                    .eq_ignore_ascii_case(&language.language_id)
        }) || languages
            .iter()
            .chain(added.iter())
            .any(|previous: &Language| {
                previous.name().eq_ignore_ascii_case(&language.name)
                    || previous
                        .language_id()
                        .eq_ignore_ascii_case(&language.language_id)
            })
        {
            return Err(format!(
                "language {} duplicates an installed or built-in language",
                language.name
            ));
        }
        let grammar_path = checked(root, &language.grammar)?;
        let bytes = fs::read(&grammar_path)
            .map_err(|error| format!("{}: {error}", grammar_path.display()))?;
        let grammar = pm_text::load_grammar(&language.language_id, &bytes)
            .map_err(|error| format!("{}: {error}", grammar_path.display()))?;
        let mut highlights = Vec::new();
        for path in &language.highlights {
            let path = checked(root, path)?;
            highlights.push(
                fs::read_to_string(&path)
                    .map_err(|error| format!("{}: {error}", path.display()))?,
            );
        }
        let mut accepted = Vec::new();
        for extension in language.extensions {
            let built_in = Language::of(Path::new(&format!("file.{extension}")))
                .is_some_and(|found| !found.is_wasm());
            if built_in || claims.contains(&extension) || !pending_claims.insert(extension.clone())
            {
                return Err(format!("file extension .{extension} is already claimed"));
            }
            accepted.push(extension);
        }
        let candidate = Language::extension(ExtensionLanguage {
            name: language.name,
            language_id: language.language_id,
            grammar,
            highlights,
            extensions: accepted,
            file_names: language.file_names,
            servers: language
                .servers
                .into_iter()
                .map(StoredServer::into_server)
                .collect(),
            line_comment: language.line_comment,
        });
        candidate.validate_highlights().map_err(|error| {
            format!(
                "{}: {error}",
                language
                    .highlights
                    .first()
                    .map(|path| root.join(path).display().to_string())
                    .unwrap_or_else(|| grammar_path.display().to_string())
            )
        })?;
        added.push(candidate);
    }
    for path in manifest.themes {
        let path = checked(root, &path)?;
        theme_texts.push(
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?,
        );
    }
    for path in manifest.keymaps {
        let path = checked(root, &path)?;
        keymap_texts.push(
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?,
        );
    }
    languages.extend(added);
    claims.extend(pending_claims);
    installed.themes.extend(theme_texts);
    installed.keymaps.extend(keymap_texts);
    Ok(())
}

/// Resolves an asset inside its extension directory.
fn checked(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if relative.is_absolute() {
        return Err(format!(
            "{} must be relative to the extension",
            relative.display()
        ));
    }
    let base = root.canonicalize().map_err(|error| error.to_string())?;
    let path = root.join(relative);
    let resolved = path
        .canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if !resolved.starts_with(base) {
        return Err(format!("{} leaves the extension directory", path.display()));
    }
    Ok(resolved)
}

/// The installed extension theme texts.
pub(super) fn themes() -> Vec<String> {
    INSTALLED
        .lock()
        .map(|installed| installed.themes.clone())
        .unwrap_or_default()
}

/// The installed extension keymap texts.
pub(super) fn keymaps() -> Vec<String> {
    INSTALLED
        .lock()
        .map(|installed| installed.keymaps.clone())
        .unwrap_or_default()
}

/// Removes the error messages from the most recent read.
pub(super) fn take_errors() -> Vec<String> {
    INSTALLED
        .lock()
        .map(|mut installed| std::mem::take(&mut installed.errors))
        .unwrap_or_default()
}
