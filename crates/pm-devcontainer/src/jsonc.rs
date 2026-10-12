//! JSON with comments and trailing commas, as `devcontainer.json` is written.

use serde_json::Value;

/// Why a declaration could not be read as JSON.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxError {
    /// The one-based line of the offending character.
    pub line: usize,
    /// The one-based column of the offending character.
    pub column: usize,
    /// What the parser expected.
    pub message: String,
}

impl std::fmt::Display for SyntaxError {
    /// Names the position and the parser's complaint.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

/// Parses `text` after blanking its comments and trailing commas.
///
/// Blanking keeps every byte offset, so a syntax error still points at the
/// character the author typed.
pub fn parse(text: &str) -> Result<Value, SyntaxError> {
    let cleaned = clean(text);
    serde_json::from_str(&cleaned).map_err(|error| SyntaxError {
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    })
}

/// Replaces comments and trailing commas by spaces, leaving strings untouched.
fn clean(text: &str) -> String {
    let characters = text
        .trim_start_matches('\u{feff}')
        .chars()
        .collect::<Vec<_>>();
    let mut cleaned = String::with_capacity(text.len());
    let mut at = 0;
    while at < characters.len() {
        let here = characters[at];
        let next = characters.get(at + 1).copied();
        match (here, next) {
            ('"', _) => at = copy_string(&characters, at, &mut cleaned),
            ('/', Some('/')) => at = blank_line(&characters, at, &mut cleaned),
            ('/', Some('*')) => at = blank_block(&characters, at, &mut cleaned),
            (',', _) if closes_next(&characters, at + 1) => {
                cleaned.push(' ');
                at += 1;
            }
            _ => {
                cleaned.push(here);
                at += 1;
            }
        }
    }
    cleaned
}

/// Copies one string literal, including its escapes, and returns where it ends.
fn copy_string(characters: &[char], start: usize, into: &mut String) -> usize {
    into.push('"');
    let mut at = start + 1;
    while at < characters.len() {
        let here = characters[at];
        into.push(here);
        at += 1;
        match here {
            '\\' => {
                if let Some(escaped) = characters.get(at) {
                    into.push(*escaped);
                    at += 1;
                }
            }
            '"' => break,
            _ => {}
        }
    }
    at
}

/// Blanks a line comment up to, not including, its newline.
fn blank_line(characters: &[char], start: usize, into: &mut String) -> usize {
    let mut at = start;
    while at < characters.len() && characters[at] != '\n' {
        into.push(' ');
        at += 1;
    }
    at
}

/// Blanks a block comment, keeping its newlines so lines stay aligned.
fn blank_block(characters: &[char], start: usize, into: &mut String) -> usize {
    let mut at = start;
    while at < characters.len() {
        let closing = characters[at] == '*' && characters.get(at + 1) == Some(&'/');
        if closing {
            into.push_str("  ");
            return at + 2;
        }
        into.push(if characters[at] == '\n' { '\n' } else { ' ' });
        at += 1;
    }
    at
}

/// Whether only whitespace and comments separate `from` and a closing bracket.
fn closes_next(characters: &[char], from: usize) -> bool {
    let mut at = from;
    while at < characters.len() {
        match (characters[at], characters.get(at + 1).copied()) {
            (space, _) if space.is_whitespace() => at += 1,
            ('/', Some('/')) => {
                while at < characters.len() && characters[at] != '\n' {
                    at += 1;
                }
            }
            ('/', Some('*')) => {
                at += 2;
                while at < characters.len()
                    && !(characters[at] == '*' && characters.get(at + 1) == Some(&'/'))
                {
                    at += 1;
                }
                at += 2;
            }
            (closing, _) => return matches!(closing, '}' | ']'),
        }
    }
    false
}
