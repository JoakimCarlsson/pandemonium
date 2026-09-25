//! What an agent asks the editor to do for it, and what the editor answers.
//!
//! The protocol makes the editor the agent's hands: it reads and writes the
//! worktree's files and runs its commands. Those answers are the window's to
//! give, because the file an agent reads is the one the reader has open,
//! unsaved edits and all, and the command it runs is a shell the reader can
//! watch. So a request leaves this crate as a [`Request`] under a ticket, and
//! comes back as an [`Answer`]; the protocol's spelling of both stays here.

use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

/// Something an agent has asked the editor to do.
#[derive(Clone, Debug)]
pub enum Request {
    /// Say what the file at `path` holds now, as the reader sees it.
    Read {
        /// The file, inside the session's worktree.
        path: PathBuf,
    },
    /// Make the file at `path` hold `text`.
    Write {
        /// The file, inside the session's worktree.
        path: PathBuf,
        /// Everything it is to hold.
        text: String,
    },
    /// Start a command in a terminal of its own.
    Run(Run),
    /// Say what the terminal has written so far, and whether it has exited.
    Output {
        /// The terminal, as the run that started it named it.
        terminal: String,
    },
    /// Answer once the terminal's command has exited, and how it did.
    Wait {
        /// The terminal, as the run that started it named it.
        terminal: String,
    },
    /// End the terminal's command, keeping what it wrote.
    Kill {
        /// The terminal, as the run that started it named it.
        terminal: String,
    },
    /// Let the terminal go: the agent will ask nothing more of it.
    Release {
        /// The terminal, as the run that started it named it.
        terminal: String,
    },
}

/// A command an agent wants run.
#[derive(Clone, Debug)]
pub struct Run {
    /// What the terminal is called from now on, in every later request about
    /// it and in the tool calls that show it.
    pub terminal: String,
    /// The program, or the whole command line when no arguments follow.
    pub command: String,
    /// The arguments, where the agent split them out.
    pub args: Vec<String>,
    /// Variables to set for it, on top of the worktree's own.
    pub env: Vec<(String, String)>,
    /// Where it runs, inside the session's worktree.
    pub cwd: PathBuf,
}

/// How a terminal's command ended.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Exit {
    /// The code it exited with, where it exited by itself.
    pub code: Option<u32>,
    /// The signal that ended it, where one did.
    pub signal: Option<String>,
}

/// What the editor answers a [`Request`] with.
#[derive(Clone, Debug)]
pub enum Answer {
    /// It was done, and there is nothing to say back.
    Done,
    /// The whole of a file that was asked to be read.
    Text(String),
    /// What a terminal has written, and how it ended where it has.
    Output {
        /// Everything it has written, oldest first.
        text: String,
        /// How it ended, once it has.
        exit: Option<Exit>,
    },
    /// How a terminal that was waited on ended.
    Exited(Exit),
    /// It could not be done, for the reason given.
    Failed(String),
}

/// What an answer has to be shaped into once it is given.
#[derive(Clone, Debug)]
pub(crate) enum Shape {
    /// Nothing but what the answer says.
    Plain,
    /// The lines of the file from `line`, at most `limit` of them.
    Lines {
        /// The first line wanted, counted from one.
        line: Option<u64>,
        /// How many lines are wanted.
        limit: Option<u64>,
    },
    /// The tail of what a terminal wrote, no more than `limit` bytes of it.
    Tail {
        /// The most bytes the agent will keep.
        limit: Option<usize>,
    },
}

/// The request `method` makes with `params`, and the shape its answer takes.
///
/// `terminal` is the name a command started now is given. A request for a
/// file outside `root`, or for something the editor does not do, is not a
/// request: the error says why.
pub(crate) fn read(
    root: &Path,
    method: &str,
    params: &Value,
    terminal: String,
) -> Result<(Request, Shape), String> {
    let terminal_named = || params["terminalId"].as_str().map(str::to_owned);
    match method {
        "fs/read_text_file" => Ok((
            Request::Read {
                path: within(root, params["path"].as_str())?,
            },
            Shape::Lines {
                line: params["line"].as_u64(),
                limit: params["limit"].as_u64(),
            },
        )),
        "fs/write_text_file" => Ok((
            Request::Write {
                path: within(root, params["path"].as_str())?,
                text: params["content"].as_str().unwrap_or_default().to_owned(),
            },
            Shape::Plain,
        )),
        "terminal/create" => Ok((
            Request::Run(Run {
                terminal,
                command: params["command"].as_str().ok_or("no command")?.to_owned(),
                args: strings(&params["args"]),
                env: variables(&params["env"]),
                cwd: match params["cwd"].as_str() {
                    Some(cwd) => within(root, Some(cwd))?,
                    None => root.to_path_buf(),
                },
            }),
            Shape::Tail {
                limit: params["outputByteLimit"]
                    .as_u64()
                    .map(|limit| limit as usize),
            },
        )),
        "terminal/output" => Ok((
            Request::Output {
                terminal: terminal_named().ok_or("no terminal")?,
            },
            Shape::Plain,
        )),
        "terminal/wait_for_exit" => Ok((
            Request::Wait {
                terminal: terminal_named().ok_or("no terminal")?,
            },
            Shape::Plain,
        )),
        "terminal/kill" => Ok((
            Request::Kill {
                terminal: terminal_named().ok_or("no terminal")?,
            },
            Shape::Plain,
        )),
        "terminal/release" => Ok((
            Request::Release {
                terminal: terminal_named().ok_or("no terminal")?,
            },
            Shape::Plain,
        )),
        _ => Err(method.to_owned()),
    }
}

/// `answer` as the reply the agent is owed, shaped by `shape`.
///
/// A run is answered with the name it was given, which `request` holds, and
/// a terminal's output with no more than the tail `limit` allows.
pub(crate) fn reply(
    request: &Request,
    shape: &Shape,
    limit: Option<usize>,
    answer: Answer,
) -> Result<Value, String> {
    match (answer, request) {
        (Answer::Failed(trouble), _) => Err(trouble),
        (Answer::Done, Request::Run(run)) => Ok(json!({ "terminalId": run.terminal })),
        (Answer::Done, _) => Ok(json!({})),
        (Answer::Text(text), _) => Ok(json!({ "content": lines(&text, shape) })),
        (Answer::Output { text, exit }, _) => {
            let (output, truncated) = tail(&text, limit);
            Ok(json!({
                "output": output,
                "truncated": truncated,
                "exitStatus": exit.as_ref().map(status),
            }))
        }
        (Answer::Exited(exit), _) => Ok(status(&exit)),
    }
}

/// How `exit` is written on the wire.
fn status(exit: &Exit) -> Value {
    json!({ "exitCode": exit.code, "signal": exit.signal })
}

/// The part of `text` a read shaped by `shape` asked for.
fn lines(text: &str, shape: &Shape) -> String {
    let Shape::Lines { line, limit } = shape else {
        return text.to_owned();
    };
    if line.is_none() && limit.is_none() {
        return text.to_owned();
    }
    let from = line.unwrap_or(1).max(1) as usize - 1;
    let count = limit.unwrap_or(u64::MAX) as usize;
    text.lines()
        .skip(from)
        .take(count)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The end of `text` that fits in `limit` bytes, and whether any was cut.
///
/// The cut is made where a character begins, so what is kept is still text.
fn tail(text: &str, limit: Option<usize>) -> (&str, bool) {
    let Some(limit) = limit.filter(|limit| text.len() > *limit) else {
        return (text, false);
    };
    let from = (text.len() - limit..=text.len())
        .find(|at| text.is_char_boundary(*at))
        .unwrap_or(text.len());
    (&text[from..], true)
}

/// The strings `list` holds, skipping whatever is not one.
fn strings(list: &Value) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

/// The variables `list` sets, as names and values.
fn variables(list: &Value) -> Vec<(String, String)> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|variable| {
            Some((
                variable["name"].as_str()?.to_owned(),
                variable["value"].as_str().unwrap_or_default().to_owned(),
            ))
        })
        .collect()
}

/// The file `path` names, once it is known to be one of the worktree's.
///
/// The agent is a program of the reader's, running as they do, and nothing
/// here stops it opening a file for itself. What this stops is the editor
/// doing it on the agent's behalf: the session was opened over one worktree,
/// so the worktree is the whole of what the editor will read, write or run
/// in, and a path that climbs out of it is refused rather than followed.
fn within(root: &Path, path: Option<&str>) -> Result<PathBuf, String> {
    let path = cleaned(&root.join(path.ok_or("no path")?));
    match path.starts_with(cleaned(root)) {
        true => Ok(path),
        false => Err(format!(
            "{} is outside this session's worktree",
            path.display()
        )),
    }
}

/// `path` with the steps that go nowhere taken out of it.
///
/// The disk is not asked: a file being written may not exist yet, and one
/// that does may be reached through a link the reader meant to follow. What
/// is resolved here is only the spelling — `.` and the `..` that a path
/// climbs out through.
fn cleaned(path: &Path) -> PathBuf {
    let mut cleaned = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                cleaned.pop();
            }
            part => cleaned.push(part),
        }
    }
    cleaned
}
