//! The problems tab of the dock: what the language servers say is wrong with the files of the project, each a row that goes to its place.

use pm_input::FocusId;
use pm_protocol::Severity;
use pm_style::{
    Role, bg, cursor_pointer, flex, flex_1, flex_col, flex_none, gap, h, items_center, min_h,
    overflow_hidden, px, rounded, size,
};
use pm_ui::{Node, node};

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
            .problems
            .iter()
            .filter(|problem| problem.diagnostic.severity == severity)
            .count()
    };
    (count(Severity::Error), count(Severity::Warning))
}

/// One row: a dot coloured by how bad it is, the first line of the message, who said it and where.
fn row(index: usize, problem: &pm_protocol::Problem) -> Node {
    let diagnostic = &problem.diagnostic;
    let message = diagnostic.message.lines().next().unwrap_or_default();
    let name = problem.path.rsplit('/').next().unwrap_or(&problem.path);
    let mut row = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(6),
            px(3),
            flex_none(),
            overflow_hidden(),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .child(node().styles([
            size(2),
            rounded(4.0),
            flex_none(),
            bg(severity_role(diagnostic.severity)),
        ]))
        .child(line(message.to_owned(), 12, 1.4, Role::Text).styles([flex_none()]));
    if let Some(source) = &diagnostic.source {
        row = row.child(line(source.clone(), 11, 1.4, Role::Subtle).styles([flex_none()]));
    }
    row.child(
        line(
            format!(
                "{name} [{}, {}]",
                diagnostic.span.start.line + 1,
                diagnostic.span.start.character + 1
            ),
            11,
            1.4,
            Role::Subtle,
        )
        .styles([flex_none()]),
    )
    .focusable(FocusId(PROBLEM_ROW_BASE + index as u64))
}

/// The tab: the list of problems, scrolled as the state says.
pub fn problems_tab(state: &WorkspaceState) -> Node {
    let mut body = node()
        .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
        .scroll_from_start(PROBLEMS_SCROLL, state.problems_scroll);
    if state.problems.is_empty() {
        return body.child(
            node()
                .styles([flex(), items_center(), h(7), px(3), flex_none()])
                .child(line(NOTHING_WRONG, 12, 1.4, Role::Subtle)),
        );
    }
    for (index, problem) in state.problems.iter().take(MOST_ROWS).enumerate() {
        body = body.child(row(index, problem));
    }
    body
}
