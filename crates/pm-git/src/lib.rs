//! Repositories, worktrees, branches, status and diffs, through the `git` program.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod carry;
mod checkpoint;
mod conflict;
mod file_log;
mod github;
mod graph;
mod operation;
mod places;
mod revision;
mod submodule;
mod words;

pub use checkpoint::{checkpoint, restore};
pub use conflict::{Sides, sides};
pub use file_log::{Blame, FileCommit, Found, blame, file_history, permalink, search};
pub use github::{Owner, Repository, owners, repositories};
pub use graph::{Edge, Half, Lanes};
pub use operation::{Operation, operation, step};
pub use places::{clone, folders};
pub use revision::revision_changes;
pub use submodule::{populate, submodule_heads, submodules};
pub use words::{Words, words};

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
    /// The words that changed inside its removed and added lines.
    #[serde(default)]
    pub words: Vec<Words>,
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
    /// The submodule the file is in, from the root of the repository, or nothing when it is in the repository itself.
    #[serde(default)]
    pub submodule: Option<String>,
    /// What a submodule checked out at another commit than the one recorded moved through, when the change is that.
    #[serde(default)]
    pub moved: Option<Moved>,
}

/// A submodule checked out at another commit than the one its repository records.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Moved {
    /// The commit recorded, abbreviated, or nothing when the submodule is new.
    pub from: Option<String>,
    /// The commit checked out, abbreviated.
    pub to: String,
    /// The subjects of the commits between them, oldest first.
    pub commits: Vec<String>,
    /// Whether it moved back: the commits are those the recorded one has that the checked out one has not.
    pub back: bool,
}

/// A worktree made for one task, on a detached head so that no branch has to exist for it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Worktree {
    /// The folder it is in.
    pub path: PathBuf,
    /// The abbreviated commit it has checked out.
    pub head: String,
    /// The branch it was given, when it was given one rather than a detached head.
    pub branch: Option<String>,
    /// The submodules that could not be checked out in it and the ignored files that could not be copied into it, each as its path and why.
    pub skipped: Vec<(String, String)>,
}

/// Where a new worktree is made and what branch it is on.
#[derive(Clone, Debug, Default)]
pub struct Placement {
    /// The folder worktrees are made in, one folder per repository inside it, or a folder inside the repository when nothing.
    pub root: Option<PathBuf>,
    /// What the worktree's branch is called, with `{title}` standing for the title made into a name, or a detached head when nothing.
    pub branch_template: Option<String>,
    /// The ignored files and folders of the repository, as paths from its root, that are copied into every new worktree.
    pub include: Vec<String>,
}

impl Placement {
    /// The folder the worktrees of `repository` are made in.
    fn folder_for(&self, repository: &Path) -> PathBuf {
        match &self.root {
            Some(root) => root.join(
                repository
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "repository".to_owned()),
            ),
            None => repository.join(WORKTREES),
        }
    }

    /// The branch a worktree whose folder is called `name` is on, or nothing for a detached head.
    fn branch_for(&self, name: &str) -> Option<String> {
        let template = self.branch_template.as_deref()?.trim();
        if template.is_empty() {
            return None;
        }
        Some(if template.contains("{title}") {
            template.replace("{title}", name)
        } else {
            format!("{}/{name}", template.trim_end_matches('/'))
        })
    }
}

/// Whether `repository` has a branch called `branch`.
fn has_branch(repository: &Path, branch: &str) -> bool {
    run(
        repository,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
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
        .env("GIT_LITERAL_PATHSPECS", "1")
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

/// The folder git keeps the state of the worktree at `directory` in, and the one it shares with every worktree of the repository, as full paths; the same folder twice for a repository's own worktree.
pub fn git_folders(directory: &Path) -> Option<(PathBuf, PathBuf)> {
    let said = run(
        directory,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
        ],
    )
    .ok()?;
    let mut lines = said.lines().map(|line| PathBuf::from(line.trim()));
    Some((lines.next()?, lines.next()?))
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

/// The files of the folder `directory`, as paths from it, at most `limit` of them: those a repository and its submodules track or do not ignore, or, in a folder that is not a repository, every file but those in hidden and bulky folders.
pub fn files(directory: &Path, limit: usize) -> Vec<String> {
    if !is_repository(directory) {
        let mut found = Vec::new();
        walk(directory, directory, limit, &mut found);
        return found;
    }
    let submodules = submodule::submodules(directory);
    let listed = |folder: &Path, arguments: &[&str]| -> Vec<String> {
        run(folder, arguments)
            .map(|listed| listed.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    };
    let untracked = ["ls-files", "--others", "--exclude-standard"];
    let mut found: Vec<String> =
        listed(directory, &["ls-files", "--cached", "--recurse-submodules"])
            .into_iter()
            .filter(|path| !submodules.contains(path))
            .chain(listed(directory, &untracked))
            .collect();
    for path in &submodules {
        found.extend(
            listed(&directory.join(path), &untracked)
                .into_iter()
                .map(|file| format!("{path}/{file}")),
        );
    }
    found.truncate(limit);
    found
}

/// The untracked folders of the repository `directory` is in, as paths from it, at most `limit` of them: those that hold no tracked file, which is how a folder with nothing in it yet is told of.
pub fn untracked_folders(directory: &Path, limit: usize) -> Vec<String> {
    if !is_repository(directory) {
        return Vec::new();
    }
    run(
        directory,
        &["ls-files", "--others", "--directory", "--exclude-standard"],
    )
    .map(|listed| {
        listed
            .lines()
            .filter_map(|line| line.strip_suffix('/'))
            .take(limit)
            .map(str::to_owned)
            .collect()
    })
    .unwrap_or_default()
}

/// The ignored files and folders of the repository `directory` is in, as paths from it, at most `limit` of them; a folder is written with a `/` at its end, and its contents are listed separately when expanded.
pub fn ignored(directory: &Path, limit: usize) -> Vec<String> {
    if !is_repository(directory) {
        return Vec::new();
    }
    run(
        directory,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--directory",
            "--exclude-standard",
        ],
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

/// Makes a worktree of its own of `repository`, named from `title`, with `base` checked out, where `placement` says and on the branch it names or a detached head, checks out its submodules, each on that branch too, and copies into it the ignored files `placement` lists.
///
/// `base` is looked up in the repository now, so a branch deleted a moment ago is refused rather than assumed.
pub fn add_worktree(
    repository: &Path,
    title: &str,
    base: &str,
    placement: &Placement,
) -> Result<Worktree, String> {
    let head = resolve(repository, base).ok_or_else(|| format!("{base} does not exist"))?;
    if placement.root.is_none() {
        exclude_worktrees(repository)?;
    }
    let slug = slug(title);
    let root = placement.folder_for(repository);
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut attempt = 0;
    let (path, branch) = loop {
        let name = if attempt == 0 {
            slug.clone()
        } else {
            format!("{slug}-{}", attempt + 1)
        };
        let path = root.join(&name);
        let branch = placement.branch_for(&name);
        let taken = branch
            .as_deref()
            .is_some_and(|branch| has_branch(repository, branch));
        if !path.exists() && !taken {
            break (path, branch);
        }
        attempt += 1;
    };
    let place = path.to_string_lossy().into_owned();
    match &branch {
        Some(branch) => run(
            repository,
            &["worktree", "add", "-b", branch, &place, &head],
        )?,
        None => run(repository, &["worktree", "add", "--detach", &place, &head])?,
    };
    let mut skipped = submodule::populate(&path);
    if let Some(branch) = &branch {
        skipped.extend(submodule::branch_all(&path, branch));
    }
    skipped.extend(carry::carry_ignored(repository, &path, &placement.include));
    Ok(Worktree {
        path,
        head,
        branch,
        skipped,
    })
}

/// Removes the worktree at `path` of `repository` with every file in it, and forgets it, then deletes `branch`, the branch it was made on from `base`, when every commit on it is also on `base` or a remote.
///
/// A folder that is already gone is only forgotten, and a branch with commits of its own is kept.
pub fn remove_worktree(
    repository: &Path,
    path: &Path,
    branch: Option<&str>,
    base: Option<&str>,
) -> Result<(), String> {
    let place = path.to_string_lossy().into_owned();
    if path.exists() {
        run(repository, &["worktree", "remove", "--force", &place])?;
    }
    run(repository, &["worktree", "prune"])?;
    if let (Some(branch), Some(base)) = (branch, base)
        && has_branch(repository, branch)
        && merged(repository, branch, base)
    {
        let _ = run(repository, &["branch", "-D", branch]);
    }
    Ok(())
}

/// Whether every commit on `branch` of `repository` is also on `base` or a remote.
fn merged(repository: &Path, branch: &str, base: &str) -> bool {
    run(
        repository,
        &[
            "rev-list",
            "--count",
            branch,
            "--not",
            base,
            "--remotes",
            "--",
        ],
    )
    .is_ok_and(|count| count.trim() == "0")
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

/// Every file of the repository `directory` is in that differs from the last commit, those inside its submodules included under their paths from its root, and each submodule checked out at another commit than the one recorded.
pub fn changes(directory: &Path) -> Vec<Change> {
    let direct = submodule::direct(directory);
    let mut found = Vec::new();
    for change in own_changes(directory) {
        if !direct.contains(&change.path) {
            found.push(change);
            continue;
        }
        let path = change.path.clone();
        let inner = directory.join(&path);
        found.extend(submodule::moved(directory, change));
        if submodule::populated(&inner) {
            found.extend(
                changes(&inner)
                    .into_iter()
                    .map(|change| submodule::within(change, &path)),
            );
        }
    }
    found
}

/// The files of the repository `directory` is in itself that differ from its last commit, a submodule as one entry.
fn own_changes(directory: &Path) -> Vec<Change> {
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
            words: words(&diff),
            diff,
            staged: !untracked && !conflicted && index != ' ' && tree == ' ',
            untracked,
            index: index_side,
            worktree: worktree_side,
            conflicted,
            from,
            submodule: None,
            moved: None,
        });
    }
    found
}

/// `path` when it names a file inside the repository and nothing else: not empty, not absolute, never stepping out with `..`, and free of line breaks and NUL.
fn inside(path: &str) -> Result<&str, String> {
    let steps_out = Path::new(path)
        .components()
        .any(|part| !matches!(part, std::path::Component::Normal(_)));
    if path.is_empty() || steps_out || path.contains(['\0', '\n', '\r']) {
        return Err(format!("{path:?} is not a path inside the repository"));
    }
    Ok(path)
}

/// Every one of `paths`, each checked to be inside the repository.
fn all_inside(paths: &[String]) -> Result<Vec<&str>, String> {
    paths.iter().map(|path| inside(path)).collect()
}

/// Runs `git` with `command` and then `paths` in each repository `paths` of `directory` are in, the repository itself or one of its submodules, each path as it is from there.
fn run_by_owner(directory: &Path, command: &[&str], paths: &[String]) -> Result<(), String> {
    for (folder, inner) in submodule::by_owner(directory, &all_inside(paths)?) {
        let mut arguments = command.to_vec();
        arguments.extend(inner.iter().map(String::as_str));
        run(&folder, &arguments)?;
    }
    Ok(())
}

/// Stages `paths`, each in the repository it is in: a file inside a submodule in the submodule, a submodule itself at the commit it is at in the repository around it.
pub fn stage(directory: &Path, paths: &[String]) -> Result<(), String> {
    run_by_owner(directory, &["add", "--"], paths)
}

/// Takes `paths` out of the next commit of the repository each is in, leaving their changes in the files.
pub fn unstage(directory: &Path, paths: &[String]) -> Result<(), String> {
    run_by_owner(directory, &["reset", "--quiet", "--"], paths)
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
    let count = format!("-{}", limit.clamp(1, HISTORY_LIMIT));
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

/// Pushes the checked out branch, after the submodules' own commits, and makes it follow what it was pushed to when it follows nothing.
fn push(directory: &Path) -> Result<(), String> {
    submodule::push_inside(directory)?;
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

/// Puts `paths` back as they are in the last commit of the repository each is in and takes them out of the next one: a file that is new is deleted, and a submodule is checked out at the commit recorded.
pub fn discard(directory: &Path, paths: &[String]) -> Result<(), String> {
    for (folder, inner) in submodule::by_owner(directory, &all_inside(paths)?) {
        discard_in(&folder, &inner)?;
    }
    Ok(())
}

/// Puts `paths` of the repository at `directory` back as they are in its last commit, and checks out each that is a submodule at the commit recorded.
fn discard_in(directory: &Path, paths: &[String]) -> Result<(), String> {
    let held = submodule::direct(directory);
    for path in paths.iter().map(String::as_str) {
        let recorded = || {
            run(
                directory,
                &["rev-parse", "--verify", "--quiet", &format!("HEAD:{path}")],
            )
            .is_ok()
        };
        if held.iter().any(|submodule| submodule == path) && recorded() {
            run(directory, &["reset", "--quiet", "--", path])?;
            run(directory, &["submodule", "update", "--", path])?;
        } else if in_head(directory, path) {
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

/// `path` written as a pattern that matches that file and no other: what a pattern reads as more than itself is escaped.
fn ignore_pattern(path: &str) -> String {
    let mut pattern = String::new();
    for (place, letter) in path.chars().enumerate() {
        if matches!(letter, '\\' | '*' | '?' | '[' | ']')
            || (place == 0 && matches!(letter, '#' | '!'))
        {
            pattern.push('\\');
        }
        pattern.push(letter);
    }
    if pattern.ends_with(' ') {
        pattern.pop();
        pattern.push_str("\\ ");
    }
    pattern
}

/// Adds `paths` to the `.gitignore` at the root of the repository `directory` is in, one line each, unless they are in it already.
pub fn ignore(directory: &Path, paths: &[String]) -> Result<(), String> {
    let paths = all_inside(paths)?;
    let root = run(directory, &["rev-parse", "--show-toplevel"])?;
    let file = Path::new(root.trim()).join(".gitignore");
    let mut text = fs::read_to_string(&file).unwrap_or_default();
    for path in paths {
        let line = format!("/{}", ignore_pattern(path));
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

/// The most commits of the history that are listed, however many are asked for.
const HISTORY_LIMIT: usize = 2000;

/// The most bytes of a file that are read to be shown.
const READ_LIMIT: usize = 2_000_000;

/// The most bytes of a file that are read whole, as a picture is.
const BYTES_LIMIT: usize = 32_000_000;

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

/// What `path` holds in the working folder, or, when `head`, in the last commit of the repository `directory` is in. No more than the limit of what is shown is ever read.
pub fn read(directory: &Path, path: &str, head: bool) -> Result<Contents, String> {
    read_at(directory, path, head.then_some("HEAD"))
}

/// The bytes `path` holds in the working folder, or in the commit `revision` of the repository `directory` is in when one is named, no more than one past `most`; the last commit of a file inside a submodule is the submodule's.
fn read_raw(
    directory: &Path,
    path: &str,
    revision: Option<&str>,
    most: usize,
) -> Result<Vec<u8>, String> {
    let path = inside(path)?;
    let limit = most as u64 + 1;
    let mut bytes = Vec::new();
    if let Some(revision) = revision {
        if revision.is_empty()
            || !revision
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            return Err(format!("{revision:?} does not name a commit"));
        }
        let (folder, inner) = if revision == "HEAD" {
            submodule::owner_of(directory, path)
        } else {
            (directory.to_path_buf(), path.to_owned())
        };
        let mut child = Command::new("git")
            .args(["show", &format!("{revision}:{inner}")])
            .env("GIT_LITERAL_PATHSPECS", "1")
            .current_dir(folder)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("could not run git: {error}"))?;
        if let Some(stdout) = child.stdout.take() {
            stdout
                .take(limit)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("could not read {path}: {error}"))?;
        }
        let cut = bytes.len() as u64 >= limit;
        if cut {
            let _ = child.kill();
        }
        let status = child
            .wait()
            .map_err(|error| format!("could not run git: {error}"))?;
        if !cut && !status.success() {
            let mut complaint = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut complaint);
            }
            return Err(complaint.trim().to_owned());
        }
    } else {
        let root = fs::canonicalize(directory)
            .map_err(|error| format!("could not read {path}: {error}"))?;
        let file = fs::canonicalize(root.join(path))
            .map_err(|error| format!("could not read {path}: {error}"))?;
        if !file.starts_with(&root) {
            return Err(format!("{path:?} is not a path inside the repository"));
        }
        fs::File::open(&file)
            .and_then(|file| file.take(limit).read_to_end(&mut bytes))
            .map_err(|error| format!("could not read {path}: {error}"))?;
    }
    Ok(bytes)
}

/// The bytes `path` holds in the working folder, or, when `head`, in the last commit of the repository `directory` is in, and whether they were cut short at the limit of what is read whole.
pub fn read_bytes(directory: &Path, path: &str, head: bool) -> Result<(Vec<u8>, bool), String> {
    let mut bytes = read_raw(directory, path, head.then_some("HEAD"), BYTES_LIMIT)?;
    let truncated = bytes.len() > BYTES_LIMIT;
    bytes.truncate(BYTES_LIMIT);
    Ok((bytes, truncated))
}

/// What `path` holds in the working folder, or in the commit `revision` of the repository `directory` is in when one is named. No more than the limit of what is shown is ever read.
pub fn read_at(directory: &Path, path: &str, revision: Option<&str>) -> Result<Contents, String> {
    let bytes = read_raw(directory, path, revision, READ_LIMIT)?;
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
