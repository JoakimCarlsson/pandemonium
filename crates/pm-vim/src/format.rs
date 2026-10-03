//! Indentation, number increments and letter rotation over selected text.

use pm_text::{Buffer, Position};

use crate::text::first_non_blank;

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
