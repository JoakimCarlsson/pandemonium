//! The review states the snapshots render: notes on the sample diff, a
//! file stopped in a merge, a commit's files, and search results being
//! edited as excerpts.

use pm_editor::Document;

use crate::git::{ChangedFile, GitSummary};
use crate::open_files::SharedDocument;
use crate::review::{ConflictView, LineNote, Revision};
use crate::search::{ProjectSearch, SearchExcerpt};

/// The conflicted file the `conflict` snapshot resolves.
const CONFLICT_PATH: &str = "crates/void/src/session.rs";

/// What the conflicted sample file holds, markers and all.
const CONFLICT_TEXT: &str = "use std::path::Path;\n\npub struct Session {\n<<<<<<< HEAD\n    pub title: String,\n    pub folder: PathBuf,\n=======\n    pub name: String,\n    pub folder: PathBuf,\n    pub branch: Option<String>,\n>>>>>>> reader-continuation\n}\n\nimpl Session {\n<<<<<<< HEAD\n    pub fn open(folder: &Path) -> Self {\n=======\n    pub fn start(folder: &Path) -> Self {\n>>>>>>> reader-continuation\n        todo!()\n    }\n}\n";

/// The diff of the second file of the sample commit.
const COMMIT_DIFF: &str = "@@ -1,4 +1,5 @@\n [package]\n name = \"void\"\n-version = \"0.3.0\"\n+version = \"0.4.0\"\n+description = \"Reads files in chunks\"\n edition = \"2024\"";

/// Puts on `git` the review state `word` names: `diff-notes` for notes on
/// the open diff, `conflict` for a file stopped in a merge, `revision` for
/// a commit's files, and `changes` for every uncommitted change at once.
pub fn sample_review(git: &mut GitSummary, word: &str) {
    let first = git.listed().first().map(|change| change.path.clone());
    match word {
        "diff-notes" => {
            git.diff = first.clone();
            git.notes = vec![LineNote {
                path: first.unwrap_or_default(),
                line: 14,
                new: true,
                text: "Start the position where the file starts, not at zero.".to_owned(),
            }];
        }
        "conflict" => {
            if let Some(change) = git
                .changes
                .iter_mut()
                .find(|change| change.path == CONFLICT_PATH)
            {
                change.conflicted = true;
                change.status = 'U';
            }
            git.operation = Some("Merging".to_owned());
            git.diff = Some(CONFLICT_PATH.to_owned());
            let mut document = Document::new(CONFLICT_TEXT, false, false);
            document.language = Some("rust".to_owned());
            if let Some(syntax) = pm_treesitter::registry::called("rust")
                .and_then(|grammar| pm_treesitter::Syntax::parse(grammar, CONFLICT_TEXT))
            {
                document.with_syntax(syntax);
            }
            git.conflict = Some(ConflictView {
                path: CONFLICT_PATH.to_owned(),
                result: SharedDocument::new(document),
                language: Some("rust".to_owned()),
                loaded: true,
                trouble: None,
            });
        }
        "revision" => {
            let mut changes: Vec<ChangedFile> =
                git.changes.iter().take(1).cloned().collect::<Vec<_>>();
            let (added, removed) = crate::git::tally(COMMIT_DIFF);
            changes.push(ChangedFile {
                path: "crates/void/Cargo.toml".to_owned(),
                status: 'M',
                added,
                removed,
                diff: COMMIT_DIFF.to_owned(),
                ..ChangedFile::default()
            });
            git.revision = Some(Revision {
                title: "Commit 4f2a9c1 \u{2014} Read files in chunks".to_owned(),
                from: None,
                to: "4f2a9c1".to_owned(),
                changes,
                loaded: true,
                trouble: None,
                restore: None,
            });
        }
        "turn-changes" => {
            let (added, removed) = crate::git::tally(COMMIT_DIFF);
            git.revision = Some(Revision {
                title: "Turn 2".to_owned(),
                to: String::new(),
                changes: vec![ChangedFile {
                    path: "crates/void/Cargo.toml".to_owned(),
                    status: 'M',
                    added,
                    removed,
                    diff: COMMIT_DIFF.to_owned(),
                    ..ChangedFile::default()
                }],
                loaded: true,
                restore: Some(crate::review::RevisionRestore::Turn {
                    session: 1,
                    turn: 1,
                }),
                ..Revision::default()
            });
        }
        "timeline" => {
            let (added, removed) = crate::git::tally(COMMIT_DIFF);
            git.revision = Some(Revision {
                title: "Timeline: Cargo.toml \u{b7} Saved 25m ago".to_owned(),
                to: "crates/void/Cargo.toml".to_owned(),
                changes: vec![ChangedFile {
                    path: "crates/void/Cargo.toml".to_owned(),
                    status: 'M',
                    added,
                    removed,
                    fetched: true,
                    diff: COMMIT_DIFF.to_owned(),
                    ..ChangedFile::default()
                }],
                loaded: true,
                restore: Some(crate::review::RevisionRestore::Version {
                    directory: String::new(),
                    path: "crates/void/Cargo.toml".to_owned(),
                    text: String::new(),
                }),
                ..Revision::default()
            });
        }
        _ => {}
    }
}

/// Shows the hits of `search` as excerpts being edited, each a stretch of the line found alone.
pub fn sample_search_edit(search: &mut ProjectSearch) {
    search.editing = true;
    search.excerpts = search
        .hits
        .iter()
        .map(|hit| {
            let mut document = Document::new(&hit.text, false, false);
            if let Some(syntax) = pm_treesitter::registry::called("rust")
                .and_then(|grammar| pm_treesitter::Syntax::parse(grammar, &hit.text))
            {
                document.with_syntax(syntax);
            }
            SearchExcerpt {
                path: hit.path.clone(),
                first: hit.line,
                original: format!("{}\n", hit.text),
                document: SharedDocument::new(document),
            }
        })
        .collect();
}
