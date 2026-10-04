//! A worktree-local chain of agent steps, turn boundaries and recoverable rewinds.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::diff::{Hunk, read, split};
use super::run::{Said, answer, git, piped};
use super::snapshot::snapshot;

/// The private chain tip, scoped by git to the current worktree.
pub const CHECKPOINT_HEAD: &str = "refs/worktree/pandemonium/head";

/// A step's provenance, read from its commit message.
#[derive(Clone, Debug)]
pub struct CheckpointStep {
    /// The full commit object id.
    pub commit: String,
    /// The operation's title.
    pub title: String,
    /// The provider's tool id, absent for reader and unattributed edits.
    pub tool: Option<String>,
    /// The provider's operation kind.
    pub kind: Option<String>,
    /// The turn that owns this step.
    pub turn: u64,
}

/// A persisted turn and its steps, independent of the loaded conversation.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    /// The monotonically increasing worktree turn number.
    pub turn: u64,
    /// The commit at the end of the turn.
    pub commit: String,
    /// The baseline after between-turn reader edits.
    pub start: String,
    /// The first line of the prompt, or the saved rewind's title.
    pub prompt: String,
    /// The changes recorded within this turn.
    pub steps: Vec<CheckpointStep>,
}

/// Appends a commit to the private chain without changing HEAD or the index.
pub fn checkpoint(root: &Path, parent: &str, tree: &str, message: &str) -> Said {
    let mut arguments = vec![
        "-c",
        "user.name=Pandemonium",
        "-c",
        "user.email=pandemonium@localhost",
        "commit-tree",
        tree,
    ];
    if !parent.is_empty() {
        arguments.extend(["-p", parent]);
    }
    let commit = piped(root, arguments, message)?.trim().to_owned();
    git(root, ["update-ref", CHECKPOINT_HEAD, &commit])?;
    Ok(commit)
}

/// Resolves a persisted turn boundary to its commit id.
pub fn checkpoint_at(root: &Path, turn: u64) -> Option<String> {
    answer(
        root,
        [
            "rev-parse",
            "--verify",
            &format!("refs/worktree/pandemonium/turns/{turn}"),
        ],
    )
    .map(|value| value.trim().to_owned())
}

/// Saves the current tip as a numbered turn boundary.
pub fn checkpoint_turn(root: &Path, turn: u64, commit: &str) -> Said {
    git(
        root,
        [
            "update-ref",
            &format!("refs/worktree/pandemonium/turns/{turn}"),
            commit,
        ],
    )
}

/// Captures the first baseline and reader edits before an agent starts writing.
pub fn begin_checkpoint(root: &Path) -> Result<u64, String> {
    let turn = checkpoints(root)
        .iter()
        .map(|turn| turn.turn)
        .max()
        .unwrap_or(0)
        + 1;
    begin_checkpoint_number(root, turn)
}

/// Starts a shared scope turn with the same number in every constituent worktree.
pub fn begin_checkpoint_number(root: &Path, turn: u64) -> Result<u64, String> {
    let tree = snapshot(root).ok_or("Could not snapshot the worktree")?;
    let mut tip =
        answer(root, ["rev-parse", "--verify", CHECKPOINT_HEAD]).map(|tip| tip.trim().to_owned());
    if tip.is_none() {
        let parent = answer(root, ["rev-parse", "--verify", "HEAD"]).unwrap_or_default();
        let initial = checkpoint(
            root,
            parent.trim(),
            &tree,
            "Before the first turn\n\nPandemonium-Turn: 0\n",
        )?;
        checkpoint_turn(root, 0, &initial)?;
        tip = Some(initial);
    }
    let mut tip = tip.unwrap_or_default();
    let old_tree = answer(root, ["rev-parse", &format!("{tip}^{{tree}}")]).unwrap_or_default();
    if old_tree.trim() != tree {
        tip = checkpoint(
            root,
            &tip,
            &tree,
            &format!("Edited by the reader\n\nPandemonium-Turn: {turn}\n"),
        )?;
    }
    git(
        root,
        [
            "update-ref",
            &format!("refs/worktree/pandemonium/starts/{turn}"),
            &tip,
        ],
    )?;
    Ok(turn)
}

/// Records changed files at a tool boundary, skipping unchanged trees.
pub fn checkpoint_step(
    root: &Path,
    turn: u64,
    title: &str,
    tool: Option<&str>,
    kind: Option<&str>,
) -> Said {
    let tree = snapshot(root).ok_or("Could not snapshot the worktree")?;
    let tip = answer(root, ["rev-parse", "--verify", CHECKPOINT_HEAD]).ok_or("No turn baseline")?;
    let tip = tip.trim();
    let old_tree =
        answer(root, ["rev-parse", &format!("{tip}^{{tree}}")]).ok_or("No checkpoint tree")?;
    if old_tree.trim() == tree {
        return Ok(tip.to_owned());
    }
    let mut message = format!(
        "{}\n\nPandemonium-Turn: {turn}\n",
        title
            .lines()
            .next()
            .filter(|line| !line.trim().is_empty())
            .unwrap_or("Worktree changed")
    );
    if let Some(tool) = tool {
        message.push_str(&format!(
            "Pandemonium-Tool: {}\n",
            tool.replace(['\r', '\n'], " ")
        ));
    }
    if let Some(kind) = kind {
        message.push_str(&format!(
            "Pandemonium-Kind: {}\n",
            kind.replace(['\r', '\n'], " ")
        ));
    }
    checkpoint(root, tip, &tree, &message)
}

/// Captures unreported changes and persists a prompt-labelled turn boundary.
pub fn end_checkpoint(root: &Path, turn: u64, prompt: &str) -> Said {
    let tip = checkpoint_step(root, turn, "Changes at turn end", None, None)?;
    let tree =
        answer(root, ["rev-parse", &format!("{tip}^{{tree}}")]).ok_or("No checkpoint tree")?;
    let commit = checkpoint(
        root,
        &tip,
        tree.trim(),
        &format!(
            "{}\n\nPandemonium-Turn: {turn}\n",
            prompt.lines().next().unwrap_or("Agent turn")
        ),
    )?;
    checkpoint_turn(root, turn, &commit)?;
    Ok(commit)
}

/// Reads the checkpoint chain's step messages, with full object ids.
pub fn checkpoint_steps(root: &Path) -> Vec<CheckpointStep> {
    let base = answer(
        root,
        [
            "rev-parse",
            "--verify",
            "refs/worktree/pandemonium/turns/0^",
        ],
    );
    let mut arguments = vec!["log", "--format=%H%x00%B%x00", CHECKPOINT_HEAD];
    let base = base.as_deref().map(str::trim);
    if let Some(base) = base {
        arguments.extend(["--not", base]);
    }
    let text = answer(root, arguments).unwrap_or_default();
    let fields = text.split('\0').collect::<Vec<_>>();
    fields
        .as_chunks::<2>()
        .0
        .iter()
        .filter_map(|fields| {
            let trailer = |name: &str| {
                fields[1]
                    .lines()
                    .find_map(|line| line.strip_prefix(name))
                    .map(str::to_owned)
            };
            Some(CheckpointStep {
                commit: fields[0].trim().to_owned(),
                title: fields[1].lines().next().unwrap_or_default().to_owned(),
                tool: trailer("Pandemonium-Tool: "),
                kind: trailer("Pandemonium-Kind: "),
                turn: trailer("Pandemonium-Turn: ")?.parse().ok()?,
            })
        })
        .collect()
}

/// Lists completed turns and saved pre-rewind states from worktree-local refs.
pub fn checkpoints(root: &Path) -> Vec<Checkpoint> {
    let refs = answer(
        root,
        [
            "for-each-ref",
            "--format=%(refname) %(objectname) %(subject)",
            "refs/worktree/pandemonium/",
        ],
    )
    .unwrap_or_default();
    let starts = refs
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let turn = parts
                .next()?
                .strip_prefix("refs/worktree/pandemonium/starts/")?
                .parse::<u64>()
                .ok()?;
            Some((turn, parts.next()?.to_owned()))
        })
        .collect::<HashMap<_, _>>();
    let steps = checkpoint_steps(root);
    let mut turns = refs
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let turn = parts
                .next()?
                .strip_prefix("refs/worktree/pandemonium/turns/")?
                .parse()
                .ok()?;
            let commit = parts.next()?.to_owned();
            let start = starts.get(&turn).cloned().unwrap_or_else(|| commit.clone());
            Some(Checkpoint {
                turn,
                commit,
                start,
                prompt: parts.next().unwrap_or_default().to_owned(),
                steps: steps
                    .iter()
                    .filter(|step| step.turn == turn)
                    .cloned()
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    turns.sort_by_key(|turn| turn.turn);
    turns
}

/// Compares two trees using the review pane's unified hunk parser.
pub fn between(root: &Path, from: &str, to: &str) -> HashMap<PathBuf, Vec<Hunk>> {
    let text = answer(
        root,
        [
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-renames",
            from,
            to,
            "--",
        ],
    )
    .unwrap_or_default();
    let mut files = split(&text)
        .into_iter()
        .map(|(path, patch)| (root.join(path), read(patch)))
        .collect::<HashMap<_, _>>();
    for path in rewind_paths(root, from, to).unwrap_or_default() {
        let absolute = root.join(&path);
        files.entry(absolute).or_insert_with(|| {
            let arguments = [
                OsStr::new("diff"),
                OsStr::new("--no-color"),
                OsStr::new("--no-ext-diff"),
                OsStr::new("--no-renames"),
                OsStr::new(from),
                OsStr::new(to),
                OsStr::new("--"),
                path.as_os_str(),
            ];
            read(&answer(root, arguments).unwrap_or_default())
        });
    }
    files
}

/// Lists changed paths without interpreting line-oriented path quoting.
pub fn rewind_paths(root: &Path, from: &str, to: &str) -> Result<Vec<PathBuf>, String> {
    let text = git(
        root,
        ["diff", "--name-only", "--no-renames", "-z", from, to, "--"],
    )?;
    Ok(text
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect())
}

/// Saves the current files, then restores a checkpoint, leaving HEAD and index alone.
pub fn rewind(root: &Path, to: &str) -> Said {
    let turn = checkpoints(root)
        .iter()
        .map(|turn| turn.turn)
        .max()
        .unwrap_or(0)
        + 1;
    rewind_number(root, to, turn)
}

/// Restores one constituent worktree with a scope-wide backup number.
pub fn rewind_number(root: &Path, to: &str, turn: u64) -> Said {
    let target = git(root, ["rev-parse", "--verify", &format!("{to}^{{commit}}")])?;
    let target = target.trim();
    let tree = snapshot(root).ok_or("Could not snapshot before rewinding")?;
    let tip = git(root, ["rev-parse", CHECKPOINT_HEAD])?;
    let destination = to
        .strip_prefix("refs/worktree/pandemonium/turns/")
        .map_or_else(|| to.to_owned(), |turn| format!("turn {turn}"));
    let saved = checkpoint(
        root,
        tip.trim(),
        &tree,
        &format!("Before rewinding to {destination}\n\nPandemonium-Turn: {turn}\n"),
    )?;
    checkpoint_turn(root, turn, &saved)?;
    let paths = rewind_paths(root, &tree, target)?;
    let target_paths = git(root, ["ls-tree", "-r", "--name-only", "-z", target])?;
    let exists = target_paths
        .split('\0')
        .map(PathBuf::from)
        .collect::<std::collections::HashSet<_>>();
    for path in paths.iter().filter(|path| !exists.contains(*path)) {
        let absolute = root.join(path);
        let result = std::fs::symlink_metadata(&absolute);
        match result {
            Ok(metadata) if metadata.is_dir() => {
                return Err(format!("Refusing to delete directory {}", path.display()));
            }
            Ok(_) => std::fs::remove_file(absolute).map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    let source = format!("--source={target}");
    let mut arguments = vec![
        OsStr::new("restore"),
        OsStr::new(&source),
        OsStr::new("--worktree"),
        OsStr::new("--"),
    ];
    arguments.extend(
        paths
            .iter()
            .filter(|path| exists.contains(*path))
            .map(|path| path.as_os_str()),
    );
    if arguments.len() > 4 {
        git(root, arguments)?;
    }
    git(
        root,
        ["update-ref", "refs/worktree/pandemonium/restored", target],
    )?;
    Ok(saved)
}

/// Attributes several hunks with one content read and one blame per file.
pub fn hunk_steps(
    root: &Path,
    path: &Path,
    revision: &str,
    hunks: &[Hunk],
    steps: &[CheckpointStep],
) -> Vec<Option<CheckpointStep>> {
    if hunks.is_empty() || steps.is_empty() {
        return vec![None; hunks.len()];
    }
    let text = path
        .strip_prefix(root)
        .ok()
        .and_then(|relative| relative.to_str())
        .and_then(|relative| answer(root, ["show", &format!("{revision}:{relative}")]))
        .unwrap_or_default();
    let lines = text.lines().collect::<Vec<_>>();
    let blame = super::blame::blame_at(root, path, revision);
    hunks
        .iter()
        .map(|hunk| attribute_hunk(hunk, &lines, &blame, steps))
        .collect()
}

/// Attributes changed new-side lines only when their text still matches the checkpoint.
pub fn hunk_step(
    root: &Path,
    path: &Path,
    revision: &str,
    hunk: &Hunk,
    steps: &[CheckpointStep],
) -> Option<CheckpointStep> {
    hunk_steps(root, path, revision, std::slice::from_ref(hunk), steps)
        .pop()
        .flatten()
}

/// Chooses a step only when it owns a strict majority of the changed new-side lines.
fn attribute_hunk(
    hunk: &Hunk,
    lines: &[&str],
    blame: &[super::blame::Blame],
    steps: &[CheckpointStep],
) -> Option<CheckpointStep> {
    let mut votes = HashMap::<&str, usize>::new();
    let changed = hunk
        .lines
        .iter()
        .filter(|line| line.kind == super::diff::LineKind::Added)
        .count();
    for line in hunk
        .lines
        .iter()
        .filter(|line| line.kind == super::diff::LineKind::Added)
    {
        let at = line.new?.checked_sub(1)?;
        if lines.get(at).copied() != Some(line.text.as_str()) {
            continue;
        }
        if let Some(blame) = blame.get(at) {
            *votes.entry(&blame.commit).or_default() += 1;
        }
    }
    let (commit, count) = votes.into_iter().max_by_key(|(_, count)| *count)?;
    if count * 2 <= changed {
        return None;
    }
    steps
        .iter()
        .find(|step| step.commit.starts_with(commit))
        .cloned()
}

/// Takes the context correction saved by a successful rewind, including after relaunch.
pub fn take_rewind_context(root: &Path) -> Option<String> {
    let message = answer(
        root,
        [
            "log",
            "-1",
            "--format=%B",
            "refs/worktree/pandemonium/restored",
        ],
    )?;
    let turn = message
        .lines()
        .find_map(|line| line.strip_prefix("Pandemonium-Turn: "))?
        .parse::<u64>()
        .ok()?;
    git(
        root,
        ["update-ref", "-d", "refs/worktree/pandemonium/restored"],
    )
    .ok()?;
    Some(format!(
        "Worktree files were restored to the end of turn {turn}. Your conversation context has not been rewound."
    ))
}
