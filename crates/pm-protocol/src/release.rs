//! The versions releases of the app are tagged with, and which of two comes later.

/// One dot-separated part of what follows the `-` of a version: a number, or a word such as `rc`; a number comes before a word.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    /// A part written in digits.
    Number(u64),
    /// A part written in anything else.
    Word(String),
}

/// What a version written `0.14.0` or `v0.14.0-rc.1` is ordered by: its numbers, whether it is the release those numbers name rather than one made before it, and the parts naming that earlier release.
fn order(version: &str) -> (Vec<u64>, bool, Vec<Part>) {
    let version = version.trim().trim_start_matches('v');
    let (numbers, before) = match version.split_once('-') {
        Some((numbers, before)) => (numbers, Some(before)),
        None => (version, None),
    };
    let parts = before
        .unwrap_or_default()
        .split('.')
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse()
                .map_or_else(|_| Part::Word(part.to_owned()), Part::Number)
        })
        .collect();
    (
        numbers
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect(),
        before.is_none(),
        parts,
    )
}

/// Whether the release `newest` comes after the version `running`.
pub fn is_newer(newest: &str, running: &str) -> bool {
    order(newest) > order(running)
}

/// Whether someone running `running` follows the release `tag`: every release made for everyone, and a pre-release only when `running` is itself a pre-release of the same numbers.
pub fn follows(tag: &str, prerelease: bool, running: &str) -> bool {
    let (numbers, _, _) = order(tag);
    let (running_numbers, running_final, _) = order(running);
    !prerelease || (!running_final && numbers == running_numbers)
}
