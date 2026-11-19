//! The diff: its lazy list keeps an opened file at its top while the diffs of
//! the files above it arrive, and keeps what the reader scrolled to in place.

use pm_components::areas::DiffEvent;

use pm_components::events::Event;
use pm_components::state::WorkspaceState;
use pm_components::target::Target;
use pm_pages::Screen;

use crate::app::headless::{Rig, SIZE};

/// A window short enough that the files below the opened one fill the diff's list past its height, so the list can bring the opened file to its top.
const SHORT: (f32, f32) = (SIZE.0, 440.0);
use crate::snapshot::Shot;

/// The file the tests open, below files whose diffs are not read yet.
const OPENED: usize = 3;

/// The diff listed without the diffs of every file but the first, opened at [`OPENED`].
fn opened() -> Rig {
    let Screen::Workspace(state) = Shot::DiffListed.screen();
    Rig::new(
        Screen::Workspace(state.handle(Event::Diff(DiffEvent::ChangedFile(OPENED)))),
        SHORT,
    )
}

/// Where the diff's list is drawn, in the coordinates of the main window's frame that the controls' regions and the pointer are given in: its bounds are in the window's own, past the band that holds its shadow.
fn list_bounds(rig: &Rig) -> Option<rustle::Rect> {
    let band = pm_components::shadow_margin();
    let list = rig.state(|state| state.diff_bounds.get())?;
    Some(rustle::Rect::new(
        rustle::Offset::new(list.origin.x - band, list.origin.y - band),
        list.size,
    ))
}

/// How far below the top of the diff's list the fold button of the file at `index` is drawn, if it is.
fn below_top(rig: &Rig, index: usize) -> Option<f32> {
    let list = list_bounds(rig)?;
    let fold = rig.region(Target::Diff(DiffEvent::DiffFold(index)))?;
    Some(fold.origin.y - list.origin.y)
}

/// Sends the diffs of the files above [`OPENED`], each of sixty added lines, far longer than the line saying it is being read.
fn send_diffs_above(rig: &mut Rig) {
    let lines: String = (0..60).map(|line| format!("+line {line}\n")).collect();
    rig.app.borrow().model.update(|screen| {
        let Screen::Workspace(state) = std::mem::take(screen);
        let paths: Vec<String> = state.sessions[0].changes[1..OPENED]
            .iter()
            .map(|change| change.path.clone())
            .collect();
        let files = paths
            .iter()
            .map(|path| pm_git::FileDiff {
                path: path.clone(),
                diff: format!("@@ -0,0 +1,60 @@\n{lines}"),
                words: Vec::new(),
            })
            .collect();
        *screen = Screen::Workspace(WorkspaceState::with_diffs(state, 0, &paths, files));
    });
    rig.settle();
}

/// Opening a file brings its header to the top of the list, and it stays there as the files above it are read and grow.
#[test]
fn an_opened_file_stays_at_the_top_as_diffs_above_arrive() {
    let mut rig = opened();
    let at = below_top(&rig, OPENED).expect("the opened file is drawn");
    assert!((0.0..40.0).contains(&at), "{at}");
    assert_eq!(below_top(&rig, 0), None);
    send_diffs_above(&mut rig);
    let after = below_top(&rig, OPENED).expect("the opened file is still drawn");
    assert!((after - at).abs() <= 1.0, "{at} then {after}");
}

/// Once the reader scrolled, the diffs that arrive above leave what is in view where it was.
#[test]
fn diffs_above_leave_a_scrolled_view_in_place() {
    let mut rig = opened();
    let list = list_bounds(&rig).expect("the list is drawn");
    rig.move_to((
        list.origin.x + list.size.width / 2.0,
        list.origin.y + list.size.height / 2.0,
    ));
    rig.wheel(30.0);
    let at = below_top(&rig, OPENED).expect("the opened file is drawn");
    send_diffs_above(&mut rig);
    let after = below_top(&rig, OPENED).expect("the opened file is still drawn");
    assert!((after - at).abs() <= 1.0, "{at} then {after}");
}
