//! The problems tab of the dock: what the language servers say is wrong with the files of the project, each a row that goes to its place.

use crate::areas::TerminalEvent;
use crate::events;
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use crate::theme::{Role, color};
use pm_protocol::Severity;
use rustle::Modifier;
use rustle::{Alignment, Arrangement};

use crate::parts::line;
use crate::state::WorkspaceState;
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
fn problem_row(index: usize, problem: &pm_protocol::Problem) {
    let diagnostic = &problem.diagnostic;
    let message = diagnostic.message.lines().next().unwrap_or_default();
    let name = problem.path.rsplit('/').next().unwrap_or(&problem.path);
    let place = format!(
        "{name} [{}, {}]",
        diagnostic.span.start.line + 1,
        diagnostic.span.start.character + 1
    );
    let id = Target::Terminal(TerminalEvent::ProblemRow(index));
    let modifier = Modifier::new()
        .fill_max_width()
        .height(theme::sp(6))
        .clip()
        .indicated(
            theme::InteractionTones::new().hover(Role::SurfaceHover),
            theme::Focused::Control(crate::focus::is_focused(id)),
        );
    let modifier = events::pressed(modifier, id).padding_xy(theme::sp(3), 0.0);
    let dot = color(severity_role(diagnostic.severity));
    rustle::row_arranged(
        modifier,
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            rustle::spacer(
                Modifier::new()
                    .size(theme::sp(2), theme::sp(2))
                    .background_rounded(dot, 4.0),
            );
            line(message.to_owned(), 12, 1.4, Role::Text);
            if let Some(source) = &diagnostic.source {
                line(source.clone(), 11, 1.4, Role::Subtle);
            }
            line(place, 11, 1.4, Role::Subtle);
        },
    );
}

/// Emits the tab: the list of problems, scrolled as the state says.
pub fn problems_tab(state: &WorkspaceState) {
    rustle::column_arranged(
        Modifier::new()
            .weight(1.0)
            .clip()
            .vertical_scroll(&state.problems_scroll)
            .fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        || {
            if state.current_problems().is_empty() {
                rustle::row_arranged(
                    Modifier::new()
                        .height(theme::sp(7))
                        .padding_xy(theme::sp(3), 0.0),
                    Arrangement::Start,
                    Alignment::Center,
                    || line(NOTHING_WRONG, 12, 1.4, Role::Subtle),
                );
                return;
            }
            for (index, problem) in state.current_problems().iter().take(MOST_ROWS).enumerate() {
                problem_row(index, problem);
            }
        },
    );
}
