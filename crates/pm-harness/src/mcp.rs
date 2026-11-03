//! The tool servers an agent is given, and the command lines that start programs.

use omc_wire::{Reach, ToolServer};
use serde::Deserialize;

/// One tool server as the server keeps it: a name, what to run or reach, and the pairs that go with it.
#[derive(Deserialize)]
struct Kept {
    /// What the server is called.
    name: String,
    /// The command that starts it, or the address it listens at.
    target: String,
    /// The environment it is started with, or the headers it is sent.
    env: Vec<(String, String)>,
    /// Whether agents are given it; a server switched off stays listed.
    #[serde(default = "given")]
    enabled: bool,
}

/// Servers are given to agents unless they were switched off.
fn given() -> bool {
    true
}

/// Describes one kept server to an agent: by its address when `target` is one, by its command otherwise.
fn described(kept: Kept) -> Option<ToolServer> {
    if kept.target.starts_with("http://") || kept.target.starts_with("https://") {
        return Some(ToolServer {
            name: kept.name,
            reach: Reach::Http {
                url: kept.target,
                headers: kept.env,
            },
        });
    }
    let mut words = command_words(&kept.target).into_iter();
    let program = words.next()?;
    Some(ToolServer {
        name: kept.name,
        reach: Reach::Command {
            program,
            args: words.collect(),
            env: kept.env,
        },
    })
}

/// The tool servers in `kept`, the text the server saves them as, ready to hand an agent when it opens a conversation; none when the text is not a list of them.
#[must_use]
pub fn mcp_servers(kept: &str) -> Vec<ToolServer> {
    serde_json::from_str::<Vec<Kept>>(kept)
        .unwrap_or_default()
        .into_iter()
        .filter(|kept| kept.enabled)
        .filter_map(described)
        .collect()
}

/// `line` split into words at spaces, keeping what is in double quotes whole; inside quotes a backslash makes the character after it part of the word.
#[must_use]
pub fn command_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        match (character, quoted) {
            ('\\', true) => {
                word.extend(characters.next());
                started = true;
            }
            ('"', _) => {
                quoted = !quoted;
                started = true;
            }
            (space, false) if space.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            (other, _) => {
                word.push(other);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// `words` written as one command line, [`command_words`] reading it back.
#[must_use]
pub fn command_line<'a>(words: impl IntoIterator<Item = &'a str>) -> String {
    words.into_iter().map(quoted).collect::<Vec<_>>().join(" ")
}

/// `word` as [`command_words`] reads it back: in quotes, with quotes and backslashes escaped, when it holds a space, a quote, a backslash or is empty.
fn quoted(word: &str) -> String {
    match word.is_empty() || word.contains(|c: char| c.is_whitespace() || c == '"' || c == '\\') {
        true => format!("\"{}\"", word.replace('\\', "\\\\").replace('"', "\\\"")),
        false => word.to_owned(),
    }
}
