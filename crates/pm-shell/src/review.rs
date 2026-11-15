//! What the Diff tab reviews besides the plain diff of one file: notes left
//! on lines for the agent, the files a commit or a branch changed, and the
//! resolver of a file a merge stopped on with the places its sides disagree.

use std::sync::Arc;

use pm_treesitter::{Grammar, registry};

use crate::git::ChangedFile;
use crate::open_files::SharedDocument;

/// A note left on a line of a file's diff, to be sent to the agent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineNote {
    /// The file, from the root of the worktree.
    pub path: String,
    /// The line, counted from 1.
    pub line: usize,
    /// Whether the line is of the new side rather than the old.
    pub new: bool,
    /// What the note says.
    pub text: String,
}

/// The files a commit changed, or a branch since it left another, as the Diff tab shows them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Revision {
    /// What the changes are named as, such as `Commit 4f2a9c1`.
    pub title: String,
    /// The branch or commit counted from, the commit's parent when nothing.
    pub from: Option<String>,
    /// The commit or branch counted to.
    pub to: String,
    /// The files changed, with their diffs.
    pub changes: Vec<ChangedFile>,
    /// Whether the server answered.
    pub loaded: bool,
    /// Why the server could not list them, when it could not.
    pub trouble: Option<String>,
}

/// Which side of a conflict to take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictSide {
    /// The branch being worked on.
    Ours,
    /// The incoming side.
    Theirs,
    /// Ours, then theirs.
    Both,
}

/// A file a merge, a rebase or a cherry-pick stopped on, being resolved.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConflictView {
    /// The file, from the root of the worktree.
    pub path: String,
    /// The result, the file as the working folder holds it, edited by hand and by the choices.
    pub result: SharedDocument,
    /// The language the file is written in, when the server knows it.
    pub language: Option<String>,
    /// Whether the server answered.
    pub loaded: bool,
    /// Why the server could not read the file, when it could not.
    pub trouble: Option<String>,
}

/// One place of a file where the two sides disagree, between git's markers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConflictRegion {
    /// The line of its `<<<<<<<` marker, counted from 0.
    pub start: usize,
    /// The line of its `>>>>>>>` marker, counted from 0.
    pub end: usize,
    /// What our side holds there.
    pub ours: String,
    /// What their side holds there.
    pub theirs: String,
    /// What the incoming side is named as after its marker.
    pub theirs_name: String,
    /// What our side is named as after its marker.
    pub ours_name: String,
}

/// The places of `text` between conflict markers, in order.
pub fn conflict_regions(text: &str) -> Vec<ConflictRegion> {
    let mut regions = Vec::new();
    let mut open: Option<ConflictRegion> = None;
    let mut in_theirs = false;
    let mut in_base = false;
    for (at, line) in text.lines().enumerate() {
        if let Some(name) = line.strip_prefix("<<<<<<<") {
            open = Some(ConflictRegion {
                start: at,
                ours_name: name.trim().to_owned(),
                ..ConflictRegion::default()
            });
            in_theirs = false;
            in_base = false;
        } else if open.is_some() && line.starts_with("|||||||") {
            in_base = true;
        } else if open.is_some() && line.starts_with("=======") {
            in_theirs = true;
            in_base = false;
        } else if let Some(name) = line.strip_prefix(">>>>>>>")
            && let Some(mut region) = open.take()
        {
            region.end = at;
            region.theirs_name = name.trim().to_owned();
            regions.push(region);
        } else if let Some(region) = &mut open {
            let side = if in_theirs {
                &mut region.theirs
            } else if in_base {
                continue;
            } else {
                &mut region.ours
            };
            side.push_str(line);
            side.push('\n');
        }
    }
    regions
}

/// `text` with the conflict at `index` among its regions replaced by `side`'s lines; the text unchanged when there is no such conflict.
pub fn settle_conflict(text: &str, index: usize, side: ConflictSide) -> String {
    let Some(region) = conflict_regions(text).into_iter().nth(index) else {
        return text.to_owned();
    };
    let taken = match side {
        ConflictSide::Ours => region.ours.clone(),
        ConflictSide::Theirs => region.theirs.clone(),
        ConflictSide::Both => format!("{}{}", region.ours, region.theirs),
    };
    let mut written = String::with_capacity(text.len());
    for (at, line) in text.split_inclusive('\n').enumerate() {
        if at == region.start {
            written.push_str(&taken);
        } else if at < region.start || at > region.end {
            written.push_str(line);
        }
    }
    written
}

/// `text` with every conflict replaced by `side`'s lines.
pub fn settle_all(text: &str, side: ConflictSide) -> String {
    let mut settled = text.to_owned();
    while !conflict_regions(&settled).is_empty() {
        settled = settle_conflict(&settled, 0, side);
    }
    settled
}

/// The message that hands `notes` to the agent: each note under the file and line it is on.
pub fn notes_prompt(notes: &[LineNote]) -> String {
    let mut prompt = String::from("Please address these review notes on the changes:\n");
    for note in notes {
        let side = if note.new { "" } else { " (old side)" };
        prompt.push_str(&format!(
            "\n- {}:{}{}: {}",
            note.path, note.line, side, note.text
        ));
    }
    prompt
}

/// The language a file at `path` is most likely written in, by its ending, as the server names installed languages.
pub fn language_by_ending(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ending = name.rsplit_once('.').map(|(_, ending)| ending)?;
    Some(match ending.to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "toml" => "toml",
        "go" => "go",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "json" => "json",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
        "cs" => "csharp",
        "css" => "css",
        "html" | "htm" => "html",
        "md" => "markdown",
        "sh" | "bash" => "bash",
        "yaml" | "yml" => "yaml",
        "lua" => "lua",
        "java" => "java",
        "rb" => "ruby",
        _ => return None,
    })
}

/// The grammar loaded for the language a file at `path` is written in, when one is.
pub fn grammar_by_ending(path: &str) -> Option<Arc<Grammar>> {
    language_by_ending(path).and_then(registry::loaded)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text of a file with one conflict between two plain lines.
    const MERGED: &str = "a\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\nb\n";

    /// A conflict is found with both sides and the names of its markers.
    #[test]
    fn finds_conflicts() {
        let regions = conflict_regions(MERGED);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].start, 1);
        assert_eq!(regions[0].end, 5);
        assert_eq!(regions[0].ours, "ours\n");
        assert_eq!(regions[0].theirs, "theirs\n");
        assert_eq!(regions[0].theirs_name, "feature");
    }

    /// Settling takes one side, or both in order, in place of the markers.
    #[test]
    fn settles_conflicts() {
        assert_eq!(
            settle_conflict(MERGED, 0, ConflictSide::Ours),
            "a\nours\nb\n"
        );
        assert_eq!(
            settle_conflict(MERGED, 0, ConflictSide::Both),
            "a\nours\ntheirs\nb\n"
        );
        assert_eq!(settle_all(MERGED, ConflictSide::Theirs), "a\ntheirs\nb\n");
    }

    /// Notes are listed under their file and line.
    #[test]
    fn notes_become_a_prompt() {
        let prompt = notes_prompt(&[LineNote {
            path: "a.rs".to_owned(),
            line: 3,
            new: true,
            text: "Rename this".to_owned(),
        }]);
        assert!(prompt.ends_with("- a.rs:3: Rename this"));
    }
}
