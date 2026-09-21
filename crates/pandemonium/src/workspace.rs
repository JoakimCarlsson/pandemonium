//! The editor workspace shown after onboarding has finished.

use pm_core::{FileTree, Project, ProjectId, Projects, Row};
use pm_gfx::Rgba;
#[cfg(not(target_os = "macos"))]
use pm_ui::button;
use pm_ui::{
    Axis, Div, LayoutIcon, Styled, Theme, TreeIcon, h_flex, layout_icon_button, sash, text,
    tree_icon, v_flex,
};

use crate::onboarding::Message;

/// Height of the content-backed window title bar.
pub const TITLEBAR_HEIGHT: f32 = 40.0;

/// Height of one line of the file tree.
const FILE_ROW_HEIGHT: f32 = 26.0;

/// How far the first level of the file tree sits from the edge.
const FILE_INSET: f32 = 6.0;

/// How far each further level of the file tree is indented.
const FILE_INDENT: f32 = 14.0;

/// Diameter of the dot marking what state a worktree is in.
const DOT_SIZE: f32 = 7.0;

/// How far the halo around a lit dot reaches past it.
const DOT_HALO: f32 = 3.0;

/// How much of its colour the halo around a lit dot carries.
const HALO_STRENGTH: f32 = 0.22;

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

/// The sessions belonging to one open project.
///
/// The project itself comes from [`Projects`]; this is only what hangs under
/// it, so the sidebar draws one list rather than reconciling two.
pub struct SidebarProject {
    /// Which project these sessions belong to.
    pub project: ProjectId,
    /// Sessions belonging to that project.
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
pub fn workspace(
    theme: &Theme,
    open: &Projects,
    sessions: &[SidebarProject],
    files: Option<&FileTree>,
    layout: Layout,
) -> Div<Message> {
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
                    body.child(projects_sidebar(
                        theme,
                        open,
                        sessions,
                        layout.primary_sidebar_width,
                    ))
                    .child(sash(Axis::Horizontal, Message::ResizeSidebar))
                })
                .child(main_area(theme, layout))
                .when(layout.secondary_sidebar_open, |body| {
                    body.child(sash(Axis::Horizontal, Message::ResizeSecondarySidebar))
                        .child(files_sidebar(theme, files, layout.secondary_sidebar_width))
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

/// Builds the files sidebar: the worktree of the active project.
fn files_sidebar(theme: &Theme, files: Option<&FileTree>, width: f32) -> Div<Message> {
    let rows = files.map(FileTree::rows).unwrap_or_default();

    v_flex()
        .w_px(width)
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .when_some(files, |sidebar, tree| sidebar.child(tree_root(theme, tree)))
        .when(files.is_none(), |sidebar| {
            sidebar.child(
                text("No project open")
                    .text_sm()
                    .font_light()
                    .color(theme.colors.text_subtle)
                    .px(3)
                    .py(2),
            )
        })
        .children(rows.iter().map(|row| file_row(theme, row)))
}

/// Builds the line above the tree naming the worktree it lists.
fn tree_root(theme: &Theme, files: &FileTree) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(26.0)
        .px(1.5)
        .items_center()
        .overflow_hidden()
        .child(
            text(root_label(files))
                .text_sm()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
}

/// The worktree's path, shortened against the home directory.
fn root_label(files: &FileTree) -> String {
    let path = files.root().display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => path.replacen(&home, "~", 1),
        _ => path,
    }
}

/// Builds one line of the file tree: chevron, icon and name.
fn file_row(theme: &Theme, row: &Row<'_>) -> Div<Message> {
    let entry = row.entry;
    let directory = entry.is_directory();
    let chevron = match (directory, row.expanded) {
        (false, _) => TreeIcon::Blank,
        (true, true) => TreeIcon::Expanded,
        (true, false) => TreeIcon::Collapsed,
    };
    let glyph = if directory {
        TreeIcon::Folder { open: row.expanded }
    } else {
        TreeIcon::File
    };

    h_flex()
        .w_full()
        .h_px(FILE_ROW_HEIGHT)
        .overflow_hidden()
        .gap(0.5)
        .items_center()
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ToggleEntry(entry.id()))
        .child(v_flex().w_px(FILE_INSET + row.depth as f32 * FILE_INDENT))
        .child(tree_icon(chevron, theme.colors.text_subtle))
        .child(tree_icon(glyph, theme.colors.text_subtle))
        .child(v_flex().w_px(4.0))
        .child(if directory {
            text(entry.name().to_owned())
        } else {
            text(entry.name().to_owned()).color(theme.colors.text_muted)
        })
}

/// Builds the projects sidebar: every open project, its sessions beneath it.
fn projects_sidebar(
    theme: &Theme,
    open: &Projects,
    sessions: &[SidebarProject],
    width: f32,
) -> Div<Message> {
    let active = open.active().map(Project::id);
    let rows = open
        .iter()
        .map(|project| {
            project_rows(
                theme,
                project,
                sessions_of(project, sessions),
                active == Some(project.id()),
            )
        })
        .collect::<Vec<_>>();

    v_flex()
        .w_px(width)
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .child(
            h_flex()
                .w_full()
                .px(3)
                .pt(2)
                .pb(1.5)
                .items_center()
                .justify_between()
                .child(
                    text("PROJECTS")
                        .text_xs()
                        .font_light()
                        .color(theme.colors.text_subtle),
                )
                .child(add_project(theme)),
        )
        .when(open.is_empty(), |sidebar| {
            sidebar.child(
                text("No projects open")
                    .text_sm()
                    .font_light()
                    .color(theme.colors.text_subtle)
                    .px(3)
                    .py(2),
            )
        })
        .children(rows)
}

/// Builds the control that asks for another repository to open.
fn add_project(theme: &Theme) -> Div<Message> {
    v_flex()
        .size_px(20.0)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(Message::OpenProject)
        .child(
            text("+")
                .text_lg()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// The sessions listed under `project`, of which there may be none.
fn sessions_of<'a>(project: &Project, sessions: &'a [SidebarProject]) -> &'a [SidebarSession] {
    sessions
        .iter()
        .find(|entry| entry.project == project.id())
        .map_or(&[], |entry| entry.sessions.as_slice())
}

/// Builds one project: its heading, its checkout, then its sessions.
///
/// The checkout comes first because it is the worktree the project was opened
/// from — the repository itself, which the sessions are worktrees beside.
/// Clicking either it or the heading points the window's files at it.
fn project_rows(
    theme: &Theme,
    project: &Project,
    sessions: &[SidebarSession],
    active: bool,
) -> Div<Message> {
    v_flex()
        .w_full()
        .child(
            h_flex()
                .w_full()
                .h_px(30.0)
                .pl(3)
                .pr(1.5)
                .items_center()
                .child(text(project.name().to_owned()).text_sm().font_light())
                .child(h_flex().flex_1())
                .child(project_menu(theme, project)),
        )
        .child(checkout_row(theme, project, active))
        .children(sessions.iter().map(|session| session_row(theme, session)))
}

/// Builds the control that opens what can be done to one project.
///
/// The menu behind it is not built yet; the control is here because this is
/// where it belongs, and it will send the same message when it is.
fn project_menu(theme: &Theme, project: &Project) -> Div<Message> {
    v_flex()
        .size_px(20.0)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(Message::ProjectMenu(project.id()))
        .child(
            text("⋯")
                .text_lg()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// Builds the row for the project's own checkout.
fn checkout_row(theme: &Theme, project: &Project, active: bool) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(32.0)
        .overflow_hidden()
        .px(3)
        .gap(2)
        .items_center()
        .when(active, |row| row.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ActivateProject(project.id()))
        .child(v_flex().w_px(12.0))
        .child(state_dot(if active {
            theme.colors.success
        } else {
            theme.colors.text_subtle
        }))
        .child(text(project.branch().to_owned()).text_sm().font_light())
        .child(h_flex().flex_1())
        .child(
            text("checkout")
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
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

/// Builds the state marker used by a checkout or session row.
///
/// The halo is the artifact's: a ring of the same colour at a fifth of its
/// strength, which is what makes a live state read as lit rather than printed.
fn state_dot(color: Rgba) -> Div<Message> {
    v_flex()
        .size_px(DOT_SIZE + DOT_HALO * 2.0)
        .items_center()
        .justify_center()
        .rounded(DOT_SIZE / 2.0 + DOT_HALO)
        .bg(color.alpha(HALO_STRENGTH))
        .child(v_flex().size_px(DOT_SIZE).rounded(DOT_SIZE / 2.0).bg(color))
}
