//! Dragging: the handles between panels, projects in the sidebar, tabs
//! between panes, rows of the project's files into folders, and the
//! minimap of the file shown.

use pm_components::areas::{DiffEvent, EditorEvent, SidebarEvent, WindowEvent};
use pm_components::target::Target;

use pm_pages::Screen;
use rustle::PointerButton;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// The first region drawn whose name `pick` accepts, from the end of the
/// list when `last`.
fn region_where(rig: &Rig, last: bool, pick: impl Fn(&Screen, Target) -> bool) -> Target {
    let tagged = rig.tagged();
    let mut ids = tagged.iter().map(|node| node.id);
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
    let from = rig.center(Target::Window(WindowEvent::SidebarSash));
    let to = (from.0 + 60.0, from.1);
    rig.move_to(from);
    rig.button_in(None, PointerButton::Primary, true, from);
    rig.move_to(to);
    assert_eq!(rig.cursor(), rustle::CursorIcon::ColResize);
    rig.button_in(None, PointerButton::Primary, false, to);
    assert_eq!(rig.state(|state| state.sidebar_width), before + 60.0);
    rig.move_to((to.0 + 100.0, to.1));
    assert_eq!(rig.state(|state| state.sidebar_width), before + 60.0);
}

/// The rectangles a line one logical pixel wide fills in the accent colour within a
/// handle's reach of `x`, as the sidebar's handle draws while the pointer is over it.
fn accent_lines_near(rig: &Rig, x: f32) -> Vec<rustle::Rect> {
    let accent = pm_components::theme::color(pm_components::theme::Role::Accent);
    rig.filled_with(accent)
        .into_iter()
        .filter(|rect| {
            rect.size.width <= 1.0
                && (rect.origin.x - x - pm_components::shadow_margin()).abs() < 8.0
        })
        .collect()
}

/// The sidebar's handle lights while the pointer is over it, and once a drag of it ends
/// and the pointer moves away, a click lands elsewhere and Tab is pressed it is dark
/// again: neither the pointer's hover nor the drag stays held.
#[test]
fn the_sidebar_handle_goes_dark_after_a_drag() {
    let mut rig = Rig::shot(Shot::Workspace);
    let from = rig.center(Target::Window(WindowEvent::SidebarSash));
    rig.move_to(from);
    assert!(
        !accent_lines_near(&rig, from.0).is_empty(),
        "the handle does not light under the pointer"
    );
    let to = (from.0 + 60.0, from.1);
    rig.drag(from, to, 6);
    let away = (to.0 + 300.0, to.1);
    rig.move_to(away);
    assert!(
        accent_lines_near(&rig, to.0).is_empty(),
        "the handle stays lit after the drag"
    );
    rig.click(away);
    rig.key(rustle::Key::Tab);
    assert!(
        accent_lines_near(&rig, to.0).is_empty(),
        "the handle lights again"
    );
    assert_eq!(rig.cursor(), rustle::CursorIcon::Default);
}

/// While the sidebar's handle is dragged the first pane follows its edge, and the review
/// panel, whose room does not change, stays drawn where it was.
#[test]
fn a_drag_moves_what_its_room_changed_and_keeps_the_rest() {
    let mut rig = Rig::shot(Shot::Workspace);
    let pane = Target::Window(WindowEvent::Pane(0));
    let review = Target::Diff(DiffEvent::ReviewFiles);
    let (pane_before, review_before) = (rig.region(pane), rig.region(review));
    assert!(pane_before.is_some() && review_before.is_some());
    let from = rig.center(Target::Window(WindowEvent::SidebarSash));
    rig.drag(from, (from.0 + 40.0, from.1), 4);
    let (pane_after, review_after) = (rig.region(pane), rig.region(review));
    assert_eq!(
        pane_after.map(|area| area.origin.x),
        pane_before.map(|area| area.origin.x + 40.0)
    );
    assert_eq!(review_after, review_before);
}

/// Dragging the handle of the review panel left makes it wider.
#[test]
fn the_review_handle_resizes_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    let before = rig.state(|state| state.review_width);
    let from = rig.center(Target::Window(WindowEvent::ReviewSash));
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
    let from = rig.center(Target::Sidebar(SidebarEvent::Project(0)));
    let below = rig.center(Target::Sidebar(SidebarEvent::Project(2)));
    rig.drag(from, (from.0, below.1 + 10.0), 8);
    let after = names(&rig);
    assert_ne!(before, after);
    assert_eq!(after.iter().position(|name| *name == before[0]), Some(2));
}

/// A project header pressed and moved less than the distance a drag takes stays where it
/// is: the press is a click, not a drag.
#[test]
fn a_short_move_of_a_project_is_no_drag() {
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
    let from = rig.center(Target::Sidebar(SidebarEvent::Project(0)));
    rig.drag(from, (from.0 + 3.0, from.1 + 4.0), 4);
    assert_eq!(names(&rig), before);
    assert!(rig.state(|state| state.project_drag.is_none()));
}

/// Dragging a tab of one pane into the other moves it there.
#[test]
fn tabs_move_between_panes() {
    let mut rig = Rig::shot(Shot::PaneSplit);
    let before = rig.state(|state| format!("{:?}", state.panes));
    let tab = region_where(&rig, false, |screen, id| screen.pane_tab_of(id).is_some());
    let other = region_where(&rig, true, |_, id| {
        matches!(id, Target::Window(WindowEvent::Pane(_)))
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
    rig.button_in(None, PointerButton::Primary, true, from);
    rig.move_to((from.0, from.1 - 20.0));
    rig.move_to(to);
    assert!(rig.read(|screen| screen.tree_dragging()));
    rig.key(rustle::Key::Escape);
    assert!(!rig.read(|screen| screen.tree_dragging()));
    rig.button_in(None, PointerButton::Primary, false, to);
    let _ = rig.app.borrow_mut().work.asked();
    rig.drag(from, to, 8);
    let asked = rig.app.borrow_mut().work.asked();
    assert!(
        asked
            .iter()
            .any(|(_, request)| format!("{request:?}").contains("RenameEntry")),
        "{asked:?}"
    );
}

/// Files from another application held over a folder of the project's files show the
/// folder they would land in, show none once they leave the window, and, let go over it,
/// are asked to be brought into it rather than attached to the prompt.
#[test]
fn files_from_outside_land_in_the_folder_under_them() {
    let mut rig = Rig::shot(Shot::Files);
    let folder = region_where(&rig, false, |screen, id| {
        screen
            .tree_row_of(id)
            .is_some_and(|row| row.folder && !row.is_root())
    });
    let to = rig.center(folder);
    let landing =
        |rig: &Rig| rig.state(|state| state.git.tree_drag.as_ref().map(|drag| drag.into.clone()));
    rig.hold_files(to);
    assert!(
        matches!(landing(&rig), Some(Some(_))),
        "{:?}",
        landing(&rig)
    );
    rig.files_leave();
    assert_eq!(landing(&rig), None);
    let _ = rig.app.borrow_mut().work.asked();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    rig.drop_files(to, vec![path]);
    assert_eq!(landing(&rig), None);
    assert!(rig.state(|state| state.attachments.is_empty()));
    let asked = rig.app.borrow_mut().work.asked();
    assert!(
        asked
            .iter()
            .any(|(_, request)| format!("{request:?}").contains("Import")),
        "{asked:?}"
    );
}

/// Pressing the minimap scrolls the file shown to the place pressed.
#[test]
fn the_minimap_scrolls_the_file() {
    let mut rig = Rig::shot(Shot::Code);
    let map = rig
        .region(Target::Editor(EditorEvent::Minimap))
        .expect("the minimap is drawn");
    let scrolled = |rig: &Rig| {
        rig.read(|state| {
            state
                .shown_viewer()
                .map(|viewer| viewer.scroll.scroll().offset())
        })
    };
    let before = scrolled(&rig);
    let at = (
        map.origin.x + map.size.width / 2.0,
        map.origin.y + map.size.height / 2.0,
    );
    rig.drag(at, (at.0, at.1 - 1.0), 1);
    let after = scrolled(&rig);
    assert_ne!(before, after);
}

/// Dragging down the gutter of the changes marks the lines passed over for a comment.
#[test]
fn the_gutter_marks_lines_for_a_comment() {
    let mut rig = Rig::shot(Shot::Diff);
    let mut gutters: Vec<(Target, f32, f32)> = rig
        .tagged()
        .iter()
        .filter(|node| matches!(node.id, Target::Diff(DiffEvent::Gutter { .. })))
        .map(|node| {
            (
                node.id,
                node.visible.origin.x + node.visible.size.width / 2.0,
                node.visible.origin.y + node.visible.size.height / 2.0,
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
