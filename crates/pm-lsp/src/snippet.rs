//! Snippets as plain text: the placeholders of a completion's snippet replaced by what they hold.

/// The text `snippet` writes once each tab stop is dropped and each placeholder is replaced by its text or first choice.
pub(crate) fn plain(snippet: &str) -> String {
    let mut out = String::new();
    let mut characters = snippet.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' => {
                if let Some(next) = characters.next() {
                    out.push(next);
                }
            }
            '$' => match characters.peek() {
                Some(digit) if digit.is_ascii_digit() => {
                    while characters.peek().is_some_and(char::is_ascii_digit) {
                        characters.next();
                    }
                }
                Some('{') => {
                    characters.next();
                    let mut body = String::new();
                    let mut depth = 1;
                    for inner in characters.by_ref() {
                        match inner {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        body.push(inner);
                    }
                    out.push_str(&placeholder(&body));
                }
                _ => out.push('$'),
            },
            _ => out.push(character),
        }
    }
    out
}

/// What the body of a `${...}` placeholder writes: the text after `n:`, or the first choice of `n|a,b|`, itself made plain.
fn placeholder(body: &str) -> String {
    let rest = body.trim_start_matches(|c: char| c.is_ascii_digit());
    if let Some(text) = rest.strip_prefix(':') {
        plain(text)
    } else if let Some(choices) = rest.strip_prefix('|') {
        choices
            .trim_end_matches('|')
            .split(',')
            .next()
            .unwrap_or_default()
            .to_owned()
    } else {
        String::new()
    }
}
