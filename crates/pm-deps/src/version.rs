//! Versions as manifests write them, compared far enough to tell whether a registry has a newer one.

use std::cmp::Ordering;

/// A version or requirement such as `1.2.3`, `^1.2` or `v0.5.0-rc.1`, reduced to its numbers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version {
    /// Major, minor and patch.
    parts: [u64; 3],
    /// How many of them were written.
    given: usize,
    /// Whether it carries a pre-release tag.
    pre: bool,
}

/// The characters that may stand before the number of a requirement that asks for one version and what is compatible with it.
const COMPATIBLE: &[char] = &['^', '~', '=', 'v', 'V', ' '];

/// The number of the requirement `text`, which holds the first digit; nothing when it holds none.
fn number_start(text: &str) -> Option<usize> {
    text.find(|character: char| character.is_ascii_digit())
}

impl Version {
    /// The version `text` names, with a leading `^`, `~`, `=` or `v` set aside; nothing when it is a range, a tag, a path or anything else that is not one plain version.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let start = number_start(text)?;
        if !text[..start]
            .chars()
            .all(|character| COMPATIBLE.contains(&character))
        {
            return None;
        }
        let number = text[start..].split('+').next().unwrap_or_default();
        let (number, pre) = match number.split_once('-') {
            Some((number, _)) => (number, true),
            None => (number, false),
        };
        let mut parts = [0; 3];
        let mut given = 0;
        for piece in number.split('.') {
            if given == 3 {
                return None;
            }
            parts[given] = piece.parse().ok()?;
            given += 1;
        }
        Some(Self { parts, given, pre })
    }

    /// Whether `latest` is ahead of the version a manifest asks for, as far as the parts the manifest wrote go: `1` is not behind `1.9.0`, and `1.2.3` is.
    pub fn is_behind(&self, latest: &Self) -> bool {
        match self.parts[..self.given].cmp(&latest.parts[..self.given]) {
            Ordering::Less => true,
            Ordering::Equal => self.pre && !latest.pre && self.given == 3,
            Ordering::Greater => false,
        }
    }
}

/// The requirement `requirement` written to ask for `latest` instead, keeping what stands before its number, such as `^` or `v`.
pub(crate) fn rewritten(requirement: &str, latest: &str) -> String {
    let requirement = requirement.trim();
    let start = number_start(requirement).unwrap_or(requirement.len());
    format!(
        "{}{}",
        &requirement[..start],
        latest.trim_start_matches(['v', 'V'])
    )
}
