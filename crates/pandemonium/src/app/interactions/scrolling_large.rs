//! Scrolling the large views through the rig as the wheel does, frame by frame, checking
//! every frame: the view keeps text drawn from the top of its area to the bottom with no
//! blank stretch, the text drawn moves as it scrolls, the far end is reached and shown, and
//! scrolling back to the start shows what was shown at first.

use pm_components::areas::TreeEvent;
use pm_components::target::Target;
use pm_pages::{Screen, WorkspaceState};

use crate::app::headless::{Rig, SIZE};

/// The tallest stretch of a view, in logical pixels, a scroll may leave without text: more
/// than any gap between the lines, rows and paragraphs of the views.
const LARGEST_GAP: f32 = 240.0;

/// How far from the top and the bottom of the window the check for blank stretches of a
/// view alone in it starts, leaving out the title bar, the strips above the views and the
/// prompt below them.
const EDGE: f32 = 160.0;

/// How far inside the edges of a view in a pane the check for blank stretches starts.
const INSET: f32 = 40.0;

/// Where a view is checked: its stretch of the window's width, left and right, and the
/// stretch of its height checked for blank space, top and bottom.
type Area = (f32, f32, f32, f32);

/// How far, in logical pixels, text may sit from where it was drawn before the view was
/// scrolled away and back. A list places its items at fractions of a pixel from where its
/// estimate of the items not yet measured puts its end, so a run can be rounded to the pixel
/// on the other side once more of them are measured.
const DRIFT: f32 = 1.0;

/// The text the last frame drew between `left` and `right` across the window, as where each
/// run starts down the window and what it says, top to bottom.
fn texts(rig: &Rig, (left, right, _, _): Area) -> Vec<(f32, String)> {
    let mut found: Vec<(f32, String)> = rig
        .commands()
        .iter()
        .filter_map(|command| match command {
            rustle::DrawCommand::Text { x, y, content, .. } => Some((*x, *y, content.clone())),
            rustle::DrawCommand::RichText { x, y, spans, .. } => Some((
                *x,
                *y,
                spans.iter().map(|span| span.text.as_str()).collect(),
            )),
            _ => None,
        })
        .filter(|(x, y, text)| {
            (left..right).contains(x) && (0.0..SIZE.1).contains(y) && !text.trim().is_empty()
        })
        .map(|(_, y, text)| (y, text))
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    found
}

/// Panics, naming `view` and `frame`, when the last frame left a stretch of the view taller
/// than [`LARGEST_GAP`] without text in `area`; below the last text only when not
/// `at_end`, where a view may end above the bottom of its area.
fn assert_covered(rig: &Rig, area: Area, view: &str, frame: usize, at_end: bool) {
    let (_, _, top, bottom) = area;
    let found = texts(rig, area);
    let mut tops: Vec<f32> = found.iter().map(|(y, _)| *y).collect();
    tops.insert(0, top);
    if !at_end {
        tops.push(bottom);
    }
    let gap = tops
        .windows(2)
        .filter(|pair| pair[1] >= top && pair[0] <= bottom)
        .map(|pair| pair[1] - pair[0])
        .fold(0.0, f32::max);
    assert!(
        gap <= LARGEST_GAP,
        "{view}: frame {frame} left {gap} pixels blank: {found:?}"
    );
}

/// Panics, naming `view`, unless `now` shows the runs `before` showed, each within
/// [`DRIFT`] of where it was. Labels cut short with an ellipsis are left out: where they are
/// cut depends on the fonts the shared font system holds, which other tests running at the
/// same time load into it.
fn assert_same(view: &str, before: &[(f32, String)], now: &[(f32, String)]) {
    let whole = |runs: &[(f32, String)]| -> Vec<(f32, String)> {
        runs.iter()
            .filter(|(_, text)| !text.ends_with('…'))
            .cloned()
            .collect()
    };
    let (before, now) = (whole(before), whole(now));
    let alike = before.len() == now.len()
        && before.iter().all(|(y, text)| {
            now.iter()
                .any(|(other, words)| words == text && (other - y).abs() <= DRIFT)
        });
    assert!(
        alike,
        "{view}: back at the start it shows something else:\n{before:?}\n{now:?}"
    );
}

/// Scrolls `view`, shown by `state`, from its start with the pointer over it: `frames`
/// frames of `step` pixels toward its far end, checking each, then to its far end and back
/// to its start. `area` says where the view is checked.
fn scroll_and_check(
    view: &str,
    state: WorkspaceState,
    area: impl Fn(&Rig) -> Area,
    step: f32,
    frames: usize,
) {
    let mut rig = Rig::new(Screen::Workspace(state), SIZE);
    let columns = area(&rig);
    rig.move_to(((columns.0 + columns.1) / 2.0, (columns.2 + columns.3) / 2.0));
    rig.wait(200);
    let start = texts(&rig, columns);
    assert_covered(&rig, columns, view, 0, false);
    let mut moved = false;
    for frame in 1..=frames {
        let before = texts(&rig, columns);
        rig.wheel(step);
        let now = texts(&rig, columns);
        assert_covered(&rig, columns, view, frame, now == before);
        moved |= now != start;
    }
    assert!(
        moved,
        "{view}: scrolling {step} pixels a frame moved nothing"
    );
    let far = 50_000.0_f32.copysign(step);
    for frame in 1..=40 {
        rig.wheel(far);
        assert_covered(&rig, columns, view, frames + frame, true);
    }
    let end = texts(&rig, columns);
    rig.wheel(far);
    assert_eq!(texts(&rig, columns), end, "{view}: the end was not reached");
    for frame in 1..=40 {
        rig.wheel(-far);
        assert_covered(&rig, columns, view, frames + 40 + frame, false);
    }
    rig.wait(200);
    assert_same(view, &start, &texts(&rig, columns));
}

/// Where the view whose bounds `bounds` reads is drawn, inset by [`INSET`] top and bottom.
fn pane(rig: &Rig, bounds: impl FnOnce(&Screen) -> Option<rustle::Rect>) -> Area {
    let area = rig.area(bounds).expect("the view is drawn");
    (
        area.origin.x,
        area.origin.x + area.size.width,
        area.origin.y + INSET,
        area.origin.y + area.size.height - INSET,
    )
}

/// Where the conversation is drawn.
fn conversation(rig: &Rig) -> Area {
    pane(rig, Screen::conversation_area)
}

/// Where the view of the file shown is drawn.
fn code(rig: &Rig) -> Area {
    pane(rig, Screen::code_area)
}

/// Where the diff is drawn.
fn diff(rig: &Rig) -> Area {
    pane(rig, |screen| {
        let Screen::Workspace(state) = screen;
        state.diff_bounds.get()
    })
}

/// Where the rows of the file tree are drawn, down the middle of the window.
fn tree(rig: &Rig) -> Area {
    let row = rig
        .region(Target::Tree(TreeEvent::TreeFile(0)))
        .expect("the tree shows its first row");
    (
        row.origin.x,
        row.origin.x + row.size.width,
        EDGE,
        SIZE.1 - EDGE,
    )
}

/// The large diff scrolls cleanly in small steps, in flicks and to both ends.
#[test]
fn a_large_diff_scrolls_cleanly() {
    let large = crate::snapshot::large_diff;
    scroll_and_check("diff, trackpad", large(), diff, 8.0, 300);
    scroll_and_check("diff, flick", large(), diff, 400.0, 150);
}

/// A long conversation of markdown replies scrolls cleanly back from its end in small
/// steps, in flicks and to both ends.
#[test]
fn a_long_chat_scrolls_cleanly() {
    let chat = || crate::snapshot::input_stress(false, true);
    scroll_and_check("chat, trackpad", chat(), conversation, -8.0, 300);
    scroll_and_check("chat, flick", chat(), conversation, -400.0, 150);
}

/// A long session of turns with tool steps scrolls cleanly back from its end in small
/// steps, in flicks and to both ends.
#[test]
fn a_long_session_scrolls_cleanly() {
    let session = crate::snapshot::long_session;
    scroll_and_check("session, trackpad", session(), conversation, -8.0, 300);
    scroll_and_check("session, flick", session(), conversation, -400.0, 150);
}

/// The chat beside the file and the diff in the heavy workspace scrolls cleanly back from
/// its end in small steps, in flicks and to both ends.
#[test]
fn the_heavy_workspace_scrolls_cleanly() {
    let heavy = crate::snapshot::heavy_workspace;
    scroll_and_check("heavy, trackpad", heavy(), conversation, -8.0, 300);
    scroll_and_check("heavy, flick", heavy(), conversation, -400.0, 150);
}

/// A large file scrolls cleanly in small steps, in flicks and to both ends.
#[test]
fn a_large_file_scrolls_cleanly() {
    let file = crate::snapshot::large_code;
    scroll_and_check("file, trackpad", file(), code, 8.0, 300);
    scroll_and_check("file, flick", file(), code, 400.0, 150);
}

/// A large tree scrolls cleanly in small steps, in flicks and to both ends.
#[test]
fn a_large_tree_scrolls_cleanly() {
    let files = crate::snapshot::large_tree;
    scroll_and_check("tree, trackpad", files(), tree, 8.0, 300);
    scroll_and_check("tree, flick", files(), tree, 400.0, 150);
}
