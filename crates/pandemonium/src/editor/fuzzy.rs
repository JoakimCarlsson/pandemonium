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

/// How `typed` matches a name: how well, and which characters of the name
/// it is made of.
pub struct Match {
    /// How good the match is, higher being better.
    pub score: i32,
    /// The indices, in characters, of the characters of the name it matched.
    pub at: Vec<usize>,
}

/// How `typed` matches `name`, or `None` when it does not match at all.
pub fn matching(typed: &str, name: &str) -> Option<Match> {
    let wanted: Vec<char> = typed.chars().flat_map(char::to_lowercase).collect();
    if wanted.is_empty() {
        return Some(Match {
            score: 0,
            at: Vec::new(),
        });
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
    let mut came_from: Vec<Vec<usize>> = Vec::with_capacity(wanted.len());
    came_from.push(vec![usize::MAX; name.len()]);
    for (index, wanted) in wanted.iter().enumerate().skip(1) {
        let mut now = vec![None; name.len()];
        let mut from = vec![usize::MAX; name.len()];
        let mut skipped: Option<(i32, usize)> = None;
        for at in 0..name.len() {
            if at >= 2 {
                skipped = skipped
                    .map(|(score, prior)| (score - 1, prior))
                    .into_iter()
                    .chain(before[at - 2].map(|score| (score - 1, at - 2)))
                    .max_by_key(|(score, _)| *score);
            }
            if lower[at] != *wanted {
                continue;
            }
            let consecutive = at
                .checked_sub(1)
                .and_then(|prior| before[prior].map(|score| (score + CONSECUTIVE, prior)));
            let best = skipped
                .into_iter()
                .chain(consecutive)
                .max_by_key(|(score, _)| *score);
            if let Some((score, prior)) = best {
                now[at] = Some(score + gain(index, at));
                from[at] = prior;
            }
        }
        came_from.push(from);
        before = now;
    }
    let (end, best) = before
        .iter()
        .enumerate()
        .filter_map(|(at, score)| score.map(|score| (at, score)))
        .max_by_key(|(_, score)| *score)?;
    let mut at = Vec::with_capacity(wanted.len());
    let mut place = end;
    for from in came_from.iter().rev() {
        at.push(place);
        place = from[place];
    }
    at.reverse();
    let prefixed = lower.starts_with(&wanted);
    Some(Match {
        score: best + PREFIX * i32::from(prefixed),
        at,
    })
}
