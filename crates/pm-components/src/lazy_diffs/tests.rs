//! Tests of asking for the diffs of the files near the view and of anchoring the diff's list to an opened file while the diffs of the files above it arrive.

use crate::focus::FocusId;

use crate::diff::header_item;
use crate::sample::Change;
use crate::state::{DIFF_FOLD_BASE, Outgoing, Page, WorkspaceState};

/// How many files the session under test has changed, none of whose diffs have been read.
const FILES: usize = 30;

/// The file opened in the tests that open one, far enough down that every file above it is still estimated.
const OPENED: usize = 5;

/// The path of the changed file at `index`.
fn path(index: usize) -> String {
    format!("src/file_{index}.rs")
}

/// A workspace reviewing its first session, which lists `FILES` changed files of two lines each without their diffs.
fn listed_workspace() -> WorkspaceState {
    let mut state = WorkspaceState::sample();
    state.page = Page::Session(0);
    state.sync_context();
    state.sessions[0].changes = (0..FILES)
        .map(|index| Change::listed(&path(index), 1, 1))
        .collect();
    state
}

/// The diffs of the files above the opened one as the server sends them: each adds sixty lines.
fn diffs_above() -> (Vec<String>, Vec<pm_git::FileDiff>) {
    let lines: String = (0..60).map(|line| format!("+line {line}\n")).collect();
    let paths: Vec<String> = (0..OPENED).map(path).collect();
    let files = paths
        .iter()
        .map(|path| pm_git::FileDiff {
            path: path.clone(),
            diff: format!("@@ -0,0 +1,60 @@\n{lines}"),
            words: Vec::new(),
        })
        .collect();
    (paths, files)
}

/// Where the header of the opened file is in the diff's list of `state`.
fn opened_item(state: &WorkspaceState) -> usize {
    header_item(state, &state.sessions[0], OPENED)
}

/// A file listed without its diff is a header and a line saying it is being read; a read one is a header, its hunk header and its rows; a folded one is its header alone.
#[test]
fn header_items_follow_what_the_files_above_show() {
    let mut state = listed_workspace();
    assert_eq!(opened_item(&state), OPENED * 2);
    let (asked, files) = diffs_above();
    state = state.with_diffs(0, &asked, files);
    assert_eq!(opened_item(&state), OPENED * 62);
    state = state.after(FocusId(DIFF_FOLD_BASE));
    assert_eq!(opened_item(&state), (OPENED - 1) * 62 + 1);
}

/// Opening a file anchors the list to it, the anchor holds while diffs above arrive and the files fold, and a scroll by hand lets it go.
#[test]
fn the_anchor_holds_until_the_diff_is_scrolled() {
    let mut state = listed_workspace();
    state.open_change(OPENED);
    assert_eq!(state.diff_anchor, Some((0, path(OPENED))));
    let (asked, files) = diffs_above();
    state = state.with_diffs(0, &asked, files);
    state = state.after(FocusId(DIFF_FOLD_BASE));
    assert_eq!(state.diff_anchor, Some((0, path(OPENED))));
    state.diff_list.scroll().jump_to(40.0);
    assert!(state.follow_scrolls());
    assert_eq!(state.diff_anchor, None);
}

/// Before the list has been laid out, the files at the top are asked for on their own, then those after them, each once while the answer is on its way.
#[test]
fn the_files_at_the_top_are_asked_first() {
    let mut state = listed_workspace();
    state.tab = crate::state::Tab::Diff;
    state.diffs_wanted();
    let asked: Vec<Vec<String>> = state
        .take_outbox()
        .into_iter()
        .filter_map(|outgoing| match outgoing {
            Outgoing::Diffs { paths, .. } => Some(paths),
            _ => None,
        })
        .collect();
    assert_eq!(asked.len(), 2);
    assert_eq!(asked[0], (0..12).map(path).collect::<Vec<_>>());
    assert_eq!(asked[1], (12..28).map(path).collect::<Vec<_>>());
    state.diffs_wanted();
    assert!(
        !state
            .take_outbox()
            .into_iter()
            .any(|outgoing| matches!(outgoing, Outgoing::Diffs { .. }))
    );
}
