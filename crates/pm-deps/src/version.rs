//! Versions as a language folder says manifests write them, compared far enough to tell whether a registry has a newer one.

use std::cmp::Ordering;

use regex::Regex;

/// A version or requirement such as `1.2.3`, `^1.2` or `v0.5.0-rc.1`, as the pattern of the language reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Version {
    /// The numbers, major first.
    parts: Vec<u64>,
    /// What stands before the number, such as `^` or `v`.
    prefix: String,
    /// The number as written, with its pre-release tag.
    number: String,
    /// Whether it carries a pre-release tag.
    pre: bool,
}

impl Version {
    /// The version `text` names as `pattern` reads it: its `version` group holds dot-separated numbers, its `prefix` group what stands before them, and its `pre` group a pre-release tag. Nothing when the pattern does not match, such as for a range, a tag, a path or an address.
    pub fn parse(pattern: &Regex, text: &str) -> Option<Self> {
        let found = pattern.captures(text.trim())?;
        let number = found.name("version")?.as_str();
        let parts = number
            .split('.')
            .map(|piece| piece.parse().ok())
            .collect::<Option<Vec<u64>>>()?;
        let pre = found.name("pre").map(|held| held.as_str().to_owned());
        Some(Self {
            parts,
            prefix: found
                .name("prefix")
                .map_or_else(String::new, |held| held.as_str().to_owned()),
            number: format!("{number}{}", pre.as_deref().unwrap_or_default()),
            pre: pre.is_some(),
        })
    }

    /// Whether `latest` is ahead of the version a manifest asks for, as far as the numbers the manifest wrote go: `1` is not behind `1.9.0`, and `1.2.3` is.
    pub fn is_behind(&self, latest: &Self) -> bool {
        let given = self.parts.len();
        let wanted: Vec<u64> = (0..given)
            .map(|at| latest.parts.get(at).copied().unwrap_or(0))
            .collect();
        match self.parts.cmp(&wanted) {
            Ordering::Less => true,
            Ordering::Equal => self.pre && !latest.pre && given >= 3,
            Ordering::Greater => false,
        }
    }

    /// What the requirement of this version is written as to ask for `latest` instead, keeping what stands before its number.
    pub fn rewritten(&self, latest: &Self) -> String {
        format!("{}{}", self.prefix, latest.number)
    }
}
