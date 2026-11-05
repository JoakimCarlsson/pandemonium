//! Tests of keeping an opened file at the top of the diff while the diffs of the files above it arrive.

use pm_input::FocusId;

use crate::diff::offset_of;
use crate::sample::Change;
use crate::state::{DIFF_FOLD_BASE, Page, WorkspaceState};

/// How many files the session under test has changed, none of whose diffs have been read.
const FILES: usize = 6;

/// The file opened in each test, far enough down that every file above it is still estimated.
const OPENED: usize = FILES - 1;

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

/// The diffs of the files above the opened one as the server sends them: each adds far more lines than the listing estimated.
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

/// Where the opened file starts in the diff of `state`.
fn opened_offset(state: &WorkspaceState) -> f32 {
    offset_of(state, &state.sessions[0], OPENED, state.diff_width)
}

/// Opening a file whose predecessors are estimated, then reading their longer diffs, leaves the diff scrolled to the opened file's header.
#[test]
fn opened_file_stays_at_top_when_diffs_above_arrive() {
    let mut state = listed_workspace();
    state.open_change(OPENED);
    let estimated = state.diff_scroll;
    assert_eq!(estimated, opened_offset(&state));
    let (asked, files) = diffs_above();
    let state = state.with_diffs(0, &asked, files);
    assert!(state.diff_scroll > estimated);
    assert_eq!(state.diff_scroll, opened_offset(&state));
}

/// Folding and unfolding a file above the opened one keeps the opened file's header at the top.
#[test]
fn folding_above_keeps_opened_file_at_top() {
    let mut state = listed_workspace();
    state.open_change(OPENED);
    let (asked, files) = diffs_above();
    state = state.with_diffs(0, &asked, files);
    state = state.after(FocusId(DIFF_FOLD_BASE));
    assert!(state.closed_diffs.contains(&path(0)));
    assert_eq!(state.diff_scroll, opened_offset(&state));
    state = state.after(FocusId(DIFF_FOLD_BASE));
    assert!(!state.closed_diffs.contains(&path(0)));
    assert_eq!(state.diff_scroll, opened_offset(&state));
}

/// Once the diff is scrolled by hand, diffs that arrive afterwards leave it where it was.
#[test]
fn diffs_after_a_scroll_leave_the_view_alone() {
    let mut state = listed_workspace();
    state.open_change(OPENED);
    state = state.scrolled_diff(-40.0, 100_000.0);
    let scrolled = state.diff_scroll;
    let (asked, files) = diffs_above();
    let state = state.with_diffs(0, &asked, files);
    assert_eq!(state.diff_scroll, scrolled);
    assert_ne!(state.diff_scroll, opened_offset(&state));
}

/// Drawing the diff narrower than it was taken to be wraps the long lines above the opened file onto more rows, and the diff follows the opened file's header down.
#[test]
fn narrower_diff_keeps_opened_file_at_top() {
    let mut state = listed_workspace();
    let long: String = (0..20)
        .map(|line| format!("+{}\n", "word ".repeat(40 + line)))
        .collect();
    let asked: Vec<String> = (0..OPENED).map(path).collect();
    let files = asked
        .iter()
        .map(|path| pm_git::FileDiff {
            path: path.clone(),
            diff: format!("@@ -0,0 +1,20 @@\n{long}"),
            words: Vec::new(),
        })
        .collect();
    state = state.with_diffs(0, &asked, files);
    state.open_change(OPENED);
    let (wide, narrow) = (state.diff_scroll, state.diff_width / 2.0);
    let state = state.measured_diff(narrow);
    assert!(state.diff_scroll > wide);
    assert_eq!(state.diff_scroll, opened_offset(&state));
}
