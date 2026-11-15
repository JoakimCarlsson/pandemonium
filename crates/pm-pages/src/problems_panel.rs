//! The dock's Problems tab: what the language servers find wrong with the
//! files open, worst first, each with its file and line.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{Label, LabelVariant, Leading, ListRow, Size, extended_colors};
use pm_shell::{Problem, ProblemSeverity};

/// Emits the list of `problems`, or a line saying there are none; pressing
/// a problem calls `open` with its file's whole path and its line, counted
/// from 1.
pub fn problems_panel(problems: &[Problem], open: Rc<dyn Fn(String, usize)>) {
    if problems.is_empty() {
        Label::new("No problems have been detected in the workspace.")
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
        return;
    }
    let scroll = remember_scroll_state();
    column_with(
        Modifier::new().fill_max_size().vertical_scroll(&scroll),
        || {
            for (index, problem) in problems.iter().enumerate() {
                key(index, || problem_row(problem, open.clone()));
            }
        },
    );
}

/// Emits one problem: its severity's mark, what it says, and where it is;
/// pressing it calls `open` with where it is.
fn problem_row(problem: &Problem, open: Rc<dyn Fn(String, usize)>) {
    let severity = problem.severity;
    let whole = format!(
        "{}/{}",
        problem.folder.trim_end_matches('/'),
        problem.path
    );
    let line = problem.line + 1;
    let mut row = ListRow::new(problem.message.clone())
        .leading(Leading::Custom(Rc::new(move || severity_mark(severity))))
        .note(format!(
            "{}:{}:{}",
            problem.path,
            problem.line + 1,
            problem.column + 1
        ))
        .size(Size::Xs)
        .on_click(move || open(whole.clone(), line));
    if let Some(source) = &problem.source {
        row = row.end_column(source.clone(), 80.0);
    }
    row.show();
}

/// Emits the glyph of `severity` in its tone.
fn severity_mark(severity: ProblemSeverity) {
    let scheme = color_scheme();
    let extended = extended_colors();
    let (glyph, tone) = match severity {
        ProblemSeverity::Error => (phosphor::X_CIRCLE, scheme.error),
        ProblemSeverity::Warning => (phosphor::WARNING, extended.warning),
        ProblemSeverity::Information => (phosphor::INFO, scheme.primary),
        ProblemSeverity::Hint => (phosphor::LIGHTBULB, scheme.on_surface_variant),
    };
    icon(glyph, 14.0, tone);
}
