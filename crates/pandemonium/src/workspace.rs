//! The editor workspace shown after onboarding has finished.

use pm_gfx::Rgba;
use pm_ui::{Axis, Div, Styled, Theme, h_flex, sash, text, v_flex};

use crate::onboarding::Message;

/// One project's sessions as presented by the workspace model.
pub struct SidebarProject {
    /// Repository name.
    pub name: String,
    /// Branch checked out in the project's working copy.
    pub branch: String,
    /// Sessions belonging to this project.
    pub sessions: Vec<SidebarSession>,
}

/// One session as presented by the workspace model.
pub struct SidebarSession {
    /// Human-readable name of the work.
    pub name: String,
    /// Compact diff summary.
    pub summary: String,
    /// Colour representing the state reported by the agent.
    pub status_color: Rgba,
    /// Whether this session is selected.
    pub selected: bool,
}

/// Builds the workspace with its resizable sessions sidebar.
pub fn workspace(theme: &Theme, projects: &[SidebarProject], sidebar_width: f32) -> Div<Message> {
    h_flex()
        .w_full()
        .h_full()
        .items_stretch()
        .child(sessions_sidebar(theme, projects, sidebar_width))
        .child(sash(Axis::Horizontal, Message::ResizeSidebar))
        .child(v_flex().flex_1().h_full().bg(theme.colors.background))
}

/// Builds the sessions sidebar from the workspace model.
fn sessions_sidebar(theme: &Theme, projects: &[SidebarProject], width: f32) -> Div<Message> {
    let rows = projects
        .iter()
        .map(|project| project_rows(theme, project))
        .collect::<Vec<_>>();

    v_flex()
        .w_px(width)
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .child(sidebar_switch(theme))
        .child(
            h_flex()
                .w_full()
                .px(3)
                .pt(2)
                .pb(1.5)
                .items_center()
                .justify_between()
                .child(text("SESSIONS").text_xs().color(theme.colors.text_subtle))
                .child(text("+").text_lg().color(theme.colors.text_subtle)),
        )
        .when(projects.is_empty(), |sidebar| {
            sidebar.child(
                text("No sessions")
                    .text_sm()
                    .color(theme.colors.text_subtle)
                    .px(3)
                    .py(2),
            )
        })
        .children(rows)
}

/// Builds the Sessions and Files mode switch.
fn sidebar_switch(theme: &Theme) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(38.0)
        .p(1)
        .gap(1)
        .items_stretch()
        .child(
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .rounded(theme.radius.md)
                .bg(theme.colors.surface_selected)
                .child(text("Sessions").text_sm().font_medium()),
        )
        .child(
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .rounded(theme.radius.md)
                .child(text("Files").text_sm().color(theme.colors.text_subtle)),
        )
}

/// Builds one project heading followed by its sessions.
fn project_rows(theme: &Theme, project: &SidebarProject) -> Div<Message> {
    v_flex()
        .w_full()
        .child(
            h_flex()
                .w_full()
                .h_px(30.0)
                .px(3)
                .items_center()
                .justify_between()
                .child(text(project.name.clone()).text_sm().font_semibold())
                .child(
                    text(project.branch.clone())
                        .text_xs()
                        .color(theme.colors.text_subtle),
                ),
        )
        .children(
            project
                .sessions
                .iter()
                .map(|session| session_row(theme, session)),
        )
}

/// Builds one session row.
fn session_row(theme: &Theme, session: &SidebarSession) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(32.0)
        .overflow_hidden()
        .px(3)
        .gap(2)
        .items_center()
        .when(session.selected, |row| {
            row.bg(theme.colors.surface_selected)
        })
        .child(v_flex().w_px(12.0))
        .child(state_dot(session.status_color))
        .child(text(session.name.clone()).text_sm())
        .child(h_flex().flex_1())
        .child(
            text(session.summary.clone())
                .text_xs()
                .color(theme.colors.text_subtle),
        )
}

/// Builds the state marker used by a session row.
fn state_dot(color: Rgba) -> Div<Message> {
    v_flex().size_px(7.0).rounded(4.0).bg(color)
}
