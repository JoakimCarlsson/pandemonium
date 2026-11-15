//! Sample files, search hits, changes and quick inputs, for snapshots of the
//! right panel's Files and Search tabs and of the quick input.

use crate::file_tree::FileTree;
use crate::git::ChangedFile;
use crate::quick::{Choosing, QuickInput, QuickItem, QuickKind, QuickPick};
use crate::search::{ProjectSearch, SearchHit};

/// A tree over a small Rust workspace, with `crates/pm-shell/src` unfolded.
pub fn sample_files() -> FileTree {
    let paths = [
        ".gitignore",
        "Cargo.lock",
        "Cargo.toml",
        "Makefile",
        "README.md",
        "assets/brand/pandemonium.svg",
        "assets/brand/png/icon-256.png",
        "crates/pm-shell/Cargo.toml",
        "crates/pm-shell/src/file_tree.rs",
        "crates/pm-shell/src/lib.rs",
        "crates/pm-shell/src/link.rs",
        "crates/pm-shell/src/model.rs",
        "crates/pm-shell/src/quick.rs",
        "crates/pm-shell/src/search.rs",
        "crates/pm-pages/src/lib.rs",
        "docs/architecture.md",
        "docs/protocol.md",
        "install.sh",
        "languages/languages.json",
        "scripts/release.sh",
    ];
    FileTree {
        root: "~/Documents/repos/pandemonium".to_owned(),
        paths: paths.iter().map(|path| (*path).to_owned()).collect(),
        folders: vec!["build/".to_owned()],
        ignored: vec!["build/".to_owned()],
        expanded: ["crates", "crates/pm-shell", "crates/pm-shell/src", "docs"]
            .iter()
            .map(|path| (*path).to_owned())
            .collect(),
        selected: Some("crates/pm-shell/src/link.rs".to_owned()),
        loaded: true,
        ..FileTree::default()
    }
}

/// Changes to a few files of [`sample_files`], to mark them in the tree.
pub fn sample_changes() -> Vec<ChangedFile> {
    [
        ("crates/pm-shell/src/link.rs", 'M', 42, 7),
        ("crates/pm-shell/src/file_tree.rs", 'A', 230, 0),
        ("docs/architecture.md", 'M', 3, 3),
    ]
    .into_iter()
    .map(|(path, status, added, removed)| ChangedFile {
        path: path.to_owned(),
        status,
        added,
        removed,
        ..ChangedFile::default()
    })
    .collect()
}

/// A search for `Request::Files` that found lines in two files.
pub fn sample_search() -> ProjectSearch {
    let hit = |path: &str, line, column, text: &str| SearchHit {
        path: path.to_owned(),
        line,
        column,
        length: 13,
        text: text.to_owned(),
    };
    ProjectSearch {
        query: "Request::Files".to_owned(),
        hits: vec![
            hit(
                "crates/pm-shell/src/link/browse.rs",
                41,
                12,
                "            Request::Files {",
            ),
            hit(
                "crates/pm-server/src/handler.rs",
                744,
                8,
                "        Request::Files {",
            ),
            hit(
                "crates/pm-server/src/handler.rs",
                1902,
                21,
                "    assert!(matches!(Request::Files { .. }, request));",
            ),
        ],
        searched: Some("Request::Files".to_owned()),
        ..ProjectSearch::default()
    }
    .with_lengths()
}

impl ProjectSearch {
    /// The search with each hit's length measured from its query.
    fn with_lengths(mut self) -> Self {
        let length = self.query.chars().count();
        for hit in &mut self.hits {
            hit.length = length;
        }
        self
    }
}

/// The quick input as `kind` shows it: `files`, `commands`, `help`, `line`, `sessions`, `text`, `theme` and the other lists of choices.
pub fn sample_quick(kind: &str) -> QuickInput {
    match kind {
        "commands" => QuickInput {
            query: ">".to_owned(),
            commands: crate::quick::command_items(
                &pm_editor::Keymap::new(pm_editor::Preset::default(), &Default::default()),
                &["Push".to_owned()],
            ),
            ..QuickInput::default()
        },
        "help" => QuickInput {
            query: "?".to_owned(),
            ..QuickInput::default()
        },
        "line" => QuickInput {
            query: ":120".to_owned(),
            ..QuickInput::default()
        },
        "sessions" => QuickInput {
            choosing: Some(Choosing::Session),
            ..QuickInput::default()
        },
        "text" => QuickInput {
            query: "%Request::Files".to_owned(),
            found: sample_search()
                .hits
                .into_iter()
                .map(|hit| QuickItem {
                    title: hit.text.trim().to_owned(),
                    detail: format!("{}:{}", hit.path, hit.line),
                    kind: QuickKind::Text,
                    keys: None,
                    matched: Vec::new(),
                    pick: QuickPick::Open {
                        path: hit.path,
                        line: Some(hit.line),
                    },
                })
                .collect(),
            ..QuickInput::default()
        },
        "theme" => QuickInput {
            choosing: Some(Choosing::Theme),
            found: ["System", "Light", "Dark", "Default", "Dusk"]
                .iter()
                .enumerate()
                .map(|(place, name)| QuickItem {
                    title: (*name).to_owned(),
                    detail: if place < 3 { "" } else { "Theme" }.to_owned(),
                    kind: if place == 0 || place == 3 {
                        QuickKind::Current
                    } else {
                        QuickKind::Choice
                    },
                    keys: None,
                    matched: Vec::new(),
                    pick: QuickPick::Choose(Choosing::Theme, place),
                })
                .collect(),
            ..QuickInput::default()
        },
        "symbols" => QuickInput {
            query: "#Link".to_owned(),
            found: located(
                QuickKind::Symbol,
                &[
                    ("Link", "crates/pm-shell/src/link.rs", 120),
                    ("Link::connect", "crates/pm-shell/src/link.rs", 180),
                    ("LinkState", "crates/pm-client/src/state.rs", 14),
                ],
            ),
            ..QuickInput::default()
        },
        "outline" => QuickInput {
            query: "@".to_owned(),
            found: located(
                QuickKind::Symbol,
                &[
                    ("root", "crates/pandemonium/src/window.rs", 40),
                    ("window_palette", "crates/pandemonium/src/window.rs", 114),
                    (
                        "load_editor_settings",
                        "crates/pandemonium/src/window.rs",
                        126,
                    ),
                ],
            ),
            ..QuickInput::default()
        },
        "references" => QuickInput {
            choosing: Some(Choosing::References),
            found: located(
                QuickKind::Symbol,
                &[
                    (
                        "let palette = window_palette(link);",
                        "crates/pandemonium/src/window.rs",
                        68,
                    ),
                    (
                        "let palette = window_palette(link);",
                        "crates/pandemonium/src/window.rs",
                        90,
                    ),
                ],
            ),
            ..QuickInput::default()
        },
        "problems" => QuickInput {
            choosing: Some(Choosing::Problems),
            found: located(
                QuickKind::Text,
                &[
                    (
                        "cannot find value `palette` in this scope",
                        "crates/pandemonium/src/window.rs",
                        70,
                    ),
                    (
                        "unused import: `std::rc::Rc`",
                        "crates/pm-shell/src/link.rs",
                        12,
                    ),
                ],
            ),
            ..QuickInput::default()
        },
        "keymap" => choices(
            Choosing::Keymap,
            &["Pandemonium", "VS Code", "JetBrains", "Sublime Text"],
        ),
        "settings" => choices(
            Choosing::Setting,
            &["General", "Appearance", "Editor", "Keymap"],
        ),
        "rewind" => choices(
            Choosing::Rewind,
            &["Add a reader that reads in chunks", "Now keep the position"],
        ),
        "branch" => choices(
            Choosing::Branch,
            &["main", "reader-continuation", "release"],
        ),
        "actions" => choices(
            Choosing::Actions,
            &[
                "Import `std::rc::Rc`",
                "Extract into function",
                "Inline variable",
            ],
        ),
        "history" => QuickInput {
            choosing: Some(Choosing::History),
            found: ["Saved 2 minutes ago", "Saved 1 hour ago", "Saved yesterday"]
                .iter()
                .enumerate()
                .map(|(place, title)| QuickItem {
                    title: (*title).to_owned(),
                    detail: format!("{} bytes", 4200 - place * 120),
                    kind: QuickKind::Choice,
                    keys: None,
                    matched: Vec::new(),
                    pick: QuickPick::Version(place as u64),
                })
                .collect(),
            ..QuickInput::default()
        },
        _ => QuickInput {
            query: "lnk".to_owned(),
            ..QuickInput::default()
        },
    }
}

/// A list of `titles` to choose among for `choosing`, the first the one that holds.
fn choices(choosing: Choosing, titles: &[&str]) -> QuickInput {
    QuickInput {
        choosing: Some(choosing),
        found: titles
            .iter()
            .enumerate()
            .map(|(place, title)| QuickItem {
                title: (*title).to_owned(),
                detail: String::new(),
                kind: if place == 0 && matches!(choosing, Choosing::Keymap | Choosing::Branch) {
                    QuickKind::Current
                } else {
                    QuickKind::Choice
                },
                keys: None,
                matched: Vec::new(),
                pick: QuickPick::Choose(choosing, place),
            })
            .collect(),
        ..QuickInput::default()
    }
}

/// Rows of `kind` for `places`, each a title, a file and a line, opening the file there.
fn located(kind: QuickKind, places: &[(&str, &str, usize)]) -> Vec<QuickItem> {
    places
        .iter()
        .map(|(title, path, line)| QuickItem {
            title: (*title).to_owned(),
            detail: format!("{path}:{line}"),
            kind,
            keys: None,
            matched: Vec::new(),
            pick: QuickPick::Open {
                path: (*path).to_owned(),
                line: Some(*line),
            },
        })
        .collect()
}
