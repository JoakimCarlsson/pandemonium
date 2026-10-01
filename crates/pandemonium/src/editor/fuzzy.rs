//! How well what has been typed matches a name, the way a completion list
//! ranks what it offers.
//!
//! The typed characters must all appear in the name, in order, and the first
//! of them must begin a word of it. Runs of consecutive characters and
//! characters that begin a word score higher, and every character skipped
//! over costs a little, so a prefix beats a scatter and `fb` finds `fooBar`.

/// Score of a character that begins a word of the name.
const WORD_START: i32 = 8;

/// Score of a character directly after the one matched before it.
const CONSECUTIVE: i32 = 5;

/// Score of a character typed in the case the name has it in.
const SAME_CASE: i32 = 1;

/// Score of a name that begins with all that was typed.
const PREFIX: i32 = 20;

/// Whether the character at `at` begins a word of `name`: the first one, one
/// after a separator, or an upper-case one after a lower-case one.
fn begins_word(name: &[char], at: usize) -> bool {
    at == 0
        || !name[at - 1].is_alphanumeric()
        || (name[at].is_uppercase() && name[at - 1].is_lowercase())
}

/// How well `typed` matches `name`, higher being better, or `None` when it
/// does not match at all.
pub fn score(typed: &str, name: &str) -> Option<i32> {
    let wanted: Vec<char> = typed.chars().flat_map(char::to_lowercase).collect();
    if wanted.is_empty() {
        return Some(0);
    }
    let name: Vec<char> = name.chars().collect();
    if wanted.len() > name.len() {
        return None;
    }
    let lower: Vec<char> = name
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let typed: Vec<char> = typed.chars().collect();
    let gain = |index: usize, at: usize| {
        1 + SAME_CASE * i32::from(typed.get(index) == Some(&name[at]))
            + WORD_START * i32::from(begins_word(&name, at))
    };

    let mut before: Vec<Option<i32>> = (0..name.len())
        .map(|at| {
            (lower[at] == wanted[0] && begins_word(&name, at))
                .then(|| gain(0, at) - at.min(8) as i32)
        })
        .collect();
    for (index, wanted) in wanted.iter().enumerate().skip(1) {
        let mut now = vec![None; name.len()];
        let mut skipped: Option<i32> = None;
        for at in 0..name.len() {
            if at >= 2 {
                skipped = skipped
                    .map(|score| score - 1)
                    .max(before[at - 2].map(|score| score - 1));
            }
            if lower[at] != *wanted {
                continue;
            }
            let next = at
                .checked_sub(1)
                .and_then(|prior| before[prior])
                .map(|score| score + CONSECUTIVE)
                .max(skipped);
            now[at] = next.map(|score| score + gain(index, at));
        }
        before = now;
    }
    let best = before.into_iter().flatten().max()?;
    let prefixed = lower.starts_with(&wanted);
    Some(best + PREFIX * i32::from(prefixed))
}
