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
    assert!(rig.state(|state| state.commenting()));
}

/// Clicking the number of a line of the changes opens the box for a comment on that line
/// alone, and a drag that ends opens it too, no longer stretching.
#[test]
fn a_click_on_the_gutter_comments_on_one_line() {
    let mut rig = Rig::shot(Shot::Diff);
    let number = rig
        .tagged()
        .iter()
        .find(|node| matches!(node.id, Target::Diff(DiffEvent::Gutter { .. })))
        .map(|node| {
            (
                node.visible.origin.x + node.visible.size.width / 2.0,
                node.visible.origin.y + node.visible.size.height / 2.0,
            )
        })
        .expect("the gutter is drawn");
    rig.click(number);
    let range = rig.state(|state| state.comment_target.and_then(|target| target.range));
    assert!(
        range.is_some_and(|range| range.end == range.start),
        "{range:?}"
    );
    assert!(rig.state(|state| state.commenting()));
}

/// Dragging the handle on the top edge of the terminal dock up makes the dock taller.
#[test]
fn the_terminal_handle_resizes_the_dock() {
    let mut rig = Rig::shot(Shot::Terminal);
    let before = rig.state(|state| state.terminal_height);
    let from = rig.center(Target::Window(WindowEvent::TerminalSash));
    rig.drag(from, (from.0, from.1 - 40.0), 4);
    assert_eq!(rig.state(|state| state.terminal_height), before + 40.0);
}

/// The handles of the sidebar and the review panel lie in the row of panels centred on the
/// edge they resize, as tall as the row under the title bar, and the dock's handle lies
/// centred on the dock's top edge, as wide as the main area.
#[test]
fn the_handles_lie_on_the_edges_they_resize() {
    let rig = Rig::shot(Shot::Workspace);
    let (width, height) = crate::app::headless::SIZE;
    let bar = pm_components::title_bar::TITLE_BAR_HEIGHT;
    let sidebar = rig
        .region(Target::Window(WindowEvent::SidebarSash))
        .expect("the sidebar's handle is laid out");
    let edge = rig.state(|state| state.sidebar_width);
    assert_eq!(sidebar.origin.x + sidebar.size.width / 2.0, edge);
    assert_eq!((sidebar.origin.y, sidebar.size.height), (bar, height - bar));
    let review = rig
        .region(Target::Window(WindowEvent::ReviewSash))
        .expect("the review panel's handle is laid out");
    let edge = width - rig.state(|state| state.review_width);
    assert_eq!(review.origin.x + review.size.width / 2.0, edge);
    assert_eq!((review.origin.y, review.size.height), (bar, height - bar));
    let rig = Rig::shot(Shot::Terminal);
    let dock = rig
        .region(Target::Window(WindowEvent::TerminalSash))
        .expect("the dock's handle is laid out");
    let main = rig
        .region(Target::Window(WindowEvent::Pane(0)))
        .expect("the pane is laid out");
    assert_eq!(
        dock.origin.y + dock.size.height / 2.0,
        main.origin.y + main.size.height
    );
    assert_eq!(dock.origin.x, main.origin.x);
}

/// While the plan shows, its handle lies centred on the plan panel's left edge, as tall as
/// the row of panels, and dragging it left makes the panel wider.
#[test]
fn the_plan_handle_lies_on_its_edge_and_resizes_it() {
    let Screen::Workspace(state) = Shot::Asking.screen();
    let state = state.handle(pm_components::events::Event::Chat(
        pm_components::areas::ChatEvent::OpenPlan,
    ));
    assert!(state.plan_shown().is_some(), "the plan shows");
    let mut rig = Rig::new(Screen::Workspace(state), crate::app::headless::SIZE);
    let (width, height) = crate::app::headless::SIZE;
    let bar = pm_components::title_bar::TITLE_BAR_HEIGHT;
    let handle = rig
        .region(Target::Window(WindowEvent::PlanSash))
        .expect("the plan's handle is laid out");
    let plan = rig.state(|state| state.plan_room(width));
    let review = rig.state(|state| {
        if state.review_session().is_some() && state.review {
            state.review_width
        } else {
            0.0
        }
    });
    assert_eq!(
        handle.origin.x + handle.size.width / 2.0,
        width - review - plan
    );
    assert_eq!((handle.origin.y, handle.size.height), (bar, height - bar));
    let from = rig.center(Target::Window(WindowEvent::PlanSash));
    rig.drag(from, (from.0 - 40.0, from.1), 4);
    assert_eq!(rig.state(|state| state.plan_room(width)), plan + 40.0);
}

/// A split lays its first side out at its share of the split, rounded to a whole pixel,
/// then the line between the sides one pixel thick and its second side over the rest; the
/// handle of each split lies centred on the line's leading edge, as long as the split is
/// across, and dragging it moves the line as far. The shot's panes are a pane beside two
/// panes one above the other: the first split side by side, the second one above the other.
#[test]
fn a_split_lays_its_panes_out_by_its_share() {
    use pm_components::panes::{Axis, Layout};
    let mut rig = Rig::shot(Shot::Panes);
    let ratios = rig.state(|state| match state.panes().layout {
        Layout::Split {
            axis: Axis::Row,
            ratio,
            first,
            ..
        } => match *first {
            Layout::Split {
                axis: Axis::Column,
                ratio: inner,
                ..
            } => (ratio, inner),
            _ => panic!("the first side is split one above the other"),
        },
        _ => panic!("the panes are split side by side"),
    });
    let pane = |rig: &Rig, index| {
        rig.region(Target::Window(WindowEvent::Pane(index)))
            .expect("the pane is laid out")
    };
    let handle = |rig: &Rig, index| {
        rig.region(Target::Window(WindowEvent::SplitSash(index)))
            .expect("the split's handle is laid out")
    };
    let (top, bottom, beside) = (pane(&rig, 0), pane(&rig, 1), pane(&rig, 2));
    let across = beside.origin.x + beside.size.width - top.origin.x;
    assert_eq!(top.size.width, (across * ratios.0).round());
    assert_eq!(beside.origin.x, top.origin.x + top.size.width + 1.0);
    let outer = handle(&rig, 0);
    assert_eq!(
        outer.origin.x + outer.size.width / 2.0,
        top.origin.x + top.size.width
    );
    assert_eq!(outer.size.height, beside.size.height);
    let down = bottom.origin.y + bottom.size.height - top.origin.y;
    assert_eq!(top.size.height, (down * ratios.1).round());
    assert_eq!(bottom.origin.y, top.origin.y + top.size.height + 1.0);
    let inner = handle(&rig, 1);
    assert_eq!(
        inner.origin.y + inner.size.height / 2.0,
        top.origin.y + top.size.height
    );
    assert_eq!(inner.size.width, top.size.width);
    let from = rig.center(Target::Window(WindowEvent::SplitSash(0)));
    rig.drag(from, (from.0 + 30.0, from.1), 3);
    assert_eq!(pane(&rig, 0).size.width, top.size.width + 30.0);
}

/// While a tab is held over the right edge of a pane, the highlight of where it would go
/// covers the right half of the pane's box, inset a little: a fraction of the box, as
/// Compose's `fillMaxWidth(0.5f)` takes it, filling the box the other way.
#[test]
fn a_tab_held_by_an_edge_lights_half_the_pane() {
    let rig = Rig::shot(Shot::PanesDragging);
    let pane = rig
        .region(Target::Window(WindowEvent::Pane(0)))
        .expect("the pane is drawn");
    let accent = pm_components::theme::color(pm_components::theme::Role::Accent);
    let lit: Vec<rustle::Rect> = rig
        .filled_with(accent)
        .into_iter()
        .chain(rig.filled_with(accent.copy(0.18 * accent.alpha())))
        .filter(|rect| rect.size.width > 40.0 && rect.size.height > 40.0)
        .collect();
    let margin = pm_components::shadow_margin();
    let right = pane.origin.x + pane.size.width + margin;
    let half = lit
        .iter()
        .find(|rect| {
            (rect.size.width - (pane.size.width / 2.0 - 8.0)).abs() < 1.5
                && (rect.origin.x + rect.size.width - (right - 4.0)).abs() < 1.5
        })
        .unwrap_or_else(|| panic!("no half of {pane:?} lit among {lit:?}"));
    assert!((half.size.height - (pane.size.height - 8.0)).abs() < 1.5);
    assert!((half.origin.y - (pane.origin.y + margin + 4.0)).abs() < 1.5);
}
