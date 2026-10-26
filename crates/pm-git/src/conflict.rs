//! The versions of a file that a merge, a rebase or a cherry-pick stopped on.

use std::path::Path;

use crate::{READ_LIMIT, inside, run};

/// What each side of a conflicted file holds, as the index keeps them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Sides {
    /// The file as the branch being worked on has it, nothing when it has none.
    pub ours: Option<String>,
    /// The file as the incoming side has it, nothing when it has none.
    pub theirs: Option<String>,
    /// The file as the two sides last had it in common, nothing when they had none.
    pub base: Option<String>,
}

/// The file `path` as the index keeps it at `stage`, 1 for the common ancestor, 2 for ours and 3 for theirs, cut at the limit of what is read, or nothing when the index has no such version.
fn staged_version(directory: &Path, path: &str, stage: u8) -> Option<String> {
    let mut text = run(directory, &["show", &format!(":{stage}:{path}")]).ok()?;
    if text.len() > READ_LIMIT {
        let mut end = READ_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    Some(text)
}

/// The three versions of the conflicted file `path` of the repository `directory` is in.
pub fn sides(directory: &Path, path: &str) -> Result<Sides, String> {
    let (directory, path) = crate::multirepo::holder(directory, inside(path)?)?;
    let directory = directory.as_path();
    let path = path.as_str();
    let sides = Sides {
        base: staged_version(directory, path, 1),
        ours: staged_version(directory, path, 2),
        theirs: staged_version(directory, path, 3),
    };
    if sides == Sides::default() {
        return Err(format!("{path} is not in conflict"));
    }
    Ok(sides)
}
