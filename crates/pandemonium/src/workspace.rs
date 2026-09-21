//! The editor workspace shown after onboarding has finished.

use pm_gfx::Rgba;
#[cfg(not(target_os = "macos"))]
use pm_ui::button;
use pm_ui::{Axis, Div, LayoutIcon, Styled, Theme, h_flex, layout_icon_button, sash, text, v_flex};

use crate::onboarding::Message;

/// Height of the content-backed window title bar.
pub const TITLEBAR_HEIGHT: f32 = 40.0;

/// Which workspace regions are visible and how large they are.
#[derive(Clone, Copy)]
pub struct Layout {
    /// Whether the primary sidebar is visible.
    pub primary_sidebar_open: bool,
    /// Width of the primary sidebar.
    pub primary_sidebar_width: f32,
    /// Whether the bottom panel is visible.
    pub bottom_panel_open: bool,
    /// Height of the bottom panel.
    pub bottom_panel_height: f32,
    /// Whether the secondary sidebar is visible.
    pub secondary_sidebar_open: bool,
    /// Width of the secondary sidebar.
    pub secondary_sidebar_width: f32,
}

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
pub fn workspace(theme: &Theme, projects: &[SidebarProject], layout: Layout) -> Div<Message> {
    v_flex()
        .w_full()
        .h_full()
        .child(titlebar(theme, layout))
        .child(
            h_flex()
                .w_full()
                .flex_1()
                .items_stretch()
                .when(layout.primary_sidebar_open, |body| {
                    body.child(sessions_sidebar(
                        theme,
                        projects,
                        layout.primary_sidebar_width,
                    ))
                    .child(sash(Axis::Horizontal, Message::ResizeSidebar))
                })
                .child(main_area(theme, layout))
                .when(layout.secondary_sidebar_open, |body| {
                    body.child(sash(Axis::Horizontal, Message::ResizeSecondarySidebar))
                        .child(
                            v_flex()
                                .w_px(layout.secondary_sidebar_width)
                                .h_full()
                                .bg(theme.colors.surface),
                        )
                }),
        )
}

/// Builds the window bar above every project and pane.
fn titlebar(theme: &Theme, layout: Layout) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(TITLEBAR_HEIGHT)
        .items_center()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .child(h_flex().flex_1())
        .child(
            h_flex()
                .gap(1)
                .items_center()
                .child(layout_icon_button(
                    LayoutIcon::PrimarySidebar,
                    layout.primary_sidebar_open,
                    Message::TogglePrimarySidebar,
                ))
                .child(layout_icon_button(
                    LayoutIcon::BottomPanel,
                    layout.bottom_panel_open,
                    Message::ToggleBottomPanel,
                ))
                .child(layout_icon_button(
                    LayoutIcon::SecondarySidebar,
                    layout.secondary_sidebar_open,
                    Message::ToggleSecondarySidebar,
                )),
        )
        .child(v_flex().w_px(10.0))
        .child(window_controls())
}

/// Builds the central pane area and optional bottom panel.
fn main_area(theme: &Theme, layout: Layout) -> Div<Message> {
    v_flex()
        .flex_1()
        .h_full()
        .child(v_flex().w_full().flex_1().bg(theme.colors.background))
        .when(layout.bottom_panel_open, |main| {
            main.child(sash(Axis::Vertical, Message::ResizeBottomPanel))
                .child(
                    v_flex()
                        .w_full()
                        .h_px(layout.bottom_panel_height)
                        .bg(theme.colors.surface),
                )
        })
}

/// Builds native-style controls for undecorated Linux and Windows windows.
#[cfg(not(target_os = "macos"))]
fn window_controls() -> Div<Message> {
    h_flex()
        .h_full()
        .child(
            button("—", Message::MinimizeWindow)
                .ghost()
                .w_px(40.0)
                .h_full(),
        )
        .child(
            button("□", Message::ToggleMaximizedWindow)
                .ghost()
                .w_px(40.0)
                .h_full(),
        )
        .child(
            button("×", Message::CloseWindow)
                .ghost()
                .w_px(40.0)
                .h_full(),
        )
}

/// Leaves window controls to macOS traffic lights.
#[cfg(target_os = "macos")]
fn window_controls() -> Div<Message> {
    h_flex()
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
                .child(
                    text("SESSIONS")
                        .text_xs()
                        .font_light()
                        .color(theme.colors.text_subtle),
                )
                .child(
                    text("+")
                        .text_lg()
                        .font_light()
                        .color(theme.colors.text_subtle),
                ),
        )
        .when(projects.is_empty(), |sidebar| {
            sidebar.child(
                text("No sessions")
                    .text_sm()
                    .font_light()
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
                .child(text("Sessions").text_sm().font_light()),
        )
        .child(
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .rounded(theme.radius.md)
                .child(
                    text("Files")
                        .text_sm()
                        .font_light()
                        .color(theme.colors.text_subtle),
                ),
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
                .child(text(project.name.clone()).text_sm().font_light())
                .child(
                    text(project.branch.clone())
                        .text_xs()
                        .font_light()
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
        .child(text(session.name.clone()).text_sm().font_light())
        .child(h_flex().flex_1())
        .child(
            text(session.summary.clone())
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// Builds the state marker used by a session row.
fn state_dot(color: Rgba) -> Div<Message> {
    v_flex().size_px(7.0).rounded(4.0).bg(color)
}
