//! The right panel's Search tab: what is looked for across the project,
//! how, and the lines the server found, grouped by file, which can be
//! edited in place as excerpts of their files.

use crate::open_files::SharedDocument;

/// How many lines around a line found an excerpt shows.
pub const EXCERPT_CONTEXT: usize = 2;

/// A search across the files of the project.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectSearch {
    /// What is looked for, as typed.
    pub query: String,
    /// Whether the query is a regular expression.
    pub regex: bool,
    /// Whether upper and lower case must match.
    pub case: bool,
    /// The lines found for `searched`.
    pub hits: Vec<SearchHit>,
    /// The query the server is asked about, while it has not answered.
    pub searching: Option<String>,
    /// The query `hits` answer.
    pub searched: Option<String>,
    /// Why the last search failed, when it did.
    pub error: Option<String>,
    /// Whether the lines found are shown as excerpts that can be edited.
    pub editing: bool,
    /// The excerpts being edited, in the order of the files and lines.
    pub excerpts: Vec<SearchExcerpt>,
}

/// A stretch of a file around lines found, being edited in place.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchExcerpt {
    /// The file, from the root of the project.
    pub path: String,
    /// The first line of the stretch, counted from 1.
    pub first: usize,
    /// What the stretch held when it was read, each line ending in a line break.
    pub original: String,
    /// The stretch as it is edited.
    pub document: SharedDocument,
}

/// A line holding what was looked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchHit {
    /// The file, from the root of the project.
    pub path: String,
    /// The line, counted from 1.
    pub line: usize,
    /// Where the match starts in the line, in characters.
    pub column: usize,
    /// How many characters the match covers.
    pub length: usize,
    /// The line's text.
    pub text: String,
}

/// What the person does in the Search tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchAction {
    /// Changes what is looked for.
    Query(String),
    /// Looks for it.
    Run,
    /// Turns regular expressions on or off, and looks again.
    Regex,
    /// Turns matching case on or off, and looks again.
    Case,
    /// Shows the lines found as excerpts that can be edited, or as plain lines again.
    Edit,
    /// Writes the edited excerpts of the file at the path into it.
    Save(String),
}

/// The stretches of a file of `count` lines to show around `lines`, each
/// counted from 1, as the first and last line of each, overlapping and
/// touching stretches joined.
pub fn excerpt_ranges(lines: &[usize], count: usize) -> Vec<(usize, usize)> {
    let mut sorted: Vec<usize> = lines.iter().copied().filter(|line| *line >= 1).collect();
    sorted.sort_unstable();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for line in sorted {
        let first = line.saturating_sub(EXCERPT_CONTEXT).max(1);
        let last = (line + EXCERPT_CONTEXT).min(count.max(1));
        match ranges.last_mut() {
            Some((_, end)) if first <= *end + 1 => *end = (*end).max(last),
            _ => ranges.push((first, last)),
        }
    }
    ranges
}

/// `text` with each stretch of `edits`, a first line counted from 1, what
/// the stretch held and what it holds now, put in; fails with the first
/// line of a stretch that no longer holds what it held.
///
/// # Errors
///
/// Fails with the first line of the first stretch whose text changed since it was read.
pub fn splice_excerpts(text: &str, edits: &[(usize, String, String)]) -> Result<String, usize> {
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
    let mut ordered: Vec<&(usize, String, String)> = edits.iter().collect();
    ordered.sort_by_key(|(first, _, _)| std::cmp::Reverse(*first));
    for (first, original, edited) in ordered {
        let start = first.saturating_sub(1);
        let span = original.split_inclusive('\n').count();
        let held: String = lines.iter().skip(start).take(span).cloned().collect();
        if held.trim_end_matches('\n') != original.trim_end_matches('\n') {
            return Err(*first);
        }
        let mut replacement: Vec<String> =
            edited.split_inclusive('\n').map(str::to_owned).collect();
        if let Some(last) = replacement.last_mut()
            && !last.ends_with('\n')
            && held.ends_with('\n')
        {
            last.push('\n');
        }
        let end = (start + span).min(lines.len());
        lines.splice(start..end, replacement);
    }
    Ok(lines.concat())
}

impl ProjectSearch {
    /// The hits grouped by file, files in the order they were first found.
    pub fn grouped(&self) -> Vec<(String, Vec<&SearchHit>)> {
        let mut groups: Vec<(String, Vec<&SearchHit>)> = Vec::new();
        for hit in &self.hits {
            match groups.iter_mut().find(|(path, _)| *path == hit.path) {
                Some((_, hits)) => hits.push(hit),
                None => groups.push((hit.path.clone(), vec![hit])),
            }
        }
        groups
    }

    /// How long a match of the query is in `text` from `column`, in characters:
    /// the query's own length for plain text, or the run that matches it ignoring case.
    pub fn match_length(&self, text: &str, column: usize) -> usize {
        if self.regex {
            return 0;
        }
        let wanted = self.query.chars().count();
        let found: String = text.chars().skip(column).take(wanted).collect();
        if found.to_lowercase() == self.query.to_lowercase() {
            wanted
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_hits_by_file_in_order() {
        let hit = |path: &str, line| SearchHit {
            path: path.into(),
            line,
            ..SearchHit::default()
        };
        let search = ProjectSearch {
            hits: vec![hit("b.rs", 1), hit("a.rs", 2), hit("b.rs", 9)],
            ..ProjectSearch::default()
        };
        let groups = search.grouped();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "b.rs");
        assert_eq!(groups[0].1.len(), 2);
    }

    /// Stretches around lines found are joined where they touch and kept inside the file.
    #[test]
    fn joins_excerpt_ranges() {
        assert_eq!(excerpt_ranges(&[1, 4, 20], 21), vec![(1, 6), (18, 21)]);
    }

    /// Edited stretches go back in place, and a stretch changed since it was read is refused.
    #[test]
    fn splices_excerpts() {
        let text = "a\nb\nc\nd\n";
        let edits = [(2, "b\nc\n".to_owned(), "B\nX\nC".to_owned())];
        assert_eq!(
            splice_excerpts(text, &edits),
            Ok("a\nB\nX\nC\nd\n".to_owned())
        );
        let stale = [(2, "q\n".to_owned(), "r".to_owned())];
        assert_eq!(splice_excerpts(text, &stale), Err(2));
    }

    #[test]
    fn measures_plain_matches_ignoring_case() {
        let search = ProjectSearch {
            query: "Link".into(),
            ..ProjectSearch::default()
        };
        assert_eq!(search.match_length("use pm_shell::link;", 14), 4);
        assert_eq!(search.match_length("use pm_shell::lunk;", 14), 0);
    }
}
