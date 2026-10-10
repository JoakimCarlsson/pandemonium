//! Carrying project rows through the visible, grouped Projects reading.

use pm_core::ProjectId;
use pm_gfx::{Point, Rect};
use pm_ui::{ResizeEvent, ResizePhase};

use crate::app::App;
use crate::project_groups::{self, ProjectRow};

/// How far a press may travel and still have been a click on the row.
const SLIP: f32 = 4.0;

/// Thickness of the line marking where a carried project would land.
const CARET: f32 = 2.0;

/// A destination within the visible project reading.
#[derive(Clone, Copy, Debug)]
struct ProjectLanding {
    /// Gap in the underlying open-project order.
    gap: usize,
    /// Group receiving the project, or the ungrouped reading.
    group: Option<usize>,
    /// Vertical position of the insertion caret.
    top: f32,
}

/// A project's row under the pointer and its destination.
#[derive(Clone, Copy, Debug)]
pub struct ProjectDrag {
    /// The project whose row was pressed.
    pub project: ProjectId,
    /// Maximum travel since the press began.
    pub travelled: f32,
    /// The visible destination, absent outside the project list.
    landing: Option<ProjectLanding>,
}

impl ProjectDrag {
    /// Whether the pointer has travelled far enough to carry the row.
    pub fn is_carried(&self) -> bool {
        self.travelled > SLIP
    }
}

impl App {
    /// Activates a pressed project, or moves a carried one within or between groups.
    pub(super) fn drag_project(&mut self, id: ProjectId, event: ResizeEvent) {
        let travelled = (event.current.x - event.start.x).hypot(event.current.y - event.start.y);
        let drag = ProjectDrag {
            project: id,
            travelled: match (event.phase, self.project_drag) {
                (ResizePhase::Started, _) | (_, None) => travelled,
                (_, Some(drag)) => drag.travelled.max(travelled),
            },
            landing: self.project_gap_at(event.current),
        };
        if event.phase != ResizePhase::Ended {
            self.project_drag = Some(drag);
            return;
        }
        self.project_drag = None;
        if drag.is_carried() {
            if let Some(landing) = drag.landing {
                self.open.move_to(drag.project, landing.gap);
                self.assign_project_group(drag.project, landing.group);
            }
        } else {
            self.activate_project(drag.project);
        }
    }

    /// Activates a project's own checkout through the shared scope seam.
    pub(super) fn activate_project(&mut self, id: ProjectId) {
        self.open.activate(id);
        self.select_checkout();
        self.store();
    }

    /// The insertion caret, excluding destinations that leave order and membership unchanged.
    pub(super) fn project_caret(&self) -> Option<Rect> {
        let drag = self.project_drag.filter(ProjectDrag::is_carried)?;
        let landing = drag.landing?;
        let from = self
            .open
            .iter()
            .position(|project| project.id() == drag.project)?;
        let project = self.open.get(drag.project)?;
        let group = project_groups::membership(&self.project_groups, &project.root().stored());
        if group == landing.group && (landing.gap == from || landing.gap == from + 1) {
            return None;
        }
        let list = self.project_list.get();
        Some(Rect::from_xywh(
            list.left(),
            landing.top - CARET / 2.0,
            list.size.width,
            CARET,
        ))
    }

    /// Visible rows and their bounds, including headings and expanded sessions.
    fn project_slots(&self) -> Vec<(ProjectRow, Rect)> {
        let list = self.project_list.get();
        let mut top = list.top();
        let row_height = self.theme().size.row;
        project_groups::rows(&self.open, &self.project_groups)
            .into_iter()
            .map(|row| {
                let count = match row {
                    ProjectRow::Heading(_) => 1,
                    ProjectRow::Project(id, _) => 1 + self.sessions.of(id).count(),
                };
                let height = row_height * count as f32;
                let bounds = Rect::from_xywh(list.left(), top, list.size.width, height);
                top += height;
                (row, bounds)
            })
            .collect()
    }

    /// Resolves a pointer to the gap and group represented by its visible row.
    fn project_gap_at(&self, point: Point) -> Option<ProjectLanding> {
        let list = self.project_list.get();
        if point.x < list.left()
            || point.x > list.right()
            || point.y < list.top()
            || point.y > list.bottom()
        {
            return None;
        }
        let mut last = None;
        for (row, bounds) in self.project_slots() {
            let landing = match row {
                ProjectRow::Heading(group) => ProjectLanding {
                    gap: self
                        .open
                        .iter()
                        .position(|project| {
                            project_groups::membership(
                                &self.project_groups,
                                &project.root().stored(),
                            ) == group
                        })
                        .unwrap_or(self.open.iter().count()),
                    group,
                    top: bounds.bottom(),
                },
                ProjectRow::Project(id, group) => {
                    let index = self.open.iter().position(|project| project.id() == id)?;
                    let after = point.y >= bounds.top() + bounds.size.height / 2.0;
                    ProjectLanding {
                        gap: index + usize::from(after),
                        group,
                        top: if after { bounds.bottom() } else { bounds.top() },
                    }
                }
            };
            if point.y <= bounds.bottom() {
                return Some(landing);
            }
            last = Some(landing);
        }
        last
    }
}
