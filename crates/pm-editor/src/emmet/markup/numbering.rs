//! Item numbering: the `$` of a repeated element, with its padding and its `@` modifiers.

/// `text` with each run of `$` replaced by the number of item `index` of `count`; `$$` pads to two digits, `$@-` counts down and `$@3` starts at three.
pub(super) fn number(text: &str, index: usize, count: usize) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut numbered = String::with_capacity(text.len());
    let mut at = 0;
    while at < characters.len() {
        if characters[at] != '$' {
            numbered.push(characters[at]);
            at += 1;
            continue;
        }
        let mut width = 0;
        while at < characters.len() && characters[at] == '$' {
            width += 1;
            at += 1;
        }
        let mut descending = false;
        let mut start = 1_usize;
        if characters.get(at) == Some(&'@') {
            let mut end = at + 1;
            if characters.get(end) == Some(&'-') {
                descending = true;
                end += 1;
            }
            let digits_from = end;
            while characters.get(end).is_some_and(char::is_ascii_digit) {
                end += 1;
            }
            if descending || end > digits_from {
                let digits: String = characters[digits_from..end].iter().collect();
                start = digits.parse().unwrap_or(1);
                at = end;
            }
        }
        let value = if descending {
            start + count.saturating_sub(1 + index)
        } else {
            start + index
        };
        numbered.push_str(&format!("{value:0width$}"));
    }
    numbered
}
