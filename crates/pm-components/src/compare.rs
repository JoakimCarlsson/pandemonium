//! Comparing the file shown with the clipboard, with what was last saved of it, or with another file of the project, and opening the difference as a diff.

use crate::excerpts::{ExcerptFile, Source};
use crate::listing::Items;
use crate::open_files::Contents;
use crate::sample::{DiffLine, Kind};
use crate::state::{Outgoing, WorkspaceState};

/// How many unchanged lines are kept around each change.
const CONTEXT: usize = 3;

/// What `text` is compared as: its line endings made plain.
fn plain(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// The key a line is compared by: the line itself, or with its white space made one space when `ignore_whitespace`.
fn key_of(line: &str, ignore_whitespace: bool) -> String {
    if ignore_whitespace {
        line.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        line.to_owned()
    }
}

/// The name of the file at `path`.
fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The lines of the diff that turns `old` into `new`, each hunk under its header; lines that differ only in white space count as the same when `ignore_whitespace`, and are shown as `new` has them.
pub(crate) fn compare_lines(old: &str, new: &str, ignore_whitespace: bool) -> Vec<DiffLine> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let old_keys: Vec<String> = old_lines
        .iter()
        .map(|line| key_of(line, ignore_whitespace))
        .collect();
    let new_keys: Vec<String> = new_lines
        .iter()
        .map(|line| key_of(line, ignore_whitespace))
        .collect();
    let operations = similar::capture_diff_slices(similar::Algorithm::Myers, &old_keys, &new_keys);
    let line = |kind, text: &str| DiffLine {
        kind,
        text: text.to_owned(),
    };
    let start = |range: &std::ops::Range<usize>| {
        if range.is_empty() {
            range.start
        } else {
            range.start + 1
        }
    };
    let mut lines = Vec::new();
    for group in similar::group_diff_ops(operations, CONTEXT) {
        let (Some(first), Some(last)) = (group.first(), group.last()) else {
            continue;
        };
        let old_range = first.old_range().start..last.old_range().end;
        let new_range = first.new_range().start..last.new_range().end;
        lines.push(line(
            Kind::Hunk,
            &format!(
                "@@ -{},{} +{},{} @@",
                start(&old_range),
                old_range.len(),
                start(&new_range),
                new_range.len()
            ),
        ));
        for operation in &group {
            let (removed, added) = (operation.old_range(), operation.new_range());
            match operation.tag() {
                similar::DiffTag::Equal => lines.extend(
                    new_lines[added]
                        .iter()
                        .map(|text| line(Kind::Context, text)),
                ),
                similar::DiffTag::Delete => lines.extend(
                    old_lines[removed]
                        .iter()
                        .map(|text| line(Kind::Remove, text)),
                ),
                similar::DiffTag::Insert => {
                    lines.extend(new_lines[added].iter().map(|text| line(Kind::Add, text)));
                }
                similar::DiffTag::Replace => {
                    lines.extend(
                        old_lines[removed]
                            .iter()
                            .map(|text| line(Kind::Remove, text)),
                    );
                    lines.extend(new_lines[added].iter().map(|text| line(Kind::Add, text)));
                }
            }
        }
    }
    lines
}

impl WorkspaceState {
    /// The session, the path and the text of the file shown, with its lines ended plainly, when a file with text is shown.
    fn compare_subject(&self) -> Option<(usize, String, String)> {
        let session = self.review_index()?;
        let viewer = self.shown_viewer()?;
        match &viewer.contents {
            Contents::Text(document) => {
                Some((session, viewer.path.clone(), plain(document.field.text())))
            }
            _ => None,
        }
    }

    /// Opens the difference between `left`, called `names.0`, and `right`, called `names.1`, as a diff of the file at `path` of the session at `session`, the lines coloured the way the file's language colours them.
    fn compare_opened(
        mut self,
        session: usize,
        path: &str,
        names: (&str, &str),
        left: &str,
        right: &str,
    ) -> Self {
        let mut diff = compare_lines(left, right, self.editor.compare_ignore_whitespace);
        let tally = (
            diff.iter().filter(|line| line.kind == Kind::Add).count(),
            diff.iter().filter(|line| line.kind == Kind::Remove).count(),
        );
        if diff.is_empty() {
            diff.push(DiffLine {
                kind: Kind::Context,
                text: "No differences between the two.".to_owned(),
            });
        }
        let file = ExcerptFile {
            path: path.to_owned(),
            blocks: Vec::new(),
            closed: false,
            mark: None,
            tally,
            editable: false,
            diff,
        };
        let label = format!("{} ↔ {}", names.0, names.1);
        self.excerpts_shown(session, Source::Revision { label }, vec![file]);
        self
    }

    /// Compares the file shown with `clipboard`, the text on the clipboard, and opens the difference: what the clipboard holds on the left, the file on the right.
    pub fn compare_with_clipboard(self, clipboard: String) -> Self {
        let Some((session, path, text)) = self.compare_subject() else {
            return self;
        };
        self.compare_opened(
            session,
            &path,
            ("Clipboard", name_of(&path)),
            &plain(&clipboard),
            &text,
        )
    }

    /// Asks the server for the file shown as the folder holds it, to compare it with the file as it is being edited.
    pub fn compare_with_saved_requested(mut self) -> Self {
        let Some((session, path, _)) = self.compare_subject() else {
            return self;
        };
        self.outbox.push(Outgoing::ReadToCompare {
            session,
            active: path.clone(),
            path,
            saved: true,
        });
        self
    }

    /// Opens the quick input at `at` over the files of the project, to pick the one to compare the file shown with.
    pub fn compare_with_file_requested(self, at: (f32, f32)) -> Self {
        let Some((_, path, _)) = self.compare_subject() else {
            return self;
        };
        let mut state = self.file_finder_requested(at);
        if let Some(Items::Files { compare, .. }) =
            state.listing.as_mut().map(|listing| &mut listing.items)
        {
            *compare = Some(path);
        }
        state
    }

    /// Asks the server for the file at `path`, chosen to be compared with the file shown, `active`.
    pub(crate) fn compare_file_chosen(mut self, active: String, path: String) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        self.listing = None;
        self.outbox.push(Outgoing::ReadToCompare {
            session,
            active,
            path,
            saved: false,
        });
        self
    }

    /// The state after the server answered with `text`, what the folder holds of the file at `path` of the session at `session`, to be compared with the file `active` as it is being edited: the file as saved on the left and as edited on the right when `saved`, and otherwise `active` on the left and `path` on the right.
    pub fn with_compared(
        self,
        session: usize,
        active: &str,
        path: &str,
        saved: bool,
        text: &str,
    ) -> Self {
        let edited = self
            .viewer_at(active)
            .and_then(|index| self.git.viewers.get(index))
            .and_then(|viewer| match &viewer.contents {
                Contents::Text(document) => Some(plain(document.field.text())),
                _ => None,
            });
        let Some(edited) = edited else {
            return self;
        };
        let text = plain(text);
        if saved {
            self.compare_opened(
                session,
                active,
                ("Saved", &format!("{} (edited)", name_of(active))),
                &text,
                &edited,
            )
        } else {
            self.compare_opened(
                session,
                active,
                (name_of(active), name_of(path)),
                &edited,
                &text,
            )
        }
    }
}
