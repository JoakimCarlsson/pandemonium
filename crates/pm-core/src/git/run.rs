//! Running git, which is the one way anything here asks git anything.
//!
//! Every question and every change is one subprocess in a worktree, so the
//! subprocess itself is written down once: the caller names the arguments
//! and reads the answer. A git that is not installed and a git that refused
//! are the same kind of failure to a caller — both come back as what git
//! would have said — so neither is spelled out at each call.

use pm_host::Location;

use pm_host::Stdio;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;

/// What git wrote, or what it complained about when it would not.
///
/// The failure carries git's own words rather than an error of ours: the
/// screen that asked shows them, and there is nothing useful to add to
/// "pathspec did not match any files".
pub type Said = Result<String, String>;

/// Runs git in the worktree at `root` with `arguments`.
pub fn git<I, S>(root: impl Into<Location>, arguments: I) -> Said
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let root = root.into();
    let output = root
        .host
        .command("git")
        .args(arguments)
        .current_dir(&root)
        .output()
        .map_err(|error| error.to_string())?;

    match output.status.success() {
        true => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        false => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            Err(format!("{stdout}{stderr}").trim().to_owned())
        }
    }
}

/// What git wrote, or nothing at all when it would not answer.
///
/// This is the form every question takes: a worktree outside a repository, a
/// path git has never heard of and a git that is not installed are all
/// nothing to show rather than something to report.
pub fn answer<I, S>(root: impl Into<Location>, arguments: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let root = root.into();
    git(&root, arguments).ok()
}

/// What git wrote, whether or not it called the run a success.
///
/// Comparing a file against nothing is the case this is for: git prints the
/// comparison and then reports the run as a failure because the two sides
/// differ. What it wrote is the answer either way.
pub fn written<I, S>(root: impl Into<Location>, arguments: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let root = root.into();
    root.host
        .command("git")
        .args(arguments)
        .current_dir(&root)
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// Runs git in `root` with `arguments`, handing it `input` to read.
///
/// This is how anything is put into git rather than asked of it: the content
/// of a blob is written to the command's own input rather than through a
/// file on the way, because a temporary file is a second thing that can go
/// wrong and nothing here needs one.
pub fn piped<I, S>(root: impl Into<Location>, arguments: I, input: &str) -> Said
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let root = root.into();
    let mut child = root
        .host
        .command("git")
        .args(arguments)
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;

    child
        .stdin
        .take()
        .ok_or_else(|| "git would not take what it was given".to_owned())
        .and_then(|mut stdin| stdin.write_all(input.as_bytes()).map_err(|e| e.to_string()))?;

    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    match output.status.success() {
        true => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        false => Err(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
    }
}

/// Runs git in `root` with `arguments`, feeding it `input` while it is
/// still answering, and answers everything it wrote.
///
/// This is for a command that answers as it reads, such as
/// `git cat-file --batch`: its answers can fill the pipe back long before
/// the questions have all gone in, so the questions are written from a
/// thread of their own rather than all at once ahead of the reading.
pub fn streamed<I, S>(root: impl Into<Location>, arguments: I, input: String) -> Option<Vec<u8>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let root = root.into();
    let mut child = root
        .host
        .command("git")
        .args(arguments)
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output().ok()?;
    writer.join().ok()?.ok()?;
    output.status.success().then_some(output.stdout)
}

/// The path git is given for `path`, which is where it sits under `root`.
///
/// Git is run in the worktree, so a path is named from the worktree down;
/// one that is not under it at all is not git's to answer about.
pub fn within<'a>(root: &Path, path: &'a Path) -> Option<&'a Path> {
    path.strip_prefix(root).ok()
}

/// The repository holding `path`, looked for from `path` up to `root`.
///
/// A worktree can be a folder of several repositories, each with its own
/// index and its own history, so a question about one file is asked of the
/// repository the file is in. A path in none of them is asked of `root`, and
/// gets the nothing a folder outside a repository answers with.
pub fn holding(root: impl Into<Location>, path: &Path) -> Location {
    let root = root.into();
    root.at(path
        .ancestors()
        .skip(1)
        .take_while(|ancestor| ancestor.starts_with(&root))
        .find(|ancestor| root.host.fs().exists(ancestor.join(".git")))
        .unwrap_or(root.as_path())
        .to_path_buf())
}
