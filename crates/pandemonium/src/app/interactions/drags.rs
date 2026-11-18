//! Dragging: the handles between panels, projects in the sidebar, tabs
//! between panes, rows of the project's files into folders, and the
//! minimap of the file shown.

use pm_components::kit::FocusId;
use pm_components::minimap::MINIMAP;
use pm_components::sash::{REVIEW_SASH, SIDEBAR_SASH};
use pm_components::state::PROJECT_BASE;
use pm_pages::Screen;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// The first region drawn whose name `pick` accepts, from the end of the
/// list when `last`.
fn region_where(rig: &Rig, last: bool, pick: impl Fn(&Screen, FocusId) -> bool) -> FocusId {
    let report = rig.report();
    let mut ids = report.regions.iter().map(|region| region.id);
    let found = if last {
        ids.rev().find(|id| rig.read(|screen| pick(screen, *id)))
    } else {
        ids.find(|id| rig.read(|screen| pick(screen, *id)))
    };
    found.expect("a region of that kind is drawn")
}

/// Dragging the handle at the sidebar's edge makes it wider by as far as it went,
/// with the cursor that says so while it is held.
#[test]
fn the_sidebar_handle_resizes_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    let before = rig.state(|state| state.sidebar_width);
    let from = rig.center(SIDEBAR_SASH);
    let to = (from.0 + 60.0, from.1);
    rig.move_to(from);
    rig.button_in(None, winit::event::MouseButton::Left, true, from);
    rig.move_to(to);
    assert_eq!(rig.cursor(), rustle::CursorIcon::ColResize);
    rig.button_in(None, winit::event::MouseButton::Left, false, to);
    assert_eq!(rig.state(|state| state.sidebar_width), before + 60.0);
    rig.move_to((to.0 + 100.0, to.1));
    assert_eq!(rig.state(|state| state.sidebar_width), before + 60.0);
}

/// Dragging the handle of the review panel left makes it wider.
#[test]
fn the_review_handle_resizes_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    let before = rig.state(|state| state.review_width);
    let from = rig.center(REVIEW_SASH);
    rig.drag(from, (from.0 - 40.0, from.1), 4);
    assert_eq!(rig.state(|state| state.review_width), before + 40.0);
}

/// Dragging a project's header below another moves it there; a press that
/// barely moves is a press, not a drag.
#[test]
fn projects_reorder_by_dragging() {
    let mut rig = Rig::shot(Shot::Workspace);
    let names = |rig: &Rig| {
        rig.state(|state| {
            state
                .projects
                .iter()
                .map(|project| project.name.clone())
                .collect::<Vec<_>>()
        })
    };
    let before = names(&rig);
    let from = rig.center(FocusId(PROJECT_BASE));
    let below = rig.center(FocusId(PROJECT_BASE + 2));
    rig.drag(from, (from.0, below.1 + 10.0), 8);
    let after = names(&rig);
    assert_ne!(before, after);
    assert_eq!(after.iter().position(|name| *name == before[0]), Some(2));
}

/// Dragging a tab of one pane into the other moves it there.
#[test]
fn tabs_move_between_panes() {
    let mut rig = Rig::shot(Shot::PaneSplit);
    let before = rig.state(|state| format!("{:?}", state.panes));
    let tab = region_where(&rig, false, |screen, id| screen.pane_tab_of(id).is_some());
    let other = region_where(&rig, true, |_, id| {
        pm_components::panes::pane_of(id).is_some()
    });
    let from = rig.center(tab);
    let to = rig.center(other);
    rig.drag(from, to, 10);
    assert_ne!(rig.state(|state| format!("{:?}", state.panes)), before);
}

/// Dragging a file of the project onto a folder asks for it to be moved
/// there, and Escape during a drag gives it up.
#[test]
fn files_drag_into_folders() {
    let mut rig = Rig::shot(Shot::Files);
    let file = region_where(&rig, true, |screen, id| {
        screen.tree_row_of(id).is_some_and(|row| !row.folder)
    });
    let folder = region_where(&rig, false, |screen, id| {
        screen
            .tree_row_of(id)
            .is_some_and(|row| row.folder && !row.is_root())
    });
    let from = rig.center(file);
    let to = rig.center(folder);
    rig.move_to(from);
    rig.button_in(None, winit::event::MouseButton::Left, true, from);
    rig.move_to((from.0, from.1 - 20.0));
    rig.move_to(to);
    assert!(rig.read(Screen::tree_dragging));
    rig.key(pm_components::kit::Key::Escape);
    assert!(!rig.read(Screen::tree_dragging));
    rig.button_in(None, winit::event::MouseButton::Left, false, to);
    let _ = rig.app.work.asked();
    rig.drag(from, to, 8);
    let asked = rig.app.work.asked();
    assert!(
        asked
            .iter()
            .any(|(_, request)| format!("{request:?}").contains("RenameEntry")),
        "{asked:?}"
    );
}

/// Pressing the minimap scrolls the file shown to the place pressed.
#[test]
fn the_minimap_scrolls_the_file() {
    let mut rig = Rig::shot(Shot::Code);
    let map = rig.report().region(MINIMAP).expect("the minimap is drawn");
    let viewer = pm_components::git_controls::VIEWER_SCROLL;
    let before = rig.report().scroller(viewer).map(|held| held.offset);
    let at = (
        map.origin.x + map.size.width / 2.0,
        map.origin.y + map.size.height / 2.0,
    );
    rig.drag(at, (at.0, at.1 - 1.0), 1);
    let after = rig.report().scroller(viewer).map(|held| held.offset);
    assert_ne!(before, after);
}

/// Dragging down the gutter of the changes marks the lines passed over for a comment.
#[test]
fn the_gutter_marks_lines_for_a_comment() {
    let mut rig = Rig::shot(Shot::Diff);
    let report = rig.report();
    let mut gutters: Vec<(FocusId, f32, f32)> = report
        .regions
        .iter()
        .filter(|region| pm_components::diff::gutter_of(region.id).is_some())
        .map(|region| {
            (
                region.id,
                region.rect.origin.x + region.rect.size.width / 2.0,
                region.rect.origin.y + region.rect.size.height / 2.0,
            )
        })
        .collect();
    gutters.sort_by(|a, b| a.2.total_cmp(&b.2));
    let first = gutters.first().copied().expect("the gutter is drawn");
    let third = gutters
        .iter()
        .copied()
        .find(|(_, x, y)| (*x - first.1).abs() < 1.0 && *y > first.2 + 30.0)
        .expect("lines below in the same gutter");
    rig.drag((first.1, first.2), (third.1, third.2), 4);
    let target = rig.state(|state| state.comment_target);
    let range = target.and_then(|target| target.range);
    assert!(
        range.is_some_and(|range| range.end > range.start),
        "{range:?}"
    );
}
