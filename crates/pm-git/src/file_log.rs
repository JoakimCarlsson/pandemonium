//! A file's own history, the commits that changed it, the address of one of its lines on the repository's host, and the lines of a repository's files that hold some text.

use std::collections::BTreeMap;
use std::path::Path;

use crate::multirepo;
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
    /// When it was written, in seconds since the epoch.
    #[serde(default)]
    pub time: i64,
    /// The first line of the commit message.
    pub summary: String,
    /// The path the file had in the commit, which differs from its path now when it was moved since.
    pub path: String,
}

/// The commits that changed `path` in the repository `directory` is in, newest first, following the file back through moves.
pub fn file_history(directory: &Path, path: &str) -> Result<Vec<FileCommit>, String> {
    let (directory, path) = multirepo::holder(directory, inside(path)?)?;
    let directory = directory.as_path();
    let path = path.as_str();
    let count = format!("-{FILE_HISTORY_LIMIT}");
    let said = run(
        directory,
        &[
            "log",
            &count,
            "--follow",
            "--name-only",
            "--format=%x1e%H%x09%h%x09%an%x09%ar%x09%at%x09%s",
            "--",
            path,
        ],
    )?;
    Ok(said
        .split('\u{1e}')
        .filter_map(|entry| {
            let mut lines = entry.lines();
            let mut fields = lines.next()?.splitn(6, '\t');
            let object = fields.next()?.to_owned();
            let id = fields.next()?.to_owned();
            let author = fields.next()?.to_owned();
            let when = fields.next()?.to_owned();
            let time = fields.next()?.parse().unwrap_or_default();
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
                time,
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

/// How one kind of host lays out the web address of a file, as templates in which `{web}` is the address of the repository, `{commit}` the commit, `{file}` the path of the file, `{start}` and `{end}` the first and last line and `{after}` the line past the last.
struct Layout {
    /// What a self-hosted server is told to use it as in the settings.
    id: &'static str,
    /// Parts of a host name that say it is a host of this kind.
    names: &'static [&'static str],
    /// The address of the whole file.
    file: &'static str,
    /// The address of one line.
    line: &'static str,
    /// The address of a range of lines.
    range: &'static str,
}

/// The kinds of host there are, the first whose name a host has winning.
const LAYOUTS: &[Layout] = &[
    Layout {
        id: "gitlab",
        names: &["gitlab"],
        file: "{web}/-/blob/{commit}/{file}",
        line: "{web}/-/blob/{commit}/{file}#L{start}",
        range: "{web}/-/blob/{commit}/{file}#L{start}-{end}",
    },
    Layout {
        id: "bitbucket",
        names: &["bitbucket"],
        file: "{web}/src/{commit}/{file}",
        line: "{web}/src/{commit}/{file}#lines-{start}",
        range: "{web}/src/{commit}/{file}#lines-{start}:{end}",
    },
    Layout {
        id: "gitea",
        names: &["codeberg", "gitea", "forgejo"],
        file: "{web}/src/commit/{commit}/{file}",
        line: "{web}/src/commit/{commit}/{file}#L{start}",
        range: "{web}/src/commit/{commit}/{file}#L{start}-L{end}",
    },
    Layout {
        id: "azure",
        names: &["dev.azure.com", "visualstudio.com"],
        file: "{web}?path=/{file}&version=GC{commit}",
        line: "{web}?path=/{file}&version=GC{commit}&line={start}&lineEnd={after}&lineStartColumn=1&lineEndColumn=1",
        range: "{web}?path=/{file}&version=GC{commit}&line={start}&lineEnd={after}&lineStartColumn=1&lineEndColumn=1",
    },
    Layout {
        id: "sourcehut",
        names: &["sr.ht"],
        file: "{web}/tree/{commit}/item/{file}",
        line: "{web}/tree/{commit}/item/{file}#L{start}",
        range: "{web}/tree/{commit}/item/{file}#L{start}-{end}",
    },
    Layout {
        id: "gitee",
        names: &["gitee"],
        file: "{web}/blob/{commit}/{file}",
        line: "{web}/blob/{commit}/{file}#L{start}",
        range: "{web}/blob/{commit}/{file}#L{start}-{end}",
    },
    Layout {
        id: "github",
        names: &["github"],
        file: "{web}/blob/{commit}/{file}",
        line: "{web}/blob/{commit}/{file}#L{start}",
        range: "{web}/blob/{commit}/{file}#L{start}-L{end}",
    },
];

/// The layout of the host `host`: the one `chosen` names for exactly that host, else the one its name gives away, else GitHub's, which most hosts that name nothing follow.
fn layout_of<'a>(host: &str, chosen: &BTreeMap<String, String>) -> &'a Layout {
    let host = host.to_lowercase();
    let by_setting = chosen
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(&host))
        .and_then(|(_, id)| {
            LAYOUTS
                .iter()
                .find(|layout| layout.id.eq_ignore_ascii_case(id))
        });
    by_setting
        .or_else(|| {
            LAYOUTS
                .iter()
                .find(|layout| layout.names.iter().any(|name| host.contains(name)))
        })
        .unwrap_or(&LAYOUTS[LAYOUTS.len() - 1])
}

/// The address `web` of a repository on Azure DevOps with the host's ssh form, `ssh.dev.azure.com/v3/organisation/project/repository`, turned into the form of its pages, `dev.azure.com/organisation/project/_git/repository`.
fn azure_pages(web: &str) -> String {
    let Some(rest) = web.strip_prefix("https://ssh.dev.azure.com/v3/") else {
        return web.to_owned();
    };
    let mut parts = rest.splitn(3, '/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(organisation), Some(project), Some(repository)) => {
            format!("https://dev.azure.com/{organisation}/{project}/_git/{repository}")
        }
        _ => web.to_owned(),
    }
}

/// The web address of `path` in the repository `directory` is in, at the commit checked out, on the host its remote is on: the remote the branch follows, `origin`, or the first there is. `lines` narrows it to a line, or to a range of lines, as the first and the last counted from 1; `hosts` says which layout the servers it names use.
pub fn permalink(
    directory: &Path,
    path: &str,
    lines: Option<(usize, usize)>,
    hosts: &BTreeMap<String, String>,
) -> Result<String, String> {
    let (directory, path) = multirepo::holder(directory, inside(path)?)?;
    let directory = directory.as_path();
    let path = path.as_str();
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
    let layout = layout_of(host, hosts);
    let web = if layout.id == "azure" {
        azure_pages(&web)
    } else {
        web
    };
    let template = match lines {
        None => layout.file,
        Some((start, end)) if start >= end => layout.line,
        Some(_) => layout.range,
    };
    let (start, end) = lines.map_or((0, 0), |(start, end)| (start, end.max(start)));
    Ok(template
        .replace("{web}", &web)
        .replace("{commit}", &commit)
        .replace("{file}", &file)
        .replace("{start}", &start.to_string())
        .replace("{end}", &end.to_string())
        .replace("{after}", &(end + 1).to_string()))
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
    if let Some(children) = multirepo::children_of(directory) {
        let mut found = Vec::new();
        for child in &children {
            let inside = search(&directory.join(child), query, regex, case)?;
            found.extend(inside.into_iter().map(|held| Found {
                path: format!("{child}/{}", held.path),
                ..held
            }));
        }
        found.truncate(MOST_FOUND);
        return Ok(found);
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
    let (directory, path) = multirepo::holder(directory, inside(path)?)?;
    let directory = directory.as_path();
    let path = path.as_str();
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
