//! Repositories, worktrees, branches, status and diffs, through the `git` program.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where, under a repository, the worktrees made for tasks are kept.
const WORKTREES: &str = ".pandemonium/worktrees";

/// The most lines of a file's diff that are read.
const DIFF_LINES: usize = 2000;

/// The longest a branch name made from a title is, in characters.
const SLUG_LENGTH: usize = 40;

/// A file that differs from the last commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    /// Where the file is, from the root of the repository.
    pub path: String,
    /// The lines of its diff, from the first hunk on, each starting with `@`, `+`, `-` or a space.
    pub diff: String,
    /// Whether the change is staged for the next commit.
    pub staged: bool,
}

/// A worktree made for one task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Worktree {
    /// The folder it is in.
    pub path: PathBuf,
    /// The branch it has checked out.
    pub branch: String,
}

/// Runs `git` with `arguments` in `directory` and returns what it wrote, or what it complained of.
fn run(directory: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
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

/// The files of the repository `directory` is in that are tracked or not ignored, as paths from its root, at most `limit` of them.
pub fn files(directory: &Path, limit: usize) -> Vec<String> {
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

/// The local branches of the repository `directory` is in, the current one first.
pub fn branches(directory: &Path) -> Vec<String> {
    let listed = run(
        directory,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .unwrap_or_default();
    let mut names: Vec<String> = listed.lines().map(str::to_owned).collect();
    if let Some(current) = current_branch(directory)
        && let Some(at) = names.iter().position(|name| *name == current)
    {
        names.swap(0, at);
    }
    names
}

/// `title` as a branch name part: lower case words joined by dashes.
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
    write!(file, "{separator}.pandemonium/\n").map_err(|error| error.to_string())
}

/// Makes a new branch off `base` in a worktree of its own under `repository`, named from `title`.
pub fn add_worktree(repository: &Path, title: &str, base: &str) -> Result<Worktree, String> {
    exclude_worktrees(repository)?;
    let slug = slug(title);
    let root = repository.join(WORKTREES);
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut attempt = 0;
    loop {
        let name = if attempt == 0 {
            slug.clone()
        } else {
            format!("{slug}-{}", attempt + 1)
        };
        let branch = format!("task/{name}");
        let path = root.join(&name);
        let taken = path.exists()
            || run(
                repository,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{branch}"),
                ],
            )
            .is_ok();
        if taken {
            attempt += 1;
            continue;
        }
        let place = path.to_string_lossy().into_owned();
        run(
            repository,
            &["worktree", "add", "-b", &branch, &place, base],
        )?;
        return Ok(Worktree { path, branch });
    }
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
