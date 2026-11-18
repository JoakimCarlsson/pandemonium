//! The problems tab of the dock: what the language servers say is wrong with the files of the project, each a row that goes to its place.

use crate::kit::sp;
use crate::kit::{
    FocusId, Hovered, Interactive, Looks, Radius, Role, Scroll, Styled, color, scroll_area,
};
use pm_protocol::Severity;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::parts::line;
use crate::state::{PROBLEM_ROW_BASE, PROBLEMS_SCROLL, WorkspaceState};
use crate::viewer::severity_role;

/// The most rows the list holds.
const MOST_ROWS: usize = 1000;

/// What the tab shows when nothing is wrong.
const NOTHING_WRONG: &str = "No problems have been detected in the workspace.";

/// How many of `state`'s problems are errors and how many warnings.
pub fn tally(state: &WorkspaceState) -> (usize, usize) {
    let count = |severity: Severity| {
        state
            .current_problems()
            .iter()
            .filter(|problem| problem.diagnostic.severity == severity)
            .count()
    };
    (count(Severity::Error), count(Severity::Warning))
}

/// Emits one row: a dot coloured by how bad it is, the first line of the message, who said it and where.
#[track_caller]
fn problem_row(index: usize, problem: &pm_protocol::Problem) {
    let diagnostic = &problem.diagnostic;
    let message = diagnostic.message.lines().next().unwrap_or_default();
    let name = problem.path.rsplit('/').next().unwrap_or(&problem.path);
    let place = format!(
        "{name} [{}, {}]",
        diagnostic.span.start.line + 1,
        diagnostic.span.start.character + 1
    );
    let id = FocusId(PROBLEM_ROW_BASE + index as u64);
    let modifier = Modifier::new()
        .fill_max_width()
        .h(6)
        .clip_to(Radius::None)
        .looks_of(Looks::new().hover(Role::SurfaceHover), id)
        .cursor(CursorIcon::Pointer)
        .target(id)
        .pad_x(3);
    let dot = color(severity_role(diagnostic.severity));
    row_arranged(
        modifier,
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            rustle::spacer(Modifier::new().square(2).background_rounded(dot, 4.0));
            line(message.to_owned(), 12, 1.4, Role::Text);
            if let Some(source) = &diagnostic.source {
                line(source.clone(), 11, 1.4, Role::Subtle);
            }
            line(place, 11, 1.4, Role::Subtle);
        },
    );
}

/// Emits the tab: the list of problems, scrolled as the state says.
#[track_caller]
pub fn problems_tab(state: &WorkspaceState) {
    scroll_area(
        Modifier::new().weight(1.0).clip_to(Radius::None),
        Scroll::from_start(PROBLEMS_SCROLL, state.problems_scroll),
        || {
            column_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    if state.current_problems().is_empty() {
                        row_arranged(
                            Modifier::new().h(7).pad_x(3),
                            Arrangement::Start,
                            Alignment::Center,
                            || {
                                line(NOTHING_WRONG, 12, 1.4, Role::Subtle);
                            },
                        );
                        return;
                    }
                    for (index, problem) in
                        state.current_problems().iter().take(MOST_ROWS).enumerate()
                    {
                        problem_row(index, problem);
                    }
                },
            );
        },
    );
}
