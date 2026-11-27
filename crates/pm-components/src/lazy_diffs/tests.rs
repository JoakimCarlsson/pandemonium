//! Tests of asking for the diffs of the files near the view and of anchoring the diff's list to an opened file while the diffs of the files above it arrive.

use crate::areas::DiffEvent;

use crate::events::Event;

use crate::diff::header_item;
use crate::sample::Change;
use crate::state::{Outgoing, Page, WorkspaceState};

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
    state = state.handle(Event::Diff(DiffEvent::DiffFold(0)));
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
    state = state.handle(Event::Diff(DiffEvent::DiffFold(0)));
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

/// A workspace reviewing `FILES` files whose diffs are read, of lines that wrap to one to
/// four rows, so the rows of the diff's list differ in height.
fn read_workspace() -> WorkspaceState {
    let state = listed_workspace();
    let paths: Vec<String> = (0..FILES).map(path).collect();
    let files = paths
        .iter()
        .enumerate()
        .map(|(file, path)| {
            let lines: String = (0..40)
                .map(|line| {
                    let words = "word ".repeat(((file * 7 + line * 13) % 4) * 40 + 2);
                    format!("+line {line} {words}\n")
                })
                .collect();
            pm_git::FileDiff {
                path: path.clone(),
                diff: format!("@@ -0,0 +1,40 @@\n{lines}"),
                words: Vec::new(),
            }
        })
        .collect();
    state.with_diffs(0, &paths, files)
}

/// The offset in view of each item of the diff's list as the latest layout placed them.
fn placed(state: &WorkspaceState) -> std::collections::BTreeMap<usize, f32> {
    state
        .diff_list
        .layout_info()
        .visible_items_info
        .iter()
        .map(|item| (item.index, item.offset))
        .collect()
}

/// Scrolls the diff by `frames` wheel steps of `step`, each frame laid out `rounds` times at
/// most, from its end back up. Returns how many frames drew an item somewhere other than
/// where a settled layout puts it, and how many moved an item in view in the frame before
/// by other than the wheel's step.
fn scroll_diff(rounds: usize, step: f32, frames: usize) -> (usize, usize) {
    let state = std::rc::Rc::new(read_workspace());
    let shown = std::rc::Rc::clone(&state);
    let mut ui = rustle::Embedded::without_renderer(
        move || crate::diff::diff(&shown, &shown.sessions[0], 900.0),
        || {},
    );
    ui.set_pixel_rounding(rustle::PixelRounding::Logical);
    ui.resize((900, 700), 1.0);
    ui.update();
    state.diff_list.scroll_to_end();
    for _ in 0..4 {
        ui.update();
    }
    ui.pointer_moved(rustle::Offset::new(450.0, 400.0));
    ui.update();
    let start = state.diff_list.scroll().offset();
    let (mut stale, mut uneven) = (0, 0);
    let mut before = placed(&state);
    for _ in 0..frames {
        ui.set_settle_rounds(rounds);
        ui.wheel(rustle::Offset::new(0.0, step));
        ui.update();
        let (drawn, offset) = (placed(&state), state.diff_list.scroll().offset());
        ui.set_settle_rounds(8);
        ui.update();
        let settled = placed(&state);
        let moved = drawn.iter().any(|(index, top)| {
            settled
                .get(index)
                .is_some_and(|other| (other - top).abs() > 0.25)
        });
        if moved || (state.diff_list.scroll().offset() - offset).abs() > 0.25 {
            stale += 1;
        }
        if drawn.iter().any(|(index, top)| {
            before
                .get(index)
                .is_some_and(|earlier| ((top - earlier).abs() - step.abs()).abs() > 0.25)
        }) {
            uneven += 1;
        }
        before = settled;
    }
    assert!(
        (start - state.diff_list.scroll().offset()).abs() > step.abs() * 2.0,
        "the diff did not scroll"
    );
    (stale, uneven)
}

/// Scrolling the diff back up through rows not measured yet, by the same step each frame,
/// draws every frame settled and moves what is in view by exactly that step: laid out once
/// per frame, a row learning its height shows the rows out of place for a frame, which the
/// frame after puts right, the flick seen while scrolling.
#[test]
fn scrolling_the_diff_draws_no_frame_out_of_place() {
    assert!(scroll_diff(1, 9.0, 150).0 > 0);
    assert_eq!(scroll_diff(8, 9.0, 150), (0, 0));
}

/// The diff of file `file` with `lines` added lines, each wrapping to one to four rows.
fn wrapped_diff(file: usize, lines: usize) -> pm_git::FileDiff {
    let body: String = (0..lines)
        .map(|line| {
            let words = "word ".repeat(((file * 7 + line * 13) % 4) * 40 + 2);
            format!("+line {line} {words}\n")
        })
        .collect();
    pm_git::FileDiff {
        path: path(file),
        diff: format!("@@ -0,0 +1,{lines} @@\n{body}"),
        words: Vec::new(),
    }
}

/// A workspace reviewing `FILES` files of forty wrapped lines each, the diffs of those from
/// `listed` on not read yet.
fn partly_read_workspace(listed: usize) -> WorkspaceState {
    let state = listed_workspace();
    let read: Vec<usize> = (0..FILES).filter(|file| *file < listed).collect();
    let paths: Vec<String> = read.iter().copied().map(path).collect();
    let files = read.iter().map(|file| wrapped_diff(*file, 40)).collect();
    state.with_diffs(0, &paths, files)
}

/// The state in `cell` after `change` made it anew, as the app replaces its state.
fn replace_state(
    cell: &std::cell::RefCell<WorkspaceState>,
    change: impl FnOnce(WorkspaceState) -> WorkspaceState,
) {
    let old = std::mem::replace(&mut *cell.borrow_mut(), WorkspaceState::sample());
    *cell.borrow_mut() = change(old);
}

/// Where the latest layout placed each item in view of the diff's list of `state`, by the
/// name [`crate::diff::item_names`] gives it.
fn placed_by_name(state: &WorkspaceState) -> std::collections::HashMap<String, f32> {
    let names = crate::diff::item_names(state, &state.sessions[0]);
    state
        .diff_list
        .layout_info()
        .visible_items_info
        .iter()
        .filter_map(|item| {
            names
                .get(item.index)
                .map(|name| (name.clone(), item.offset))
        })
        .collect()
}

/// Scrolls the diff of `state` up by `step` a frame for `frames` frames from the header of
/// file `from`, each frame laid out at most four times as a window lays it out, calling `between` with the
/// state and the frame's number before the frame. Returns the frames in which something in
/// view in the frame before and in this one moved by other than the wheel's step, each with
/// what moved.
fn scroll_up_through(
    state: WorkspaceState,
    from: usize,
    step: f32,
    frames: usize,
    mut between: impl FnMut(&std::cell::RefCell<WorkspaceState>, usize),
) -> Vec<(usize, Vec<(String, f32)>)> {
    let cell = std::rc::Rc::new(std::cell::RefCell::new(state));
    let shown = std::rc::Rc::clone(&cell);
    let mut ui = rustle::Embedded::without_renderer(
        move || {
            let state = shown.borrow();
            crate::diff::diff(&state, &state.sessions[0], 900.0);
        },
        || {},
    );
    ui.set_pixel_rounding(rustle::PixelRounding::Logical);
    ui.resize((900, 700), 1.0);
    ui.set_settle_rounds(4);
    ui.update();
    {
        let state = cell.borrow();
        let item = header_item(&state, &state.sessions[0], from);
        state
            .diff_list
            .scroll_to_item(item, rustle::ItemAlign::Start);
    }
    for _ in 0..6 {
        ui.update();
    }
    ui.pointer_moved(rustle::Offset::new(450.0, 400.0));
    ui.update();
    let mut before = placed_by_name(&cell.borrow());
    let (mut moved, mut followed) = (Vec::new(), 0);
    for frame in 0..frames {
        between(&cell, frame);
        ui.invalidate();
        ui.wheel(rustle::Offset::new(0.0, step));
        ui.update();
        let now = placed_by_name(&cell.borrow());
        followed += usize::from(now.keys().any(|name| before.contains_key(name)));
        let wrong: Vec<(String, f32)> = now
            .iter()
            .filter_map(|(name, top)| {
                let earlier = before.get(name)?;
                let by = top - earlier;
                ((by - step).abs() > 0.25).then(|| (name.clone(), by))
            })
            .collect();
        if !wrong.is_empty() {
            moved.push((frame, wrong));
        }
        before = now;
    }
    assert_eq!(followed, frames, "an item in view left it within a frame");
    moved
}

/// Scrolling up with nothing arriving moves everything in view by the wheel's step.
#[test]
fn scrolling_up_the_diff_moves_it_by_the_step() {
    let moved = scroll_up_through(partly_read_workspace(FILES), 12, 9.0, 120, |_, _| {});
    assert!(moved.is_empty(), "{moved:?}");
}

/// A refresh sending the same diffs again while the diff is scrolled up moves nothing.
#[test]
fn the_same_diffs_arriving_while_scrolling_up_move_nothing() {
    let moved = scroll_up_through(partly_read_workspace(FILES), 12, 9.0, 120, |cell, frame| {
        if frame % 20 == 10 {
            replace_state(cell, |state| {
                let paths: Vec<String> = (0..FILES).map(path).collect();
                let files = (0..FILES).map(|file| wrapped_diff(file, 40)).collect();
                state.with_diffs(0, &paths, files)
            });
        }
    });
    assert!(moved.is_empty(), "{moved:?}");
}

/// Diffs of the files above the view changing, or a file added above it, while the diff
/// is scrolled up move nothing in view.
#[test]
fn files_changing_above_the_view_while_scrolling_up_move_nothing() {
    let moved = scroll_up_through(partly_read_workspace(FILES), 12, 9.0, 120, |cell, frame| {
        if frame == 30 {
            replace_state(cell, |state| {
                let paths: Vec<String> = (0..4).map(path).collect();
                let files = (0..4).map(|file| wrapped_diff(file, 55)).collect();
                state.with_diffs(0, &paths, files)
            });
        }
        if frame == 60 {
            cell.borrow_mut().sessions[0]
                .changes
                .insert(0, Change::new("src/added.rs", "@@ -0,0 +1,2 @@\n+a\n+b\n"));
        }
    });
    assert!(moved.is_empty(), "{moved:?}");
}

/// Diffs read as the diff is scrolled up to the files not read yet, the way the app asks
/// for them and they arrive a frame later, move nothing in view.
#[test]
fn diffs_read_while_scrolling_up_to_them_move_nothing() {
    let asked: std::cell::RefCell<Vec<Vec<String>>> = std::cell::RefCell::default();
    let moved = scroll_up_through(partly_read_workspace(0), 10, 9.0, 400, |cell, _| {
        let answer = std::mem::take(&mut *asked.borrow_mut());
        for paths in answer {
            replace_state(cell, |state| {
                let files = paths
                    .iter()
                    .map(|name| {
                        let file = (0..FILES).position(|at| path(at) == *name).unwrap_or(0);
                        wrapped_diff(file, 40)
                    })
                    .collect();
                state.with_diffs(0, &paths, files)
            });
        }
        let mut state = cell.borrow_mut();
        state.tab = crate::state::Tab::Diff;
        state.diffs_wanted();
        for outgoing in state.take_outbox() {
            if let Outgoing::Diffs { paths, .. } = outgoing {
                asked.borrow_mut().push(paths);
            }
        }
    });
    assert!(moved.is_empty(), "{moved:?}");
}

/// Measures the frames of scrolling the diff up a trackpad's step at a time: those that
/// compose items newly near the view, as the view crosses what the list composed, against
/// the rest, which only lay out again.
#[test]
#[ignore = "manual CPU benchmark: cargo test -p pm-components crossing_frames -- --ignored --nocapture"]
fn benchmark_crossing_frames_of_the_diff() {
    let state = std::rc::Rc::new(partly_read_workspace(FILES));
    let shown = std::rc::Rc::clone(&state);
    let mut ui = rustle::Embedded::without_renderer(
        move || crate::diff::diff(&shown, &shown.sessions[0], 900.0),
        || {},
    );
    ui.resize((900, 700), 1.0);
    ui.set_settle_rounds(4);
    ui.update();
    state.diff_list.scroll_to_end();
    for _ in 0..6 {
        ui.update();
    }
    ui.pointer_moved(rustle::Offset::new(450.0, 400.0));
    ui.update();
    let mut measured = Vec::new();
    for _ in 0..1500 {
        ui.wheel(rustle::Offset::new(0.0, 9.0));
        let started = std::time::Instant::now();
        ui.update();
        let total = started.elapsed().as_secs_f64() * 1000.0;
        measured.push((ui.update_timing().compose.as_secs_f64() * 1000.0, total));
    }
    let mut composes: Vec<f64> = measured.iter().map(|frame| frame.0).collect();
    composes.sort_by(f64::total_cmp);
    let usual = composes[composes.len() / 2];
    let of = |crossing: bool| -> Vec<f64> {
        measured
            .iter()
            .filter(|frame| (frame.0 > usual * 4.0) == crossing)
            .map(|frame| frame.1)
            .collect()
    };
    let (crossing, plain) = (of(true), of(false));
    for (name, mut frames) in [("crossing", crossing), ("plain", plain)] {
        if frames.is_empty() {
            continue;
        }
        frames.sort_by(f64::total_cmp);
        let at = |share: f64| frames[((frames.len() - 1) as f64 * share) as usize];
        eprintln!(
            "{name:<9} {:>5} frames  p50 {:.3}  p95 {:.3}  max {:.3} ms",
            frames.len(),
            at(0.5),
            at(0.95),
            at(1.0)
        );
    }
}
