//! Which language servers read the files of a language: those its folder declares, those other languages' folders say they also serve it, and which of them run for a file as the reader chose.

use std::path::Path;

use serde_json::Value;

use crate::install::{installed, installed_manifest};
use crate::manifest::{Companion, Detect, ServerSpec, glob};

/// The word in a list of servers that stands for those that run when the reader names none.
pub const WILDCARD: &str = "...";

/// One language server a language can be read by, and what makes it run by itself.
#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    /// The server.
    pub spec: ServerSpec,
    /// The scopes of the language its completions are offered in; everywhere when none.
    pub scopes: Vec<String>,
    /// The endings of the files it reads, without the dot; every file when none.
    pub suffixes: Vec<String>,
    /// What a project holds that needs it; every project when nothing.
    pub detect: Detect,
    /// Whether the language's own folder names it as the language's server, which runs unless the reader turned it off.
    pub primary: bool,
    /// The language whose folder declares it, which a refusal of that language's programs applies to.
    pub origin: String,
}

impl Candidate {
    /// A server that reads every file of its language in every project.
    fn plain(spec: ServerSpec, primary: bool, origin: &str) -> Self {
        Self {
            spec,
            scopes: Vec::new(),
            suffixes: Vec::new(),
            detect: Detect::default(),
            primary,
            origin: origin.to_owned(),
        }
    }

    /// The companion `companion` of a language.
    fn of_companion(companion: Companion, origin: &str) -> Self {
        Self {
            spec: companion.server,
            scopes: companion.scopes,
            suffixes: companion.suffixes,
            detect: companion.detect,
            primary: false,
            origin: origin.to_owned(),
        }
    }

    /// Whether it reads the file at `path`, by the ending of its name.
    fn reads(&self, path: &str) -> bool {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        self.suffixes.is_empty()
            || name.rsplit_once('.').is_some_and(|(_, suffix)| {
                self.suffixes
                    .iter()
                    .any(|held| held.eq_ignore_ascii_case(suffix))
            })
    }
}

/// Whether the project at `root` holds what `detect` asks for: a dependency in its `package.json` or a file at its root; true when it asks for nothing.
pub(crate) fn needed(root: &Path, detect: &Detect) -> bool {
    if detect.dependencies.is_empty() && detect.files.is_empty() {
        return true;
    }
    let depends = std::fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|package| {
            [
                "dependencies",
                "devDependencies",
                "peerDependencies",
                "optionalDependencies",
            ]
            .iter()
            .filter_map(|section| package.get(section)?.as_object())
            .any(|held| {
                detect
                    .dependencies
                    .iter()
                    .any(|name| held.contains_key(name))
            })
        });
    depends
        || std::fs::read_dir(root).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                detect.files.iter().any(|pattern| glob(pattern, &name))
            })
        })
}

/// Every language server the installed language `language` can be read by: its own, its companions, and those other installed languages declare that name it among the `languages` they serve.
pub(crate) fn candidates_of(language: &str, own: Option<ServerSpec>) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = own
        .into_iter()
        .map(|spec| Candidate::plain(spec, true, language))
        .collect();
    if let Some(manifest) = installed_manifest(language) {
        found.extend(
            manifest
                .companions
                .into_iter()
                .map(|companion| Candidate::of_companion(companion, language)),
        );
    }
    for (id, manifest) in installed() {
        if id == language {
            continue;
        }
        let claims = |spec: &ServerSpec| spec.languages.iter().any(|held| held == language);
        let others = manifest
            .server
            .into_iter()
            .map(|spec| Candidate::plain(spec, false, &id))
            .chain(
                manifest
                    .companions
                    .into_iter()
                    .map(|companion| Candidate::of_companion(companion, &id)),
            )
            .filter(|offer| claims(&offer.spec));
        for offer in others {
            if !found.iter().any(|held| held.spec.id == offer.spec.id) {
                found.push(offer);
            }
        }
    }
    found
}

/// The servers that run for the file at `path` of the project at `root`, written in `language`, when the reader named none: the language's own and each of the others that reads the file, that the project needs and that does not wait to be named.
pub(crate) fn by_default(
    offers: &[Candidate],
    language: &str,
    root: &Path,
    path: &str,
) -> Vec<Candidate> {
    offers
        .iter()
        .filter(|offer| {
            offer.primary
                || (!offer.spec.is_opt_in_for(language)
                    && offer.reads(path)
                    && needed(root, &offer.detect))
        })
        .cloned()
        .collect()
}

/// The servers that run for the file when the reader named `names`, in the order named: a name picks the server with that id, an opt-in one included, and `...` stands for those that run by default; a name no server answers to is left out.
pub(crate) fn named(
    offers: &[Candidate],
    names: &[String],
    language: &str,
    root: &Path,
    path: &str,
) -> Vec<Candidate> {
    let mut chosen: Vec<Candidate> = Vec::new();
    for name in names {
        let more: Vec<Candidate> = if name == WILDCARD {
            by_default(offers, language, root, path)
        } else {
            offers
                .iter()
                .filter(|offer| offer.spec.id == *name)
                .cloned()
                .collect()
        };
        for offer in more {
            if !chosen.iter().any(|held| held.spec.id == offer.spec.id) {
                chosen.push(offer);
            }
        }
    }
    chosen
}
