//! The editor workspace shown after onboarding has finished.

use std::collections::HashMap;
use std::path::PathBuf;

use pm_core::{FileStatus, FileTree, Project, ProjectId, Projects, Row};
use pm_gfx::{Point, Rect, Rgba};
use pm_text::Severity;
#[cfg(not(target_os = "macos"))]
use pm_ui::button;
use pm_ui::{
    Axis, Div, Element, IconName, IconSize, LayoutIcon, MenuItem, Styled, Theme, h_flex, icon,
    icon_button, layout_icon_button, menu, menu_entry, menu_separator, overlay, rule, sash, tab,
    tab_bar, text, v_flex,
};

use crate::editor::{FileId, OpenFile};
use crate::message::Message;
use crate::panes::PaneId;
use crate::terminal::{Shell, ShellEntry, ShellId, terminal_view};

/// How far the tab under the pointer sits from the pointer itself.
const CARRIED_OFFSET: f32 = 8.0;

/// How much shorter than its bar a control sitting inside one is drawn.
const BAR_INSET: f32 = 4.0;

/// Height of the line naming a project above its sessions.
const PROJECT_HEADER_HEIGHT: f32 = 30.0;

/// How far the first level of the file tree sits from the edge.
const FILE_INSET: f32 = 6.0;

/// How far each further level of the file tree is indented.
const FILE_INDENT: f32 = 14.0;

/// Diameter of the dot marking what state a worktree is in.
const DOT_SIZE: f32 = 7.0;

/// How far the halo around a lit dot reaches past it.
const DOT_HALO: f32 = 3.0;

/// Width the primary sidebar opens at, and the range it resizes within.
pub const PRIMARY_SIDEBAR_WIDTH: f32 = 252.0;

/// Smallest and largest width the primary sidebar resizes to.
pub const PRIMARY_SIDEBAR_RANGE: (f32, f32) = (160.0, 480.0);

/// Height the bottom panel opens at.
pub const BOTTOM_PANEL_HEIGHT: f32 = 220.0;

/// Smallest and largest height the bottom panel resizes to.
pub const BOTTOM_PANEL_RANGE: (f32, f32) = (120.0, 600.0);

/// Width the secondary sidebar opens at.
pub const SECONDARY_SIDEBAR_WIDTH: f32 = 252.0;

/// Smallest and largest width the secondary sidebar resizes to.
pub const SECONDARY_SIDEBAR_RANGE: (f32, f32) = (160.0, 480.0);

/// Which workspace regions are visible and how large they are.
///
/// This is what the window remembers of itself between launches, so it is
/// both what a frame is drawn from and what [`crate::config`] writes down.
#[derive(Clone, Copy, Debug)]
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

impl Default for Layout {
    /// The regions a first launch opens with.
    fn default() -> Self {
        Self {
            primary_sidebar_open: true,
            primary_sidebar_width: PRIMARY_SIDEBAR_WIDTH,
            bottom_panel_open: false,
            bottom_panel_height: BOTTOM_PANEL_HEIGHT,
            secondary_sidebar_open: true,
            secondary_sidebar_width: SECONDARY_SIDEBAR_WIDTH,
        }
    }
}

/// The worktree the files sidebar lists, and what git makes of it.
pub struct Worktree<'a> {
    /// The tree itself, when the window has a project open.
    pub tree: Option<&'a FileTree>,
    /// What git makes of each of its files, once git has been asked.
    pub status: Option<&'a HashMap<PathBuf, FileStatus>>,
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

/// What the terminal panel is showing.
///
/// The panel is one pane and a list of what else could be in it, which is one
/// thing to pass around rather than three.
pub struct Panel {
    /// The shell the pane draws, when the project has one running.
    pub shell: Option<Shell>,
    /// Every shell of the project, for the list beside the pane.
    pub shells: Vec<ShellEntry>,
    /// Whether keystrokes are going to the pane.
    pub focused: bool,
}

/// What the window's panes are showing, and what is open over them.
///
/// The panes travel together because the screen is one arrangement of them,
/// and the menu travels with them because it is opened from one of their
/// tabs and drawn over all of them. The tree of editor panes arrives built:
/// what is in a pane and which of them has the keyboard is the window's to
/// say, not the screen's.
pub struct Panes {
    /// The tree of editor panes, as the window has divided it.
    pub editor: Box<dyn Element<Message>>,
    /// The file the focused pane is showing, for the status bar to read.
    pub showing: Option<OpenFile>,
    /// The part of the window a tab being carried would take over.
    pub drop: Option<Rect>,
    /// The tab the pointer is carrying, where it is and what it is called.
    pub carried: Option<(Point, String)>,
    /// The terminal panel and the shells running in it.
    pub terminal: Panel,
    /// The tab menu that is open, and what it holds.
    pub menu: Option<(TabMenu, Vec<MenuItem<Message>>)>,
    /// What is drawn over the panes, each at a point of its own.
    ///
    /// A picker, a completion list and a hint are placed rather than laid
    /// out: they belong over whatever the window is showing, at a point the
    /// window worked out, and take no room from it.
    pub overlays: Vec<Overlaid>,
}

/// One thing drawn over the panes, and whether it is modal.
pub struct Overlaid {
    /// Where its top left corner would like to be.
    pub at: Point,
    /// What is drawn there.
    pub content: Box<dyn Element<Message>>,
    /// What a click anywhere else sends, when it is modal.
    pub backdrop: Option<Message>,
}

/// A menu of what can be done to one tab, open at a point of the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabMenu {
    /// Where the pointer was when it was asked for.
    pub at: Point,
    /// Which tab it was asked for.
    pub target: MenuTarget,
}

/// The tab a menu was opened from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuTarget {
    /// A file open in one of the editor panes.
    File(PaneId, FileId),
    /// One of the editor panes itself.
    Pane(PaneId),
    /// A shell running in the terminal panel.
    Terminal(ShellId),
    /// The text one of the editor panes is showing.
    Text(PaneId),
    /// The fixes a language server offered where the cursor is.
    CodeActions,
    /// One entry of the file tree.
    Entry(pm_core::EntryId),
    /// A file with changes that are not on disk, being closed.
    Unsaved(PaneId, FileId),
}

/// Builds the workspace with its resizable sessions sidebar.
pub fn workspace(
    theme: &Theme,
    open: &Projects,
    sessions: &[SidebarProject],
    files: Worktree<'_>,
    layout: Layout,
    panes: Panes,
) -> Div<Message> {
    let status = Status::of(open, sessions, &panes, layout);
    let Panes {
        editor,
        drop,
        carried,
        terminal: panel,
        menu: open_menu,
        overlays,
        ..
    } = panes;

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
                .child(main_area(theme, layout, panel, editor))
                .when(layout.secondary_sidebar_open, |body| {
                    body.child(sash(Axis::Horizontal, Message::ResizeSecondarySidebar))
                        .child(files_sidebar(theme, &files, layout.secondary_sidebar_width))
                }),
        )
        .child(rule(theme))
        .child(status_bar(theme, status))
        .when_some(drop, |screen, area| {
            screen.child(overlay(area.origin, drop_area(theme, area)))
        })
        .when_some(carried, |screen, (at, name)| {
            screen.child(overlay(
                Point::new(at.x + CARRIED_OFFSET, at.y + CARRIED_OFFSET),
                carried_tab(theme, name),
            ))
        })
        .children(overlays.into_iter().flat_map(|overlaid| {
            let Overlaid {
                at,
                content,
                backdrop: sheet,
            } = overlaid;
            let sheet = sheet.map(|message| overlay(Point::new(0.0, 0.0), backdrop(message)));
            sheet.into_iter().chain([overlay(at, content)])
        }))
        .when_some(open_menu, |screen, (open, items)| {
            screen
                .child(overlay(
                    Point::new(0.0, 0.0),
                    backdrop(Message::DismissMenu),
                ))
                .child(overlay(open.at, menu(theme, items)))
        })
}

/// Builds the wash over the part of the window a carried tab would take.
///
/// The wash is the answer to "where would this land": the whole of a pane it
/// would join, the half it would divide off, or the hairline between two tabs
/// it would be dropped between.
fn drop_area(theme: &Theme, area: Rect) -> Div<Message> {
    v_flex()
        .w_px(area.size.width)
        .h_px(area.size.height)
        .bg(theme.colors.drop_target)
        .border_2(theme.colors.text_muted)
}

/// Builds the tab drawn under the pointer while it carries one.
fn carried_tab(theme: &Theme, name: String) -> Div<Message> {
    h_flex()
        .px(1.5)
        .h_px(theme.size.tab_bar - BAR_INSET)
        .items_center()
        .gap(1)
        .rounded(theme.radius.md)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border_focused)
        .child(
            icon(IconName::File)
                .size(IconSize::Medium)
                .color(theme.colors.text_subtle),
        )
        .child(text(name).text_sm().font_light())
}

/// Builds the sheet under an open menu, which puts it away when clicked.
///
/// The sheet is what makes a menu modal: it covers the window, so the click
/// that dismisses the menu is not also the click that pressed whatever was
/// underneath it.
fn backdrop(message: Message) -> Div<Message> {
    v_flex()
        .w_full()
        .h_full()
        .on_click(message)
        .on_secondary_click(message)
}

/// The things that can be done to one shell's tab.
pub fn terminal_menu(shells: &[ShellEntry], id: ShellId) -> Vec<MenuItem<Message>> {
    let others = shells.len() > 1;

    vec![
        menu_entry("Close", Some(Message::CloseTerminal(id))),
        menu_entry(
            "Close Others",
            others.then_some(Message::CloseOtherTerminals(id)),
        ),
        menu_separator(),
        menu_entry("Close All", Some(Message::CloseAllTerminals)),
    ]
}

/// What the status bar reports about the window as it stands.
///
/// The bar states the window's own situation — which worktree it is pointed
/// at and what is running in it — so it is read off the same model the rest
/// of the screen is drawn from rather than kept alongside it.
struct Status {
    /// Name of the active project, when the window has one.
    project: Option<String>,
    /// Branch the active project's worktree is on.
    branch: Option<String>,
    /// How many sessions that project has.
    sessions: usize,
    /// How many shells are running in the worktree.
    shells: usize,
    /// Whether the panel those shells are shown in is open.
    panel_open: bool,
    /// Where the cursor is in the file the pane is showing.
    cursor: Option<(usize, usize)>,
    /// How many cursors that file has, when it has more than one.
    cursors: Option<usize>,
    /// How that file is indented.
    indent: Option<String>,
    /// What that file is written in.
    language: Option<&'static str>,
    /// How many errors and warnings a server has reported in it.
    problems: (usize, usize),
}

impl Status {
    /// Reads the status of the window out of what the screen was given.
    fn of(open: &Projects, sessions: &[SidebarProject], panes: &Panes, layout: Layout) -> Self {
        let active = open.active();
        let showing = panes.showing.as_ref().map(|file| file.borrow());
        let buffer = showing.as_ref().map(|document| document.buffer());

        Self {
            project: active.map(|project| project.name().to_owned()),
            branch: active.map(|project| project.branch().to_owned()),
            sessions: active.map_or(0, |project| sessions_of(project, sessions).len()),
            shells: panes.terminal.shells.len(),
            panel_open: layout.bottom_panel_open,
            cursor: buffer.map(|buffer| {
                let head = buffer.selection().head;
                (head.line + 1, head.column + 1)
            }),
            cursors: buffer
                .map(|buffer| buffer.selections().len())
                .filter(|count| *count > 1),
            indent: buffer.map(|buffer| {
                let indent = buffer.indent();
                match indent.tabs {
                    true => "Tabs".to_owned(),
                    false => format!("Spaces: {}", indent.width),
                }
            }),
            language: buffer.map(|buffer| {
                buffer
                    .language()
                    .map_or("Plain Text", pm_text::Language::name)
            }),
            problems: buffer.map_or((0, 0), |buffer| {
                let errors = buffer
                    .diagnostics()
                    .iter()
                    .filter(|found| found.severity == Severity::Error)
                    .count();
                let warnings = buffer
                    .diagnostics()
                    .iter()
                    .filter(|found| found.severity == Severity::Warning)
                    .count();
                (errors, warnings)
            }),
        }
    }
}

/// Builds the bar along the bottom of the window.
///
/// The bar spans everything — sidebars, panel and panes alike — because what
/// it states is the window's, not one region's: the worktree the window is
/// pointed at on the left, what is running in it on the right.
fn status_bar(theme: &Theme, status: Status) -> Div<Message> {
    let Status {
        project,
        branch,
        sessions,
        shells,
        panel_open,
        cursor,
        cursors,
        indent,
        language,
        problems,
    } = status;

    h_flex()
        .w_full()
        .h_px(theme.size.bar)
        .px(0.5)
        .gap(0.5)
        .items_center()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .when(project.is_none(), |bar| {
            bar.child(status_item(
                theme,
                Some(IconName::Folder),
                "No project open",
                None,
                false,
            ))
        })
        .when_some(project, |bar, name| {
            bar.child(status_item(
                theme,
                Some(IconName::Folder),
                name,
                None,
                false,
            ))
        })
        .when_some(branch, |bar, branch| {
            bar.child(status_item(
                theme,
                Some(IconName::GitBranch),
                branch,
                None,
                false,
            ))
        })
        .when(sessions > 0, |bar| {
            bar.child(status_item(
                theme,
                Some(IconName::GitFork),
                counted(sessions, "session"),
                None,
                false,
            ))
        })
        .when(problems != (0, 0), |bar| {
            bar.child(status_item(
                theme,
                Some(IconName::Warning),
                format!("{} · {}", problems.0, problems.1),
                None,
                false,
            ))
        })
        .child(h_flex().flex_1())
        .when_some(cursor, |bar, (line, column)| {
            bar.child(status_item(
                theme,
                None,
                format!("Ln {line}, Col {column}"),
                None,
                false,
            ))
        })
        .when_some(cursors, |bar, count| {
            bar.child(status_item(
                theme,
                None,
                format!("{count} cursors"),
                None,
                true,
            ))
        })
        .when_some(indent, |bar, indent| {
            bar.child(status_item(theme, None, indent, None, false))
        })
        .when_some(language, |bar, language| {
            bar.child(status_item(theme, None, language, None, false))
        })
        .child(status_item(
            theme,
            Some(IconName::Terminal),
            counted(shells, "shell"),
            Some(Message::ToggleBottomPanel),
            panel_open,
        ))
}

/// Builds one reading in the status bar: its icon, its text, its action.
///
/// An item that carries a `message` is a control and lights under the
/// pointer; one without is a reading and stays where the eye left it. The
/// icon is as optional as the action: a line and column number is already
/// named by the numbers themselves.
fn status_item(
    theme: &Theme,
    glyph: Option<IconName>,
    label: impl Into<String>,
    message: Option<Message>,
    active: bool,
) -> Div<Message> {
    let color = if active {
        theme.colors.text
    } else {
        theme.colors.text_muted
    };

    h_flex()
        .h_px(theme.size.bar - BAR_INSET)
        .px(1)
        .gap(0.75)
        .items_center()
        .overflow_hidden()
        .rounded(theme.radius.md)
        .when_some(message, |item, message| {
            item.hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(message)
        })
        .when_some(glyph, |item, glyph| {
            item.child(icon(glyph).size(IconSize::XSmall).color(color))
        })
        .child(text(label.into()).text_xs().font_light().color(color))
}

/// `count` written out with `noun`, pluralized the way English does it.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Builds the window bar above every project and pane.
fn titlebar(theme: &Theme, layout: Layout) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.titlebar)
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
        .child(v_flex().w(2.5))
        .child(window_controls())
}

/// Builds the central pane area and optional bottom panel.
fn main_area(
    theme: &Theme,
    layout: Layout,
    panel: Panel,
    editor: Box<dyn Element<Message>>,
) -> Div<Message> {
    v_flex()
        .flex_1()
        .h_full()
        .child(v_flex().w_full().flex_1().overflow_hidden().child(editor))
        .when(layout.bottom_panel_open, |main| {
            main.child(sash(Axis::Vertical, Message::ResizeBottomPanel))
                .child(terminal_panel(theme, layout.bottom_panel_height, panel))
        })
}

/// Builds the bottom panel: the shells of the worktree the window is pointed at.
///
/// A terminal is a tab in a bar of tabs with the pane beneath it, which is
/// what every other pane in the window will look like: the panel is where
/// terminals happen to live today, not a dock of its own with its own rules.
fn terminal_panel(theme: &Theme, height: f32, panel: Panel) -> Div<Message> {
    let Panel {
        shell,
        shells,
        focused,
    } = panel;
    let missing = shell.is_none();

    v_flex()
        .w_full()
        .h_px(height)
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(terminal_tabs(theme, &shells))
        .when_some(shell, |panel, shell| {
            panel.child(
                terminal_view(shell, focused, Message::FocusTerminal)
                    .on_scroll(Message::ScrollTerminal),
            )
        })
        .when(missing, |panel| {
            panel.child(
                text("No shell is running in this worktree")
                    .text_sm()
                    .font_light()
                    .color(theme.colors.text_subtle)
                    .px(2)
                    .py(1.5),
            )
        })
}

/// Builds the terminal panel's bar of tabs and its own actions.
fn terminal_tabs(theme: &Theme, shells: &[ShellEntry]) -> Div<Message> {
    let tabs = shells
        .iter()
        .map(|shell| {
            tab(
                IconName::Terminal,
                shell.name.clone(),
                Message::SelectTerminal(shell.id),
                Message::CloseTerminal(shell.id),
                Message::ShowTerminalMenu(shell.id),
            )
            .active(shell.active)
        })
        .collect();
    let actions = h_flex()
        .h_full()
        .px(1.5)
        .gap(1)
        .items_center()
        .child(icon_button(theme, IconName::Plus, Message::NewTerminal))
        .child(icon_button(
            theme,
            IconName::Close,
            Message::ToggleBottomPanel,
        ));

    tab_bar(theme, tabs, actions)
}

/// Builds native-style controls for undecorated Linux and Windows windows.
#[cfg(not(target_os = "macos"))]
fn window_controls() -> Div<Message> {
    h_flex()
        .h_full()
        .child(button("—", Message::MinimizeWindow).ghost().w(10).h_full())
        .child(
            button("□", Message::ToggleMaximizedWindow)
                .ghost()
                .w(10)
                .h_full(),
        )
        .child(button("×", Message::CloseWindow).ghost().w(10).h_full())
}

/// Leaves window controls to macOS traffic lights.
#[cfg(target_os = "macos")]
fn window_controls() -> Div<Message> {
    h_flex()
}

/// Builds the files sidebar: the worktree of the active project.
fn files_sidebar(theme: &Theme, files: &Worktree<'_>, width: f32) -> Div<Message> {
    let rows = files.tree.map(FileTree::rows).unwrap_or_default();

    v_flex()
        .w_px(width)
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .when_some(files.tree, |sidebar, tree| {
            sidebar.child(tree_root(theme, tree))
        })
        .when(files.tree.is_none(), |sidebar| {
            sidebar.child(
                text("No project open")
                    .text_sm()
                    .font_light()
                    .color(theme.colors.text_subtle)
                    .px(3)
                    .py(2),
            )
        })
        .children(rows.iter().map(|row| {
            let status = files
                .status
                .and_then(|status| status.get(row.entry.path()))
                .copied();
            file_row(theme, row, status)
        }))
}

/// Builds the line above the tree naming the worktree it lists.
fn tree_root(theme: &Theme, files: &FileTree) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
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
    shortened(files.root())
}

/// `path` written the way a prompt writes it, against the home directory.
fn shortened(path: &std::path::Path) -> String {
    let path = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => path.replacen(&home, "~", 1),
        _ => path,
    }
}

/// Builds one line of the file tree: chevron, icon and name.
fn file_row(theme: &Theme, row: &Row<'_>, status: Option<FileStatus>) -> Div<Message> {
    let entry = row.entry;
    let directory = entry.is_directory();
    let chevron = match (directory, row.expanded) {
        (false, _) => None,
        (true, true) => Some(IconName::ChevronDown),
        (true, false) => Some(IconName::ChevronRight),
    };
    let glyph = match (directory, row.expanded) {
        (false, _) => IconName::File,
        (true, true) => IconName::FolderOpen,
        (true, false) => IconName::Folder,
    };

    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .overflow_hidden()
        .gap(0.5)
        .items_center()
        .hover_bg(theme.colors.surface_hover)
        .on_click(if directory {
            Message::ToggleEntry(entry.id())
        } else {
            Message::OpenFile(entry.id())
        })
        .on_secondary_click(Message::ShowEntryMenu(entry.id()))
        .child(v_flex().w_px(FILE_INSET + row.depth as f32 * FILE_INDENT))
        .child(
            h_flex()
                .w_px(IconSize::Medium.pixels())
                .items_center()
                .justify_center()
                .when_some(chevron, |slot, chevron| {
                    slot.child(
                        icon(chevron)
                            .size(IconSize::Medium)
                            .color(theme.colors.text_subtle),
                    )
                }),
        )
        .child(
            icon(glyph)
                .size(IconSize::Medium)
                .color(theme.colors.text_subtle),
        )
        .child(v_flex().w(1))
        .child(match (status_color(theme, status), directory) {
            (Some(color), _) => text(entry.name().to_owned()).color(color),
            (None, true) => text(entry.name().to_owned()),
            (None, false) => text(entry.name().to_owned()).color(theme.colors.text_muted),
        })
}

/// The colour a name is written in, given what git makes of it.
fn status_color(theme: &Theme, status: Option<FileStatus>) -> Option<Rgba> {
    Some(match status? {
        FileStatus::Modified => theme.colors.warning,
        FileStatus::Added | FileStatus::Untracked => theme.colors.success,
        FileStatus::Deleted => theme.colors.text_subtle,
        FileStatus::Conflicted => theme.colors.danger,
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
        .size_px(theme.size.icon_control)
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
                .h_px(PROJECT_HEADER_HEIGHT)
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
        .size_px(theme.size.icon_control)
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
        .h_px(theme.size.field)
        .overflow_hidden()
        .px(3)
        .gap(2)
        .items_center()
        .when(active, |row| row.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ActivateProject(project.id()))
        .child(v_flex().w(3))
        .child(state_dot(
            theme,
            if active {
                theme.colors.success
            } else {
                theme.colors.text_subtle
            },
        ))
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
        .h_px(theme.size.field)
        .overflow_hidden()
        .px(3)
        .gap(2)
        .items_center()
        .when(session.selected, |row| {
            row.bg(theme.colors.surface_selected)
        })
        .child(v_flex().w(3))
        .child(state_dot(theme, session.status_color))
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
fn state_dot(theme: &Theme, color: Rgba) -> Div<Message> {
    v_flex()
        .size_px(DOT_SIZE + DOT_HALO * 2.0)
        .items_center()
        .justify_center()
        .rounded(theme.radius.full)
        .bg(color.alpha(theme.emphasis.halo))
        .child(
            v_flex()
                .size_px(DOT_SIZE)
                .rounded(theme.radius.full)
                .bg(color),
        )
}
