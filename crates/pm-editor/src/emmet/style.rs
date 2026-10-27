//! Style abbreviations written out as property declarations.

mod tables;

use super::data::Style;
use super::snippet::escape;

/// The declarations `abbreviation` expands to, one per line, or nothing when it is not an abbreviation.
pub(super) fn expand(abbreviation: &str, tables: &Style) -> Option<String> {
    if abbreviation.chars().any(char::is_whitespace) {
        return None;
    }
    let mut stops = 0;
    let mut lines = Vec::new();
    for part in split_declarations(abbreviation)? {
        lines.push(declaration(part, &mut stops, tables)?);
    }
    Some(format!("{}$0", lines.join("\n")))
}

/// `text` cut at each `+` that is not inside parentheses; nothing when the parentheses do not pair or a part is empty.
fn split_declarations(text: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut depth = 0_usize;
    let mut from = 0;
    for (at, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            '+' if depth == 0 => {
                parts.push(&text[from..at]);
                from = at + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    parts.push(&text[from..]);
    parts.iter().all(|part| !part.is_empty()).then_some(parts)
}

/// One declaration line for `part`, counting the value stops it adds in `stops`.
fn declaration(part: &str, stops: &mut usize, tables: &Style) -> Option<String> {
    let (body, important) = match part.strip_suffix('!') {
        Some(body) => (body, " !important"),
        None => (part, ""),
    };
    if body.is_empty() {
        return None;
    }
    let (property, value) = match body.split_once(':') {
        Some((property, value)) => from_colon(property, value, tables)?,
        None => from_abbreviation(body, tables)?,
    };
    let value = match value {
        Some(value) => escape(&value),
        None => {
            *stops += 1;
            format!("${{{}:value}}", *stops)
        }
    };
    Some(format!("{property}: {value}{important};"))
}

/// The property and value of `property:value`; a value of `None` is still to be written.
fn from_colon(property: &str, value: &str, tables: &Style) -> Option<(String, Option<String>)> {
    let property = match tables.property(property) {
        Some(property) => property.to_string(),
        None if is_identifier(property) => property.to_string(),
        None => return None,
    };
    if value.is_empty() {
        return Some((property, None));
    }
    let value = if value.chars().all(|c| c.is_ascii_alphabetic() || c == '-') {
        tables.keyword(&property, value).to_string()
    } else {
        numeric_value(&property, value, tables).unwrap_or_else(|| value.to_string())
    };
    Some((property, Some(value)))
}

/// Whether `text` can be a property name written out in full.
fn is_identifier(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '-')
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The property and value of an abbreviation such as `m10-20`, `df` or `posa`; a value of `None` is still to be written.
fn from_abbreviation(body: &str, tables: &Style) -> Option<(String, Option<String>)> {
    if tables.is_property(body) {
        return Some((body.to_string(), None));
    }
    let head_length = body
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(body.len());
    let (head, rest) = body.split_at(head_length);
    if head.is_empty() {
        return None;
    }
    if let Some(property) = tables.property(head) {
        let value = if rest.is_empty() {
            None
        } else {
            Some(numeric_value(property, rest, tables)?)
        };
        return Some((property.to_string(), value));
    }
    if !rest.is_empty() {
        return None;
    }
    (1..head.len()).rev().find_map(|cut| {
        let (name, word) = head.split_at(cut);
        let property = tables.alias(name)?;
        let value = tables.keyword_alias(property, word)?;
        Some((property.to_string(), Some(value.to_string())))
    })
}

/// The value written as numbers, units, keywords and colours joined by `-`, such as `10-20` or `0a`; nothing when it is not written that way.
fn numeric_value(property: &str, text: &str, tables: &Style) -> Option<String> {
    let characters: Vec<char> = text.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut at = 0;
    while at < characters.len() {
        if at > 0 && characters[at] == '-' {
            at += 1;
            if at == characters.len() {
                return None;
            }
        }
        let character = characters[at];
        if character == '#' {
            let from = at;
            at += 1;
            while characters
                .get(at)
                .is_some_and(|c| c.is_ascii_alphanumeric())
            {
                at += 1;
            }
            if at == from + 1 {
                return None;
            }
            tokens.push(characters[from..at].iter().collect());
        } else if character.is_ascii_digit() || matches!(character, '.' | '-') {
            let from = at;
            if character == '-' {
                at += 1;
            }
            let digits_from = at;
            while characters.get(at).is_some_and(char::is_ascii_digit) {
                at += 1;
            }
            if characters.get(at) == Some(&'.') {
                at += 1;
                while characters.get(at).is_some_and(char::is_ascii_digit) {
                    at += 1;
                }
            }
            let number: String = characters[from..at].iter().collect();
            if !number[digits_from - from..]
                .chars()
                .any(|c| c.is_ascii_digit())
            {
                return None;
            }
            let letters_from = at;
            if characters.get(at) == Some(&'%') {
                at += 1;
            } else {
                while characters.get(at).is_some_and(char::is_ascii_alphabetic) {
                    at += 1;
                }
            }
            let letters: String = characters[letters_from..at].iter().collect();
            if letters.is_empty() || tables.unit(property, &letters).is_some() {
                tokens.push(with_unit(property, &number, &letters, tables));
            } else {
                tokens.push(with_unit(property, &number, "", tables));
                tokens.push(tables.keyword(property, &letters).to_string());
            }
        } else if character.is_ascii_alphabetic() {
            let from = at;
            while characters.get(at).is_some_and(char::is_ascii_alphabetic) {
                at += 1;
            }
            let word: String = characters[from..at].iter().collect();
            tokens.push(tables.keyword(property, &word).to_string());
        } else {
            return None;
        }
    }
    (!tokens.is_empty()).then(|| tokens.join(" "))
}

/// `number` written with `letters` as its unit, or with the unit `property` takes by default.
fn with_unit(property: &str, number: &str, letters: &str, tables: &Style) -> String {
    if let Some(unit) = tables.unit(property, letters) {
        return format!("{number}{unit}");
    }
    let is_zero = number.chars().all(|c| matches!(c, '0' | '-'));
    if is_zero || tables.is_unitless(property) {
        number.to_string()
    } else {
        format!("{number}{}", tables.default_unit)
    }
}
