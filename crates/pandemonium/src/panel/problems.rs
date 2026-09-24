//! The panel's list of problems: every diagnostic of the open files, under
//! the file it is in.

use pm_text::{Position, Severity};
use pm_ui::{
    Bounds, Div, IconName, IconSize, Scrolled, Styled, Theme, h_flex, icon, measured, scroll_area,
    text, v_flex,
};

use crate::editor::FileId;
use crate::message::Message;

/// One open file with something to say about it, and what is said.
pub struct ProblemFile {
    /// Which open file this is.
    pub file: FileId,
    /// What the file is called.
    pub name: String,
    /// The directory it sits in, from the worktree down.
    pub directory: String,
    /// What the servers have said about it, in the order they said it.
    pub problems: Vec<Problem>,
}

/// One thing a server has said about one place in a file.
pub struct Problem {
    /// How much it matters.
    pub severity: Severity,
    /// The first line of what it says.
    pub message: String,
    /// Which tool said it, when the server names one.
    pub source: Option<String>,
    /// Where it starts.
    pub at: Position,
}

/// Builds the list of problems, scrolled by `scroll` and measured into
/// `area` so the wheel knows when it is over it.
pub(super) fn problems_view(
    theme: &Theme,
    files: &[ProblemFile],
    scroll: Scrolled,
    area: Bounds,
) -> Div<Message> {
    if files.is_empty() {
        return v_flex().w_full().px(2).py(1.5).child(
            text("No problems have been detected in the open files.")
                .text_sm()
                .font_light()
                .color(theme.colors.text_subtle),
        );
    }
    let rows = files.iter().flat_map(|file| {
        std::iter::once(file_row(theme, file)).chain(
            file.problems
                .iter()
                .map(|problem| problem_row(theme, file.file, problem)),
        )
    });
    v_flex().w_full().flex_1().overflow_hidden().child(measured(
        area,
        scroll_area(scroll, v_flex().w_full().py(0.5).children(rows))
            .w_full()
            .flex_1(),
    ))
}

/// Builds the row naming one file, with how many problems it has.
fn file_row(theme: &Theme, file: &ProblemFile) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(1.5)
        .gap(0.75)
        .items_center()
        .overflow_hidden()
        .child(
            icon(IconName::ChevronDown)
                .size(IconSize::XSmall)
                .color(theme.colors.text_subtle),
        )
        .child(
            icon(IconName::File)
                .size(IconSize::Small)
                .color(theme.colors.text_subtle),
        )
        .child(
            text(file.name.clone())
                .text_sm()
                .font_light()
                .color(theme.colors.text),
        )
        .child(
            text(file.directory.clone())
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
        .child(badge(theme, file.problems.len()))
}

/// Builds the row of one problem; clicking it goes to where it is.
fn problem_row(theme: &Theme, file: FileId, problem: &Problem) -> Div<Message> {
    let place = format!(
        "[Ln {}, Col {}]",
        problem.at.line + 1,
        problem.at.column + 1
    );
    let said = match &problem.source {
        Some(source) => format!("{source} {place}"),
        None => place,
    };
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .pl(5)
        .pr(1.5)
        .gap(0.75)
        .items_center()
        .overflow_hidden()
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::OpenProblem(file, problem.at))
        .child(
            icon(IconName::Warning)
                .size(IconSize::XSmall)
                .color(severity_color(theme, problem.severity)),
        )
        .child(
            text(problem.message.clone())
                .text_sm()
                .font_light()
                .color(theme.colors.text),
        )
        .child(
            text(said)
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// Builds the round count beside a name.
pub(super) fn badge(theme: &Theme, count: usize) -> Div<Message> {
    h_flex()
        .px(0.75)
        .items_center()
        .rounded(theme.radius.full)
        .bg(theme.colors.surface_selected)
        .child(
            text(count.to_string())
                .text_xs()
                .color(theme.colors.text_muted),
        )
}

/// The colour a problem's mark is drawn in, by how much it matters.
fn severity_color(theme: &Theme, severity: Severity) -> pm_gfx::Rgba {
    match severity {
        Severity::Error => theme.colors.danger,
        Severity::Warning => theme.colors.warning,
        Severity::Information | Severity::Hint => theme.colors.text_subtle,
    }
}
