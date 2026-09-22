//! Running git, which is the one way anything here asks git anything.
//!
//! Every question and every change is one subprocess in a worktree, so the
//! subprocess itself is written down once: the caller names the arguments
//! and reads the answer. A git that is not installed and a git that refused
//! are the same kind of failure to a caller — both come back as what git
//! would have said — so neither is spelled out at each call.

use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// What git wrote, or what it complained about when it would not.
///
/// The failure carries git's own words rather than an error of ours: the
/// screen that asked shows them, and there is nothing useful to add to
/// "pathspec did not match any files".
pub type Said = Result<String, String>;

/// Runs git in the worktree at `root` with `arguments`.
pub fn git<I, S>(root: &Path, arguments: I) -> Said
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;

    match output.status.success() {
        true => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        false => Err(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
    }
}

/// What git wrote, or nothing at all when it would not answer.
///
/// This is the form every question takes: a worktree outside a repository, a
/// path git has never heard of and a git that is not installed are all
/// nothing to show rather than something to report.
pub fn answer<I, S>(root: &Path, arguments: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    git(root, arguments).ok()
}

/// What git wrote, whether or not it called the run a success.
///
/// Comparing a file against nothing is the case this is for: git prints the
/// comparison and then reports the run as a failure because the two sides
/// differ. What it wrote is the answer either way.
pub fn written<I, S>(root: &Path, arguments: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("git")
        .args(arguments)
        .current_dir(root)
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
pub fn piped<I, S>(root: &Path, arguments: I, input: &str) -> Said
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new("git")
        .args(arguments)
        .current_dir(root)
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

/// The path git is given for `path`, which is where it sits under `root`.
///
/// Git is run in the worktree, so a path is named from the worktree down;
/// one that is not under it at all is not git's to answer about.
pub fn within<'a>(root: &Path, path: &'a Path) -> Option<&'a Path> {
    path.strip_prefix(root).ok()
}
