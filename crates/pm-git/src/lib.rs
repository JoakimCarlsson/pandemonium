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
mod multirepo;
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
pub use multirepo::{Layout, child_repositories, layout};
pub use operation::{Operation, operation, step};
pub use places::{clone, folders};
pub use revision::revision_changes;
pub use submodule::{populate, submodule_heads, submodules};
pub use words::{Words, words};

/// Where, under a repository, the worktrees made for tasks are kept.
const WORKTREES: &str = ".pandemonium/worktrees";

/// The most lines of a file's diff that are read.
pub const DIFF_LINES: usize = 2000;

/// The longest a folder name made from a title is, in characters.
const SLUG_LENGTH: usize = 40;

/// A file that differs from the last commit.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Change {
    /// Where the file is, from the root of the folder asked about.
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
    /// The submodule the file is in, from the root of the folder asked about, or nothing when it is in a repository itself.
    #[serde(default)]
    pub submodule: Option<String>,
    /// The repository the file is in, as its folder name, when the folder asked about holds several repositories side by side, or nothing when it is one repository.
    #[serde(default)]
    pub repository: Option<String>,
    /// What a submodule checked out at another commit than the one recorded moved through, when the change is that.
    #[serde(default)]
    pub moved: Option<Moved>,
    /// How many lines the change puts in, counted over the whole file.
    #[serde(default)]
    pub added: usize,
    /// How many lines the change takes out, counted over the whole file.
    #[serde(default)]
    pub removed: usize,
    /// Whether `diff` and `words` are left out, to be asked for with [`diffs`] when the file is shown.
    #[serde(default)]
    pub lazy: bool,
}

/// The diff of one file, as [`diffs`] answers.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileDiff {
    /// Where the file is, from the root of the folder asked about.
    pub path: String,
    /// The lines of its diff, from the first hunk on.
    pub diff: String,
    /// The words that changed inside its removed and added lines.
    #[serde(default)]
    pub words: Vec<Words>,
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

/// The paths `git ls-files` lists in `directory` for `arguments`, each exactly as named on disk: the listing is nul separated, so no name is quoted, escaped or split, whatever it holds. Nothing when git fails.
fn ls_files(directory: &Path, arguments: &[&str]) -> Vec<String> {
    let mut command = vec!["ls-files", "-z"];
    command.extend_from_slice(arguments);
    run(directory, &command)
        .map(|listing| checkpoint::listed(&listing))
        .unwrap_or_default()
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

/// Whether `directory` is inside a Git repository. A folder that only holds repositories is not; [`layout`] tells those apart.
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

/// The files of the folder `directory`, as paths from it, at most `limit` of them: those a repository and its submodules track or do not ignore, those of each repository in a folder that holds repositories, or, in a folder with none, every file but those in hidden and bulky folders.
pub fn files(directory: &Path, limit: usize) -> Vec<String> {
    match layout(directory) {
        Layout::Repositories(children) => {
            return multirepo::gathered(directory, &children, limit, files);
        }
        Layout::Neither => {
            let mut found = Vec::new();
            walk(directory, directory, limit, &mut found);
            return found;
        }
        Layout::Repository => {}
    }
    let submodules = submodule::own_submodules(directory);
    let untracked = ["--others", "--exclude-standard"];
    let mut found: Vec<String> = ls_files(directory, &["--cached", "--recurse-submodules"])
        .into_iter()
        .filter(|path| !submodules.contains(path))
        .chain(ls_files(directory, &untracked))
        .collect();
    for path in &submodules {
        found.extend(
            ls_files(&directory.join(path), &untracked)
                .into_iter()
                .map(|file| format!("{path}/{file}")),
        );
    }
    found.truncate(limit);
    found
}

/// The untracked folders of the repository `directory` is in, as paths from it, at most `limit` of them: those that hold no tracked file, which is how a folder with nothing in it yet is told of.
pub fn untracked_folders(directory: &Path, limit: usize) -> Vec<String> {
    match layout(directory) {
        Layout::Repositories(children) => {
            return multirepo::gathered(directory, &children, limit, untracked_folders);
        }
        Layout::Neither => return Vec::new(),
        Layout::Repository => {}
    }
    ls_files(
        directory,
        &["--others", "--directory", "--exclude-standard"],
    )
    .iter()
    .filter_map(|line| line.strip_suffix('/'))
    .take(limit)
    .map(str::to_owned)
    .collect()
}

/// The ignored files of the repository `directory` is in, as paths from it, at most `limit` of them.
pub fn ignored(directory: &Path, limit: usize) -> Vec<String> {
    match layout(directory) {
        Layout::Repositories(children) => {
            return multirepo::gathered(directory, &children, limit, ignored);
        }
        Layout::Neither => return Vec::new(),
        Layout::Repository => {}
    }
    let mut found = ls_files(directory, &["--others", "--ignored", "--exclude-standard"]);
    found.truncate(limit);
    found
}

/// The top-level ignored folders of the repository `directory` is in, including empty ones.
pub fn ignored_folders(directory: &Path, limit: usize) -> Vec<String> {
    match layout(directory) {
        Layout::Repositories(children) => {
            return multirepo::gathered(directory, &children, limit, ignored_folders);
        }
        Layout::Neither => return Vec::new(),
        Layout::Repository => {}
    }
    ls_files(
        directory,
        &["--others", "--ignored", "--directory", "--exclude-standard"],
    )
    .iter()
    .filter_map(|line| line.strip_suffix('/'))
    .take(limit)
    .map(str::to_owned)
    .collect()
}

/// The branch `directory` has checked out, or nothing when it has none or is not a repository. Of a folder of repositories, the one branch they are all on, or the branches they are on, each named once and told apart by a dot.
pub fn current_branch(directory: &Path) -> Option<String> {
    if let Layout::Repositories(children) = layout(directory) {
        let mut names: Vec<String> = Vec::new();
        for name in children
            .iter()
            .filter_map(|child| current_branch(&directory.join(child)))
        {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        return (!names.is_empty()).then(|| names.join(" · "));
    }
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

/// The branches of the repository `directory` is in as it has them now: the local ones, the current one first, then those of its remotes. Of a folder of repositories, those every one of them has, then those only some have.
pub fn branches(directory: &Path) -> Vec<String> {
    if let Layout::Repositories(children) = layout(directory) {
        let lists: Vec<Vec<String>> = children
            .iter()
            .map(|child| branches(&directory.join(child)))
            .collect();
        let mut names: Vec<String> = lists.iter().flatten().fold(Vec::new(), |mut names, name| {
            if !names.contains(name) {
                names.push(name.clone());
            }
            names
        });
        names.sort_by_key(|name| !lists.iter().all(|list| list.contains(name)));
        return names;
    }
    let mut names = references(directory, "refs/heads");
    if let Some(current) = current_branch(directory)
        && let Some(at) = names.iter().position(|name| *name == current)
    {
        names.swap(0, at);
    }
    names.extend(references(directory, "refs/remotes"));
    names
}

/// The commit `reference` names in the repository `directory` is in, abbreviated, or nothing when it names none. Of a folder of repositories, the one it names in the first that has it.
pub fn resolve(directory: &Path, reference: &str) -> Option<String> {
    if let Layout::Repositories(children) = layout(directory) {
        return children
            .iter()
            .find_map(|child| resolve(&directory.join(child), reference));
    }
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

/// The repositories a worktree of `repository` is made of: the empty name for a repository, which is made into the worktree itself, or each repository of a folder of them by its folder name, each made into a folder of that name inside the worktree.
fn members(repository: &Path) -> Result<Vec<String>, String> {
    match layout(repository) {
        Layout::Repository => Ok(vec![String::new()]),
        Layout::Repositories(children) => Ok(children),
        Layout::Neither => Err(format!("{} holds no git repository", repository.display())),
    }
}

/// The folder of `member`, one of [`members`], inside `root`.
fn folder_in(root: &Path, member: &str) -> PathBuf {
    if member.is_empty() {
        root.to_path_buf()
    } else {
        root.join(member)
    }
}

/// `text` said of `member`: by its name when it is one repository of several, as it is when it is the only one.
fn named(member: &str, text: &str) -> String {
    if member.is_empty() {
        text.to_owned()
    } else {
        format!("{member}: {text}")
    }
}

/// Makes a worktree of its own of `repository`, named from `title`, with `base` checked out, where `placement` says and on the branch it names or a detached head, checks out its submodules, each on that branch too, and copies into it the ignored files `placement` lists.
///
/// For a folder that holds several repositories the worktree is a folder, and each repository gets a worktree of its own in it under the repository's name, all on the one branch. When one of them cannot be made, those made before it are removed again and the one that failed is named.
///
/// `base` is looked up in each repository now, so a branch deleted a moment ago is refused rather than assumed.
pub fn add_worktree(
    repository: &Path,
    title: &str,
    base: &str,
    placement: &Placement,
) -> Result<Worktree, String> {
    let members = members(repository)?;
    let mut heads = members
        .iter()
        .map(|member| {
            resolve(&folder_in(repository, member), base)
                .ok_or_else(|| named(member, &format!("{base} does not exist")))
        })
        .collect::<Result<Vec<String>, String>>()?;
    if placement.root.is_none() && members.len() == 1 && members[0].is_empty() {
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
        let taken = branch.as_deref().is_some_and(|branch| {
            members
                .iter()
                .any(|member| has_branch(&folder_in(repository, member), branch))
        });
        if !path.exists() && !taken {
            break (path, branch);
        }
        attempt += 1;
    };
    for (at, member) in members.iter().enumerate() {
        let place = folder_in(&path, member).to_string_lossy().into_owned();
        let source = folder_in(repository, member);
        let added = match &branch {
            Some(branch) => run(
                &source,
                &["worktree", "add", "-b", branch, &place, &heads[at]],
            ),
            None => run(
                &source,
                &["worktree", "add", "--detach", &place, &heads[at]],
            ),
        };
        if let Err(reason) = added {
            return Err(undone(
                repository,
                &path,
                &members[..at],
                &heads,
                branch.as_deref(),
                named(member, &reason),
            ));
        }
    }
    let mut skipped = Vec::new();
    for member in &members {
        let place = folder_in(&path, member);
        let mut inside = submodule::populate(&place);
        if let Some(branch) = &branch {
            inside.extend(submodule::branch_all(&place, branch));
        }
        inside.extend(carry::carry_ignored(
            &folder_in(repository, member),
            &place,
            &placement.include,
        ));
        skipped.extend(inside.into_iter().map(|(what, why)| {
            let what = if member.is_empty() {
                what
            } else {
                format!("{member}/{what}")
            };
            (what, why)
        }));
    }
    Ok(Worktree {
        path,
        head: heads.swap_remove(0),
        branch,
        skipped,
    })
}

/// `failure` with what became of the worktrees `made` of the repositories of `repository`: they are removed again, with the folder `path` they were made in, and any that cannot be is named.
fn undone(
    repository: &Path,
    path: &Path,
    made: &[String],
    heads: &[String],
    branch: Option<&str>,
    failure: String,
) -> String {
    let stuck: Vec<String> = made
        .iter()
        .zip(heads)
        .filter_map(|(member, head)| {
            remove_in(
                &folder_in(repository, member),
                &folder_in(path, member),
                branch,
                Some(head),
                false,
            )
            .err()
            .map(|reason| named(member, &reason))
        })
        .collect();
    if made.iter().all(|member| !member.is_empty()) && stuck.is_empty() {
        let _ = fs::remove_dir_all(path);
    }
    match (made.is_empty(), stuck.is_empty()) {
        (true, _) => failure,
        (false, true) => format!("{failure}; the worktrees made before it were removed again"),
        (false, false) => format!(
            "{failure}; the worktrees made before it could not all be removed: {}",
            stuck.join("; ")
        ),
    }
}

/// Removes the worktree at `path` of `repository` with every file in it, and forgets it, then deletes `branch`, the branch it was made on from `base`, when every commit on it is also on `base` or a remote.
///
/// A folder that is already gone is only forgotten, and a branch with commits of its own is kept unless `whole` is set.
fn remove_in(
    repository: &Path,
    path: &Path,
    branch: Option<&str>,
    base: Option<&str>,
    whole: bool,
) -> Result<(), String> {
    let place = path.to_string_lossy().into_owned();
    if path.exists() {
        run(repository, &["worktree", "remove", "--force", &place])?;
    }
    run(repository, &["worktree", "prune"])?;
    if let (Some(branch), Some(base)) = (branch, base)
        && has_branch(repository, branch)
        && (whole || merged(repository, branch, base))
    {
        let _ = run(repository, &["branch", "-D", branch]);
    }
    Ok(())
}

/// Removes the worktree at `path` of `repository` with every file in it, and forgets it, then deletes `branch`, the branch it was made on from `base`, when every commit on it is also on `base` or a remote.
///
/// A folder that is already gone is only forgotten, and a branch with commits of its own is kept. For a folder that holds several repositories every repository's worktree and its branch are removed, whatever commits the branch has, one that fails does not stop the rest and is named in the error, and the folder they were in goes when all are removed.
pub fn remove_worktree(
    repository: &Path,
    path: &Path,
    branch: Option<&str>,
    base: Option<&str>,
) -> Result<(), String> {
    let members = members(repository)?;
    let failures: Vec<String> = members
        .iter()
        .filter_map(|member| {
            remove_in(
                &folder_in(repository, member),
                &folder_in(path, member),
                branch,
                base,
                !members[0].is_empty(),
            )
            .err()
            .map(|reason| named(member, &reason))
        })
        .collect();
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }
    if members.iter().all(|member| !member.is_empty()) && path.exists() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())?;
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

/// Every file of the repository `directory` is in that differs from the last commit, those inside its submodules included under their paths from its root, and each submodule checked out at another commit than the one recorded. Of a folder of repositories, those of every repository in it, under their paths from the folder and each naming its repository.
pub fn changes(directory: &Path) -> Vec<Change> {
    match layout(directory) {
        Layout::Repositories(children) => children
            .iter()
            .flat_map(|child| {
                repository_changes(&directory.join(child))
                    .into_iter()
                    .map(move |change| in_repository(change, child))
            })
            .collect(),
        Layout::Repository => repository_changes(directory),
        Layout::Neither => Vec::new(),
    }
}

/// `change`, of a file of the repository `child` of a folder of repositories, as one of the folder.
fn in_repository(mut change: Change, child: &str) -> Change {
    change.path = format!("{child}/{}", change.path);
    change.from = change.from.map(|from| format!("{child}/{from}"));
    change.submodule = change.submodule.map(|inner| format!("{child}/{inner}"));
    change.repository = Some(child.to_owned());
    change
}

/// Every file of the one repository `directory` is in that differs from the last commit, submodules included, the files themselves without their diffs.
fn repository_changes(directory: &Path) -> Vec<Change> {
    let direct = submodule::direct(directory);
    let mut found = Vec::new();
    for mut change in own_changes(directory) {
        if !direct.contains(&change.path) {
            found.push(change);
            continue;
        }
        let path = change.path.clone();
        let inner = directory.join(&path);
        change.diff = file_diff(directory, &path);
        change.words = words(&change.diff);
        (change.added, change.removed) = tally(&change.diff);
        change.lazy = false;
        found.extend(submodule::moved(directory, change));
        if submodule::populated(&inner) {
            found.extend(
                repository_changes(&inner)
                    .into_iter()
                    .map(|change| submodule::within(change, &path)),
            );
        }
    }
    found
}

/// The files of the repository `directory` is in itself that differ from its last commit, a submodule as one entry, each with how many lines it changes and without its diff.
fn own_changes(directory: &Path) -> Vec<Change> {
    let Ok(status) = run(directory, &["status", "--porcelain=v1", "-z", "-uall"]) else {
        return Vec::new();
    };
    let counted = counts(directory);
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
        let (added, removed) = if untracked {
            (lines_of(&directory.join(path)), 0)
        } else {
            counted.get(path).copied().unwrap_or_default()
        };
        let side = |mark: char| (!matches!(mark, ' ' | '!')).then_some(mark);
        let (index_side, worktree_side) = if untracked {
            (None, Some('?'))
        } else {
            (side(index), side(tree))
        };
        found.push(Change {
            path: path.to_owned(),
            words: Vec::new(),
            diff: String::new(),
            staged: !untracked && !conflicted && index != ' ' && tree == ' ',
            untracked,
            index: index_side,
            worktree: worktree_side,
            conflicted,
            from,
            submodule: None,
            repository: None,
            moved: None,
            added,
            removed,
            lazy: true,
        });
    }
    found
}

/// How many lines each file of the repository `directory` is in has put in and taken out since the last commit, by path, as one run of git tells it; a file git does not track is not in it.
fn counts(directory: &Path) -> std::collections::HashMap<String, (usize, usize)> {
    let said = run(
        directory,
        &["diff", "HEAD", "--numstat", "-z", "--no-renames"],
    )
    .or_else(|_| {
        run(
            directory,
            &["diff", "--cached", "--numstat", "-z", "--no-renames"],
        )
    })
    .unwrap_or_default();
    said.split('\0')
        .filter_map(|entry| {
            let mut parts = entry.splitn(3, '\t');
            let added = parts.next()?;
            let removed = parts.next()?;
            let path = parts.next()?;
            Some((
                path.to_owned(),
                (added.parse().unwrap_or(0), removed.parse().unwrap_or(0)),
            ))
        })
        .collect()
}

/// How many lines the file at `path` has, or none when it is not text.
fn lines_of(path: &Path) -> usize {
    let Ok(held) = fs::read(path) else {
        return 0;
    };
    if held.iter().take(8000).any(|byte| *byte == 0) {
        return 0;
    }
    let breaks = held.iter().filter(|byte| **byte == b'\n').count();
    breaks + usize::from(held.last().is_some_and(|last| *last != b'\n'))
}

/// How many lines `diff` puts in and takes out.
fn tally(diff: &str) -> (usize, usize) {
    diff.lines().fold((0, 0), |(added, removed), line| {
        match line.as_bytes().first() {
            Some(b'+') => (added + 1, removed),
            Some(b'-') => (added, removed + 1),
            _ => (added, removed),
        }
    })
}

/// The diff of the file at `path` of the repository `directory` is in against the last commit, from its first hunk on, or all of it as added when git does not track it.
fn file_diff(directory: &Path, path: &str) -> String {
    let shown = run(directory, &["diff", "HEAD", "--no-color", "--", path])
        .or_else(|_| run(directory, &["diff", "--cached", "--no-color", "--", path]))
        .unwrap_or_default();
    let diff = hunks(&shown);
    if diff.is_empty() && run(directory, &["ls-files", "--error-unmatch", "--", path]).is_err() {
        return added(directory, path);
    }
    diff
}

/// How many files of one request have their diffs read at the same time, each by its own `git`.
const DIFF_WORKERS: usize = 8;

/// The diffs of `paths` of the folder `directory`, each file's from the repository it is in: one of a folder that holds several, or a submodule. A path no repository of the folder holds is left out.
pub fn diffs(directory: &Path, paths: &[String]) -> Vec<FileDiff> {
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    let Ok(owners) = submodule::by_owner(directory, &paths) else {
        return Vec::new();
    };
    let jobs: Vec<(PathBuf, String, String)> = owners
        .into_iter()
        .flat_map(|(folder, inner)| {
            let from_root = folder.strip_prefix(directory).unwrap_or(Path::new(""));
            inner
                .into_iter()
                .map(|path| {
                    let shown = from_root.join(&path).to_string_lossy().replace('\\', "/");
                    (folder.clone(), path, shown)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done: std::sync::Mutex<Vec<Option<FileDiff>>> =
        std::sync::Mutex::new((0..jobs.len()).map(|_| None).collect());
    let workers = std::thread::available_parallelism()
        .map_or(4, usize::from)
        .clamp(1, DIFF_WORKERS)
        .min(jobs.len());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((folder, path, shown)) = jobs.get(at) else {
                        break;
                    };
                    let diff = file_diff(folder, path);
                    let read = FileDiff {
                        words: words(&diff),
                        path: shown.clone(),
                        diff,
                    };
                    if let Ok(mut held) = done.lock() {
                        held[at] = Some(read);
                    }
                }
            });
        }
    });
    done.into_inner()
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect()
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

/// Runs `git` with `command` and then `paths` in each repository `paths` of `directory` are in, a repository of a folder that holds several, the repository itself or one of their submodules, each path as it is from there.
fn run_by_owner(directory: &Path, command: &[&str], paths: &[String]) -> Result<(), String> {
    for (folder, inner) in submodule::by_owner(directory, &all_inside(paths)?)? {
        let mut arguments = command.to_vec();
        arguments.extend(inner.iter().map(String::as_str));
        run(&folder, &arguments)?;
    }
    Ok(())
}

/// Stages `paths`, each in the repository it is in: a file inside a submodule in the submodule, a submodule itself at the commit it is at in the repository around it, a file of a folder that holds several repositories in the one it is in.
pub fn stage(directory: &Path, paths: &[String]) -> Result<(), String> {
    run_by_owner(directory, &["add", "--"], paths)
}

/// Takes `paths` out of the next commit of the repository each is in, leaving their changes in the files.
pub fn unstage(directory: &Path, paths: &[String]) -> Result<(), String> {
    run_by_owner(directory, &["reset", "--quiet", "--"], paths)
}

/// Whether the repository `directory` is in has anything a commit would take: what is staged, or, when `tracked`, a change to a tracked file.
fn has_to_commit(directory: &Path, tracked: bool) -> bool {
    let staged = run(directory, &["diff", "--cached", "--quiet"]).is_err();
    staged
        || (tracked
            && run(
                directory,
                &["status", "--porcelain", "--untracked-files=no"],
            )
            .is_ok_and(|said| !said.trim().is_empty()))
}

/// Commits what is staged, under `message`; with nothing staged, `tracked` takes the changes of every tracked file instead, and `amend` rewrites the last commit in place of making another. In a folder that holds several repositories, each repository that has something to commit commits it and the rest are left as they are.
pub fn commit(directory: &Path, message: &str, tracked: bool, amend: bool) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("A commit needs a message".to_owned());
    }
    if let Layout::Repositories(children) = layout(directory) {
        let ready: Vec<String> = children
            .into_iter()
            .filter(|child| has_to_commit(&directory.join(child), tracked))
            .collect();
        if ready.is_empty() {
            return Err("Nothing to commit in any repository".to_owned());
        }
        return multirepo::each(directory, &ready, |child| {
            commit(child, message, tracked, amend)
        });
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

/// What the checked out branch of the repository `directory` is in follows, and how far apart they are. Of a folder of repositories, how far apart they are all together, following something only when every one of them does.
pub fn tracking(directory: &Path) -> Tracking {
    if let Layout::Repositories(children) = layout(directory) {
        let each: Vec<Tracking> = children
            .iter()
            .map(|child| tracking(&directory.join(child)))
            .collect();
        return Tracking {
            upstream: each
                .iter()
                .all(|held| held.upstream.is_some())
                .then(|| each.first().and_then(|held| held.upstream.clone()))
                .flatten(),
            ahead: each.iter().map(|held| held.ahead).sum(),
            behind: each.iter().map(|held| held.behind).sum(),
        };
    }
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

/// Where one repository of a folder that holds several stands.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Standing {
    /// The repository's folder name.
    pub path: String,
    /// The branch it has checked out, when it is on one.
    pub branch: Option<String>,
    /// The commit it is at, abbreviated.
    pub head: Option<String>,
    /// What its branch follows and how far apart they are.
    pub tracking: Tracking,
}

/// Where each repository of the folder `directory` stands, in the order of their names; nothing when the folder is a repository itself or holds none.
pub fn standings(directory: &Path) -> Vec<Standing> {
    child_repositories(directory)
        .into_iter()
        .map(|path| {
            let folder = directory.join(&path);
            Standing {
                branch: current_branch(&folder),
                head: resolve(&folder, "HEAD"),
                tracking: tracking(&folder),
                path,
            }
        })
        .collect()
}

/// Talks to the remote of the repository `directory` is in, or of each repository in a folder that holds several, naming those that fail: `fetch` from all of them, `pull`, `pull_rebase`, `push` the branch, publishing it when it follows nothing, `force_push` without overwriting what was not seen, or `sync` by pulling and then pushing.
pub fn remote(directory: &Path, operation: &str) -> Result<(), String> {
    if let Layout::Repositories(children) = layout(directory) {
        return multirepo::each(directory, &children, |child| remote(child, operation));
    }
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
    if let Layout::Repositories(children) = layout(directory) {
        return children
            .iter()
            .flat_map(|child| commits_since(&directory.join(child), base))
            .collect();
    }
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
    for (folder, inner) in submodule::by_owner(directory, &all_inside(paths)?)? {
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
    let children = multirepo::children_of(directory);
    let mut groups: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for path in paths {
        let (folder, inner) = multirepo::split(directory, children.as_deref(), path)?;
        match groups.iter_mut().find(|(known, _)| *known == folder) {
            Some((_, group)) => group.push(inner),
            None => groups.push((folder, vec![inner])),
        }
    }
    for (folder, inner) in groups {
        ignore_in(&folder, &inner)?;
    }
    Ok(())
}

/// Adds `paths` to the `.gitignore` at the root of the repository `directory` is in.
fn ignore_in(directory: &Path, paths: &[String]) -> Result<(), String> {
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

/// Checks out the branch `name`, in every repository of a folder that holds several; one of a remote's gets the local branch of its name, made to follow it when there is none yet.
pub fn checkout(directory: &Path, name: &str) -> Result<(), String> {
    if let Layout::Repositories(children) = layout(directory) {
        return multirepo::each(directory, &children, |child| checkout(child, name));
    }
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

/// Makes the branch `name` at the commit the folder is at and checks it out, in every repository of a folder that holds several.
pub fn create_branch(directory: &Path, name: &str) -> Result<(), String> {
    if let Layout::Repositories(children) = layout(directory) {
        return multirepo::each(directory, &children, |child| create_branch(child, name));
    }
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
            submodule::owner_of(directory, path)?
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

#[cfg(test)]
mod listing_tests;
