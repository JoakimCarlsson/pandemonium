//! The tool servers an agent is started with, as the protocol describes them.

use serde::Deserialize;
use serde_json::{Value, json};

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

/// The pairs of `env` as the protocol writes them: a name and a value each.
fn pairs(env: Vec<(String, String)>) -> Vec<Value> {
    env.into_iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect()
}

/// Describes one kept server to an agent: by its address when `target` is one, by its command otherwise.
fn described(kept: Kept) -> Option<Value> {
    if kept.target.starts_with("http://") || kept.target.starts_with("https://") {
        return Some(json!({
            "type": "http",
            "name": kept.name,
            "url": kept.target,
            "headers": pairs(kept.env),
        }));
    }
    let mut words = command_words(&kept.target).into_iter();
    let command = words.next()?;
    Some(json!({
        "name": kept.name,
        "command": command,
        "args": words.collect::<Vec<_>>(),
        "env": pairs(kept.env),
    }))
}

/// The tool servers in `kept`, the text the server saves them as, ready to hand an agent when it opens a conversation; none when the text is not a list of them.
#[must_use]
pub fn mcp_servers(kept: &str) -> Vec<Value> {
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
