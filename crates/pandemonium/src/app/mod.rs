//! The window, what it is showing, and the frame it draws each redraw.
//!
//! [`App`] is the binary's whole state: one window, the GPU resources bound to
//! it and the model the frame is built from. It wires the layers and
//! implements none of them — every frame is `pm-ui` elements built from that
//! model, submitted to `pm-gfx` as one draw list.

mod agent;
mod arrival;
mod clicks;
mod client;
mod commands;
mod debug;
mod dialog;
mod disk;
mod drag;
mod excerpts;
mod input;
mod language;
mod listing;
mod modal;
mod notice;
mod outline;
mod operations;
mod panel;
mod panes;
mod picker;
mod places;
mod reading;
mod reorder;
mod review;
mod search;
mod session;
mod settings;
mod tasks;
mod terminal;
mod tree;
mod views;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use pm_core::{FileTree, Projects, Scope, Sessions};
use pm_gfx::{DrawList, Point, Quad, Rect, Renderer, Size};
use pm_text::Position;
use pm_ui::{
    Appearance, Axis, ResizeEdge, ResizeEvent, ResizePhase, ResizeState, Scroll, Styled, Theme, Ui,
};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::agent::Talks;
use crate::app::clicks::Clicks;
use crate::app::drag::{Geometry, TabDrag};
use crate::app::places::Trail;
use crate::arrival::Arrival;
use crate::config::{self, FontSlot, Preference, Preferences, Restored, ServerList, WindowState};
use crate::desktop;
use crate::editor::{self, Files};
use crate::keymap::Resolver;
use crate::message::Message;
use crate::notice::Notices;
use crate::onboarding;
use crate::panel::{Panel, PanelView};
use crate::panes::{Item, PaneTree, Saved};
use crate::review::Review;
use crate::settings::Settings;
use crate::terminal::{Shell, Terminals};
use crate::workspace::{
    self, BOTTOM_PANEL_RANGE, Layout, MenuTarget, PRIMARY_SIDEBAR_RANGE, Panes,
    SECONDARY_SIDEBAR_RANGE, SidebarView, TabMenu,
};

/// The blames that have come back from the threads that asked for them.
type Blamed = Arc<Mutex<Vec<(editor::FileId, Vec<pm_core::Blame>)>>>;

/// Results sent by background language server installers.
type InstalledServers = Arc<Mutex<Vec<(&'static str, Result<(), String>)>>>;

/// Which box of text the keyboard is going to, when it is going to one.
///
/// A window has more than one thing that is written in and only one keyboard,
/// so which box has it is one answer rather than a flag per box: two flags
/// can both be true, and there is no such state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Writing {
    /// The commit message of the active project's review.
    Commit,
    /// The prompt of one agent session.
    Prompt(crate::agent::TalkId),
    /// The console of the program one worktree is debugging.
    Console(Scope),
}

/// What the window is woken up for from outside the event loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wake {
    /// A terminal's child has written something that is waiting to be read.
    Terminal,
    /// An agent has said something that is waiting to be taken in.
    Agent,
    /// A language server has said something about a file that is open.
    Language,
    /// A language server installation has finished.
    Install,
    /// A blame has come back for a file that asked for one.
    Blame,
    /// A remote Git operation has finished.
    Git,
    /// A repository being cloned has finished being cloned.
    Clone,
    /// Something wrote into a worktree the window is watching.
    Disk,
    /// A debug adapter has said something about the program it is debugging.
    Debug,
    /// Git has answered a question asked away from the window.
    Reading,
    /// A release newer than this build has been found.
    Release,
    /// Something listed for the picker away from the window has come back.
    Listing,
    /// The reader has chosen something in one of the platform's pickers.
    Chosen,
    /// A move, a copy or a removal in the file tree has finished.
    Shifted,
    /// A picture has been decoded away from the window.
    Picture,
    /// Something has been read off the clipboard into a prompt.
    Paste,
    /// Files have been carried onto the window from outside it.
    Arrival,
}

/// The remote operation currently running for the active project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RemoteOperation {
    /// Updating remote references.
    Fetch,
    /// Bringing remote commits into the worktree.
    Pull,
    /// Sending local commits to a remote.
    Push,
    /// Bringing remote commits in and sending local ones out, in that order.
    Sync,
}

impl RemoteOperation {
    /// What the commit button says while this runs.
    pub(super) fn doing(self) -> &'static str {
        match self {
            Self::Fetch => "Fetching…",
            Self::Pull => "Pulling…",
            Self::Push => "Pushing…",
            Self::Sync => "Syncing…",
        }
    }

    /// What a notice says once this has gone through.
    pub(super) fn done(self) -> &'static str {
        match self {
            Self::Fetch => "Fetched",
            Self::Pull => "Pulled",
            Self::Push => "Pushed",
            Self::Sync => "Synced",
        }
    }

    /// What a notice says when this has not.
    pub(super) fn failed(self) -> &'static str {
        match self {
            Self::Fetch => "Fetch failed in",
            Self::Pull => "Pull failed in",
            Self::Push => "Push failed in",
            Self::Sync => "Sync failed in",
        }
    }
}

/// The conductor window, the GPU resources bound to it and what it is showing.
pub struct App {
    /// The platform window, once the event loop has opened one.
    window: Option<Arc<Window>>,
    /// Whether that window has the keyboard, which is whether the reader is
    /// looking at it rather than at another application.
    window_focused: bool,
    /// Whether that window is hidden from sight, minimised or covered, so
    /// nothing drawn by the clock alone would be seen.
    window_occluded: bool,
    /// The device and surface drawing into that window.
    renderer: Option<Renderer>,
    /// The element tree's focus, hover and hit regions between frames.
    ui: Option<Ui<Message>>,
    /// The draw list, reused every frame.
    list: Option<DrawList>,
    /// The preferences the window draws and behaves by.
    preferences: Preferences,
    /// Whether the first run's setup has been finished.
    onboarded: bool,
    /// Which page the settings pane shows, and how far down it.
    settings: Settings,
    /// The keymap a keypress is resolved against.
    resolver: Resolver,
    /// Modal editing: the registers, the last change and the macros every
    /// file shares.
    vim: pm_vim::Vim,
    /// The modifiers held down right now.
    modifiers: ModifiersState,
    /// Last pointer position in logical window coordinates.
    pointer: Option<Point>,
    /// Time of the last press on empty title-bar space.
    last_titlebar_click: Option<Instant>,
    /// How far the page is scrolled.
    scroll: Scroll,
    /// The part of a line the wheel has moved a pane drawn in whole lines
    /// that has not yet come to a line, in logical pixels.
    wheel_carry: f32,
    /// The sessions of those projects: a worktree apiece, to work an agent in.
    sessions: Sessions,
    /// The session the window is pointed at, once one has been picked.
    session: Option<pm_core::SessionId>,
    /// How many agents were in the middle of a turn when git was last asked.
    working: usize,
    /// What a session being named is cut from, while one is being named.
    session_base: Option<String>,
    /// What the session whose repositories are being picked is called.
    session_name: String,
    /// The repositories ticked for that session.
    session_picks: BTreeSet<PathBuf>,
    /// The branches the open project menu offers to cut a session from.
    session_bases: Vec<String>,
    /// Whether that menu is showing them.
    showing_bases: bool,
    /// The projects this window holds open.
    open: Projects,
    /// One file tree per worktree, so each keeps what it has expanded.
    files: BTreeMap<Scope, FileTree>,
    /// What each of those worktrees has changed, and what git said about it.
    reviews: BTreeMap<Scope, Review>,
    /// Each of those worktrees followed on disk, for what others write into it.
    watchers: BTreeMap<Scope, pm_core::Watcher>,
    /// The changes the reader is being asked whether to throw away.
    discarding: Vec<crate::review::ChangeId>,
    /// Whether keystrokes go to the list of changes.
    changes_focused: bool,
    /// What the file tree is being asked whether to take off the disk.
    removing: Vec<std::path::PathBuf>,
    /// The rows each worktree's file tree has selected.
    selections: BTreeMap<Scope, crate::tree::Selection>,
    /// How far each worktree's file tree is scrolled.
    tree_scrolls: BTreeMap<Scope, pm_ui::Scrolled>,
    /// The name being typed into the file tree, if one is.
    tree_edit: Option<crate::tree::Edit>,
    /// A move the file tree was asked to make, waiting on the servers to
    /// say what it changes elsewhere.
    moving: Option<operations::Moving>,
    /// The spans a server said the selection can grow through, for the file
    /// and version it said them about.
    selection_ranges: Option<(editor::FileId, i32, Vec<std::ops::Range<pm_text::Position>>)>,
    /// When the settings file was last written, as its language servers were
    /// last read from it.
    settings_seen: Option<std::time::SystemTime>,
    /// What was cut or copied out of the file tree.
    tree_clipboard: Option<crate::tree::Clipboard>,
    /// The moves, copies and removals in the tree that have finished and
    /// have not been taken in yet.
    shifted: Arc<Mutex<Vec<tree::Shifted>>>,
    /// The rows of the file tree the pointer is carrying.
    entry_drag: Option<crate::tree::EntryDrag>,
    /// What files carried in from outside have done away from the window
    /// and has not been taken in yet.
    arrivals: crate::arrival::Arrivals,
    /// The directory files carried in from outside would land in.
    arriving: Option<std::path::PathBuf>,
    /// The project row the pointer is carrying up or down the sidebar.
    project_drag: Option<reorder::ProjectDrag>,
    /// Where the projects sidebar's rows came out in the last frame.
    project_list: pm_ui::Bounds,
    /// Whether keystrokes go to the file tree.
    tree_focused: bool,
    /// Where the file tree's rows came out in the last frame.
    tree_rows: pm_ui::Bounds,
    /// Where the file tree's scrolled area came out in the last frame.
    tree_area: pm_ui::Bounds,
    /// Where the name being typed into the file tree came out.
    tree_field: pm_ui::Bounds,
    /// Current width and drag state of the sessions sidebar.
    sidebar: ResizeState,
    /// Current height and drag state of the bottom panel.
    bottom_panel: ResizeState,
    /// Current height and drag state of the Source Control graph.
    history_graph: ResizeState,
    /// Current height and drag state of the box agents' prompts are written in.
    prompt_box: ResizeState,
    /// Current width and drag state of the secondary sidebar.
    secondary_sidebar: ResizeState,
    /// Whether the primary sidebar is visible.
    primary_sidebar_open: bool,
    /// Whether the bottom panel is visible.
    bottom_panel_open: bool,
    /// Which of the bottom panel's views is in front.
    panel_view: PanelView,
    /// How far the bottom panel's list of problems is scrolled.
    problems_scroll: pm_ui::Scrolled,
    /// Where the bottom panel's list of problems came out last frame.
    problems_area: pm_ui::Bounds,
    /// Whether the secondary sidebar is visible.
    secondary_sidebar_open: bool,
    /// Which of the worktree's two lists that sidebar is showing.
    secondary_sidebar_view: SidebarView,
    /// Whether the Source Control graph is visible.
    history_graph_open: bool,
    /// Whether the Source Control changes section is expanded.
    changes_section_open: bool,
    /// The box of text keystrokes go to, if they go to one.
    writing: Option<Writing>,
    /// The size and state the window is written down with.
    window_state: WindowState,
    /// Whether the event loop should close after the current event.
    close_requested: bool,
    /// The files the window has open, and the servers behind them.
    editor: Files,
    /// The pictures the window has open.
    images: crate::image::Images,
    /// What the markdown the panes are rendering keeps between frames.
    renders: crate::markdown::Renders,
    /// The file followed and symbols shown by each worktree's outline.
    outlines: crate::outline::Store,
    /// Each worktree's changes as excerpts, for the panes editing them.
    excerpts: BTreeMap<Scope, crate::excerpts::OpenExcerpts>,
    /// Each worktree's search pane state.
    searches: BTreeMap<Scope, search::ProjectSearch>,
    /// Which project search field has the keyboard.
    project_search_field: Option<editor::SearchField>,
    /// The servers a language runs, in place of the ones it names or after them.
    language_servers: BTreeMap<String, ServerList>,
    /// The agents the reader added, beside the ones the editor ships.
    agent_servers: Vec<pm_acp::Agent>,
    /// How the window is divided into panes, and which of them has the keyboard.
    panes: PaneTree,
    /// The panes the last launch left, until the window is ready to open them.
    saved: Saved,
    /// Where those panes and their tabs came out in the last frame.
    geometry: Geometry,
    /// The tab the pointer is carrying, if it is carrying one.
    drag: Option<TabDrag>,
    /// Whether keystrokes go to the editor pane rather than to the window.
    editor_focused: bool,
    /// Whether keystrokes go to the focused pane's search bar.
    search_focused: bool,
    /// How much larger or smaller than usual the editor's text is drawn.
    zoom: f32,
    /// Where the window has been, for going back and reopening tabs.
    trail: Trail,
    /// The list the window is asking the reader to choose from, if it is.
    picker: Option<crate::picker::Picker>,
    /// Pointer position of the status-bar branch control anchoring its popover.
    branch_picker_at: Option<Point>,
    /// Pointer position of the agent control anchoring its choices.
    agent_picker_at: Option<Point>,
    /// Bounds of the Source Control commit split button from the last frame.
    commit_bounds: pm_ui::Bounds,
    /// Bounds of the title bar's command center from the last frame.
    command_center_bounds: pm_ui::Bounds,
    /// Bounds of the Graph history-reference filter from the last frame.
    history_refs_bounds: pm_ui::Bounds,
    /// Bounds of the Source Control Graph from the last frame.
    history_graph_bounds: pm_ui::Bounds,
    /// Where the Source Control list of changes came out last frame.
    changes_area: pm_ui::Bounds,
    /// Whether the Graph includes all history references.
    history_all: bool,
    /// The action requested before opening the stash picker.
    stash_action: Option<crate::review::StashAction>,
    /// Whether stashes existed when the source control menu opened.
    stash_available: bool,
    /// The full object captured when a history row menu opened.
    history_menu_object: Option<String>,
    /// The project and worktree whose history row opened the menu.
    history_menu_scope: Option<Scope>,
    /// Remote Git work currently running away from the UI thread.
    remote_operation: Option<RemoteOperation>,
    /// When the spinner shown while a remote is waited on last turned.
    spun: std::time::Instant,
    /// Completed remote Git work waiting for the event loop.
    git_results: Arc<Mutex<Vec<(Scope, pm_core::Said)>>>,
    /// What git is being asked about the worktrees away from the window.
    readings: reading::Readings,
    /// What the pickers have gathering away from the window.
    listings: listing::Listings,
    /// The repositories a clone has finished with, and where they landed.
    cloned: Arc<Mutex<Vec<Result<std::path::PathBuf, String>>>>,
    /// The question the window is asking before it acts, if it is asking one.
    prompt: Option<crate::prompt::Prompt>,
    /// What could be written where the cursor is, while the list is up.
    completions: Option<crate::editor::Completions>,
    /// A completion put in before its server had filled it in, waiting for
    /// the edits that come with it.
    taken_completion: Option<crate::app::language::TakenCompletion>,
    /// What the editor has to say about a place, and where to say it.
    hint: Option<editor::Shown>,
    /// The name the pointer is over, while the key that links it is held.
    link: Option<crate::app::language::Link>,
    /// Where the caret is in its blink.
    blink: editor::Blink,
    /// Where the pointer has been resting, and since when.
    resting: Option<(Instant, Point)>,
    /// The fixes a server last offered, for the menu that shows them.
    code_actions: Vec<language::OfferedCodeAction>,
    /// The questions asked of servers and not yet answered.
    asked: Vec<language::Pending>,
    /// Whether the servers being waited on were asked by a save.
    saving: bool,
    /// The query the servers were last asked for workspace symbols, and the
    /// rows their answers have come to so far.
    workspace_symbols: (Option<String>, Vec<crate::picker::Row>),
    /// The files of the worktree in front, offered below the symbols the
    /// servers find so that a name always finds something.
    workspace_files: Vec<crate::picker::Row>,
    /// The pane whose tabs are being closed, while one of them is asked about.
    closing: Option<crate::panes::PaneId>,
    /// The blames that have come back and not yet been taken in.
    blamed: Blamed,
    /// The tab menu that is open over the panes, if one is.
    menu: Option<TabMenu>,
    /// The last press in the editor pane, for selecting a word.
    text_clicks: Clicks<Position>,
    /// Whether the pointer gesture over text grows the selection already there.
    ///
    /// Decided when the press lands and kept for the drag that follows, so
    /// letting go of shift halfway through does not drop the anchor the
    /// press chose.
    text_extends: bool,
    /// Whether the pointer gesture over text is a control click following a name.
    ///
    /// The drag and the release that finish such a press are part of the
    /// click, not a selection: the answer to where the name is defined can
    /// land before the button is let go, and a wobble of the pointer after
    /// it would put the cursor back where the click was.
    text_follows: bool,
    /// The last press on the terminal's grid, for telling a double one apart.
    screen_clicks: Clicks<pm_vt::Place>,
    /// What the drag over the terminal's grid grows its selection by.
    screen_unit: pm_vt::Unit,
    /// The last press on an agent's transcript, for selecting a word.
    agent_clicks: Clicks<crate::agent::Spot>,
    /// Whether the drag over an agent's transcript grows by whole words.
    agent_words: bool,
    /// The last press on a row of the file tree, for keeping a file open.
    tree_clicks: Clicks<pm_core::EntryId>,
    /// The last press on a tab, for keeping a previewed file open.
    tab_clicks: Clicks<Item>,
    /// The last watch expression pressed, for editing on a double click.
    watch_clicks: Clicks<usize>,
    /// The agent sessions the window is running, one per project.
    agents: Talks,
    /// The shells the window is running, one per project.
    terminals: Terminals,
    /// Task runs and their reported results.
    tasks: crate::tasks::Tasks,
    /// Debug scenarios waiting for a task to finish.
    pending_debug: std::collections::BTreeMap<crate::tasks::RunId, (Scope, pm_dap::Scenario)>,
    /// Worktrees whose invalid tasks file has already been reported.
    task_errors: std::collections::BTreeSet<std::path::PathBuf>,
    /// The terminals agents have started among those shells, and what the
    /// agents are waiting to hear about them.
    errands: client::Errands,
    /// The logins running in terminals, as the conversation each is for and
    /// the worktree and shell it runs as.
    logins: Vec<(crate::agent::TalkId, Scope, crate::terminal::ShellId)>,
    /// What the reader is being told about in the status bar.
    notices: Notices,
    /// Servers already offered or tried this launch.
    offered_servers: BTreeSet<&'static str>,
    /// Running installs and their status-bar notices.
    installing_servers: BTreeMap<&'static str, crate::notice::NoticeId>,
    /// Results delivered by installer worker threads.
    installed_servers: InstalledServers,
    /// The breakpoints each worktree keeps, and the program each debugs.
    debuggers: crate::debug::Debuggers,
    /// The breakpoint a prompt is editing.
    breakpoint_prompt: Option<(Scope, std::path::PathBuf, usize)>,
    /// The watch row a prompt is editing, or none when adding.
    watch_prompt: Option<(Scope, Option<usize>)>,
    /// The process chosen for an attach adapter picker.
    attach_pid: Option<u32>,
    /// Whether keystrokes go to the terminal rather than to the window.
    terminal_focused: bool,
    /// How far back the terminal was scrolled when a scrollbar drag began.
    terminal_scroll_origin: Option<usize>,
    /// How far down the editor was scrolled when a scrollbar drag began.
    editor_scroll_origin: Option<usize>,
    /// How far down the conversation was scrolled when a scrollbar drag began.
    agent_scroll_origin: Option<f32>,
    /// Whether a release newer than this build has been published.
    update_available: bool,
    /// Set when the search for one finds it, until the window takes it in.
    released: Arc<Mutex<bool>>,
    /// What has been read off the clipboard for a prompt and not yet put
    /// into it.
    pastes: Arc<Mutex<Vec<(crate::agent::TalkId, Pasting)>>>,
    /// What the reader has chosen in the platform's pickers and the window
    /// has not taken in yet.
    choices: dialog::Choices,
    /// How the reader threads wake the event loop.
    proxy: EventLoopProxy<Wake>,
    /// Which wakes are on their way and not yet taken in, so a thread that
    /// has something to say a thousand times a second wakes the window once.
    pending: Pending,
}

/// How many kinds of [`Wake`] there are.
const WAKES: usize = Wake::Arrival as usize + 1;

/// One flag per kind of [`Wake`], set while one is on its way.
type Pending = Arc<[AtomicBool; WAKES]>;

/// What a paste into a prompt came to, read away from the window.
pub(super) enum Pasting {
    /// An image, ready to attach.
    Image(crate::agent::Pasted),
    /// Text, to type in.
    Text(String),
}

/// A handle a thread behind the window wakes it with through `proxy`,
/// sending `wake` unless one is already on its way by `pending`.
fn waker_through(
    proxy: &EventLoopProxy<Wake>,
    pending: &Pending,
    wake: Wake,
) -> Arc<dyn Fn() + Send + Sync> {
    let proxy = Mutex::new(proxy.clone());
    let pending = pending.clone();
    Arc::new(move || {
        if !pending[wake as usize].swap(true, Ordering::AcqRel)
            && let Ok(proxy) = proxy.lock()
        {
            let _ = proxy.send_event(wake);
        }
    })
}

/// When the settings file was last written, if it can be told.
fn settings_written() -> Option<std::time::SystemTime> {
    std::fs::metadata(config::settings_file()?)
        .and_then(|found| found.modified())
        .ok()
}

impl App {
    /// Hands the editor the language servers the settings name, and the
    /// settings each of them runs with.
    fn apply_language_servers(&mut self) {
        let (replace, add) = partition_language_servers(&self.language_servers);
        self.editor.set_language_servers(&replace);
        self.editor.add_language_servers(&add);
    }

    /// Reads the language servers the settings file names again when it has
    /// been written since they were last read, which is what saving it in a
    /// pane does, and hands running servers their new settings.
    pub(super) fn follow_server_settings(&mut self) {
        let written = settings_written();
        if written.is_none() || written == self.settings_seen {
            return;
        }
        self.settings_seen = written;
        let Some(servers) = config::language_servers() else {
            return;
        };
        if servers == self.language_servers {
            return;
        }
        self.language_servers = servers;
        self.apply_language_servers();
        self.editor.refresh();
        self.request_redraw();
    }
}

/// The replacement lists and the added lists in `configured`.
fn partition_language_servers(
    configured: &BTreeMap<String, ServerList>,
) -> (
    BTreeMap<String, Vec<pm_text::Server>>,
    BTreeMap<String, Vec<pm_text::Server>>,
) {
    let mut replace = BTreeMap::new();
    let mut add = BTreeMap::new();
    for (language, list) in configured {
        match list {
            ServerList::Replace(servers) => {
                replace.insert(language.clone(), servers.clone());
            }
            ServerList::Add(servers) => {
                add.insert(language.clone(), servers.clone());
            }
        }
    }
    (replace, add)
}

impl App {
    /// The app as the last launch left it, woken through `proxy`.
    pub fn restored(proxy: EventLoopProxy<Wake>) -> Self {
        let restored = config::load();
        let mut open = Projects::new();
        for root in &restored.projects {
            let _ = open.find_or_open(root);
        }
        open.activate_first();
        if let Some(active) = restored.active.as_ref() {
            let _ = open.find_or_open(active);
        }

        let layout = restored.layout;
        let saved = restored.panes;

        let files = open
            .iter()
            .map(|project| (Scope::checkout(project.id()), FileTree::new(project.root())))
            .collect();
        let pending: Pending = Arc::new(std::array::from_fn(|_| AtomicBool::new(false)));
        crate::image::wake_with(waker_through(&proxy, &pending, Wake::Picture));

        let mut notices = Notices::default();
        for error in config::take_extension_errors() {
            notices.trouble(error, None);
        }
        Self {
            window: None,
            window_focused: true,
            window_occluded: false,
            renderer: None,
            ui: None,
            list: None,
            preferences: restored.preferences,
            onboarded: restored.onboarded,
            settings: Settings::default(),
            resolver: Resolver::default(),
            vim: pm_vim::Vim::default(),
            modifiers: ModifiersState::default(),
            pointer: None,
            last_titlebar_click: None,
            scroll: Scroll::default(),
            wheel_carry: 0.0,
            sessions: Sessions::new(),
            session: None,
            working: 0,
            session_base: None,
            session_name: String::new(),
            session_picks: BTreeSet::new(),
            session_bases: Vec::new(),
            showing_bases: false,
            open,
            files,
            reviews: BTreeMap::new(),
            watchers: BTreeMap::new(),
            discarding: Vec::new(),
            changes_focused: false,
            removing: Vec::new(),
            selections: BTreeMap::new(),
            tree_scrolls: BTreeMap::new(),
            tree_edit: None,
            moving: None,
            selection_ranges: None,
            settings_seen: None,
            tree_clipboard: None,
            shifted: Arc::default(),
            entry_drag: None,
            arrivals: Arc::default(),
            arriving: None,
            project_drag: None,
            project_list: drag::unmeasured(),
            tree_focused: false,
            tree_rows: drag::unmeasured(),
            tree_area: drag::unmeasured(),
            tree_field: drag::unmeasured(),
            sidebar: ResizeState::new(
                layout.primary_sidebar_width,
                PRIMARY_SIDEBAR_RANGE.0,
                PRIMARY_SIDEBAR_RANGE.1,
            ),
            bottom_panel: ResizeState::new(
                layout.bottom_panel_height,
                BOTTOM_PANEL_RANGE.0,
                BOTTOM_PANEL_RANGE.1,
            ),
            history_graph: ResizeState::new(
                layout.history_graph_height,
                workspace::HISTORY_GRAPH_RANGE.0,
                workspace::HISTORY_GRAPH_RANGE.1,
            ),
            prompt_box: ResizeState::new(
                layout.prompt_height,
                workspace::PROMPT_RANGE.0,
                workspace::PROMPT_RANGE.1,
            ),
            secondary_sidebar: ResizeState::new(
                layout.secondary_sidebar_width,
                SECONDARY_SIDEBAR_RANGE.0,
                SECONDARY_SIDEBAR_RANGE.1,
            ),
            primary_sidebar_open: layout.primary_sidebar_open,
            bottom_panel_open: layout.bottom_panel_open,
            panel_view: PanelView::default(),
            problems_scroll: pm_ui::Scrolled::default(),
            problems_area: pm_ui::Bounds::default(),
            secondary_sidebar_open: layout.secondary_sidebar_open,
            secondary_sidebar_view: layout.secondary_sidebar_view,
            history_graph_open: layout.history_graph_open,
            changes_section_open: layout.changes_section_open,
            writing: None,
            window_state: restored.window,
            close_requested: false,
            editor: Files::default(),
            images: crate::image::Images::default(),
            renders: crate::markdown::Renders::default(),
            outlines: crate::outline::Store::default(),
            excerpts: BTreeMap::new(),
            searches: BTreeMap::new(),
            project_search_field: None,
            language_servers: restored.language_servers,
            agent_servers: restored.agent_servers,
            panes: PaneTree::default(),
            saved,
            geometry: Geometry::default(),
            drag: None,
            editor_focused: false,
            search_focused: false,
            zoom: 1.0,
            trail: Trail::default(),
            picker: None,
            branch_picker_at: None,
            agent_picker_at: None,
            commit_bounds: Rc::new(Cell::new(Rect::from_xywh(0.0, 0.0, 0.0, 0.0))),
            command_center_bounds: Rc::new(Cell::new(Rect::from_xywh(0.0, 0.0, 0.0, 0.0))),
            history_refs_bounds: Rc::new(Cell::new(Rect::from_xywh(0.0, 0.0, 0.0, 0.0))),
            history_graph_bounds: Rc::new(Cell::new(Rect::from_xywh(0.0, 0.0, 0.0, 0.0))),
            changes_area: pm_ui::Bounds::default(),
            history_all: layout.history_all,
            stash_action: None,
            stash_available: false,
            history_menu_object: None,
            history_menu_scope: None,
            remote_operation: None,
            spun: std::time::Instant::now(),
            git_results: Arc::new(Mutex::new(Vec::new())),
            readings: reading::Readings::default(),
            listings: listing::Listings::default(),
            cloned: Arc::new(Mutex::new(Vec::new())),
            prompt: None,
            completions: None,
            taken_completion: None,
            hint: None,
            link: None,
            blink: editor::Blink::default(),
            resting: None,
            code_actions: Vec::new(),
            asked: Vec::new(),
            saving: false,
            workspace_symbols: (None, Vec::new()),
            workspace_files: Vec::new(),
            closing: None,
            blamed: Arc::new(Mutex::new(Vec::new())),
            text_clicks: Clicks::default(),
            text_extends: false,
            text_follows: false,
            screen_clicks: Clicks::default(),
            screen_unit: pm_vt::Unit::Cell,
            agent_clicks: Clicks::default(),
            agent_words: false,
            tree_clicks: Clicks::default(),
            tab_clicks: Clicks::default(),
            watch_clicks: Clicks::default(),
            menu: None,
            agents: Talks::default(),
            terminals: Terminals::default(),
            tasks: crate::tasks::Tasks::default(),
            pending_debug: std::collections::BTreeMap::new(),
            task_errors: std::collections::BTreeSet::new(),
            errands: client::Errands::default(),
            logins: Vec::new(),
            notices,
            offered_servers: BTreeSet::new(),
            installing_servers: BTreeMap::new(),
            installed_servers: Arc::new(Mutex::new(Vec::new())),
            debuggers: crate::debug::Debuggers::default(),
            breakpoint_prompt: None,
            watch_prompt: None,
            attach_pid: None,
            terminal_focused: false,
            terminal_scroll_origin: None,
            editor_scroll_origin: None,
            agent_scroll_origin: None,
            update_available: false,
            released: Arc::new(Mutex::new(false)),
            pastes: Arc::new(Mutex::new(Vec::new())),
            choices: dialog::Choices::default(),
            proxy,
            pending,
        }
    }

    /// The shell of the worktree the window is pointed at, started if need be.
    ///
    /// A shell belongs to the worktree the window is pointed at, the same one
    /// the file tree lists, and it is started the first time its pane is
    /// drawn rather than when the project is opened.
    fn active_shell(&mut self) -> Option<Shell> {
        let scope = self.scope()?;
        let root = self.root_of(scope)?;
        let env = self.worktree_env(scope);
        self.terminals.open(scope, &root, &env)
    }

    /// Closes the panel once the worktree's last shell has exited.
    fn close_empty_panel(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.showing_terminals() && self.terminals.count(scope) == 0 {
            self.bottom_panel_open = false;
            self.terminal_focused = false;
        }
    }

    /// Takes the keyboard away from the panes, for a click elsewhere.
    ///
    /// The click that lands back in a pane brings it straight back, so a
    /// press is free to drop focus without knowing where it landed.
    pub(super) fn release_pane_focus(&mut self) {
        self.writing = None;
        self.terminal_focused = false;
        self.editor_focused = false;
        self.changes_focused = false;
        self.tree_focused = false;
    }

    /// What the focused pane holds, for the keymap's `when` clauses.
    pub(super) fn focused_pane_kind(&self) -> Option<&'static str> {
        let showing = |shown: fn(crate::panes::Item) -> bool| self.active_tab().is_some_and(shown);

        match self.writing {
            Some(Writing::Prompt(_)) => return Some("prompt"),
            Some(Writing::Console(_)) => return Some("console"),
            Some(Writing::Commit) => return Some("commit"),
            None => {}
        }
        match (self.editor_focused, self.terminal_focused) {
            (true, _) if showing(|item| item.review().is_some()) => Some("review"),
            (true, _) if showing(|item| matches!(item, Item::Outline(_))) => Some("outline"),
            (true, _) if showing(|item| matches!(item, Item::Search(_))) => Some("search"),
            (true, _) if showing(|item| item.change().is_some()) => Some("diff"),
            (true, _) if showing(|item| item.session().is_some()) => Some("agent"),
            (true, _) if showing(crate::panes::Item::is_window_wide) => Some("settings"),
            (true, _) => Some("file"),
            (_, true) => Some("terminal"),
            _ => None,
        }
    }

    /// The file keystrokes are going to, if the pane is focused.
    pub(super) fn focused_file(&self) -> Option<editor::OpenFile> {
        self.editor_focused.then(|| self.active_file()).flatten()
    }

    /// Places the cursor where a press landed, or selects to where it reached.
    ///
    /// A second press in the same place takes the word under it and a third
    /// takes the line, which are the gestures the element tree cannot tell
    /// the window about on its own. Shift keeps the selection's anchor and
    /// moves its head to the pointer, so the text between the cursor and the
    /// click is what ends up selected. Alt puts another cursor down instead
    /// of moving the one there is, alt with shift draws a box, and control
    /// follows the name under the pointer to where it is defined.
    ///
    /// Only the press begins a gesture. The release that ends one says the
    /// same thing over again, and a click counted twice is a click that
    /// selects a word nobody double-clicked.
    fn select_text(
        &mut self,
        pane: crate::panes::PaneId,
        phase: ResizePhase,
        anchor: Position,
        head: Position,
    ) {
        self.focus_pane(pane);
        self.search_focused = false;
        self.dismiss_popup();

        let pressed = phase == ResizePhase::Started;
        let still = anchor == head;
        if pressed {
            self.text_extends = self.extends_text();
            self.text_follows = still && self.modifiers.control_key();
        }

        if self.text_follows {
            if pressed {
                self.follow_link(head);
            }
            return;
        }
        if self.modifiers.alt_key() && self.modifiers.shift_key() {
            self.text_clicks.clear();
            return self.edit_active(|buffer| buffer.box_selection(anchor, head));
        }
        if pressed && still && self.modifiers.alt_key() {
            self.text_clicks.clear();
            return self.edit_active(|buffer| {
                buffer.add_cursor(pm_text::Selection::at(head));
            });
        }
        if self.text_extends {
            if still && !pressed {
                return;
            }
            self.text_clicks.clear();
            return self.edit_active(|buffer| {
                buffer.collapse_cursors();
                buffer.place(head, true);
            });
        }
        if still && !pressed {
            return;
        }

        let presses = if still {
            self.text_clicks.press(anchor)
        } else {
            self.text_clicks.clear();
            0
        };
        self.edit_active(|buffer| {
            buffer.collapse_cursors();
            match presses {
                1 => buffer.place(head, false),
                2 => buffer.select_word(head),
                3 => buffer.select_line_text(head),
                _ => {
                    buffer.place(anchor, false);
                    buffer.place(head, true);
                }
            }
        });
    }

    /// Whether a press with the modifiers held grows the selection already there.
    ///
    /// Shift alone does. Alt with shift draws a box and control follows a
    /// name, and neither of those is a selection growing from the cursor.
    fn extends_text(&self) -> bool {
        self.modifiers.shift_key() && !self.modifiers.alt_key() && !self.modifiers.control_key()
    }

    /// Follows the name under the pointer to where it is defined.
    ///
    /// The place is taken from under the pointer rather than from where a
    /// caret would land: aiming at the right half of the last letter of a
    /// name is aiming at the name, not at what follows it.
    fn follow_link(&mut self, pressed: Position) {
        let at = self
            .pointer
            .and_then(|point| self.place_under(point))
            .map_or(pressed, |(_, at)| at);
        self.place_cursor(at, false);
        self.act(crate::keymap::Action::GoToDefinition);
    }

    /// Scrolls the editor by a drag on one of its scrollbars.
    ///
    /// Where the view stood when the drag began is remembered, because every
    /// frame of a drag reports travel from the same press: adding the travel
    /// to where the view has already moved would run away from the pointer.
    fn drag_editor_scrollbar(
        &mut self,
        pane: crate::panes::PaneId,
        axis: editor::ScrollAxis,
        event: ResizeEvent,
        step: f32,
    ) {
        self.focus_pane(pane);
        let Some(file) = self.active_file() else {
            return;
        };
        let mut document = file.borrow_mut();
        let vertical = axis == editor::ScrollAxis::Vertical;
        let at = if vertical {
            document.scroll()
        } else {
            document.column()
        };
        let base = match event.phase {
            ResizePhase::Started => at,
            _ => self.editor_scroll_origin.unwrap_or(at),
        };
        self.editor_scroll_origin = match event.phase {
            ResizePhase::Ended => None,
            _ => Some(base),
        };

        let along = if vertical {
            Axis::Vertical
        } else {
            Axis::Horizontal
        };
        let travelled = event.delta(along) * step;
        let reached = (base as f32 + travelled).round().max(0.0) as usize;
        if vertical {
            document.scroll_to(reached);
        } else {
            document.scroll_to_column(reached);
        }
    }

    /// Selects every line a drag down the gutter reaches.
    fn select_lines(&mut self, pane: crate::panes::PaneId, anchor: Position, head: Position) {
        self.focus_pane(pane);
        self.text_clicks.clear();
        self.edit_active(|buffer| {
            let (first, last) = (anchor.line.min(head.line), anchor.line.max(head.line));
            buffer.select_line(Position::new(first, 0));
            let start = buffer.selection().start();
            buffer.select_line(Position::new(last, 0));
            let end = buffer.selection().end();
            buffer.select_range(start..end);
        });
    }

    /// Opens the menu of what can be done to the text, where the pointer is.
    ///
    /// A press outside the selection places the cursor first, the way every
    /// editor does: the menu is about what is under the pointer, and a menu
    /// offering to cut something the reader cannot see is offering nothing.
    fn open_editor_menu(&mut self, pane: crate::panes::PaneId) {
        self.focus_pane(pane);
        if let (Some(pointer), Some(file)) = (self.pointer, self.active_file()) {
            let at = file.borrow().position_at(pointer);
            let selection = file.borrow().buffer().selection();
            if selection.is_empty() || at < selection.start() || at > selection.end() {
                self.place_cursor(at, false);
            }
        }
        self.open_menu(MenuTarget::Text(pane));
    }

    /// Starts another shell in the worktree the window is pointed at.
    fn start_shell(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(root) = self.root_of(scope) else {
            return;
        };
        let env = self.worktree_env(scope);
        self.terminals.start(scope, &root, &env);
        self.show_panel(PanelView::Terminal);
    }

    /// Ends one shell, closing the panel when it was the worktree's last.
    ///
    /// An empty panel is a panel with nothing to show, so it goes away the
    /// way it would have if the shell had exited on its own.
    fn stop_shell(&mut self, shell: crate::terminal::ShellId) {
        let Some(scope) = self.scope() else {
            return;
        };
        self.tasks.stop_shell(scope, shell);
        self.hear_finished_tasks();
        self.terminals.stop(scope, shell);
        self.close_empty_panel();
    }

    /// Scrolls the terminal by a drag on its scrollbar.
    ///
    /// The offset the drag started from is remembered, because every frame
    /// of the drag reports travel from the same press: adding the travel to
    /// where the view has already moved would run away from the pointer.
    fn drag_terminal_scrollbar(&mut self, event: ResizeEvent, lines_per_pixel: f32) {
        let Some(shell) = self.scope().and_then(|scope| self.terminals.active(scope)) else {
            return;
        };

        let mut shell = shell.borrow_mut();
        let base = match event.phase {
            ResizePhase::Started => shell.grid().offset(),
            _ => self
                .terminal_scroll_origin
                .unwrap_or_else(|| shell.grid().offset()),
        };
        self.terminal_scroll_origin = match event.phase {
            ResizePhase::Ended => None,
            _ => Some(base),
        };

        let travelled = event.delta(Axis::Vertical) * lines_per_pixel;
        let offset = (base as f32 - travelled).round().max(0.0) as usize;
        shell.scroll_to(offset);
    }

    /// The shell keystrokes are going to, if any is focused.
    pub(super) fn focused_shell(&self) -> Option<Shell> {
        if !self.terminal_focused || !self.showing_terminals() {
            return None;
        }
        self.terminals.active(self.scope()?)
    }

    /// The theme this frame is drawn from: the chosen family, in whichever
    /// appearance the theme mode resolves to, with the reader's colours
    /// painted over it and set in the reader's fonts.
    pub(super) fn theme(&self) -> Theme {
        let preferences = &self.preferences;
        let appearance = preferences.theme_mode.resolve(self.system_appearance());
        let chosen = crate::theme::family(preferences.theme_family).variant(appearance);
        Theme {
            text: preferences.fonts.scale(chosen.text),
            ..preferences.theme_overrides.apply(chosen)
        }
        .zoomed(self.zoom)
    }

    /// The appearance the desktop asks for, defaulting to dark.
    fn system_appearance(&self) -> Appearance {
        match self.window.as_ref().and_then(|window| window.theme()) {
            Some(winit::window::Theme::Light) => Appearance::Light,
            _ => Appearance::Dark,
        }
    }

    /// Folds a message in, writes the preferences down and redraws.
    fn apply(&mut self, message: Message) {
        if self.apply_outline(message) {
            self.request_redraw();
            return;
        }
        if let Message::ShowTabMenu(pane, item) = message {
            self.open_menu(MenuTarget::Tab(pane, item));
            return;
        }
        if let Message::ShowChangeMenu(index) = message {
            self.aim_at_change(index);
            self.open_menu(MenuTarget::Change);
            return;
        }
        if let Message::ShowHistoryMenu(repository, row) = message {
            self.history_menu_scope = self.scope();
            self.history_menu_object = self
                .review()
                .filter(|review| review.active() == repository)
                .and_then(|review| review.history(self.history_all).get(row))
                .map(|commit| commit.object.clone());
            if self.history_menu_object.is_some() {
                self.open_menu(MenuTarget::History(repository, row));
            }
            return;
        }
        if let Message::ChoosePrompt(place) = message {
            let taken = self.prompt.take().and_then(|asked| asked.taken(place));
            match taken {
                Some(taken) => return self.apply(taken),
                None => return self.request_redraw(),
            }
        }
        if message == Message::DismissPrompt {
            self.prompt = None;
            self.request_redraw();
            return;
        }
        if message == Message::ConfirmAbortMerge {
            self.change_by(Review::abort_operation);
            return;
        }
        if message == Message::ConfirmAmend {
            self.change_by(Review::amend);
            return;
        }
        if message == Message::ConfirmSkipOperation {
            self.change_by(Review::skip_operation);
            return;
        }
        if message == Message::ConfirmDiscard {
            self.discard_change();
            self.request_redraw();
            return;
        }
        if let Message::ShowPaneMenu(pane) = message {
            self.open_menu(MenuTarget::Pane(pane));
            return;
        }
        if let Message::ShowTerminalMenu(id) = message {
            self.open_menu(MenuTarget::Terminal(id));
            return;
        }
        if let Message::ProjectMenu(id) = message {
            self.open_project_menu(id);
            return;
        }
        if message == Message::AddProjectMenu {
            self.open_menu(MenuTarget::Projects);
            return;
        }
        if message == Message::ShowSessionBases {
            self.showing_bases = !self.showing_bases;
            self.request_redraw();
            return;
        }
        if message == Message::ShowHistoryRefsMenu {
            let bounds = self.history_refs_bounds.get();
            self.menu = Some(TabMenu {
                at: Point::new((bounds.right() - 190.0).max(8.0), bounds.bottom() + 2.0),
                target: MenuTarget::HistoryRefs,
            });
            self.request_redraw();
            return;
        }
        if let Message::SetHistoryFilter(all) = message {
            self.history_all = all;
            self.store();
            self.request_redraw();
            return;
        }
        if message == Message::RevealCurrentHistoryItem {
            self.history_all = false;
            self.store();
            self.request_redraw();
            return;
        }
        if message == Message::ShowCommitMenu {
            let bounds = self.commit_bounds.get();
            self.menu = Some(TabMenu {
                at: Point::new((bounds.right() - 190.0).max(8.0), bounds.bottom() + 2.0),
                target: MenuTarget::Commit,
            });
            self.request_redraw();
            return;
        }
        if message == Message::ShowSourceControlMenu {
            self.open_menu(MenuTarget::SourceControl);
            return;
        }
        if let Message::ShowAgentsMenu(standing) = message {
            self.open_agents_menu(standing);
            return;
        }
        if let Message::ShowEntryMenu(_) | Message::ShowTreeMenu = message {
            self.tree_command(message);
            return;
        }
        self.menu = None;
        if self.tree_command(message) {
            return;
        }
        if self.diagram_command(message) {
            return;
        }
        if self.debug_command(message) {
            self.request_redraw();
            return;
        }
        if self.tab_command(message) {
            self.request_redraw();
            return;
        }
        if let Message::ResizeSidebar(event) = message {
            self.sidebar
                .resize(event, Axis::Horizontal, ResizeEdge::End);
            self.store_settled(event);
            self.request_redraw();
            return;
        }
        if let Message::ResizeBottomPanel(event) = message {
            self.bottom_panel
                .resize(event, Axis::Vertical, ResizeEdge::Start);
            self.store_settled(event);
            self.request_redraw();
            return;
        }
        if let Message::ResizeSecondarySidebar(event) = message {
            self.secondary_sidebar
                .resize(event, Axis::Horizontal, ResizeEdge::Start);
            self.store_settled(event);
            self.request_redraw();
            return;
        }
        if let Message::ResizeHistoryGraph(event) = message {
            self.history_graph
                .resize(event, Axis::Vertical, ResizeEdge::Start);
            self.store_settled(event);
            self.request_redraw();
            return;
        }
        if let Message::ResizeAgentPrompt(event) = message {
            self.prompt_box
                .resize(event, Axis::Vertical, ResizeEdge::Start);
            self.store_settled(event);
            self.request_redraw();
            return;
        }
        if message == Message::ToggleHistoryGraph {
            self.history_graph_open = !self.history_graph_open;
            self.store();
            self.request_redraw();
            return;
        }
        if message == Message::ToggleChangesSection {
            self.changes_section_open = !self.changes_section_open;
            self.store();
            self.request_redraw();
            return;
        }
        if message == Message::TogglePrimarySidebar {
            self.primary_sidebar_open = !self.primary_sidebar_open;
            self.store();
            self.request_redraw();
            return;
        }
        if message == Message::ToggleBottomPanel {
            self.bottom_panel_open = !self.bottom_panel_open;
            self.terminal_focused = self.showing_terminals();
            self.store();
            self.request_redraw();
            return;
        }
        if self.panel_command(message) {
            self.request_redraw();
            return;
        }
        if let Message::PointTerminal(phase, anchor, head) = message {
            self.point_terminal(phase, anchor, head);
            self.request_redraw();
            return;
        }
        if message == Message::ShowScreenMenu {
            self.focus_terminal();
            self.open_menu(MenuTarget::Screen);
            return;
        }
        if let Message::Act(action) = message {
            return self.act(action);
        }
        if let Message::ActOnTerminal(action) = message {
            self.focus_terminal();
            self.act(action);
            return;
        }
        if let Message::SelectItem(pane, item) = message {
            self.select_tab(pane, item);
            self.request_redraw();
            return;
        }
        if let Message::DragTab(pane, item, event) = message {
            self.drag_tab(pane, item, event);
            self.request_redraw();
            return;
        }
        if let Message::CloseItem(pane, item) = message {
            self.close_item(pane, item);
            self.request_redraw();
            return;
        }
        if let Message::SaveAndClose(pane, file) = message {
            self.save_and_close(pane, file);
            self.request_redraw();
            return;
        }
        if let Message::DiscardAndClose(pane, file) = message {
            self.discard_and_close(pane, file);
            self.request_redraw();
            return;
        }
        if let Message::FocusPane(pane) = message {
            self.focus_pane(pane);
            self.request_redraw();
            return;
        }
        if let Message::SplitPane(pane, direction) = message {
            self.split_pane(pane, None, direction);
            self.request_redraw();
            return;
        }
        if let Message::SplitItem(pane, item, direction) = message {
            self.split_pane(pane, Some(item), direction);
            self.request_redraw();
            return;
        }
        if let Message::PreviewFile(pane) = message {
            self.open_file_preview(pane);
            self.request_redraw();
            return;
        }
        if let Message::ClosePane(pane) = message {
            self.close_pane(pane);
            self.request_redraw();
            return;
        }
        if let Message::ResizeSplit(split, divider, event, scale) = message {
            if let Some(axis) = self.panes.split_axis(split) {
                let scope = self.scope();
                self.panes.resize(
                    split,
                    divider,
                    event.delta(axis) * scale,
                    event.phase,
                    scope,
                );
            }
            self.store_settled(event);
            self.request_redraw();
            return;
        }
        if let Message::SelectText(pane, phase, anchor, head) = message {
            self.select_text(pane, phase, anchor, head);
            self.request_redraw();
            return;
        }
        if let Message::ScrollEditor(pane, axis, event, step) = message {
            self.drag_editor_scrollbar(pane, axis, event, step);
            self.request_redraw();
            return;
        }
        if let Message::SelectExcerpt(pane, phase, file, anchor, head) = message {
            self.select_excerpt(pane, phase, file, anchor, head);
            self.request_redraw();
            return;
        }
        if let Message::OpenExcerptFile(pane, index) = message {
            self.open_excerpt_file(pane, index);
            self.request_redraw();
            return;
        }
        if let Message::JumpTo(pane, at) = message {
            self.focus_pane(pane);
            if let Some(from) = self.here() {
                self.trail.jumped(from);
            }
            self.place_cursor(at, false);
            self.request_redraw();
            return;
        }
        if let Message::ScrollEditorTo(pane, line) = message {
            self.focus_pane(pane);
            self.with_document(|document| {
                let half = document.rows() / 2;
                document.scroll_to(line.saturating_sub(half));
            });
            self.request_redraw();
            return;
        }
        if let Message::SelectLines(pane, anchor, head) = message {
            self.select_lines(pane, anchor, head);
            self.request_redraw();
            return;
        }
        if let Message::ToggleFold(pane, at) = message {
            self.focus_pane(pane);
            self.with_document(|document| document.toggle_fold(at.line));
            self.request_redraw();
            return;
        }
        if let Message::ShowEditorMenu(pane) = message {
            self.open_editor_menu(pane);
            return;
        }
        if let Message::PlacePicker(caret) = message {
            if let Some(picker) = self.picker.as_mut() {
                picker.edit(|field| field.place(caret));
            }
            self.request_redraw();
            return;
        }
        if let Message::ChoosePicker(place) = message {
            self.choose_picker(place);
            self.request_redraw();
            return;
        }
        if let Message::ChooseCompletion(place) = message {
            self.take_completion(place);
            self.request_redraw();
            return;
        }
        if let Message::TakeCodeAction(index) = message {
            self.take_code_action(index);
            self.request_redraw();
            return;
        }
        if message == Message::DismissPopup {
            self.dismiss_popup();
            self.dismiss_picker();
            self.request_redraw();
            return;
        }
        if let Message::PaneAction(pane, action) = message {
            self.focus_pane(pane);
            return self.act(action);
        }
        if let Message::FocusSearch(pane, field, caret) = message {
            self.focus_pane(pane);
            self.focus_search(field, caret);
            self.request_redraw();
            return;
        }
        if let Message::FocusProjectSearch(pane, field, caret) = message {
            self.focus_pane(pane);
            if let Some(Item::Search(scope)) = self.active_tab()
                && let Some(search) = self.searches.get_mut(&scope)
            {
                match field {
                    editor::SearchField::Query => search.query.place(caret),
                    editor::SearchField::Replacement => search.replacement.place(caret),
                }
                self.project_search_field = Some(field);
            }
            self.request_redraw();
            return;
        }
        if let Message::ToggleProjectSearch(pane, option) = message {
            self.focus_pane(pane);
            self.toggle_project_search(option);
            self.request_redraw();
            return;
        }
        if message == Message::ConfirmProjectReplace {
            self.replace_project(false, true);
            self.request_redraw();
            return;
        }
        if let Message::ToggleSearchCase(pane) = message {
            self.focus_pane(pane);
            self.with_document(|document| {
                document.search_with(super::editor::Search::toggle_case);
            });
            self.request_redraw();
            return;
        }
        if let Message::ToggleSearchWord(pane) = message {
            self.focus_pane(pane);
            self.with_document(|document| {
                document.search_with(super::editor::Search::toggle_whole_word);
            });
            self.request_redraw();
            return;
        }
        if let Message::ToggleSearchRegex(pane) = message {
            self.focus_pane(pane);
            self.with_document(|document| {
                document.search_with(super::editor::Search::toggle_regex);
            });
            self.request_redraw();
            return;
        }
        if let Message::CloseSearch(pane) = message {
            self.focus_pane(pane);
            self.close_search();
            self.request_redraw();
            return;
        }
        if let Message::ToggleSearchReplace(pane) = message {
            self.focus_pane(pane);
            self.search_focused = true;
            self.with_document(|document| {
                document.search_with(|search, _| search.toggle_replacing());
            });
            self.request_redraw();
            return;
        }
        if message == Message::NewTerminal {
            self.start_shell();
            self.terminal_focused = true;
            self.request_redraw();
            return;
        }
        if let Message::SelectTerminal(id) = message {
            if let Some(scope) = self.scope() {
                self.terminals.activate(scope, id);
            }
            self.terminal_focused = true;
            self.request_redraw();
            return;
        }
        if let Message::ScrollTerminal(event, lines_per_pixel) = message {
            self.drag_terminal_scrollbar(event, lines_per_pixel);
            self.request_redraw();
            return;
        }
        if let Message::CloseTerminal(id) = message {
            self.stop_shell(id);
            self.request_redraw();
            return;
        }
        if let Message::SetSidebarView(view) = message {
            self.secondary_sidebar_view = view;
            self.secondary_sidebar_open = true;
            self.store();
            self.request_redraw();
            return;
        }
        if message == Message::ShowStatusBranches {
            self.branch_picker_at = self.pointer;
            self.open_picker(crate::picker::Kind::Branches);
            self.request_redraw();
            return;
        }
        if message == Message::CreateTypedBranch {
            let name = self
                .picker
                .as_ref()
                .map(|picker| picker.field().value().trim().to_owned())
                .unwrap_or_default();
            self.picker = None;
            self.create_branch(&name);
            self.request_redraw();
            return;
        }
        if message == Message::PushBranch {
            self.push_branch();
            self.request_redraw();
            return;
        }
        if message == Message::SyncBranch {
            self.remote_operation(RemoteOperation::Sync, pm_core::sync);
            self.request_redraw();
            return;
        }
        if message == Message::Fetch {
            self.remote_operation(RemoteOperation::Fetch, pm_core::fetch);
            self.request_redraw();
            return;
        }
        if message == Message::Pull {
            self.remote_operation(RemoteOperation::Pull, |root| pm_core::pull(root, false));
            self.request_redraw();
            return;
        }
        if message == Message::PullRebase {
            self.remote_operation(RemoteOperation::Pull, |root| pm_core::pull(root, true));
            self.request_redraw();
            return;
        }
        if message == Message::ForcePush {
            self.remote_operation(RemoteOperation::Push, pm_core::force_push);
            self.request_redraw();
            return;
        }
        if message == Message::ChooseFetchRemote {
            self.open_picker(crate::picker::Kind::FetchRemotes);
            self.request_redraw();
            return;
        }
        if message == Message::ChoosePushRemote {
            self.open_picker(crate::picker::Kind::PushRemotes);
            self.request_redraw();
            return;
        }
        if self.agent_command(message) {
            self.request_redraw();
            return;
        }
        if self.notice_command(message) {
            self.request_redraw();
            return;
        }
        if let Message::InstallLanguageServer(command) = message {
            self.start_server_install(command, true);
            return;
        }
        if message == Message::OpenServerLog {
            self.open_server_log();
            return self.request_redraw();
        }
        if self.session_command(message) {
            self.request_redraw();
            return;
        }
        if self.review_command(message) {
            self.request_redraw();
            return;
        }
        if message == Message::ToggleSecondarySidebar {
            self.secondary_sidebar_open = !self.secondary_sidebar_open;
            self.store();
            self.request_redraw();
            return;
        }
        if message == Message::OpenProject {
            self.ask_project();
            return;
        }
        if message == Message::CloneProject {
            self.open_picker(crate::picker::Kind::CloneUrl);
            self.request_redraw();
            return;
        }
        if let Message::CloseProject(id) = message {
            if let Some(root) = self
                .open
                .get(id)
                .map(|project| project.root().to_path_buf())
            {
                let mut roots = vec![root];
                roots.extend(self.sessions.of(id).flat_map(|session| {
                    std::iter::once(session.root())
                        .chain(session.roots())
                        .map(|root| root.to_path_buf())
                }));
                self.editor.close_project(id, &roots);
                self.images.close_project(id);
            }
            self.open.remove(id);
            self.files.retain(|scope, _| scope.project() != id);
            self.reviews.retain(|scope, _| scope.project() != id);
            self.excerpts.retain(|scope, _| scope.project() != id);
            self.trail.close_project(id);
            self.terminals.close(id);
            self.tasks.forget(id);
            self.pending_debug
                .retain(|_, (scope, _)| scope.project() != id);
            self.debuggers.forget(|scope| scope.project() == id);
            self.agents.close_project(id);
            self.sessions.close_project(id);
            self.drop_project_tabs(id);
            self.store();
            self.request_redraw();
            return;
        }
        if let Message::DragProject(id, event) = message {
            self.drag_project(id, event);
            self.request_redraw();
            return;
        }
        if message == Message::MinimizeWindow {
            if let Some(window) = self.window.as_ref() {
                window.set_minimized(true);
            }
            return;
        }
        if message == Message::ToggleMaximizedWindow {
            if let Some(window) = self.window.as_ref() {
                window.set_maximized(!window.is_maximized());
            }
            return;
        }
        if message == Message::CloseWindow {
            self.close_requested = true;
            return;
        }
        if let Message::ShowSettingsPage(page) = message {
            self.settings.show(page);
            self.request_redraw();
            return;
        }
        if let Message::ShowSettingsSection(section) = message {
            self.settings.show_section(section);
            self.request_redraw();
            return;
        }
        if let Message::ToggleSettingsPage(page) = message {
            self.settings.toggle(page);
            self.request_redraw();
            return;
        }
        if message == Message::OpenSettings {
            self.open_settings();
            self.request_redraw();
            return;
        }
        if message == Message::OpenRepository {
            desktop::browse(crate::release::REPOSITORY);
            return;
        }
        if let Message::AddWorktreePath(list) = message {
            self.ask_worktree_path(list);
            self.request_redraw();
            return;
        }
        if message == Message::EditWorktreePort {
            self.ask_worktree_port();
            self.request_redraw();
            return;
        }
        if self.settings_command(message) {
            self.request_redraw();
            return;
        }
        if message == Message::Finish {
            self.onboarded = true;
        }
        if self.preferences.apply(message) {
            self.follow_preferences();
        }
        if matches!(
            message,
            Message::SetKeymap(_)
                | Message::ResetPreference(
                    Preference::Keymap | Preference::Keybindings | Preference::Binding(_)
                )
        ) {
            self.resolver.set_keymap(self.preferences.keymap_in_force());
        }
        self.store();
        self.request_redraw();
    }

    /// Carries out a command about what a project has changed, if `message`
    /// is one.
    ///
    /// Every one of them ends the same way — git is asked to do something and
    /// then asked what the worktree now holds — so they are gathered here
    /// rather than spread through the window's own state.
    fn review_command(&mut self, message: Message) -> bool {
        match message {
            Message::OpenReview => self.open_review(),
            Message::OpenExcerpts => self.open_excerpts(),
            Message::RefreshChanges => self.refresh_changes(),
            Message::ToggleChangeStaged(index) => self.toggle_change_staged(index),
            Message::ToggleGroupStaged(repository, group) => {
                self.toggle_group_staged(repository, group);
            }
            Message::ToggleHunkStaged(index, staged, hunk) => {
                self.toggle_hunk_staged(index, staged, hunk);
            }
            Message::RestoreHunk(index, staged, hunk) => self.restore_hunk(index, staged, hunk),
            Message::SelectChange(index) => {
                let marking = self.modifiers.super_key() || self.modifiers.control_key();
                self.select_change(index, marking, self.modifiers.shift_key());
            }
            Message::StageSelection => self.change_selection(Review::stage),
            Message::UnstageSelection => self.change_selection(Review::unstage),
            Message::DiscardSelection => self.ask_to_discard(),
            Message::StageAll => self.change_by(Review::stage_all),
            Message::UnstageAll => self.change_by(Review::unstage_all),
            Message::Commit => self.change_by(Review::commit),
            Message::Amend => {
                let filled = self.review_mut().is_some_and(Review::prefill_last_message);
                if !filled {
                    let pushed = self
                        .review()
                        .and_then(|review| review.head())
                        .is_some_and(|head| head.upstream.is_some() && head.ahead == 0);
                    if pushed {
                        self.ask_first(crate::prompt::Prompt::asking(
                            "Rewrite pushed commit?".to_owned(),
                            vec!["A force push will be needed.".to_owned()],
                            vec![
                                crate::prompt::Answer::new("Amend", Message::ConfirmAmend),
                                crate::prompt::Answer::cancel(),
                            ],
                        ));
                    } else {
                        self.change_by(Review::amend);
                    }
                }
            }
            Message::StashPush => self.open_picker(crate::picker::Kind::StashMessage),
            Message::ShowStashes(action) => {
                self.stash_action = Some(action);
                self.open_picker(crate::picker::Kind::Stashes);
            }
            Message::DropStash(index) => self.ask_first(crate::prompt::Prompt::asking(
                "Drop stash?".to_owned(),
                vec![format!("stash@{{{index}}} will be removed.")],
                vec![
                    crate::prompt::Answer::new("Drop Stash", Message::ConfirmDropStash(index)),
                    crate::prompt::Answer::cancel(),
                ],
            )),
            Message::ConfirmDropStash(index) => self
                .change_by(|review| review.stash_action(index, crate::review::StashAction::Drop)),
            Message::CherryPickHistory => {
                if self.scope() == self.history_menu_scope
                    && let Some(object) = self.history_menu_object.take()
                {
                    self.change_by(|review| review.cherry_pick(object));
                }
            }
            Message::CopyCommitHash => {
                if let Some(object) = self.history_menu_object.take() {
                    desktop::copy(object);
                }
            }
            Message::AbortMerge => {
                let name = self
                    .review()
                    .and_then(|review| review.head())
                    .and_then(|head| head.operation.as_ref())
                    .map(pm_core::Operation::name)
                    .unwrap_or("Operation");
                self.ask_first(crate::prompt::Prompt::asking(
                    format!("Abort {name}?"),
                    vec![format!("The {name} resolution will be discarded.")],
                    vec![
                        crate::prompt::Answer::new(
                            format!("Abort {name}"),
                            Message::ConfirmAbortMerge,
                        ),
                        crate::prompt::Answer::cancel(),
                    ],
                ));
            }
            Message::SkipOperation => self.ask_first(crate::prompt::Prompt::asking(
                "Skip commit?".to_owned(),
                vec!["The stopped commit will be skipped.".to_owned()],
                vec![
                    crate::prompt::Answer::new("Skip Commit", Message::ConfirmSkipOperation),
                    crate::prompt::Answer::cancel(),
                ],
            )),
            Message::CommitAndPush => {
                self.change_by(|review| review.commit().map(crate::review::Work::then_push));
            }
            Message::OpenChange(index) => self.open_change(index),
            Message::OpenChangeDiff(index) => self.open_change_diff(index),
            Message::PreviousHunk => self.step_hunk(false),
            Message::NextHunk => self.step_hunk(true),
            Message::OpenChangeFile(index) => self.open_change_file(index),
            Message::ConflictAction(file, line, action) => {
                self.conflict_action(file, line, action);
            }
            Message::CopyChangePath(index) => self.copy_changed_path(index, false),
            Message::CopyChangeRelativePath(index) => self.copy_changed_path(index, true),
            Message::RevealChange(index) => self.reveal_change(index),
            Message::ExpandChange(index) => {
                let marking = self.modifiers.super_key() || self.modifiers.control_key();
                let ranging = self.modifiers.shift_key();
                match marking || ranging {
                    true => self.select_change(index, marking, ranging),
                    false => {
                        if let Some(review) = self.review_mut() {
                            review.toggle(index);
                        }
                    }
                }
            }
            Message::ShowInputMenu => self.open_menu(MenuTarget::Input),
            Message::EditText(action) => self.act(action),
            Message::WriteCommit(repository, phase, anchor, head) => {
                if let Some(review) = self.review_mut() {
                    review.activate(repository);
                }
                self.point_in(Writing::Commit, phase, anchor, head);
            }
            Message::InRepository(repository, action) => self.in_repository(repository, action),
            _ => return false,
        }
        true
    }

    /// Has git carry out the work `change` makes of the review, away from
    /// the window, and reads the worktree again after it.
    fn change_by(&mut self, change: impl FnOnce(&Review) -> Option<crate::review::Work>) {
        let work = self.review().and_then(change);
        self.work_here(work);
    }

    /// Opens the menu for `target` where the pointer is.
    ///
    /// The menu is placed rather than anchored: the pointer is the one place
    /// every tab, however narrow and however far along the bar, agrees on.
    fn open_menu(&mut self, target: MenuTarget) {
        if target == MenuTarget::SourceControl {
            self.stash_available = self
                .review()
                .is_some_and(|review| !review.stashes().is_empty());
        }
        self.menu = self.pointer.map(|at| TabMenu { at, target });
        self.request_redraw();
    }

    /// Asks `question`, which nothing else answers until it is answered.
    fn ask_first(&mut self, question: crate::prompt::Prompt) {
        self.prompt = Some(question);
        self.release_pane_focus();
        self.request_redraw();
    }

    /// Puts away the question that is open, saying whether there was one.
    pub(super) fn dismiss_prompt(&mut self) -> bool {
        self.prompt.take().is_some()
    }

    /// Puts away the menu that is open, saying whether there was one.
    pub(super) fn dismiss_menu(&mut self) -> bool {
        self.menu.take().is_some()
    }

    /// Carries out a command from a tab menu, if `message` is one.
    ///
    /// The menu commands are collected here because they are one family:
    /// every one of them acts on the tabs of one pane or on the file behind
    /// one of them, and none of them touches the window's own state.
    fn tab_command(&mut self, message: Message) -> bool {
        match message {
            Message::DismissMenu => {}
            Message::CloseOtherTabs(pane, item) => {
                self.close_saved_tabs(pane, |held| held == item);
            }
            Message::CloseTabsLeft(pane, item) => {
                let kept = self.tabs_from(pane, item, false);
                self.close_saved_tabs(pane, move |held| kept.contains(&held));
            }
            Message::CloseTabsRight(pane, item) => {
                let kept = self.tabs_from(pane, item, true);
                self.close_saved_tabs(pane, move |held| kept.contains(&held));
            }
            Message::CloseSavedTabs(pane) => self.close_saved_tabs(pane, |_| false),
            Message::CloseAllTabs(pane) => self.close_every_tab(pane),
            Message::CopyFilePath(file) => {
                if let Some(path) = self.editor.path(file) {
                    desktop::copy(path.display().to_string());
                }
            }
            Message::CopyFileRelativePath(file) => {
                if let Some(path) = self.relative_path(file) {
                    desktop::copy(path);
                }
            }
            Message::RevealFile(file) => {
                if let Some(path) = self.editor.path(file) {
                    desktop::reveal(&path);
                }
            }
            Message::OpenFileInTerminal(file) => self.start_shell_beside(file),
            Message::KeepFileOpen(file) => self.editor.keep(file),
            Message::TogglePin(pane, item) => self.toggle_pin(pane, item),
            Message::CloseOtherTerminals(id) => {
                if let Some(scope) = self.scope() {
                    self.tasks.stop_shells_except(scope, Some(id));
                    self.hear_finished_tasks();
                    self.terminals.stop_others(scope, id);
                }
            }
            Message::CloseAllTerminals => {
                if let Some(scope) = self.scope() {
                    self.tasks.stop_shells_except(scope, None);
                    self.hear_finished_tasks();
                    self.terminals.stop_all(scope);
                }
                self.close_empty_panel();
            }
            _ => return false,
        }
        true
    }

    /// The path of the file `id` names, from its own worktree down.
    fn relative_path(&self, id: crate::editor::FileId) -> Option<String> {
        let root = self.root_of(self.editor.scope_of(id)?)?;
        let path = self.editor.path(id)?;
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        Some(relative.display().to_string())
    }

    /// Starts a shell in the directory the file `id` names sits in.
    fn start_shell_beside(&mut self, id: crate::editor::FileId) {
        let Some(scope) = self.editor.scope_of(id) else {
            return;
        };
        let Some(directory) = self
            .editor
            .path(id)
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        else {
            return;
        };
        let env = self.worktree_env(scope);
        self.terminals.start(scope, &directory, &env);
        self.show_panel(PanelView::Terminal);
        self.terminal_focused = true;
        self.editor_focused = false;
    }

    /// Fetches the repository at `url` into a directory the reader picks.
    ///
    /// A clone is the one git operation that takes as long as the network
    /// does and has no worktree to report into, so it runs on a thread of
    /// its own and the window is told when it lands rather than waiting for
    /// it. Where it goes is asked first, because that is the reader's.
    pub(super) fn clone_project(&mut self, url: &str) {
        let url = url.trim().to_owned();
        if url.is_empty() {
            return;
        }
        self.ask_clone_into(url);
    }

    /// Fetches the repository at `url` into `under`, on a thread of its own.
    fn clone_into(&mut self, url: String, under: PathBuf) {
        let cloned = self.cloned.clone();
        let wake = self.waker(Wake::Clone);
        std::thread::spawn(move || {
            let landed = pm_core::clone(&url, &under);
            if let Ok(mut cloned) = cloned.lock() {
                cloned.push(landed);
            }
            wake();
        });
    }

    /// Opens what a clone has finished fetching, or says why it did not.
    fn take_clones(&mut self) {
        let finished = self
            .cloned
            .lock()
            .map(|mut cloned| std::mem::take(&mut *cloned))
            .unwrap_or_default();

        for landed in finished {
            match landed {
                Ok(root) => {
                    let _ = self.open.find_or_open(root);
                    self.read_new_worktrees();
                    self.store();
                }
                Err(trouble) => self.say_trouble("The repository could not be cloned", &trouble),
            }
        }
    }

    /// Reads the worktree of any project the window has just opened.
    fn read_new_worktrees(&mut self) {
        let missing = self
            .open
            .iter()
            .map(|project| Scope::checkout(project.id()))
            .filter(|scope| !self.files.contains_key(scope))
            .collect::<Vec<_>>();
        for scope in missing {
            self.point_at(scope);
        }
    }

    /// The window as it stands, in the shape a launch restores it from.
    fn state(&self) -> Restored {
        Restored {
            preferences: self.preferences.clone(),
            onboarded: self.onboarded,
            projects: self.open.roots(),
            active: self
                .open
                .active()
                .map(|project| project.root().to_path_buf()),
            layout: self.layout(),
            panes: self.saved_panes(),
            window: self.window_state,
            language_servers: self.language_servers.clone(),
            agent_servers: self.agent_servers.clone(),
        }
    }

    /// Which regions are showing right now, and how large they are.
    fn layout(&self) -> Layout {
        Layout {
            primary_sidebar_open: self.primary_sidebar_open,
            primary_sidebar_width: self.sidebar.extent(),
            bottom_panel_open: self.bottom_panel_open,
            bottom_panel_height: self.bottom_panel.extent(),
            secondary_sidebar_open: self.secondary_sidebar_open,
            secondary_sidebar_width: self.secondary_sidebar.extent(),
            secondary_sidebar_view: self.secondary_sidebar_view,
            history_graph_height: self.history_graph.extent(),
            history_graph_open: self.history_graph_open,
            changes_section_open: self.changes_section_open,
            history_all: self.history_all,
            prompt_height: self.prompt_box.extent(),
        }
    }

    /// Takes down the window's size, keeping the size it un-maximizes to.
    ///
    /// A maximized window's size is the screen's, not the one the next launch
    /// should open at, so only its state is taken down while it is maximized.
    fn remember_window(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let maximized = window.is_maximized();
        if !maximized {
            let scale = window.scale_factor() as f32;
            let size = window.inner_size();
            self.window_state.width = size.width as f32 / scale;
            self.window_state.height = size.height as f32 / scale;
        }
        self.window_state.maximized = maximized;
    }

    /// Writes the window down once a resize gesture has come to rest.
    ///
    /// A drag reports every pointer move; the file is only interested in
    /// where the edge was let go of.
    fn store_settled(&mut self, event: ResizeEvent) {
        if event.phase == ResizePhase::Ended {
            self.store();
        }
    }

    /// The box of text that has the keyboard, to write in.
    pub(super) fn written_in(&mut self) -> Option<&mut crate::input::Input> {
        match self.writing? {
            Writing::Commit => self.review_mut()?.message_mut(),
            Writing::Prompt(session) => self
                .agents
                .get_mut(session)
                .map(crate::agent::Talk::prompt_mut),
            Writing::Console(scope) => self
                .debuggers
                .get_mut(scope)
                .map(crate::debug::Debugger::console_mut),
        }
    }

    /// Gives the keyboard to `writing`, taking it from whatever had it.
    pub(super) fn write_in(&mut self, writing: Writing) {
        self.release_pane_focus();
        self.writing = Some(writing);
    }

    /// Answers a press, a drag or a release of the pointer in a box of text.
    ///
    /// A press in a box is also what gives it the keyboard, so this is the
    /// whole of how one is written in: there is nothing to focus first. The
    /// presses are counted the way they are in the editor, so a box selects a
    /// word on the second and its line on the third. Shift keeps the
    /// selection's anchor and moves its head to the pointer.
    pub(super) fn point_in(
        &mut self,
        writing: Writing,
        phase: ResizePhase,
        anchor: Position,
        head: Position,
    ) {
        self.write_in(writing);
        let pressed = phase == ResizePhase::Started;
        let still = anchor == head;
        if pressed {
            self.text_extends = self.extends_text();
        }
        if still && !pressed {
            return;
        }
        let extend = self.text_extends;
        let presses = match (still, extend) {
            (true, false) => self.text_clicks.press(anchor),
            _ => {
                self.text_clicks.clear();
                0
            }
        };
        if let Some(input) = self.written_in() {
            input.point(phase, anchor, head, presses, extend);
        }
    }

    /// Writes the window's preferences, projects and layout down.
    fn store(&mut self) {
        self.remember_window();
        config::save(&self.state());
    }

    /// Asks whether a newer release has been published, away from the window.
    fn look_for_release(&self) {
        let released = self.released.clone();
        let wake = self.waker(Wake::Release);
        std::thread::spawn(move || {
            if crate::release::available() {
                if let Ok(mut released) = released.lock() {
                    *released = true;
                }
                wake();
            }
        });
    }

    /// A handle the threads behind the window wake it with, sending `wake`.
    pub(super) fn waker(&self, wake: Wake) -> Arc<dyn Fn() + Send + Sync> {
        waker_through(&self.proxy, &self.pending, wake)
    }

    /// Asks the platform for another frame.
    fn request_redraw(&self) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    /// What is drawn over the panes: a picker, a completion list, a hint.
    ///
    /// Each is placed where it belongs rather than laid out — the picker
    /// over the command center in the title bar, a completion list under the
    /// word it completes, a hint beside the cursor — because none of them
    /// takes room from the screen they cover.
    fn overlays(&self, theme: &Theme) -> Vec<workspace::Overlaid> {
        let mut overlays = Vec::new();

        let window = self.renderer.as_ref().map_or(Size::zero(), Renderer::size);

        if let Some(picker) = self.picker.as_ref() {
            let agent_choices = matches!(
                picker.kind(),
                crate::picker::Kind::Modes | crate::picker::Kind::Knob
            );
            let branch_anchor = self.branch_picker_at.filter(|_| {
                matches!(
                    picker.kind(),
                    crate::picker::Kind::Branches | crate::picker::Kind::NewBranch
                )
            });
            let anchor = match agent_choices {
                true => self.agent_picker_at.map(|anchor| (anchor.x, anchor)),
                false => branch_anchor.map(|anchor| (anchor.x - 24.0, anchor)),
            };
            let (point, width) = match anchor {
                Some((left, anchor)) => {
                    let width = crate::picker::width(picker.kind());
                    let height = crate::picker::height(theme, picker);
                    let point = Point::new(
                        left.clamp(8.0, (window.width - width - 8.0).max(8.0)),
                        (anchor.y - height - 8.0).max(8.0),
                    );
                    (point, width)
                }
                None => self.over_command_center(window),
            };
            overlays.push(workspace::Overlaid {
                at: point,
                content: Box::new(crate::picker::picker(
                    theme,
                    picker,
                    width,
                    self.caret_solid(),
                )),
                backdrop: (!agent_choices).then_some(Message::DismissPopup),
            });
        }

        if let Some(asked) = self.prompt.as_ref() {
            let width = crate::prompt::WIDTH.min((window.width - 24.0).max(1.0));
            overlays.push(workspace::Overlaid {
                at: Point::new(0.0, 0.0),
                content: Box::new(
                    pm_ui::v_flex()
                        .w_px(window.width)
                        .h_px(window.height)
                        .items_center()
                        .justify_center()
                        .child(crate::prompt::prompt(theme, asked, width)),
                ),
                backdrop: Some(Message::DismissPrompt),
            });
        }

        if let Some(completions) = self.completions.as_ref() {
            overlays.push(workspace::Overlaid {
                at: completions.at(),
                content: Box::new(editor::completion_list(theme, completions)),
                backdrop: None,
            });
            if let Some(documentation) = completions.documentation() {
                let said = editor::Shown {
                    at: completions.beside(),
                    said: Some(documentation.to_owned()),
                    language: self
                        .active_file()
                        .and_then(|document| document.borrow().buffer().language()),
                    ..editor::Shown::default()
                };
                overlays.push(workspace::Overlaid {
                    at: said.at,
                    content: Box::new(editor::hint(theme, &said)),
                    backdrop: None,
                });
            }
        }

        if let Some(hint) = self.hint.as_ref().filter(|hint| !hint.is_empty()) {
            overlays.push(workspace::Overlaid {
                at: hint.at,
                content: Box::new(editor::hint(theme, hint)),
                backdrop: None,
            });
        }
        overlays
    }

    /// Where a picker not anchored to a control is drawn, and how wide: over
    /// the title bar's command center, as wide as it came out last frame.
    fn over_command_center(&self, window: Size) -> (Point, f32) {
        let bar = self.command_center_bounds.get();
        if bar.size.width <= 0.0 {
            let width = crate::picker::WIDTH;
            return (
                Point::new(window.width / 2.0 - width / 2.0, crate::picker::TOP),
                width,
            );
        }
        (Point::new(bar.origin.x, bar.origin.y), bar.size.width)
    }

    /// Where on screen the cursor of the focused pane last came out.
    pub(super) fn cursor_point(&self) -> Point {
        if let Some(Item::Excerpts(scope)) = self.active_tab()
            && let Some(caret) = self
                .excerpts
                .get(&scope)
                .and_then(|excerpts| excerpts.borrow().caret())
        {
            return caret;
        }
        if let Some(Item::Search(scope)) = self.active_tab()
            && let Some(caret) = self
                .searches
                .get(&scope)
                .and_then(|search| search.excerpts.borrow().caret())
        {
            return caret;
        }
        let Some(file) = self.active_file() else {
            return Point::new(0.0, 0.0);
        };
        let document = file.borrow();
        let head = document.buffer().selection().head;
        let at = document.point_of(head);
        Point::new(at.x, at.y + document.layout().cell.height)
    }

    /// Builds the frame and hands it to the renderer.
    fn draw(&mut self) {
        self.see_shown_agents();
        self.follow_agents();
        self.settle_excerpts();
        self.refresh_annotations();
        let shell = self
            .showing_terminals()
            .then(|| self.active_shell())
            .flatten();
        let shells = self
            .scope()
            .map(|scope| self.terminals.list(scope))
            .unwrap_or_default();
        let theme = self.theme();
        let panel = Panel {
            view: self.panel_view,
            shell,
            shells,
            focused: self.terminal_focused,
            linking: self.modifiers.control_key(),
            problems: match self.bottom_panel_open {
                true => self.problems(),
                false => Vec::new(),
            },
            problems_scroll: self.problems_scroll.clone(),
            problems_area: self.problems_area.clone(),
            debug: self.debug_in_panel(&theme),
        };
        let showing = self.active_file();
        let drop = self
            .drop_highlight()
            .or_else(|| {
                self.entry_drop_pane()
                    .and_then(|pane| self.geometry.pane_bounds(pane))
            })
            .or_else(|| self.project_caret());
        let carried = self.carried_tab().or_else(|| self.carried_entries());
        let tree_scroll = self.tree_scroll();
        let layout = self.layout();
        let editor = self.pane_view(&theme);
        let menu = self.menu_items();
        let overlays = self.overlays(&theme);
        let scope = self.scope();
        let sidebar = self.sidebar_projects();
        let caret = self.caret_solid();
        let files = workspace::Worktree {
            listing: scope.and_then(|scope| self.files.get(&scope)).map(|tree| {
                crate::tree::Listing {
                    tree,
                    review: scope.and_then(|scope| self.reviews.get(&scope)),
                    selection: scope.and_then(|scope| self.selections.get(&scope)),
                    edit: self.tree_edit.as_ref(),
                    clipboard: self.tree_clipboard.as_ref(),
                    dropping: self
                        .entry_drag
                        .as_ref()
                        .filter(|drag| drag.is_carried())
                        .and_then(|drag| drag.target.as_deref())
                        .or(self.arriving.as_deref()),
                    focused: self.tree_focused,
                    caret,
                    scroll: tree_scroll,
                    rows: self.tree_rows.clone(),
                    area: self.tree_area.clone(),
                    field: self.tree_field.clone(),
                }
            }),
            review: scope.and_then(|scope| self.reviews.get(&scope)),
            committing: self.writing == Some(Writing::Commit),
            caret,
            commit_bounds: self.commit_bounds.clone(),
            history_refs_bounds: self.history_refs_bounds.clone(),
            history_graph_bounds: self.history_graph_bounds.clone(),
            changes_area: self.changes_area.clone(),
            history_all: self.history_all,
            history_graph_height: layout.history_graph_height,
            history_graph_open: layout.history_graph_open,
            changes_section_open: layout.changes_section_open,
        };
        let activity = self
            .active_file_id()
            .and_then(|file| self.server_activity(file));
        let (Some(renderer), Some(ui), Some(list)) =
            (self.renderer.as_mut(), self.ui.as_mut(), self.list.as_mut())
        else {
            return;
        };

        ui.set_theme(theme);
        let fonts = &self.preferences.fonts;
        renderer.text().set_families(
            fonts.family(FontSlot::Interface),
            fonts.family(FontSlot::Buffer),
        );

        let size = renderer.size();
        self.scroll.set_viewport(size);
        list.reset(size);
        list.quad(Quad::filled(
            Rect::from_xywh(0.0, 0.0, size.width, size.height),
            theme.colors.background,
        ));

        let page = if self.onboarded {
            workspace::workspace(
                &theme,
                workspace::ProjectList {
                    open: &self.open,
                    sessions: &sidebar,
                    bounds: self.project_list.clone(),
                },
                files,
                layout,
                self.command_center_bounds.clone(),
                Panes {
                    editor,
                    recording: self.preferences.vim_mode.then(|| self.vim.recording()),
                    showing,
                    drop,
                    carried,
                    panel,
                    agents: self
                        .open
                        .active()
                        .map_or(0, |project| self.agents.count(project.id())),
                    tally: self.agents.tally(),
                    notice: self.notices.shown(),
                    activity,
                    menu,
                    overlays,
                },
                self.update_available,
            )
        } else {
            onboarding::page(&theme, &self.preferences)
        };
        let painted = ui.draw(
            renderer.text(),
            list,
            self.scroll.content_space(),
            self.scroll.origin(),
            page,
        );
        self.scroll.set_content_height(painted.height);

        renderer.render(list);
        self.update_pointer_cursor();
    }
}

impl ApplicationHandler<Wake> for App {
    /// Waits for the next event, for the pointer to have rested long enough,
    /// for the caret to turn over, or for the spinner's next frame.
    ///
    /// The window is otherwise woken only by something happening; a pointer
    /// holding still, a caret blinking and a remote being waited on are the
    /// things it has to notice by the clock.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.offer_missing_servers();
        if self.settle_moving() {
            self.request_redraw();
        }
        let expired = self.notices.expire(Instant::now());
        let seen = !self.window_occluded;
        let next_annotation = self.next_annotation().filter(|_| seen);
        let annotation_due = next_annotation.is_some_and(|at| at <= Instant::now());
        if (self.rested() || self.blinked() || (seen && self.spun()) || expired || annotation_due)
            && seen
        {
            self.request_redraw();
        }
        let next = [
            self.next_rest(),
            self.next_blink(),
            self.next_spin().filter(|_| seen),
            self.notices.next_expiry(),
            next_annotation,
            self.next_move(),
        ]
        .into_iter()
        .flatten()
        .min();
        event_loop.set_control_flow(match next {
            Some(when) => winit::event_loop::ControlFlow::WaitUntil(when),
            None => winit::event_loop::ControlFlow::Wait,
        });
    }

    /// Applies what the shells have written and draws the result.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: Wake) {
        self.pending[event as usize].store(false, Ordering::Release);
        match event {
            Wake::Terminal => {
                let pumped = self.terminals.pump();
                let tasks_pumped = self.tasks.pump();
                self.hear_finished_tasks();
                let logged_in = self.follow_logins();
                if pumped | tasks_pumped | self.follow_errands() | logged_in {
                    self.hear_failed_shells();
                    self.close_empty_panel();
                    self.request_redraw();
                }
            }
            Wake::Agent => {
                let before = self.agents.tally();
                if self.agents.pump() {
                    self.apply_agent_options();
                    self.serve_agents();
                    self.refresh_agent_history();
                    self.hear_ended_agents();
                    self.call_reader(before);
                    self.follow_agents();
                    self.reread_worked_sessions();
                    self.request_redraw();
                }
                if self.agents.take_renamed() {
                    self.store();
                }
            }
            Wake::Install => self.finish_server_installs(),
            Wake::Language => {
                self.hear_server_troubles();
                if self.settle_moving() {
                    self.request_redraw();
                }
                let answered = self.collect_answers();
                if self.editor.refresh() || answered {
                    self.request_redraw();
                }
            }
            Wake::Blame => {
                if self.collect_blame() {
                    self.request_redraw();
                }
            }
            Wake::Git => {
                let finished = self
                    .git_results
                    .lock()
                    .map(|mut results| std::mem::take(&mut *results))
                    .unwrap_or_default();
                for (scope, said) in finished {
                    if let Some(kind) = self.remote_operation {
                        self.hear_remote(scope, kind, &said);
                    }
                    if let Some(review) = self.reviews.get_mut(&scope) {
                        review.settle(said);
                    }
                    self.reread_review_later(scope);
                }
                self.remote_operation = None;
                self.request_redraw();
            }
            Wake::Clone => {
                self.take_clones();
                self.request_redraw();
            }
            Wake::Disk => {
                if self.take_disk() {
                    self.request_redraw();
                }
            }
            Wake::Debug => {
                if self.take_debugged() {
                    self.request_redraw();
                }
            }
            Wake::Reading => {
                if self.take_readings() {
                    self.request_redraw();
                }
            }
            Wake::Release => {
                if self.released.lock().is_ok_and(|released| *released) {
                    self.update_available = true;
                    self.request_redraw();
                }
            }
            Wake::Listing => {
                if self.take_listings() {
                    self.request_redraw();
                }
            }
            Wake::Picture => self.request_redraw(),
            Wake::Shifted => {
                if self.take_shifted() {
                    self.request_redraw();
                }
            }
            Wake::Chosen => {
                if self.take_chosen() {
                    self.request_redraw();
                }
            }
            Wake::Paste => {
                if self.take_pastes() {
                    self.request_redraw();
                }
            }
            Wake::Arrival => self.take_arrivals(),
        }
    }

    /// Opens the window and builds its renderer once the platform is ready.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("Pandemonium")
            .with_inner_size(LogicalSize::new(
                self.window_state.width,
                self.window_state.height,
            ))
            .with_maximized(self.window_state.maximized)
            .with_window_icon(crate::emblem::window_icon());
        #[cfg(target_os = "linux")]
        let attributes = {
            use winit::platform::wayland::WindowAttributesExtWayland;
            use winit::platform::x11::WindowAttributesExtX11;

            let attributes =
                WindowAttributesExtWayland::with_name(attributes, crate::emblem::APP_ID, "");
            WindowAttributesExtX11::with_name(
                attributes,
                crate::emblem::APP_ID,
                crate::emblem::APP_ID,
            )
        };
        #[cfg(target_os = "macos")]
        let attributes = {
            use winit::platform::macos::WindowAttributesExtMacOS;

            attributes
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_fullsize_content_view(true)
        };
        #[cfg(not(target_os = "macos"))]
        let attributes = attributes.with_decorations(false);
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => fail_to_start(&format!("could not open a window: {error}")),
        };

        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        match Renderer::new(window.clone(), size.width, size.height, scale) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => fail_to_start(&error.to_string()),
        }
        crate::arrival::listen(&window, self.arrivals.clone(), self.waker(Wake::Arrival));
        self.window = Some(window);
        self.resolver.set_keymap(self.preferences.keymap_in_force());

        self.terminals.set_notify(self.waker(Wake::Terminal));
        self.agents.set_notify(self.waker(Wake::Agent));
        self.debuggers.set_notify(self.waker(Wake::Debug));
        self.editor.set_notify(self.waker(Wake::Language));
        if let Some(directory) = config::servers() {
            pm_text::program::set_servers(directory);
        }
        if let Some(directory) = config::logs() {
            self.editor.set_logs(directory);
        }
        self.settings_seen = settings_written();
        self.apply_language_servers();
        self.follow_preferences();
        self.reread_changes_now();

        let saved = std::mem::take(&mut self.saved);
        self.restore_panes(&saved);

        self.ui = Some(Ui::new(self.theme()));
        self.list = Some(DrawList::new(Size::zero()));
        self.look_for_release();
    }

    /// Routes window events to the UI and the renderer.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);

        match event {
            WindowEvent::CloseRequested => {
                self.store();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height, scale);
                }
                self.remember_window();
                self.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let (Some(renderer), Some(window)) =
                    (self.renderer.as_mut(), self.window.as_ref())
                {
                    let size = window.inner_size();
                    renderer.resize(size.width, size.height, scale_factor as f32);
                }
                self.request_redraw();
            }
            WindowEvent::ThemeChanged(_) => self.request_redraw(),
            WindowEvent::Focused(focused) => {
                self.window_focused = focused;
                self.request_redraw();
            }
            WindowEvent::Occluded(occluded) => {
                self.window_occluded = occluded;
                if !occluded {
                    self.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_moved(Point::new(
                    position.x as f32 / scale,
                    position.y as f32 / scale,
                ));
            }
            WindowEvent::CursorLeft { .. } => self.pointer_left(),
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => self.pointer_button(state),
            WindowEvent::MouseInput {
                button: MouseButton::Right,
                state: ElementState::Pressed,
                ..
            } => self.secondary_pressed(),
            WindowEvent::MouseInput {
                button: button @ (MouseButton::Back | MouseButton::Forward),
                state: ElementState::Pressed,
                ..
            } => self.travelled(button == MouseButton::Back),
            WindowEvent::MouseWheel { delta, .. } => {
                let (across, down) = match delta {
                    MouseScrollDelta::LineDelta(columns, lines) => {
                        (columns * input::WHEEL_STEP, lines * input::WHEEL_STEP)
                    }
                    MouseScrollDelta::PixelDelta(position) => {
                        (position.x as f32 / scale, position.y as f32 / scale)
                    }
                };
                let sensitivity = self.preferences.scroll_sensitivity;
                let (across, down) = (across * sensitivity, down * sensitivity);
                if self.modifiers.shift_key() {
                    self.scroll_across(-down);
                } else if across != 0.0 {
                    self.scroll_across(across);
                    self.scroll_by(down);
                } else {
                    self.scroll_by(down);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                if let Some(pointer) = self.pointer {
                    self.follow_pointer(pointer);
                }
                self.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    self.key_pressed(&event);
                }
            }
            WindowEvent::HoveredFile(_) => self.arrive(Arrival::Hovering(None)),
            WindowEvent::HoveredFileCancelled => self.arrive(Arrival::Left),
            WindowEvent::DroppedFile(path) => self.arrive(Arrival::Dropped {
                at: None,
                paths: vec![path],
            }),
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }

        if self.close_requested {
            self.store();
            event_loop.exit();
        }
    }
}

/// Reports why the window could not start, on stderr and in a native dialog
/// for a launch from the desktop that has no terminal to read, then exits.
fn fail_to_start(reason: &str) -> ! {
    eprintln!("pandemonium: {reason}");
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Pandemonium could not start")
        .set_description(reason)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    std::process::exit(1)
}
