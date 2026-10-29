//! Wrapping prose to a width, as `gq` does: paragraphs are refilled with the indentation and the comment or list marker they began with.

use crate::buffer::{Buffer, Position};

/// Comment and prose markers wrapping keeps.
const LEADERS: [&str; 11] = [
    "///", "//!", "//", "#", "--", ";;", ";", "*", ">", "- ", "%",
];

/// Wraps the paragraphs between the lines `first` and `last` to `width` characters.
pub(crate) fn rewrap(buffer: &mut Buffer, first: usize, last: usize, width: usize) {
    let mut out = Vec::new();
    let mut paragraph = Vec::new();
    let mut prefix = String::new();
    for line in first..=last {
        let text = buffer.line_text(line);
        let (lead, body) = split_leader(&text);
        if body.trim().is_empty() || (!paragraph.is_empty() && lead != prefix) {
            wrap_paragraph(&mut paragraph, &prefix, width, &mut out);
        }
        if body.trim().is_empty() {
            out.push(text.trim_end().to_owned());
        } else {
            prefix = lead;
            paragraph.push(body.to_owned());
        }
    }
    wrap_paragraph(&mut paragraph, &prefix, width, &mut out);
    let range = Position::new(first, 0)..Position::new(last, buffer.line_len(last));
    let wrapped = out.join("\n");
    if buffer.text_in(range.clone()) != wrapped {
        buffer.replace(range, &wrapped);
    }
}

/// Writes `paragraph` as lines of at most `width` characters, each starting with `prefix`.
fn wrap_paragraph(paragraph: &mut Vec<String>, prefix: &str, width: usize, out: &mut Vec<String>) {
    if paragraph.is_empty() {
        return;
    }
    let text = paragraph.join(" ");
    let mut line = prefix.to_owned();
    let mut has_word = false;
    for word in text.split_whitespace() {
        if has_word && line.chars().count() + 1 + word.chars().count() > width {
            out.push(line);
            line = prefix.to_owned();
            has_word = false;
        }
        if has_word {
            line.push(' ');
        }
        line.push_str(word);
        has_word = true;
    }
    if has_word {
        out.push(line);
    }
    paragraph.clear();
}

/// Splits `text` into its indentation and marker, and its body.
fn split_leader(text: &str) -> (String, &str) {
    let indent = text.len() - text.trim_start().len();
    let rest = &text[indent..];
    let marker = LEADERS
        .iter()
        .find(|leader| rest.starts_with(**leader))
        .map_or(0, |leader| leader.len());
    let after = &rest[marker..];
    let spaces = after.len() - after.trim_start().len();
    let split = indent + marker + spaces;
    let lead = if marker > 0 && spaces == 0 && !after.is_empty() {
        format!("{} ", &text[..split])
    } else {
        text[..split].to_owned()
    };
    (lead, &text[split..])
}
