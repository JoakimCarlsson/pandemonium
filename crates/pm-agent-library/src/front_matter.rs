//! The fields at the head of a markdown file, between two `---` lines, as the agents write them.

/// The fields of `text`'s front matter, by name; a field written over several `- item` lines holds them joined by commas. A file with no front matter has none.
pub(crate) fn fields(text: &str) -> Vec<(String, String)> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Vec::new();
    }
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in lines.take_while(|line| line.trim() != "---") {
        if let Some(item) = line.trim().strip_prefix("- ")
            && line.starts_with(char::is_whitespace)
            && let Some((_, value)) = fields.last_mut()
        {
            if !value.is_empty() {
                value.push_str(", ");
            }
            value.push_str(unquoted(item));
        } else if let Some((name, value)) = line.split_once(':')
            && !line.starts_with(char::is_whitespace)
        {
            fields.push((name.trim().to_owned(), unquoted(value.trim()).to_owned()));
        }
    }
    fields
}

/// The value of the field `name` in `fields`, when it is there and not empty.
pub(crate) fn field<'a>(fields: &'a [(String, String)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(held, _)| held == name)
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.is_empty())
}

/// The items of a field written as `a, b` or `[a, b]`; a comma inside braces, as in the pattern `*.{js,ts}`, does not part two items.
pub(crate) fn list(value: &str) -> Vec<String> {
    let inner = value.trim();
    let inner = inner
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(inner);
    let mut items = Vec::new();
    let mut item = String::new();
    let mut depth = 0usize;
    for character in inner.chars() {
        match character {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(std::mem::take(&mut item));
                continue;
            }
            _ => {}
        }
        item.push(character);
    }
    items.push(item);
    items
        .iter()
        .map(|item| unquoted(item.trim()).to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

/// `value` without the quotes around it.
fn unquoted(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .unwrap_or(value)
}
