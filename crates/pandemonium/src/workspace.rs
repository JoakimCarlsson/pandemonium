//! The editor workspace shown after onboarding has finished.

use pm_core::{Project, ProjectId, Projects, Scope, SessionId};
use pm_gfx::{Point, Rect, Rgba};
use pm_text::Severity;
use pm_ui::{
    Axis, Bounds, Div, Element, IconName, IconSize, LayoutIcon, MenuItem, Styled, Text, Theme,
    button, h_flex, icon, icon_button, layout_icon_button, measured, menu, menu_entry,
    menu_separator, overlay, overlay_above, rule, sash, text, v_flex, view_tab,
};

use crate::agent::{Standing, Tally, standing_color};
use crate::editor::{FileId, OpenFile};
use crate::keymap::Action;
use crate::message::Message;
use crate::notice::{Shown, Tone};
use crate::panel::{Panel, PanelView, bottom_panel};
use crate::panes::{Item, PaneId};
use crate::review::{Review, SourceControlControls, changes_sidebar};
use crate::terminal::{ShellEntry, ShellId};

/// How far the tab under the pointer sits from the pointer itself.
const CARRIED_OFFSET: f32 = 8.0;

/// Widest the command center is drawn: as wide as the picker it opens, so
/// the picker lands over it.
const COMMAND_CENTER_WIDTH: f32 = crate::picker::WIDTH;

/// Narrowest the command center is squeezed to in a narrow window.
const COMMAND_CENTER_MIN_WIDTH: f32 = 160.0;

/// How tall the command center is drawn inside the title bar.
const COMMAND_CENTER_HEIGHT: f32 = 26.0;

/// How much shorter than its bar a control sitting inside one is drawn.
const BAR_INSET: f32 = 4.0;

/// How far a session row sits in from the project row above it.
const SESSION_INDENT: f32 = 12.0;

/// Maximum branch reading width.
const PROJECT_READING_WIDTH: f32 = 96.0;

/// Width of the bar marking the row the window is pointed at.
const MARKER_WIDTH: f32 = 2.0;

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

/// Height the Source Control graph opens at.
pub const HISTORY_GRAPH_HEIGHT: f32 = 190.0;

/// Smallest and largest height the Source Control graph resizes to.
pub const HISTORY_GRAPH_RANGE: (f32, f32) = (80.0, 520.0);

/// Height an agent's prompt box opens at: three lines of text.
pub const PROMPT_HEIGHT: f32 = 84.0;

/// Smallest and largest height an agent's prompt box resizes to.
pub const PROMPT_RANGE: (f32, f32) = (28.0, 560.0);

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
    /// Which of the worktree's two lists that sidebar is showing.
    pub secondary_sidebar_view: SidebarView,
    /// Height of the Source Control graph.
    pub history_graph_height: f32,
    /// Whether the Source Control graph is visible.
    pub history_graph_open: bool,
    /// Whether the Source Control changes section is expanded.
    pub changes_section_open: bool,
    /// Whether the Graph includes every history reference.
    pub history_all: bool,
    /// Height of the box an agent's prompt is written in.
    pub prompt_height: f32,
}

/// What the sidebar beside the panes is listing.
///
/// The worktree is one thing looked at two ways: the files it holds, and
/// what has changed in them. They share a sidebar because they are both the
/// worktree, and a reader is looking at one or the other.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SidebarView {
    /// Every file of the worktree.
    #[default]
    Files,
    /// Everything that has changed in it.
    Changes,
}

impl SidebarView {
    /// Both views, in the order the switch offers them.
    pub const ALL: [Self; 2] = [Self::Files, Self::Changes];

    /// What the switch calls this view.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Changes => "Changes",
        }
    }
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
            secondary_sidebar_view: SidebarView::default(),
            history_graph_height: HISTORY_GRAPH_HEIGHT,
            history_graph_open: true,
            changes_section_open: true,
            history_all: false,
            prompt_height: PROMPT_HEIGHT,
        }
    }
}

/// The worktree the sidebar beside the panes lists, either way it lists it.
pub struct Worktree<'a> {
    /// The tree itself, when the window has a project open.
    pub listing: Option<crate::tree::Listing<'a>>,
    /// What has changed in it, once git has been asked.
    pub review: Option<&'a Review>,
    /// Whether the commit message is where keystrokes are going.
    pub committing: bool,
    /// Whether the focused input caret is in its visible blink phase.
    pub caret: bool,
    /// Where the Source Control commit split button was drawn last frame.
    pub commit_bounds: Bounds,
    /// Where the Graph reference filter was drawn last frame.
    pub history_refs_bounds: Bounds,
    /// Bounds of the Graph panel from the last frame.
    pub history_graph_bounds: Bounds,
    /// Where the Source Control list of changes was drawn last frame.
    pub changes_area: Bounds,
    /// Whether the Graph shows every history reference.
    pub history_all: bool,
    /// Height of the Source Control graph.
    pub history_graph_height: f32,
    /// Whether the Source Control graph is visible.
    pub history_graph_open: bool,
    /// Whether the Source Control changes section is expanded.
    pub changes_section_open: bool,
}

/// The projects the sidebar lists, and where its rows came out last frame.
pub struct ProjectList<'a> {
    /// Every project the window holds open, in the order they are listed.
    pub open: &'a Projects,
    /// The sessions hanging under each of them.
    pub sessions: &'a [SidebarProject],
    /// Where the rows were drawn, for a project carried up or down them.
    pub bounds: Bounds,
}

/// The sessions belonging to one open project.
///
/// The project itself comes from [`Projects`]; this is only what hangs under
/// it, so the sidebar draws one list rather than reconciling two.
pub struct SidebarProject {
    /// Which project these sessions belong to.
    pub project: ProjectId,
    /// Whether the window is pointed at the project's own checkout.
    pub at_checkout: bool,
    /// Sessions belonging to that project.
    pub sessions: Vec<SidebarSession>,
}

/// One session as presented by the workspace model.
pub struct SidebarSession {
    /// Which session the row is of.
    pub id: SessionId,
    /// Human-readable name of the work.
    pub name: String,
    /// How many lines its worktree has added since it was cut.
    pub added: usize,
    /// How many lines its worktree has taken out since it was cut.
    pub removed: usize,
    /// Colour representing the state reported by the agent.
    pub status_color: Rgba,
    /// How many review comments on its worktree are waiting to be sent.
    pub pending: usize,
    /// Whether this session is selected.
    pub selected: bool,
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
    /// While modal editing is on, the register being recorded into, if any.
    pub recording: Option<Option<char>>,
    /// The part of the window a tab being carried would take over.
    pub drop: Option<Rect>,
    /// The tab the pointer is carrying, where it is and what it is called.
    pub carried: Option<(Point, String)>,
    /// The bottom panel and the views in it.
    pub panel: Panel,
    /// How many agents are running in the active project's worktree.
    pub agents: usize,
    /// How every agent in the window stands, across all its projects.
    pub tally: Tally,
    /// The newest thing the reader is being told about, if anything.
    pub notice: Option<Shown>,
    /// What a language server behind the focused file says it is working on.
    pub activity: Option<String>,
    /// The focused buffer's configured server with the worst lifecycle state.
    pub server: Option<pm_text::ServerStatus>,
    /// The current rotation of the language server spinner.
    pub server_turn: f32,
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
    /// Whether the panel grows upward from `at`.
    pub above: bool,
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
    /// One tab of one of the editor panes.
    Tab(PaneId, Item),
    /// One of the editor panes itself.
    Pane(PaneId),
    /// The control that adds another project to the window.
    Projects,
    /// One of the projects the window holds open.
    Project(ProjectId),
    /// One of the sessions hanging under one of them.
    Session(SessionId),
    /// A shell running in the bottom panel.
    Terminal(ShellId),
    /// What the bottom panel's shell is showing.
    Screen,
    /// The text one of the editor panes is showing.
    Text(PaneId),
    /// The breakpoint column of a pane at a source line.
    Breakpoint(PaneId, usize),
    /// The box of text that is being written in.
    Input,
    /// The transcript of one agent session.
    AgentText(crate::agent::TalkId, Option<usize>),
    /// The MCP servers one agent session was opened with.
    AgentMcp(crate::agent::TalkId),
    /// One of the agents the reader added, by its place in the list.
    AgentServer(usize),
    /// One of the tool servers every agent is started with, by its place in the list.
    McpServer(usize),
    /// The fixes a language server offered where the cursor is.
    CodeActions,
    /// One entry of the file tree, and whatever is selected with it.
    Entry(pm_core::EntryId),
    /// The file tree itself, from the space below its rows.
    Tree,
    /// A file with changes that are not on disk, being closed.
    Unsaved(PaneId, FileId),
    /// The list of what a project has changed, on the rows it is acting on.
    Change,
    /// The split button beside the Source Control commit message.
    Commit,
    /// The Source Control action menu.
    SourceControl,
    /// The Graph history-reference filter.
    HistoryRefs,
    /// A commit row in the active repository's history.
    History(usize, usize),
    /// The status bar's count of agents standing one way.
    Agents(Standing),
}

/// Builds the workspace with its resizable sessions sidebar.
pub fn workspace(
    theme: &Theme,
    projects: ProjectList<'_>,
    files: Worktree<'_>,
    layout: Layout,
    command_center: Bounds,
    panes: Panes,
    update: bool,
) -> Div<Message> {
    let ProjectList {
        open,
        sessions,
        bounds: project_list,
    } = projects;
    let status = Status::of(open, sessions, &panes, layout, &files);
    let Panes {
        editor,
        drop,
        carried,
        panel,
        menu: open_menu,
        overlays,
        ..
    } = panes;

    v_flex()
        .w_full()
        .h_full()
        .child(titlebar(
            theme,
            layout,
            whereabouts(open, sessions),
            command_center,
            update,
        ))
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
                        project_list,
                        layout.primary_sidebar_width,
                    ))
                    .child(sash(Axis::Horizontal, Message::ResizeSidebar))
                })
                .child(main_area(theme, layout, panel, editor))
                .when(layout.secondary_sidebar_open, |body| {
                    body.child(sash(Axis::Horizontal, Message::ResizeSecondarySidebar))
                        .child(worktree_sidebar(theme, &files, layout))
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
                above,
            } = overlaid;
            let sheet = sheet.map(|message| overlay(Point::new(0.0, 0.0), backdrop(message)));
            let panel = match above {
                true => overlay_above(at, content),
                false => overlay(at, content),
            };
            sheet.into_iter().chain([panel])
        }))
        .when_some(open_menu, |screen, (open, items)| {
            screen
                .child(overlay(
                    Point::new(0.0, 0.0),
                    backdrop(Message::DismissMenu),
                ))
                .child(match open.target {
                    MenuTarget::Agents(_) | MenuTarget::AgentMcp(_) => {
                        overlay_above(open.at, menu(theme, items))
                    }
                    _ => overlay(open.at, menu(theme, items)),
                })
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

/// The ways a project is added to the window.
///
/// A repository the reader already has is opened where it sits; one they
/// have not is fetched first. Both end in the same place — a project in the
/// window — so both are offered from the one control that adds one.
pub fn add_project_items() -> Vec<MenuItem<Message>> {
    vec![
        menu_entry("Open Folder…", Some(Message::OpenProject)),
        menu_entry("Clone from a URL…", Some(Message::CloneProject)),
    ]
}

/// The things that can be done to one session.
///
/// A session is a worktree and the work in it: going to it is what the row
/// itself does, so what is left is ending it, which takes the worktree away.
pub fn session_menu_items(
    session: SessionId,
    scope: Scope,
    detail: String,
) -> Vec<MenuItem<Message>> {
    vec![
        menu_entry("Go to Session", Some(Message::SelectSession(session))),
        menu_entry(detail, None),
        menu_entry("Run Checks", Some(Message::RunChecks(scope))),
        menu_entry("Show Check Output", Some(Message::ShowCheckOutput(scope))),
        menu_separator(),
        menu_entry("Finish Session…", Some(Message::FinishSession(session))),
    ]
}

/// The things that can be done to one project.
///
/// Cutting a session is the whole of it. For one repository the line opens
/// onto every branch there is to cut one from, the checked-out one first. A
/// folder of several has no one set of branches to offer, so its line asks
/// for the session's name and the repositories it works in, each cut from
/// what it has checked out. Nothing here starts an agent — a session is a
/// worktree first, and what is run in it comes after.
pub fn project_menu_items(
    project: &Project,
    bases: &[String],
    showing_bases: bool,
) -> Vec<MenuItem<Message>> {
    let id = project.id();
    let from = bases
        .iter()
        .enumerate()
        .map(|(place, base)| {
            menu_entry(
                format!("Based on {base}"),
                Some(Message::NewSessionFrom(id, place)),
            )
        })
        .collect::<Vec<_>>();

    let session = match project.repositories().len() > 1 {
        true => menu_entry("New Session…", Some(Message::NewSession)),
        false => pm_ui::menu_submenu(
            "New Session From…",
            (!from.is_empty()).then_some(Message::ShowSessionBases),
            showing_bases,
            from,
        ),
    };

    vec![
        session,
        menu_entry("Run Checks", Some(Message::RunChecks(Scope::checkout(id)))),
        menu_entry(
            "Show Check Output",
            Some(Message::ShowCheckOutput(Scope::checkout(id))),
        ),
        menu_separator(),
        menu_entry("Open Project…", Some(Message::OpenProject)),
        menu_entry("Close Project", Some(Message::CloseProject(id))),
    ]
}

/// The things that can be done to one shell's tab.
pub fn terminal_menu(shells: &[ShellEntry], id: ShellId) -> Vec<MenuItem<Message>> {
    let others = shells.len() > 1;

    vec![
        menu_entry("Rename…", Some(Message::RenameTerminal(id))),
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
    /// How many files that project has changed.
    changes: usize,
    /// How many shells are running in the worktree.
    shells: usize,
    /// How many agents are running in it.
    agents: usize,
    /// How every agent in the window stands, whichever project it is in.
    tally: Tally,
    /// The newest thing the reader is being told about, if anything.
    notice: Option<Shown>,
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
    /// What a language server behind that file says it is working on.
    activity: Option<String>,
    /// The server state drawn beside the language name.
    server: Option<pm_text::ServerStatus>,
    /// The current rotation of the language server spinner.
    server_turn: f32,
    /// The mode modal editing has that file in, with the keys typed towards
    /// a command and the register being recorded into.
    modal: Option<String>,
    /// How many errors and warnings a server has reported in it.
    problems: (usize, usize),
}

impl Status {
    /// Reads the status of the window out of what the screen was given.
    fn of(
        open: &Projects,
        sessions: &[SidebarProject],
        panes: &Panes,
        layout: Layout,
        files: &Worktree<'_>,
    ) -> Self {
        let (project, pointed) = pointed_at(open, sessions);
        let showing = panes.showing.as_ref().map(|file| file.borrow());
        let buffer = showing.as_ref().map(|document| document.buffer());

        Self {
            project: project.map(|project| project.name().to_owned()),
            branch: pointed.or_else(|| {
                files
                    .review
                    .and_then(crate::review::Review::head)
                    .map(pm_core::Head::name)
            }),
            sessions: project.map_or(0, |project| sessions_of(project, sessions).len()),
            changes: files.review.map_or(0, |review| review.changed().len()),
            shells: panes.panel.shells.len(),
            agents: panes.agents,
            tally: panes.tally,
            notice: panes.notice.clone(),
            activity: panes.activity.clone(),
            server: panes.server.clone(),
            server_turn: panes.server_turn,
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
            modal: panes
                .recording
                .zip(showing.as_ref())
                .map(|(recording, document)| {
                    let state = document.modal();
                    let mode = state.mode().label();
                    let recording = recording.map(|name| format!(" · recording @{name}"));
                    match state.pending() {
                        Some(pending) => {
                            format!("{mode} · {pending}{}", recording.unwrap_or_default())
                        }
                        None => format!("{mode}{}", recording.unwrap_or_default()),
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
        changes,
        shells,
        agents,
        tally,
        notice,
        panel_open,
        cursor,
        cursors,
        indent,
        language,
        activity,
        server,
        server_turn,
        problems,
        modal,
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
                Some(Message::ShowStatusBranches),
                false,
            ))
        })
        .when(changes > 0, |bar| {
            bar.child(
                status_item(
                    theme,
                    Some(IconName::GitCompare),
                    changes.to_string(),
                    Some(Message::SetSidebarView(SidebarView::Changes)),
                    false,
                )
                .tooltip(counted(changes, "change")),
            )
        })
        .when(sessions > 0, |bar| {
            bar.child(
                status_item(
                    theme,
                    Some(IconName::GitFork),
                    sessions.to_string(),
                    None,
                    false,
                )
                .tooltip(counted(sessions, "session")),
            )
        })
        .when(problems != (0, 0), |bar| {
            bar.child(status_item(
                theme,
                Some(IconName::Warning),
                format!("{} · {}", problems.0, problems.1),
                Some(Message::TogglePanelView(PanelView::Problems)),
                false,
            ))
        })
        .children(notice.map(|shown| notice_item(theme, shown)))
        .child(h_flex().flex_1())
        .children(agent_tally(theme, tally))
        .when_some(modal, |bar, modal| {
            bar.child(status_item(theme, None, modal, None, true))
        })
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
        .when_some(activity, |bar, activity| {
            bar.child(status_item(
                theme,
                Some(IconName::LoadCircle),
                activity,
                Some(Message::OpenServerLog),
                false,
            ))
        })
        .when_some(language, |bar, language| {
            let Some(server) = server else {
                return bar.child(status_item(theme, None, language, None, false));
            };
            bar.child(language_server_item(theme, language, server, server_turn))
        })
        .child(
            status_item(
                theme,
                Some(IconName::Sparkle),
                match agents {
                    0 => "New agent".to_owned(),
                    running => running.to_string(),
                },
                Some(Message::NewAgentSession),
                agents > 0,
            )
            .tooltip(match agents {
                0 => "New agent".to_owned(),
                running => counted(running, "agent"),
            }),
        )
        .child(
            status_item(
                theme,
                Some(IconName::Terminal),
                if shells == 0 {
                    String::new()
                } else {
                    shells.to_string()
                },
                Some(Message::TogglePanelView(PanelView::Terminal)),
                panel_open,
            )
            .tooltip(counted(shells, "shell")),
        )
}

/// Builds the status bar's notice: what happened, which takes the reader to
/// it when clicked, and the control that lets go of it.
///
/// Only the newest is drawn, with a count of the ones behind it; dismissing
/// it brings up the next.
fn notice_item(theme: &Theme, shown: Shown) -> Div<Message> {
    let (glyph, color) = match shown.tone {
        Tone::Trouble => (IconName::Warning, theme.colors.danger),
        Tone::Done => (IconName::Check, theme.colors.success),
    };
    let label = match shown.more {
        0 => shown.text,
        more => format!("{} (+{more})", shown.text),
    };
    h_flex()
        .items_center()
        .overflow_hidden()
        .child(
            h_flex()
                .h_px(theme.size.bar - BAR_INSET)
                .px(1)
                .gap(0.75)
                .items_center()
                .overflow_hidden()
                .rounded(theme.radius.md)
                .hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(Message::FollowNotice(shown.id))
                .child(icon(glyph).size(IconSize::XSmall).color(color))
                .child(text(label).text_xs().font_light().color(theme.colors.text)),
        )
        .child(
            v_flex()
                .size_px(theme.size.bar - BAR_INSET)
                .items_center()
                .justify_center()
                .rounded(theme.radius.md)
                .hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(Message::DismissNotice(shown.id))
                .child(
                    icon(IconName::Close)
                        .size(IconSize::XSmall)
                        .color(theme.colors.text_subtle),
                ),
        )
}

/// Builds the status bar's count of agents by how they stand, one reading
/// per state that any agent is in.
///
/// The count is the window's, not the active project's: an agent waiting on
/// the reader in a project nobody is looking at is the one most worth
/// knowing about.
fn agent_tally(theme: &Theme, tally: Tally) -> Vec<Div<Message>> {
    [
        (Standing::Working, tally.working, "working"),
        (Standing::Waiting, tally.waiting, "needs you"),
        (Standing::Done, tally.done, "done"),
        (Standing::Idle, tally.idle, "idle"),
        (Standing::Stopped, tally.stopped, "stopped"),
    ]
    .into_iter()
    .filter(|(_, count, _)| *count > 0)
    .map(|(standing, count, label)| {
        h_flex()
            .h_px(theme.size.bar - BAR_INSET)
            .px(1)
            .gap(0.75)
            .items_center()
            .rounded(theme.radius.md)
            .hover_bg(theme.colors.surface_hover)
            .active_bg(theme.colors.surface_active)
            .on_click(Message::ShowAgentsMenu(standing))
            .child(text("●").text_xs().color(standing_color(theme, standing)))
            .child(
                text(format!("{count} {label}"))
                    .text_xs()
                    .font_light()
                    .color(theme.colors.text_muted),
            )
    })
    .collect()
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

/// Draws the configured server name and state beside the buffer's language.
fn language_server_item(
    theme: &Theme,
    language: &str,
    server: pm_text::ServerStatus,
    turn: f32,
) -> Div<Message> {
    let (glyph, color, detail) = match &server.state {
        pm_text::ServerState::Ready => (None, theme.colors.text_muted, "Ready".to_owned()),
        pm_text::ServerState::Starting => (
            Some(IconName::LoadCircle),
            theme.colors.text_muted,
            "Starting".to_owned(),
        ),
        pm_text::ServerState::Indexing => (
            Some(IconName::LoadCircle),
            theme.colors.text_muted,
            "Indexing".to_owned(),
        ),
        pm_text::ServerState::Missing => (
            Some(IconName::Warning),
            theme.colors.danger,
            "Missing".to_owned(),
        ),
        pm_text::ServerState::Failed { reason } => (
            Some(IconName::Warning),
            theme.colors.danger,
            format!("Failed: {reason}"),
        ),
    };
    let rotation = if matches!(
        server.state,
        pm_text::ServerState::Starting | pm_text::ServerState::Indexing
    ) {
        turn
    } else {
        0.0
    };
    status_item(
        theme,
        None,
        format!("{language} · {}", server.command),
        Some(Message::OpenServerLog),
        false,
    )
    .when_some(glyph, |item, glyph| {
        item.child(
            icon(glyph)
                .size(IconSize::XSmall)
                .color(color)
                .rotate(rotation),
        )
    })
    .tooltip(detail)
}

/// `count` written out with `noun`, pluralized the way English does it.
pub fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Builds the window bar above every project and pane.
///
/// The bar is three columns of which the outer two share what is left over
/// equally, so the command center between them sits in the middle of the
/// window whatever is drawn either side of it.
fn titlebar(
    theme: &Theme,
    layout: Layout,
    here: String,
    bounds: Bounds,
    update: bool,
) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.titlebar)
        .items_center()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .child(
            h_flex()
                .flex_1()
                .h_full()
                .gap(0.5)
                .px(2)
                .items_center()
                .justify_end()
                .child(
                    icon_button(theme, IconName::ArrowLeft, Message::Act(Action::GoBack))
                        .tooltip("Go Back"),
                )
                .child(
                    icon_button(theme, IconName::ArrowRight, Message::Act(Action::GoForward))
                        .tooltip("Go Forward"),
                ),
        )
        .child(measured(bounds, command_center(theme, here)))
        .child(
            h_flex()
                .flex_1()
                .h_full()
                .items_center()
                .justify_end()
                .child(
                    h_flex()
                        .gap(1)
                        .items_center()
                        .when(update, |bar| {
                            bar.child(
                                button("Update", Message::OpenRepository)
                                    .filled()
                                    .h_px(COMMAND_CENTER_HEIGHT),
                            )
                        })
                        .child(
                            icon_button(theme, IconName::Settings, Message::OpenSettings)
                                .tooltip("Open Settings"),
                        )
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
                .child(window_controls()),
        )
}

/// Builds the box in the middle of the title bar that says where the window
/// is pointed and opens the file picker there when it is pressed.
///
/// Where it comes out is measured, so the picker it opens is drawn over it
/// and exactly as wide.
fn command_center(theme: &Theme, here: String) -> Div<Message> {
    h_flex()
        .flex_1()
        .min_w_px(COMMAND_CENTER_MIN_WIDTH)
        .max_w_px(COMMAND_CENTER_WIDTH)
        .h_px(COMMAND_CENTER_HEIGHT)
        .px(2)
        .gap(1.5)
        .items_center()
        .justify_center()
        .overflow_hidden()
        .bg(theme.colors.background)
        .border_1(theme.colors.border)
        .rounded(theme.radius.md)
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(Message::Act(Action::ShowFiles))
        .tooltip("Search files, > for commands, # for symbols")
        .child(
            icon(IconName::Search)
                .size(IconSize::Small)
                .color(theme.colors.text_subtle),
        )
        .child(text(here).text_sm().color(theme.colors.text_subtle))
}

/// What the command center says the window is pointed at: the active
/// project, and the session in it when one is in front.
fn whereabouts(open: &Projects, sessions: &[SidebarProject]) -> String {
    let Some(project) = open.active() else {
        return "Search".to_owned();
    };
    let session = sessions
        .iter()
        .find(|entry| entry.project == project.id() && !entry.at_checkout)
        .and_then(|entry| entry.sessions.iter().find(|session| session.selected));
    match session {
        Some(session) => format!("{} · {}", project.name(), session.name),
        None => project.name().to_owned(),
    }
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
                .child(bottom_panel(theme, layout.bottom_panel_height, panel))
        })
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

/// Builds the sidebar beside the panes: the switch, then what it is showing.
fn worktree_sidebar(theme: &Theme, files: &Worktree<'_>, layout: Layout) -> Div<Message> {
    let width = layout.secondary_sidebar_width;
    let view = layout.secondary_sidebar_view;

    v_flex()
        .w_px(width)
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .child(view_switch(theme, view))
        .child(match view {
            SidebarView::Files => crate::tree::files_sidebar(theme, files.listing.as_ref(), width),
            SidebarView::Changes => changes_sidebar(
                theme,
                files.review,
                files.committing,
                width,
                SourceControlControls {
                    solid: files.caret,
                    commit_bounds: files.commit_bounds.clone(),
                    history_refs_bounds: files.history_refs_bounds.clone(),
                    history_graph_bounds: files.history_graph_bounds.clone(),
                    changes_area: files.changes_area.clone(),
                    history_all: files.history_all,
                    history_graph_height: files.history_graph_height,
                    history_graph_open: files.history_graph_open,
                    changes_section_open: files.changes_section_open,
                },
            ),
        })
}

/// Builds the switch between the worktree's files and what has changed, as
/// a bar as tall as the panes' bars of tabs so the two line up.
fn view_switch(theme: &Theme, view: SidebarView) -> Div<Message> {
    v_flex()
        .w_full()
        .h_px(theme.size.tab_bar)
        .child(
            h_flex()
                .w_full()
                .flex_1()
                .items_stretch()
                .children(SidebarView::ALL.map(|offered| {
                    view_tab(
                        theme,
                        offered.label(),
                        offered == view,
                        None,
                        Message::SetSidebarView(offered),
                    )
                    .flex_1()
                })),
        )
        .child(rule(theme))
}

/// `path` written the way a prompt writes it, against the home directory.
pub fn shortened(path: &std::path::Path) -> String {
    let path = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => path.replacen(&home, "~", 1),
        _ => path,
    }
}

/// Builds the projects sidebar: every open project, its sessions beneath it.
///
/// A project's own row is its checkout — what the repository is called, and
/// the branch it has out — and every row under it is a session of it, saying
/// how far that worktree has drifted. The list is one reading, taken down the
/// window: what is being worked on, and how much of it there is. The rows
/// leave where they came out in `list`, so a project carried up or down them
/// can be told where it would land.
fn projects_sidebar(
    theme: &Theme,
    open: &Projects,
    sessions: &[SidebarProject],
    list: Bounds,
    width: f32,
) -> Div<Message> {
    let rows = open
        .iter()
        .filter_map(|project| {
            let entry = sessions
                .iter()
                .find(|entry| entry.project == project.id())?;
            Some(project_rows(theme, project, entry))
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
            sidebar.child(open_project(theme))
        })
        .child(measured(list, v_flex().w_full().children(rows)))
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
        .on_click(Message::AddProjectMenu)
        .child(
            text("+")
                .text_lg()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// Builds the line a window with nothing open offers a repository through.
fn open_project(theme: &Theme) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(3)
        .items_center()
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::AddProjectMenu)
        .child(
            text("Open a project…")
                .text_sm()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// The active project, and what the window is pointed at within it.
///
/// A project is read through one worktree at a time: its own checkout, named
/// by the branch it has out, or the session whose row the reader picked,
/// named by what they called it. The bar states that one place, because it
/// is the one every other reading in it is about.
fn pointed_at<'a>(
    open: &'a Projects,
    sessions: &[SidebarProject],
) -> (Option<&'a Project>, Option<String>) {
    let Some(project) = open.active() else {
        return (None, None);
    };
    let session = sessions_of(project, sessions)
        .iter()
        .find(|session| session.selected)
        .map(|session| session.name.clone());

    (
        Some(project),
        session.or_else(|| project.branch().map(str::to_owned)),
    )
}

/// The sessions listed under `project`, of which there may be none.
fn sessions_of<'a>(project: &Project, sessions: &'a [SidebarProject]) -> &'a [SidebarSession] {
    sessions
        .iter()
        .find(|entry| entry.project == project.id())
        .map_or(&[], |entry| entry.sessions.as_slice())
}

/// Builds one project: its own row, then a row for each of its sessions.
fn project_rows(theme: &Theme, project: &Project, entry: &SidebarProject) -> Div<Message> {
    v_flex()
        .w_full()
        .child(project_row(theme, project, entry))
        .children(
            entry
                .sessions
                .iter()
                .map(|session| session_row(theme, session)),
        )
}

/// Builds the row standing for a project's own checkout.
///
/// The project is the worktree its sessions were cut from, so its row names
/// it and states the branch it has out — `main`, most of the time — and the
/// sessions under it are read against that. A folder of several
/// repositories states how many it holds, and a plain folder states nothing
/// beside its name. The row is carried to reorder the projects, and a press
/// that goes nowhere activates it.
fn project_row(theme: &Theme, project: &Project, entry: &SidebarProject) -> Div<Message> {
    let selected = entry.at_checkout;
    let id = project.id();
    row(theme, selected)
        .on_drag(move |event| Message::DragProject(id, event))
        .on_secondary_click(Message::ProjectMenu(project.id()))
        .child(marker(theme, selected))
        .child(v_flex().w(2))
        .child(named(
            text(project.name().to_owned())
                .text_sm()
                .font_medium()
                .color(theme.colors.text),
        ))
        .children(project_reading(project).map(|said| {
            h_flex()
                .max_w_px(PROJECT_READING_WIDTH)
                .overflow_hidden()
                .tooltip(said.clone())
                .child(reading(theme, said))
        }))
}

/// What a project's row states beside its name: the branch it has out, or
/// how many repositories it holds when it holds several.
fn project_reading(project: &Project) -> Option<String> {
    match (project.branch(), project.repositories().len()) {
        (Some(branch), _) => Some(branch.to_owned()),
        (None, 0) => None,
        (None, count) => Some(format!("{count} repositories")),
    }
}

/// Builds one session row: its state, what it is called, how far it has gone.
fn session_row(theme: &Theme, session: &SidebarSession) -> Div<Message> {
    row(theme, session.selected)
        .on_click(Message::SelectSession(session.id))
        .on_secondary_click(Message::SessionMenu(session.id))
        .child(marker(theme, session.selected))
        .child(v_flex().w_px(SESSION_INDENT))
        .child(state_dot(theme, session.status_color))
        .child(v_flex().w(1))
        .child(named(
            text(session.name.clone())
                .text_sm()
                .font_light()
                .color(theme.colors.text),
        ))
        .when(session.pending > 0, |row| {
            row.child(
                text(format!("● {} ", session.pending))
                    .text_xs()
                    .font_mono()
                    .color(theme.colors.accent),
            )
        })
        .child(drift(theme, session.added, session.removed))
}

/// Builds the box a project or session row is laid out in.
fn row(theme: &Theme, selected: bool) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .overflow_hidden()
        .pr(3)
        .items_center()
        .when(selected, |row| row.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
}

/// Builds the part of a row its name sits in, which is the part that gives way.
///
/// A name is as long as whoever wrote it made it; the reading at the other
/// end is three numbers wide and is what the column is scanned for. So the
/// name is what a narrow sidebar takes the space from.
fn named(label: Text) -> Div<Message> {
    h_flex()
        .flex_1()
        .h_full()
        .items_center()
        .overflow_hidden()
        .child(label)
}

/// Builds the reading at the right-hand end of a row.
fn reading(theme: &Theme, said: String) -> Text {
    text(said)
        .text_xs()
        .font_mono()
        .color(theme.colors.text_subtle)
}

/// Builds how far a session has drifted: lines added, then lines taken out.
///
/// Each count is written in the colour a diff uses for that kind of line,
/// the same green and red the review states an addition and a deletion in.
fn drift(theme: &Theme, added: usize, removed: usize) -> Div<Message> {
    h_flex()
        .gap(1)
        .items_center()
        .child(count(format!("+{added}"), theme.colors.success))
        .child(count(format!("−{removed}"), theme.colors.danger))
}

/// One count in a drift reading, in `color`.
fn count(said: String, color: Rgba) -> Text {
    text(said).text_xs().font_mono().color(color)
}

/// Builds the bar down the left edge of the row the window is pointed at.
///
/// One worktree is being read at a time, and the sidebar says which: the bar
/// is drawn in every row so that the rows line up whether or not they are
/// the one, and coloured in only the one.
fn marker(theme: &Theme, selected: bool) -> Div<Message> {
    v_flex().w_px(MARKER_WIDTH).h_full().bg(match selected {
        true => theme.colors.border_selected,
        false => theme.colors.surface,
    })
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
