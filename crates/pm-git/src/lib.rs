//! Repositories, worktrees, branches, status and diffs, through the `git` program.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod checkpoint;
mod github;
mod graph;
mod operation;
mod places;

pub use checkpoint::{checkpoint, restore};
pub use github::{Owner, Repository, owners, repositories};
pub use graph::{Edge, Half, Lanes};
pub use operation::{Operation, operation, step};
pub use places::{clone, folders};

/// Where, under a repository, the worktrees made for tasks are kept.
const WORKTREES: &str = ".pandemonium/worktrees";

/// The most lines of a file's diff that are read.
const DIFF_LINES: usize = 2000;

/// The longest a folder name made from a title is, in characters.
const SLUG_LENGTH: usize = 40;

/// A file that differs from the last commit.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Change {
    /// Where the file is, from the root of the repository.
    pub path: String,
    /// The lines of its diff, from the first hunk on, each starting with `@`, `+`, `-` or a space.
    pub diff: String,
    /// Whether the change is staged for the next commit.
    pub staged: bool,
    /// Whether the file is one git does not track yet.
    #[serde(default)]
    pub untracked: bool,
    /// What the index holds about the file, as the letter git writes it with (`M`, `A`, `D`, `R`, `U`), when it holds anything.
    #[serde(default)]
    pub index: Option<char>,
    /// What the working folder holds about the file that the index does not, as the letter git writes it with (`M`, `D`, `?`, `U`), when it holds anything.
    #[serde(default)]
    pub worktree: Option<char>,
    /// Whether the file is changed on both sides of a merge.
    #[serde(default)]
    pub conflicted: bool,
    /// Where the file was before it was renamed, when it was.
    #[serde(default)]
    pub from: Option<String>,
}

/// A worktree made for one task, on a detached head so that no branch has to exist for it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Worktree {
    /// The folder it is in.
    pub path: PathBuf,
    /// The abbreviated commit it has checked out.
    pub head: String,
}

/// Runs `git` with `arguments` in `directory` and returns what it wrote, or what it complained of.
fn run(directory: &Path, arguments: &[&str]) -> Result<String, String> {
    run_indexed(directory, arguments, None)
}

/// Runs `git` with `arguments` in `directory`, staging into the file `index` in place of the repository's own when one is given, and returns what it wrote, or what it complained of.
fn run_indexed(
    directory: &Path,
    arguments: &[&str],
    index: Option<&Path>,
) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null());
    if let Some(index) = index {
        command.env("GIT_INDEX_FILE", index);
    }
    let output = command
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let complaint = String::from_utf8_lossy(&output.stderr);
        Err(complaint.trim().to_owned())
    }
}

/// Whether `directory` is inside a Git repository.
pub fn is_repository(directory: &Path) -> bool {
    directory.is_dir() && run(directory, &["rev-parse", "--git-dir"]).is_ok()
}

/// The folders a plain folder's listing does not go into.
const SKIPPED: [&str; 4] = ["node_modules", "target", "build", "dist"];

/// The files under `directory`, as paths from `root`, at most `limit` of them, leaving out hidden and bulky folders.
fn walk(root: &Path, directory: &Path, limit: usize, found: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if found.len() >= limit {
            return;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if !name.starts_with('.') && !SKIPPED.contains(&name.as_str()) {
                walk(root, &path, limit, found);
            }
        } else if kind.is_file()
            && let Ok(relative) = path.strip_prefix(root)
        {
            found.push(relative.to_string_lossy().into_owned());
        }
    }
}

/// The files of the folder `directory`, as paths from it, at most `limit` of them: those a repository tracks or does not ignore, or, in a folder that is not a repository, every file but those in hidden and bulky folders.
pub fn files(directory: &Path, limit: usize) -> Vec<String> {
    if !is_repository(directory) {
        let mut found = Vec::new();
        walk(directory, directory, limit, &mut found);
        return found;
    }
    run(
        directory,
        &["ls-files", "--cached", "--others", "--exclude-standard"],
    )
    .map(|listed| listed.lines().take(limit).map(str::to_owned).collect())
    .unwrap_or_default()
}

/// The branch `directory` has checked out, or nothing when it has none or is not a repository.
pub fn current_branch(directory: &Path) -> Option<String> {
    let name = run(directory, &["rev-parse", "--abbrev-ref", "HEAD"]).ok()?;
    let name = name.trim();
    (!name.is_empty() && name != "HEAD").then(|| name.to_owned())
}

/// The names of the references under `prefix` in the repository `directory` is in, leaving out a remote's `HEAD`.
fn references(directory: &Path, prefix: &str) -> Vec<String> {
    run(
        directory,
        &["for-each-ref", "--format=%(refname:short)", prefix],
    )
    .unwrap_or_default()
    .lines()
    .filter(|name| !name.ends_with("/HEAD") && *name != "origin")
    .map(str::to_owned)
    .collect()
}

/// The branches of the repository `directory` is in as it has them now: the local ones, the current one first, then those of its remotes.
pub fn branches(directory: &Path) -> Vec<String> {
    let mut names = references(directory, "refs/heads");
    if let Some(current) = current_branch(directory)
        && let Some(at) = names.iter().position(|name| *name == current)
    {
        names.swap(0, at);
    }
    names.extend(references(directory, "refs/remotes"));
    names
}

/// The commit `reference` names in the repository `directory` is in, abbreviated, or nothing when it names none.
pub fn resolve(directory: &Path, reference: &str) -> Option<String> {
    let commit = format!("{reference}^{{commit}}");
    run(
        directory,
        &["rev-parse", "--verify", "--quiet", "--short", &commit],
    )
    .ok()
    .map(|short| short.trim().to_owned())
    .filter(|short| !short.is_empty())
}

/// `title` as a folder name: lower case words joined by dashes.
fn slug(title: &str) -> String {
    let mut slug = String::new();
    for character in title.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug: String = slug.chars().take(SLUG_LENGTH).collect();
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "work".to_owned()
    } else {
        slug.to_owned()
    }
}

/// Keeps the folder the worktrees are made in out of the repository's own status.
fn exclude_worktrees(repository: &Path) -> Result<(), String> {
    let common = run(repository, &["rev-parse", "--git-common-dir"])?;
    let common = repository.join(common.trim());
    let exclude = common.join("info/exclude");
    let held = fs::read_to_string(&exclude).unwrap_or_default();
    if held.lines().any(|line| line.trim() == ".pandemonium/") {
        return Ok(());
    }
    fs::create_dir_all(common.join("info")).map_err(|error| error.to_string())?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(exclude)
        .map_err(|error| error.to_string())?;
    let separator = if held.is_empty() || held.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    writeln!(file, "{separator}.pandemonium/").map_err(|error| error.to_string())
}

/// Makes a worktree of its own under `repository`, named from `title`, with `base` checked out on a detached head.
///
/// `base` is looked up in the repository now, so a branch deleted a moment ago is refused rather than assumed.
pub fn add_worktree(repository: &Path, title: &str, base: &str) -> Result<Worktree, String> {
    let head = resolve(repository, base).ok_or_else(|| format!("{base} does not exist"))?;
    exclude_worktrees(repository)?;
    let slug = slug(title);
    let root = repository.join(WORKTREES);
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut attempt = 0;
    let path = loop {
        let name = if attempt == 0 {
            slug.clone()
        } else {
            format!("{slug}-{}", attempt + 1)
        };
        let path = root.join(name);
        if !path.exists() {
            break path;
        }
        attempt += 1;
    };
    let place = path.to_string_lossy().into_owned();
    run(repository, &["worktree", "add", "--detach", &place, &head])?;
    Ok(Worktree { path, head })
}

/// The lines of the diff `text` holds, from the first hunk on.
fn hunks(text: &str) -> String {
    text.lines()
        .skip_while(|line| !line.starts_with("@@"))
        .take(DIFF_LINES)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The diff of a file that is new, as every line added.
fn added(directory: &Path, path: &str) -> String {
    let Ok(held) = fs::read_to_string(directory.join(path)) else {
        return String::new();
    };
    let lines: Vec<&str> = held.lines().take(DIFF_LINES).collect();
    let mut diff = format!("@@ -0,0 +1,{} @@", lines.len());
    for line in lines {
        diff.push_str("\n+");
        diff.push_str(line);
    }
    diff
}

/// Whether the two sides of a status pair say the file is changed on both sides of a merge.
fn unmerged(index: char, tree: char) -> bool {
    index == 'U' || tree == 'U' || matches!((index, tree), ('A', 'A') | ('D', 'D'))
}

/// Every file of the repository `directory` is in that differs from the last commit.
pub fn changes(directory: &Path) -> Vec<Change> {
    let Ok(status) = run(directory, &["status", "--porcelain=v1", "-z", "-uall"]) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut entries = status.split('\0').filter(|entry| entry.len() > 3);
    while let Some(entry) = entries.next() {
        let (marks, path) = entry.split_at(3);
        let mut marks = marks.chars();
        let (index, tree) = (marks.next().unwrap_or(' '), marks.next().unwrap_or(' '));
        let from = if index == 'R' || index == 'C' {
            entries.next().map(str::to_owned)
        } else {
            None
        };
        let untracked = index == '?';
        let conflicted = unmerged(index, tree);
        let diff = if untracked {
            added(directory, path)
        } else {
            let against_head = run(directory, &["diff", "HEAD", "--no-color", "--", path]);
            let shown = against_head
                .or_else(|_| run(directory, &["diff", "--cached", "--no-color", "--", path]))
                .unwrap_or_default();
            hunks(&shown)
        };
        let side = |mark: char| (!matches!(mark, ' ' | '!')).then_some(mark);
        let (index_side, worktree_side) = if untracked {
            (None, Some('?'))
        } else {
            (side(index), side(tree))
        };
        found.push(Change {
            path: path.to_owned(),
            diff,
            staged: !untracked && !conflicted && index != ' ' && tree == ' ',
            untracked,
            index: index_side,
            worktree: worktree_side,
            conflicted,
            from,
        });
    }
    found
}

/// Stages `paths`.
pub fn stage(directory: &Path, paths: &[String]) -> Result<(), String> {
    let mut arguments = vec!["add", "--"];
    arguments.extend(paths.iter().map(String::as_str));
    run(directory, &arguments).map(|_| ())
}

/// Takes `paths` out of the next commit, leaving their changes in the files.
pub fn unstage(directory: &Path, paths: &[String]) -> Result<(), String> {
    let mut arguments = vec!["reset", "--quiet", "--"];
    arguments.extend(paths.iter().map(String::as_str));
    run(directory, &arguments).map(|_| ())
}

/// Commits what is staged, under `message`; with nothing staged, `tracked` takes the changes of every tracked file instead, and `amend` rewrites the last commit in place of making another.
pub fn commit(directory: &Path, message: &str, tracked: bool, amend: bool) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("A commit needs a message".to_owned());
    }
    let mut arguments = vec!["commit", "--quiet"];
    if amend {
        arguments.push("--amend");
    }
    if tracked {
        arguments.push("--all");
    }
    arguments.extend(["-m", message]);
    run(directory, &arguments).map(|_| ())
}

/// One commit of the history of a folder.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Commit {
    /// The abbreviated object name.
    pub id: String,
    /// The branches and tags pointing at the commit, as git decorates them.
    pub refs: Vec<String>,
    /// The first line of the commit message.
    pub summary: String,
    /// How the commit's row of the graph is drawn.
    pub lanes: Lanes,
}

/// One line of the history as git lists it, before the lanes are laid out.
struct Listed {
    /// The full object name.
    object: String,
    /// The full object names of the parents, first parent first.
    parents: Vec<String>,
    /// The abbreviated object name.
    id: String,
    /// The branches and tags pointing at the commit.
    refs: Vec<String>,
    /// The first line of the commit message.
    summary: String,
}

/// Reads one tab-separated line of the history.
fn listed(line: &str) -> Option<Listed> {
    let mut fields = line.splitn(5, '\t');
    let object = fields.next()?.to_owned();
    let parents = fields
        .next()?
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let id = fields.next()?.to_owned();
    let refs = fields
        .next()?
        .split(", ")
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    let summary = fields.next().unwrap_or_default().to_owned();
    Some(Listed {
        object,
        parents,
        id,
        refs,
        summary,
    })
}

/// The most recent `limit` commits of the repository `directory` is in, with the lanes of the graph they are drawn in; every reference's when `all`, and what the checked out branch reaches when not.
pub fn history(directory: &Path, limit: usize, all: bool) -> Vec<Commit> {
    let count = format!("-{}", limit.max(1));
    let mut arguments = vec![
        "log",
        &count,
        "--date-order",
        "--pretty=format:%H%x09%P%x09%h%x09%D%x09%s",
    ];
    if all {
        arguments.push("--all");
    }
    let listed: Vec<Listed> = run(directory, &arguments)
        .map(|said| said.lines().filter_map(listed).collect())
        .unwrap_or_default();
    let rows = graph::lanes(
        listed
            .iter()
            .map(|commit| (commit.object.as_str(), commit.parents.as_slice())),
    );
    listed
        .into_iter()
        .zip(rows)
        .map(|(commit, lanes)| Commit {
            id: commit.id,
            refs: commit.refs,
            summary: commit.summary,
            lanes,
        })
        .collect()
}

/// How a branch stands against the one it follows.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tracking {
    /// The branch it follows, when it follows one.
    pub upstream: Option<String>,
    /// How many commits it has that the one it follows has not.
    pub ahead: usize,
    /// How many commits the one it follows has that it has not.
    pub behind: usize,
}

/// What the checked out branch of the repository `directory` is in follows, and how far apart they are.
pub fn tracking(directory: &Path) -> Tracking {
    let Ok(upstream) = run(
        directory,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    ) else {
        return Tracking::default();
    };
    let counts = run(
        directory,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
    )
    .unwrap_or_default();
    let mut numbers = counts
        .split_whitespace()
        .filter_map(|number| number.parse().ok());
    Tracking {
        upstream: Some(upstream.trim().to_owned()),
        ahead: numbers.next().unwrap_or(0),
        behind: numbers.next().unwrap_or(0),
    }
}

/// Talks to the remote of the repository `directory` is in: `fetch` from all of them, `pull`, `pull_rebase`, `push` the branch, publishing it when it follows nothing, `force_push` without overwriting what was not seen, or `sync` by pulling and then pushing.
pub fn remote(directory: &Path, operation: &str) -> Result<(), String> {
    match operation {
        "fetch" => run(directory, &["fetch", "--all", "--prune"]).map(|_| ()),
        "pull" => run(directory, &["pull", "--prune"]).map(|_| ()),
        "pull_rebase" => run(directory, &["pull", "--prune", "--rebase"]).map(|_| ()),
        "force_push" => run(directory, &["push", "--force-with-lease"]).map(|_| ()),
        "push" => push(directory),
        "sync" => {
            run(directory, &["pull", "--prune"])?;
            push(directory)
        }
        other => Err(format!("{other} is not something to ask of a remote")),
    }
}

/// Pushes the checked out branch, and makes it follow what it was pushed to when it follows nothing.
fn push(directory: &Path) -> Result<(), String> {
    if tracking(directory).upstream.is_some() {
        return run(directory, &["push"]).map(|_| ());
    }
    let remotes = run(directory, &["remote"]).unwrap_or_default();
    let remote = remotes
        .lines()
        .find(|remote| *remote == "origin")
        .or_else(|| remotes.lines().next())
        .ok_or_else(|| "this repository has no remote to publish to".to_owned())?;
    run(directory, &["push", "--set-upstream", remote, "HEAD"]).map(|_| ())
}

/// The messages of the commits `directory`'s branch has that `base` has not, oldest first.
pub fn commits_since(directory: &Path, base: &str) -> Vec<String> {
    run(
        directory,
        &["log", "--reverse", "--format=%s", &format!("{base}..HEAD")],
    )
    .map(|log| log.lines().map(str::to_owned).collect())
    .unwrap_or_default()
}

/// Whether `path` is in the last commit of the repository `directory` is in.
fn in_head(directory: &Path, path: &str) -> bool {
    run(directory, &["cat-file", "-e", &format!("HEAD:{path}")]).is_ok()
}

/// Puts `paths` back as they are in the last commit and takes them out of the next one: a file that is new is deleted.
pub fn discard(directory: &Path, paths: &[String]) -> Result<(), String> {
    for path in paths {
        if in_head(directory, path) {
            run(
                directory,
                &[
                    "restore",
                    "--source=HEAD",
                    "--staged",
                    "--worktree",
                    "--",
                    path,
                ],
            )?;
        } else {
            run(
                directory,
                &[
                    "rm",
                    "-rf",
                    "--cached",
                    "--ignore-unmatch",
                    "--quiet",
                    "--",
                    path,
                ],
            )?;
            run(directory, &["clean", "-fdq", "--", path])?;
        }
    }
    Ok(())
}

/// Adds `paths` to the `.gitignore` at the root of the repository `directory` is in, one line each, unless they are in it already.
pub fn ignore(directory: &Path, paths: &[String]) -> Result<(), String> {
    let root = run(directory, &["rev-parse", "--show-toplevel"])?;
    let file = Path::new(root.trim()).join(".gitignore");
    let mut text = fs::read_to_string(&file).unwrap_or_default();
    for path in paths {
        let line = format!("/{path}");
        if text.lines().any(|held| held == line) {
            continue;
        }
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&line);
        text.push('\n');
    }
    fs::write(&file, text).map_err(|error| format!("could not write .gitignore: {error}"))
}

/// Checks out the branch `name`; one of a remote's gets the local branch of its name, made to follow it when there is none yet.
pub fn checkout(directory: &Path, name: &str) -> Result<(), String> {
    run(directory, &["switch", "--quiet", name])
        .or_else(|first| {
            let Some((_, local)) = name.split_once('/') else {
                return Err(first);
            };
            if references(directory, "refs/heads")
                .iter()
                .any(|held| held == local)
            {
                run(directory, &["switch", "--quiet", local])
            } else {
                run(directory, &["switch", "--quiet", "--track", name])
            }
        })
        .map(|_| ())
}

/// Makes the branch `name` at the commit the folder is at and checks it out.
pub fn create_branch(directory: &Path, name: &str) -> Result<(), String> {
    run(directory, &["switch", "--quiet", "--create", name]).map(|_| ())
}

/// The most bytes of a file that are read to be shown.
const READ_LIMIT: usize = 2_000_000;

/// What a file holds, read for showing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Contents {
    /// The text of the file, cut short when it was longer than could be shown.
    Text {
        /// The text.
        text: String,
        /// Whether it was cut short.
        truncated: bool,
    },
    /// The file is not text.
    Binary,
}

/// What `path` holds in the working folder, or, when `head`, in the last commit of the repository `directory` is in.
pub fn read(directory: &Path, path: &str, head: bool) -> Result<Contents, String> {
    let bytes = if head {
        let output = Command::new("git")
            .args(["show", &format!("HEAD:{path}")])
            .current_dir(directory)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("could not run git: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        output.stdout
    } else {
        fs::read(directory.join(path)).map_err(|error| format!("could not read {path}: {error}"))?
    };
    if bytes.iter().take(8000).any(|byte| *byte == 0) {
        return Ok(Contents::Binary);
    }
    let truncated = bytes.len() > READ_LIMIT;
    let kept = &bytes[..bytes.len().min(READ_LIMIT)];
    Ok(Contents::Text {
        text: String::from_utf8_lossy(kept).into_owned(),
        truncated,
    })
}

/// One saved stash entry.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stash {
    /// The stash's index in the current list.
    pub index: usize,
    /// The message shown for the stash.
    pub message: String,
    /// Its creation time as Unix seconds.
    pub when: i64,
}

/// The stashes of the repository `directory` is in, newest first.
pub fn stashes(directory: &Path) -> Vec<Stash> {
    run(directory, &["stash", "list", "--format=%gd%x09%ct%x09%gs"])
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let index = fields
                .next()?
                .strip_prefix("stash@{")?
                .strip_suffix('}')?
                .parse()
                .ok()?;
            let when = fields.next()?.parse().ok()?;
            let message = fields.next()?.to_owned();
            Some(Stash {
                index,
                message,
                when,
            })
        })
        .collect()
}

/// Saves the changes of the repository `directory` is in, untracked files included, under `message` when there is one, or `apply`, `pop` or `drop` the stash at `index`.
pub fn stash(
    directory: &Path,
    action: &str,
    index: Option<usize>,
    message: Option<&str>,
) -> Result<(), String> {
    let name = format!("stash@{{{}}}", index.unwrap_or(0));
    match action {
        "push" => {
            let mut arguments = vec!["stash", "push", "--include-untracked"];
            if let Some(message) = message.filter(|message| !message.trim().is_empty()) {
                arguments.extend(["-m", message]);
            }
            run(directory, &arguments)
        }
        "apply" | "pop" | "drop" => run(directory, &["stash", action, &name]),
        other => Err(format!("{other} is not something to do with a stash")),
    }
    .map(|_| ())
}

/// Applies `commit` as a new commit on the checked out branch.
pub fn cherry_pick(directory: &Path, commit: &str) -> Result<(), String> {
    run(directory, &["cherry-pick", commit]).map(|_| ())
}
