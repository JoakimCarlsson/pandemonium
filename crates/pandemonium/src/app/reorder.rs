//! Carrying a project's row up or down the projects sidebar to reorder them.
//!
//! The row is picked up by the same press that would have activated it, and
//! only travel tells the two apart: a press that goes nowhere activates the
//! project, and one that travels puts it in the gap between two others. The
//! rows are all one height, so the gaps are read off the list's bounds and
//! how many rows each project draws, rather than off every row's own.

use pm_core::ProjectId;
use pm_gfx::{Point, Rect};
use pm_ui::{ResizeEvent, ResizePhase};

use crate::app::App;

/// How far a press may travel and still have been a click on the row.
const SLIP: f32 = 4.0;

/// How thick the line marking where a carried project would land is drawn.
const CARET: f32 = 2.0;

/// A project's row under the pointer, and the gap letting go would put it in.
#[derive(Clone, Copy, Debug)]
pub struct ProjectDrag {
    /// The project whose row was pressed.
    pub project: ProjectId,
    /// How far the pointer has travelled since the press.
    pub travelled: f32,
    /// The gap it would land in, counted before the project at that place.
    pub gap: usize,
}

impl ProjectDrag {
    /// Whether the pointer has gone far enough to have carried the row.
    pub fn is_carried(&self) -> bool {
        self.travelled > SLIP
    }
}

impl App {
    /// Answers a press, a drag or a release on the row of project `id`.
    pub(super) fn drag_project(&mut self, id: ProjectId, event: ResizeEvent) {
        let travelled = (event.current.x - event.start.x).hypot(event.current.y - event.start.y);
        let drag = ProjectDrag {
            project: id,
            travelled: match (event.phase, self.project_drag) {
                (ResizePhase::Started, _) | (_, None) => travelled,
                (_, Some(drag)) => drag.travelled.max(travelled),
            },
            gap: self.project_gap_at(event.current),
        };

        if event.phase != ResizePhase::Ended {
            self.project_drag = Some(drag);
            return;
        }
        self.project_drag = None;
        match drag.is_carried() {
            true => {
                self.open.move_to(drag.project, drag.gap);
                self.store();
            }
            false => self.activate_project(drag.project),
        }
    }

    /// Makes `id` the active project, pointed at its own checkout.
    pub(super) fn activate_project(&mut self, id: ProjectId) {
        self.open.activate(id);
        self.select_checkout();
        self.store();
    }

    /// The line across the sidebar where the carried project would land.
    ///
    /// Nothing is drawn while the gap either side of the project itself is
    /// the one under the pointer, because letting go there moves nothing.
    pub(super) fn project_caret(&self) -> Option<Rect> {
        let drag = self.project_drag.filter(ProjectDrag::is_carried)?;
        let from = self
            .open
            .iter()
            .position(|project| project.id() == drag.project)?;
        if drag.gap == from || drag.gap == from + 1 {
            return None;
        }
        let list = self.project_list.get();
        let top = list.top() + self.project_heights().take(drag.gap).sum::<f32>();
        Some(Rect::from_xywh(
            list.left(),
            top - CARET / 2.0,
            list.size.width,
            CARET,
        ))
    }

    /// The gap between projects nearest `point`, down the sidebar's rows.
    fn project_gap_at(&self, point: Point) -> usize {
        let mut top = self.project_list.get().top();
        let mut gap = 0;
        for height in self.project_heights() {
            if point.y < top + height / 2.0 {
                break;
            }
            top += height;
            gap += 1;
        }
        gap
    }

    /// How tall each open project's rows are drawn: its own and its sessions'.
    fn project_heights(&self) -> impl Iterator<Item = f32> + '_ {
        let row = self.theme().size.row;
        self.open
            .iter()
            .map(move |project| (1 + self.sessions.of(project.id()).count()) as f32 * row)
    }
}
