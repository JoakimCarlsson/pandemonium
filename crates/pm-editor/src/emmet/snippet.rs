//! Writing literal text into snippet syntax.

/// `text` with every `\`, `$` and `}` escaped so a snippet reads it back as itself.
pub(super) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(character, '\\' | '$' | '}') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}
