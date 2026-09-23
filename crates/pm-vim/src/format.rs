//! Rewriting lines rather than moving through them: wrapping them to a
//! width, indenting them the way the lines around them are, counting the
//! numbers in them up and down.

use pm_text::{Buffer, Position};

use crate::text::first_non_blank;

/// The markers a line of prose-like text may start with and keep when it is
/// wrapped: comment tokens and the quote and bullet a paragraph carries.
const LEADERS: [&str; 11] = [
    "///", "//!", "//", "#", "--", ";;", ";", "*", ">", "- ", "%",
];

/// Wraps the lines from `first` to `last` at `width` columns, one paragraph
/// at a time, keeping each paragraph's indentation and comment marker.
pub(crate) fn rewrap(buffer: &mut Buffer, first: usize, last: usize, width: usize) {
    let lines = (first..=last)
        .map(|line| buffer.line_text(line))
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut prefix = String::new();
    let flush = |paragraph: &mut Vec<String>, prefix: &str, out: &mut Vec<String>| {
        if paragraph.is_empty() {
            return;
        }
        let words = paragraph
            .iter()
            .flat_map(|line| line.split_whitespace())
            .collect::<Vec<_>>();
        let room = width.saturating_sub(prefix.chars().count()).max(1);
        let mut line = String::new();
        for word in words {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > room {
                out.push(format!("{prefix}{line}"));
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            out.push(format!("{prefix}{line}"));
        }
        paragraph.clear();
    };
    for text in &lines {
        let (lead, body) = split_leader(text);
        if body.trim().is_empty() {
            flush(&mut paragraph, &prefix, &mut out);
            out.push(text.trim_end().to_owned());
            continue;
        }
        if !paragraph.is_empty() && lead != prefix {
            flush(&mut paragraph, &prefix, &mut out);
        }
        prefix = lead;
        paragraph.push(body.to_owned());
    }
    flush(&mut paragraph, &prefix, &mut out);
    let range = Position::new(first, 0)..Position::new(last, buffer.line_len(last));
    let rewrapped = out.join("\n");
    if buffer.text_in(range.clone()) != rewrapped {
        buffer.grouped(|buffer| buffer.replace(range, &rewrapped));
    }
}

/// `text` split into the indentation and marker it starts with, spaces
/// after the marker included, and the rest.
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
    let lead = match marker > 0 && spaces == 0 && !after.is_empty() {
        true => format!("{} ", &text[..split]),
        false => text[..split].to_owned(),
    };
    (lead, &text[split..])
}

/// Indents the lines from `first` to `last` from the line above them: one
/// step in after a line that opens a bracket, one step out for a line that
/// closes one.
pub(crate) fn reindent(buffer: &mut Buffer, first: usize, last: usize) {
    let step = buffer.indent().step();
    let width = step.chars().count().max(1);
    let mut previous = (0..first)
        .rev()
        .find(|line| !buffer.line_text(*line).trim().is_empty())
        .map(|line| buffer.line_text(line));
    let mut edits = Vec::new();
    for line in first..=last {
        let text = buffer.line_text(line);
        let body = text.trim_start();
        if body.is_empty() {
            continue;
        }
        let mut level = previous.as_deref().map_or(0, |above| {
            let depth = leading_columns(above, width) / width;
            let opens = above.trim_end().ends_with(['{', '(', '[', ':']);
            depth + usize::from(opens)
        });
        if body.starts_with(['}', ')', ']']) {
            level = level.saturating_sub(1);
        }
        let indented = format!("{}{body}", step.repeat(level));
        if indented != text {
            edits.push((
                Position::new(line, 0)..Position::new(line, first_non_blank(buffer, line)),
                step.repeat(level),
            ));
        }
        previous = Some(indented);
    }
    if !edits.is_empty() {
        buffer.apply_edits(edits);
    }
}

/// How many columns the indentation of `text` comes to, a tab counting as
/// one step of `width`.
fn leading_columns(text: &str, width: usize) -> usize {
    text.chars()
        .take_while(|ch| ch.is_whitespace())
        .map(|ch| if ch == '\t' { width } else { 1 })
        .sum()
}

/// The number at or after `column` on `text`, and what it becomes with
/// `delta` added: its span in columns and its new text.
///
/// Hexadecimal and binary keep their prefix and their width; a decimal
/// number takes a minus sign in front of it as its own.
pub(crate) fn increment(text: &str, column: usize, delta: i64) -> Option<(usize, usize, String)> {
    let numbers = regex::Regex::new(r"0[xX][0-9a-fA-F]+|0[bB][01]+|-?\d+").ok()?;
    let found = numbers
        .find_iter(text)
        .find(|found| text[..found.end()].chars().count() > column)?;
    let start = text[..found.start()].chars().count();
    let end = start + found.as_str().chars().count();
    let number = found.as_str();
    let (radix, digits) = match number.get(..2) {
        Some("0x" | "0X") => (16, &number[2..]),
        Some("0b" | "0B") => (2, &number[2..]),
        _ => (10, number),
    };
    if radix != 10 {
        let value = u64::from_str_radix(digits, radix).ok()?;
        let next = value.wrapping_add_signed(delta);
        let width = digits.len();
        let upper = digits.chars().any(|ch| ch.is_ascii_uppercase());
        let written = match (radix, upper) {
            (16, true) => format!("{next:0width$X}"),
            (16, false) => format!("{next:0width$x}"),
            _ => format!("{next:0width$b}"),
        };
        return Some((start + 2, end, written));
    }
    let value = digits.parse::<i64>().ok()?;
    Some((start, end, value.saturating_add(delta).to_string()))
}

/// `text` with every letter moved thirteen places along the alphabet.
pub(crate) fn rot13(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'a'..='z' => (((ch as u8 - b'a' + 13) % 26) + b'a') as char,
            'A'..='Z' => (((ch as u8 - b'A' + 13) % 26) + b'A') as char,
            other => other,
        })
        .collect()
}
