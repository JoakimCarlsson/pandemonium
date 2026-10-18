//! Installing an extension: its folder kept, its languages handed to the language index, and taking it off again.

use std::path::Path;

use pm_policy::{Capability, Grants};

use crate::package::{Package, root};

/// What installing an extension came to.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Installed {
    /// The extension's name, as requests name it.
    pub id: String,
    /// Why part of it could not be installed, while the rest is.
    pub problems: Vec<String>,
}

/// The extension in the folder `source`, read without installing anything, to show the reader what it asks for.
pub fn inspect(source: &Path) -> Result<Package, String> {
    Package::read(source)
}

/// Copies the folder `from` to `to`, files and folders alike, leaving out what starts with a dot and anything that is a link.
fn copy_folder(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|error| error.to_string())?;
    let entries = std::fs::read_dir(from)
        .map_err(|error| format!("could not read {}: {error}", from.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            copy_folder(&entry.path(), &to.join(&name))?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), to.join(&name))
                .map_err(|error| format!("could not copy {}: {error}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Whether installing the language in `folder` fetches anything from the network: its grammar when it does not carry one, and its language server when it brings an install.
fn fetches(folder: &Path) -> bool {
    let Ok(manifest) = pm_lsp::Manifest::read(folder) else {
        return false;
    };
    let grammar = !folder.join("grammar.wasm").is_file() && !manifest.grammar_file.url.is_empty();
    grammar
        || manifest
            .server
            .is_some_and(|server| server.install.is_some())
}

/// Installs the extension in the folder `source`: keeps its folder, calls `kept` so that what the extension may do is enforced before any of its languages exist, then installs each language it adds through the language index like any other, telling `progress` the language and what is being done.
///
/// A language that has to fetch files is not installed unless `grants` allow downloading; that, and a language server that could not be installed, come back in the problems while the rest of the extension is installed.
pub fn install(
    source: &Path,
    grants: &Grants,
    kept: &dyn Fn(),
    progress: &dyn Fn(&str, &str),
) -> Result<Installed, String> {
    let package = Package::read(source)?;
    let root = root()?;
    let staging = root.join(format!(".{}.partial", package.id));
    let _ = std::fs::remove_dir_all(&staging);
    if let Err(reason) = copy_folder(source, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(reason);
    }
    let folder = root.join(&package.id);
    if folder != source {
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::rename(&staging, &folder).map_err(|error| error.to_string())?;
    } else {
        let _ = std::fs::remove_dir_all(&staging);
    }
    let package = Package::read(&folder)?;
    kept();
    let mut problems = Vec::new();
    for (language, from) in package.languages() {
        if fetches(&from)
            && let Err(denied) = grants.check(Capability::Download)
        {
            problems.push(format!("{language} was not installed: {denied}"));
            continue;
        }
        progress(&package.id, "Adding its languages");
        pm_lsp::add_folder(&from, &language)?;
        match pm_lsp::install(&language, &|_, step| progress(&package.id, step)) {
            Ok(more) => problems.extend(more),
            Err(reason) => problems.push(format!("{language}: {reason}")),
        }
    }
    Ok(Installed {
        id: package.id,
        problems,
    })
}

/// Takes the extension `id` off this machine, with the languages it added.
pub fn remove(id: &str) -> Result<(), String> {
    if let Some(package) = crate::package::named(id) {
        for (language, _) in package.languages() {
            pm_lsp::remove(&language)?;
            pm_lsp::remove_folder(&language)?;
        }
    }
    if !crate::package::is_name(id) {
        return Err(format!("{id} is not the name of an extension"));
    }
    let folder = root()?.join(id);
    if !folder.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&folder).map_err(|error| format!("could not remove {id}: {error}"))
}
