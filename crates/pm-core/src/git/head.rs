//! Where a worktree stands: the branch it is on, and how far it has drifted
//! from the branch that one follows.
//!
//! This is read from the same answer the file statuses are, because git
//! offers both in one go: asking twice would be two subprocesses to say one
//! thing about one worktree.

/// Where a worktree stands against the branch it follows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Head {
    /// The branch checked out, or nothing while the head is detached.
    pub branch: Option<String>,
    /// The commit checked out, shortened to the length git prints.
    pub commit: Option<String>,
    /// The branch this one follows, when it follows one.
    pub upstream: Option<String>,
    /// Commits this branch has that the one it follows has not.
    pub ahead: usize,
    /// Commits the branch it follows has that this one has not.
    pub behind: usize,
}

/// How much of a commit hash names it where a branch would be named.
const SHORT_HASH: usize = 7;

/// What git calls a head that is on no branch at all.
const DETACHED: &str = "(detached)";

impl Head {
    /// What to call this head where one line has room for it.
    ///
    /// A branch is its name; a detached head is the commit it is on, because
    /// that is the only thing there is to call it.
    pub fn name(&self) -> String {
        match (&self.branch, &self.commit) {
            (Some(branch), _) => branch.clone(),
            (None, Some(commit)) => commit.clone(),
            (None, None) => String::new(),
        }
    }

    /// Takes in one `# branch.…` header of a status, ignoring any other.
    ///
    /// The headers come before the files and each says one thing, so this is
    /// called with every line git begins with `#` and picks out the four it
    /// knows; a worktree with no commits yet sends fewer of them, and what it
    /// does not send stays as it was.
    pub(super) fn read(&mut self, header: &str) {
        let Some((field, value)) = header.trim_start_matches('#').trim().split_once(' ') else {
            return;
        };
        match field {
            "branch.oid" => self.commit = shortened(value),
            "branch.head" if value == DETACHED => self.branch = None,
            "branch.head" => self.branch = Some(value.to_owned()),
            "branch.upstream" => self.upstream = Some(value.to_owned()),
            "branch.ab" => self.drift(value),
            _ => {}
        }
    }

    /// Takes in how far the branch has drifted, as `+2 -1`.
    fn drift(&mut self, counts: &str) {
        for count in counts.split_whitespace() {
            let (sign, number) = count.split_at(1);
            let Ok(number) = number.parse() else {
                continue;
            };
            match sign {
                "+" => self.ahead = number,
                "-" => self.behind = number,
                _ => {}
            }
        }
    }
}

/// `commit` cut to the length a hash is printed at, unless it is no commit.
///
/// A worktree with nothing committed yet reports its head as the literal
/// word git uses for it, which names no commit and so is not one.
fn shortened(commit: &str) -> Option<String> {
    let real = commit
        .chars()
        .all(|character| character.is_ascii_hexdigit());
    real.then(|| commit.chars().take(SHORT_HASH).collect())
}
