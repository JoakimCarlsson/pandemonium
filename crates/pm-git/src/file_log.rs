//! A file's own history, the commits that changed it, the address of one of its lines on the repository's host, and the lines of a repository's files that hold some text.

use std::path::Path;

use crate::{inside, run};

/// How many commits of a file's history are listed at most.
const FILE_HISTORY_LIMIT: usize = 500;

/// A commit that changed a file.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileCommit {
    /// The full object name.
    pub object: String,
    /// The abbreviated object name.
    pub id: String,
    /// Who wrote it.
    pub author: String,
    /// How long ago it was written, as git says it.
    pub when: String,
    /// The first line of the commit message.
    pub summary: String,
    /// The path the file had in the commit, which differs from its path now when it was moved since.
    pub path: String,
}

/// The commits that changed `path` in the repository `directory` is in, newest first, following the file back through moves.
pub fn file_history(directory: &Path, path: &str) -> Result<Vec<FileCommit>, String> {
    let path = inside(path)?;
    let count = format!("-{FILE_HISTORY_LIMIT}");
    let said = run(
        directory,
        &[
            "log",
            &count,
            "--follow",
            "--name-only",
            "--format=%x1e%H%x09%h%x09%an%x09%ar%x09%s",
            "--",
            path,
        ],
    )?;
    Ok(said
        .split('\u{1e}')
        .filter_map(|entry| {
            let mut lines = entry.lines();
            let mut fields = lines.next()?.splitn(5, '\t');
            let object = fields.next()?.to_owned();
            let id = fields.next()?.to_owned();
            let author = fields.next()?.to_owned();
            let when = fields.next()?.to_owned();
            let summary = fields.next().unwrap_or_default().to_owned();
            let named = lines
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or(path)
                .to_owned();
            Some(FileCommit {
                object,
                id,
                author,
                when,
                summary,
                path: named,
            })
        })
        .collect())
}

/// The web address of the repository a remote address `remote` names, such as `https://github.com/owner/name` for `git@github.com:owner/name.git`.
fn web_address(remote: &str) -> Option<String> {
    let remote = remote.trim().trim_end_matches('/');
    let remote = remote.strip_suffix(".git").unwrap_or(remote);
    let (host, rest) = if let Some(rest) = remote
        .strip_prefix("https://")
        .or_else(|| remote.strip_prefix("http://"))
    {
        let rest = rest.split_once('@').map_or(rest, |(_, after)| after);
        rest.split_once('/')?
    } else if let Some(rest) = remote.strip_prefix("ssh://") {
        let rest = rest.split_once('@').map_or(rest, |(_, after)| after);
        let (host, path) = rest.split_once('/')?;
        (host.split(':').next()?, path)
    } else {
        let rest = remote.split_once('@').map_or(remote, |(_, after)| after);
        rest.split_once(':')?
    };
    if host.is_empty() || rest.is_empty() {
        return None;
    }
    Some(format!("https://{host}/{rest}"))
}

/// The web address of line `line`, counted from 1, of `path` in the repository `directory` is in, at the commit checked out, on the host its remote is on: the remote the branch follows, `origin`, or the first there is.
pub fn permalink(directory: &Path, path: &str, line: usize) -> Result<String, String> {
    let path = inside(path)?;
    let followed = run(
        directory,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
    .ok()
    .and_then(|upstream| {
        upstream
            .trim()
            .split_once('/')
            .map(|(remote, _)| remote.to_owned())
    });
    let remotes = run(directory, &["remote"]).unwrap_or_default();
    let remote = followed
        .or_else(|| {
            remotes
                .lines()
                .find(|remote| *remote == "origin")
                .map(str::to_owned)
        })
        .or_else(|| remotes.lines().next().map(str::to_owned))
        .ok_or_else(|| "this repository has no remote".to_owned())?;
    let address = run(directory, &["remote", "get-url", &remote])?;
    let web = web_address(&address)
        .ok_or_else(|| format!("{} is not an address with a web page", address.trim()))?;
    let commit = run(directory, &["rev-parse", "HEAD"])?.trim().to_owned();
    let prefix = run(directory, &["rev-parse", "--show-prefix"])
        .unwrap_or_default()
        .trim()
        .to_owned();
    let file = format!("{prefix}{path}");
    let host = web
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default();
    Ok(if host.contains("gitlab") {
        format!("{web}/-/blob/{commit}/{file}#L{line}")
    } else if host.contains("bitbucket") {
        format!("{web}/src/{commit}/{file}#lines-{line}")
    } else if host.contains("codeberg") || host.contains("gitea") || host.contains("forgejo") {
        format!("{web}/src/commit/{commit}/{file}#L{line}")
    } else {
        format!("{web}/blob/{commit}/{file}#L{line}")
    })
}

/// One line of a file of a repository that a search found.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Found {
    /// The file, from the root of the folder searched.
    pub path: String,
    /// The line, counted from 1.
    pub line: usize,
    /// The column the match starts at, in bytes, counted from 1.
    pub column: usize,
    /// The line as the file holds it.
    pub text: String,
}

/// How many lines a search of a repository hands back at most.
const MOST_FOUND: usize = 500;

/// The lines of the files of the repository `directory` is in, tracked or not but not ignored, that hold `query`: as typed, or as a regular expression when `regex`; matching case only when `case`.
pub fn search(
    directory: &Path,
    query: &str,
    regex: bool,
    case: bool,
) -> Result<Vec<Found>, String> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let mut arguments = vec![
        "grep",
        "-n",
        "--column",
        "-I",
        "--untracked",
        "--full-name",
        "--no-color",
    ];
    arguments.push(if regex { "-E" } else { "-F" });
    if !case {
        arguments.push("-i");
    }
    arguments.extend(["-e", query, "--", "."]);
    let said = match run(directory, &arguments) {
        Ok(said) => said,
        Err(complaint) if complaint.trim().is_empty() => String::new(),
        Err(complaint) => return Err(complaint),
    };
    let prefix = run(directory, &["rev-parse", "--show-prefix"])
        .unwrap_or_default()
        .trim()
        .to_owned();
    Ok(said
        .lines()
        .filter_map(|held| {
            let (path, rest) = held.split_once(':')?;
            let (line, rest) = rest.split_once(':')?;
            let (column, text) = rest.split_once(':')?;
            Some(Found {
                path: path.strip_prefix(&prefix).unwrap_or(path).to_owned(),
                line: line.parse().ok()?,
                column: column.parse().ok()?,
                text: text.to_owned(),
            })
        })
        .take(MOST_FOUND)
        .collect())
}

/// Who last changed a line of a file, and in which commit.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Blame {
    /// The abbreviated object name of the commit, empty for a line not committed yet.
    pub id: String,
    /// Who wrote it.
    pub author: String,
    /// When, as seconds since the Unix epoch.
    pub when: i64,
    /// The first line of the commit message.
    pub summary: String,
}

/// Who last changed line `line`, counted from 1, of `path` in the repository `directory` is in, as the file is on disk.
pub fn blame(directory: &Path, path: &str, line: usize) -> Result<Blame, String> {
    let path = inside(path)?;
    let range = format!("{line},{line}");
    let said = run(
        directory,
        &["blame", "--porcelain", "-L", &range, "--", path],
    )?;
    let mut lines = said.lines();
    let object = lines
        .next()
        .and_then(|first| first.split_whitespace().next())
        .unwrap_or_default();
    let mut found = Blame {
        id: if object.chars().all(|character| character == '0') {
            String::new()
        } else {
            object.chars().take(8).collect()
        },
        ..Blame::default()
    };
    for held in lines {
        if let Some(author) = held.strip_prefix("author ") {
            found.author = author.to_owned();
        } else if let Some(time) = held.strip_prefix("author-time ") {
            found.when = time.trim().parse().unwrap_or_default();
        } else if let Some(summary) = held.strip_prefix("summary ") {
            found.summary = summary.to_owned();
        }
    }
    Ok(found)
}
