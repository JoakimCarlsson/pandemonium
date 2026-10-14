//! Repositories, worktrees, branches, status and diffs, through the `git` program.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod checkpoint;

pub use checkpoint::{checkpoint, restore};

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
        if index == 'R' || index == 'C' {
            entries.next();
        }
        let untracked = index == '?';
        let diff = if untracked {
            added(directory, path)
        } else {
            let against_head = run(directory, &["diff", "HEAD", "--no-color", "--", path]);
            let shown = against_head
                .or_else(|_| run(directory, &["diff", "--cached", "--no-color", "--", path]))
                .unwrap_or_default();
            hunks(&shown)
        };
        found.push(Change {
            path: path.to_owned(),
            diff,
            staged: !untracked && index != ' ' && tree == ' ',
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

/// Commits what is staged, under `message`.
pub fn commit(directory: &Path, message: &str) -> Result<(), String> {
    run(directory, &["commit", "--quiet", "-m", message]).map(|_| ())
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
