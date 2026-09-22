//! The window, what it is showing, and the frame it draws each redraw.
//!
//! [`App`] is the binary's whole state: one window, the GPU resources bound to
//! it and the model the frame is built from. It wires the layers and
//! implements none of them — every frame is `pm-ui` elements built from that
//! model, submitted to `pm-gfx` as one draw list.

mod agent;
mod clicks;
mod commands;
mod drag;
mod input;
mod language;
mod panes;
mod picker;
mod places;
mod review;
mod session;
mod tree;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use std::collections::BTreeMap;

use pm_core::{FileTree, Projects, Scope, Sessions};
use pm_gfx::{DrawList, Point, Quad, Rect, Renderer, Size};
use pm_text::Position;
use pm_ui::{
    Appearance, Axis, ResizeEdge, ResizeEvent, ResizePhase, ResizeState, Scroll, Theme, Ui, family,
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
use crate::config::{self, Restored, WindowState};
use crate::desktop;
use crate::editor::{self, Files};
use crate::keymap::Resolver;
use crate::message::Message;
use crate::onboarding::{self, Setup};
use crate::panes::{Item, PaneTree, Saved};
use crate::review::Review;
use crate::terminal::{Shell, Terminals};
use crate::workspace::{
    self, BOTTOM_PANEL_RANGE, Layout, MenuTarget, PRIMARY_SIDEBAR_RANGE, Panel, Panes,
    SECONDARY_SIDEBAR_RANGE, SidebarView, TabMenu,
};

/// The blames that have come back from the threads that asked for them.
type Blamed = Arc<Mutex<Vec<(editor::FileId, Vec<pm_core::Blame>)>>>;

/// How wide the picker is drawn, for centring it over the window.
const PICKER_WIDTH: f32 = 620.0;

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
    /// A blame has come back for a file that asked for one.
    Blame,
    /// A remote Git operation has finished.
    Git,
}

/// The remote operation currently represented by the Source Control button.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RemoteOperation {
    /// Updating remote references.
    Fetch,
    /// Bringing remote commits into the worktree.
    Pull,
    /// Sending local commits to a remote.
    Push,
}

impl RemoteOperation {
    /// Present-progress label used by the split button.
    fn label(self) -> &'static str {
        match self {
            Self::Fetch => "Fetching…",
            Self::Pull => "Pulling…",
            Self::Push => "Pushing…",
        }
    }
}

/// The conductor window, the GPU resources bound to it and what it is showing.
pub struct App {
    /// The platform window, once the event loop has opened one.
    window: Option<Arc<Window>>,
    /// The device and surface drawing into that window.
    renderer: Option<Renderer>,
    /// The element tree's focus, hover and hit regions between frames.
    ui: Option<Ui<Message>>,
    /// The draw list, reused every frame.
    list: Option<DrawList>,
    /// What the onboarding screen has decided so far.
    setup: Setup,
    /// The keymap a keypress is resolved against.
    resolver: Resolver,
    /// The modifiers held down right now.
    modifiers: ModifiersState,
    /// Last pointer position in logical window coordinates.
    pointer: Option<Point>,
    /// Time of the last press on empty title-bar space.
    last_titlebar_click: Option<Instant>,
    /// How far the page is scrolled.
    scroll: Scroll,
    /// The sessions of those projects: a worktree apiece, to work an agent in.
    sessions: Sessions,
    /// The session the window is pointed at, once one has been picked.
    session: Option<pm_core::SessionId>,
    /// How many agents were in the middle of a turn when git was last asked.
    working: usize,
    /// What a session being named is cut from, while one is being named.
    session_base: Option<String>,
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
    /// The changes the reader is being asked whether to throw away.
    discarding: Vec<crate::review::ChangeId>,
    /// Whether keystrokes go to the list of changes.
    changes_focused: bool,
    /// What a path prompt is aimed at, while one is open.
    path_target: Option<std::path::PathBuf>,
    /// Current width and drag state of the sessions sidebar.
    sidebar: ResizeState,
    /// Current height and drag state of the bottom panel.
    bottom_panel: ResizeState,
    /// Current width and drag state of the secondary sidebar.
    secondary_sidebar: ResizeState,
    /// Whether the primary sidebar is visible.
    primary_sidebar_open: bool,
    /// Whether the bottom panel is visible.
    bottom_panel_open: bool,
    /// Whether the secondary sidebar is visible.
    secondary_sidebar_open: bool,
    /// Which of the worktree's two lists that sidebar is showing.
    secondary_sidebar_view: SidebarView,
    /// The box of text keystrokes go to, if they go to one.
    writing: Option<Writing>,
    /// The size and state the window is written down with.
    window_state: WindowState,
    /// Whether the event loop should close after the current event.
    close_requested: bool,
    /// The files the window has open, and the servers behind them.
    editor: Files,
    /// The servers to run for a language, in place of the ones it names.
    language_servers: BTreeMap<String, Vec<pm_text::Server>>,
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
    /// Bounds of the Source Control branch control from the last frame.
    branch_bounds: pm_ui::Bounds,
    /// Bounds of the Source Control remote split button from the last frame.
    remote_bounds: pm_ui::Bounds,
    /// Remote Git work currently running away from the UI thread.
    remote_operation: Option<RemoteOperation>,
    /// Completed remote Git work waiting for the event loop.
    git_results: Arc<Mutex<Vec<(Scope, pm_core::Said)>>>,
    /// Next time the remote-operation spinner advances.
    remote_tick: Instant,
    /// Frame of the animated remote-operation spinner.
    remote_phase: usize,
    /// The question the window is asking before it acts, if it is asking one.
    prompt: Option<crate::prompt::Prompt>,
    /// What could be written where the cursor is, while the list is up.
    completions: Option<crate::editor::Completions>,
    /// What the editor has to say about a place, and where to say it.
    hint: Option<editor::Shown>,
    /// The name the pointer is over, while the key that links it is held.
    link: Option<crate::app::language::Link>,
    /// Where the caret is in its blink.
    blink: editor::Blink,
    /// Where the pointer has been resting, and since when.
    resting: Option<(Instant, Point)>,
    /// The fixes a server last offered, for the menu that shows them.
    code_actions: Vec<pm_text::CodeAction>,
    /// The questions asked of servers and not yet answered.
    asked: Vec<language::Pending>,
    /// Whether the formatting being waited on was asked for by a save.
    saving: bool,
    /// The pane whose tabs are being closed, while one of them is asked about.
    closing: Option<crate::panes::PaneId>,
    /// The blames that have come back and not yet been taken in.
    blamed: Blamed,
    /// The tab menu that is open over the panes, if one is.
    menu: Option<TabMenu>,
    /// The last press in the editor pane, for selecting a word.
    text_clicks: Clicks<Position>,
    /// The last press on a row of the file tree, for keeping a file open.
    tree_clicks: Clicks<pm_core::EntryId>,
    /// The last press on a tab, for keeping a previewed file open.
    tab_clicks: Clicks<Item>,
    /// The agent sessions the window is running, one per project.
    agents: Talks,
    /// The shells the window is running, one per project.
    terminals: Terminals,
    /// Whether keystrokes go to the terminal rather than to the window.
    terminal_focused: bool,
    /// How far back the terminal was scrolled when a scrollbar drag began.
    terminal_scroll_origin: Option<usize>,
    /// How far down the editor was scrolled when a scrollbar drag began.
    editor_scroll_origin: Option<usize>,
    /// How the reader threads wake the event loop.
    proxy: EventLoopProxy<Wake>,
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

        Self {
            window: None,
            renderer: None,
            ui: None,
            list: None,
            setup: restored.setup,
            resolver: Resolver::default(),
            modifiers: ModifiersState::default(),
            pointer: None,
            last_titlebar_click: None,
            scroll: Scroll::default(),
            sessions: Sessions::new(),
            session: None,
            working: 0,
            session_base: None,
            session_bases: Vec::new(),
            showing_bases: false,
            open,
            files,
            reviews: BTreeMap::new(),
            discarding: Vec::new(),
            changes_focused: false,
            path_target: None,
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
            secondary_sidebar: ResizeState::new(
                layout.secondary_sidebar_width,
                SECONDARY_SIDEBAR_RANGE.0,
                SECONDARY_SIDEBAR_RANGE.1,
            ),
            primary_sidebar_open: layout.primary_sidebar_open,
            bottom_panel_open: layout.bottom_panel_open,
            secondary_sidebar_open: layout.secondary_sidebar_open,
            secondary_sidebar_view: layout.secondary_sidebar_view,
            writing: None,
            window_state: restored.window,
            close_requested: false,
            editor: Files::default(),
            language_servers: restored.language_servers,
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
            branch_bounds: Rc::new(Cell::new(Rect::from_xywh(0.0, 0.0, 0.0, 0.0))),
            remote_bounds: Rc::new(Cell::new(Rect::from_xywh(0.0, 0.0, 0.0, 0.0))),
            remote_operation: None,
            git_results: Arc::new(Mutex::new(Vec::new())),
            remote_tick: Instant::now(),
            remote_phase: 0,
            prompt: None,
            completions: None,
            hint: None,
            link: None,
            blink: editor::Blink::default(),
            resting: None,
            code_actions: Vec::new(),
            asked: Vec::new(),
            saving: false,
            closing: None,
            blamed: Arc::new(Mutex::new(Vec::new())),
            text_clicks: Clicks::default(),
            tree_clicks: Clicks::default(),
            tab_clicks: Clicks::default(),
            menu: None,
            agents: Talks::default(),
            terminals: Terminals::default(),
            terminal_focused: false,
            terminal_scroll_origin: None,
            editor_scroll_origin: None,
            proxy,
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
        self.terminals.open(scope, &root)
    }

    /// Closes the panel once the worktree's last shell has exited.
    fn close_empty_panel(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.bottom_panel_open && self.terminals.count(scope) == 0 {
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
    }

    /// What the focused pane holds, for the keymap's `when` clauses.
    pub(super) fn focused_pane_kind(&self) -> Option<&'static str> {
        let showing = |shown: fn(crate::panes::Item) -> bool| self.active_tab().is_some_and(shown);

        match self.writing {
            Some(Writing::Prompt(_)) => return Some("prompt"),
            Some(Writing::Commit) => return Some("commit"),
            None => {}
        }
        match (self.editor_focused, self.terminal_focused) {
            (true, _) if showing(|item| item.review().is_some()) => Some("review"),
            (true, _) if showing(|item| item.change().is_some()) => Some("diff"),
            (true, _) if showing(|item| item.session().is_some()) => Some("agent"),
            (true, _) => Some("file"),
            (_, true) => Some("terminal"),
            _ => None,
        }
    }

    /// The file keystrokes are going to, if the pane is focused.
    pub(super) fn focused_file(&self) -> Option<editor::OpenFile> {
        self.editor_focused.then(|| self.active_file()).flatten()
    }

    /// Opens the file the tree entry `id` names in the editor pane.
    ///
    /// One click previews the file and two keep it, the way every editor
    /// with a preview tab behaves: clicking down a tree leaves one tab
    /// behind, and the file you meant stays when you ask for it twice.
    ///
    /// Reaching another file this way is a jump like any other, so where the
    /// pane was is taken down on the trail and going back returns to it.
    fn open_file(&mut self, id: pm_core::EntryId) {
        let preview = self.tree_clicks.press(id) < 2;
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(root) = self.root_of(scope) else {
            return;
        };
        let Some(path) = self.files.get(&scope).and_then(|tree| {
            tree.rows()
                .iter()
                .find(|row| row.entry.id() == id)
                .map(|row| row.entry.path().to_path_buf())
        }) else {
            return;
        };

        if let Some(file) = self.editor.open(scope, &root, &path, preview) {
            if self.active_tab() != Some(crate::panes::Item::File(file))
                && let Some(from) = self.here()
            {
                self.trail.jumped(from);
            }
            self.show_file(self.panes.focus(), file, preview);
        }
    }

    /// Places the cursor where a press landed, or selects to where it reached.
    ///
    /// A second press in the same place takes the word under it and a third
    /// takes the line, which are the gestures the element tree cannot tell
    /// the window about on its own. Alt puts another cursor down instead of
    /// moving the one there is, alt with shift draws a box, and control
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

        if pressed && still && self.modifiers.control_key() {
            return self.follow_link(head);
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
                3 => buffer.select_line(head),
                _ => {
                    buffer.place(anchor, false);
                    buffer.place(head, true);
                }
            }
        });
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
        self.terminals.start(scope, &root);
        self.bottom_panel_open = true;
    }

    /// Ends one shell, closing the panel when it was the worktree's last.
    ///
    /// An empty panel is a panel with nothing to show, so it goes away the
    /// way it would have if the shell had exited on its own.
    fn stop_shell(&mut self, shell: crate::terminal::ShellId) {
        let Some(scope) = self.scope() else {
            return;
        };
        self.terminals.stop(scope, shell);
        if self.terminals.count(scope) == 0 {
            self.bottom_panel_open = false;
            self.terminal_focused = false;
        }
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
        if !self.terminal_focused || !self.bottom_panel_open {
            return None;
        }
        self.terminals.active(self.scope()?)
    }

    /// The theme this frame is drawn from: the chosen family, in whichever
    /// appearance the theme mode resolves to.
    pub(super) fn theme(&self) -> Theme {
        let appearance = self.setup.theme_mode.resolve(self.system_appearance());
        family(self.setup.theme_family)
            .variant(appearance)
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
        if let Message::ShowTabMenu(pane, item) = message {
            self.open_menu(MenuTarget::Tab(pane, item));
            return;
        }
        if let Message::ShowChangeMenu(index) = message {
            self.aim_at_change(index);
            self.open_menu(MenuTarget::Change);
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
        if message == Message::ConfirmDiscard {
            self.discard_change();
            self.request_redraw();
            return;
        }
        if message == Message::ConfirmDelete {
            self.delete_path();
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
        if message == Message::ShowSessionBases {
            self.showing_bases = !self.showing_bases;
            self.request_redraw();
            return;
        }
        if message == Message::ShowRemoteMenu {
            let bounds = self.remote_bounds.get();
            self.menu = Some(TabMenu {
                at: Point::new((bounds.right() - 190.0).max(8.0), bounds.bottom() + 2.0),
                target: MenuTarget::Remote,
            });
            self.request_redraw();
            return;
        }
        self.menu = None;
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
        if message == Message::TogglePrimarySidebar {
            self.primary_sidebar_open = !self.primary_sidebar_open;
            self.store();
            self.request_redraw();
            return;
        }
        if message == Message::ToggleBottomPanel {
            self.bottom_panel_open = !self.bottom_panel_open;
            self.terminal_focused = self.bottom_panel_open;
            self.store();
            self.request_redraw();
            return;
        }
        if message == Message::FocusTerminal {
            self.terminal_focused = true;
            self.editor_focused = false;
            self.request_redraw();
            return;
        }
        if let Message::OpenFile(id) = message {
            self.open_file(id);
            self.request_redraw();
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
        if let Message::ClosePane(pane) = message {
            self.close_pane(pane);
            self.request_redraw();
            return;
        }
        if let Message::ResizeSplit(split, divider, event, scale) = message {
            if let Some(axis) = self.panes.split_axis(split) {
                self.panes
                    .resize(split, divider, event.delta(axis) * scale, event.phase);
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
        if let Message::ShowEntryMenu(id) = message {
            self.open_entry_menu(id);
            return;
        }
        if let Message::NewFileIn(id) = message {
            self.prompt_for_path(crate::picker::Kind::NewFile, id);
            self.request_redraw();
            return;
        }
        if let Message::NewFolderIn(id) = message {
            self.prompt_for_path(crate::picker::Kind::NewFolder, id);
            self.request_redraw();
            return;
        }
        if let Message::RenameEntry(id) = message {
            self.prompt_for_path(crate::picker::Kind::RenamePath, id);
            self.request_redraw();
            return;
        }
        if let Message::DeleteEntry(id) = message {
            self.prompt_for_delete(id);
            self.request_redraw();
            return;
        }
        if let Message::CopyEntryPath(id) = message {
            self.copy_entry_path(id, false);
            self.request_redraw();
            return;
        }
        if let Message::CopyEntryRelativePath(id) = message {
            self.copy_entry_path(id, true);
            self.request_redraw();
            return;
        }
        if let Message::RevealEntry(id) = message {
            self.reveal_entry(id);
            self.request_redraw();
            return;
        }
        if let Message::OpenEntryInTerminal(id) = message {
            self.open_entry_in_terminal(id);
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
        if message == Message::ShowBranches {
            let bounds = self.branch_bounds.get();
            self.branch_picker_at = Some(Point::new(bounds.left(), bounds.top()));
            self.open_picker(crate::picker::Kind::Branches);
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
            self.open_project();
            self.read_new_worktrees();
            self.store();
            self.request_redraw();
            return;
        }
        if let Message::CloseProject(id) = message {
            if let Some(root) = self
                .open
                .get(id)
                .map(|project| project.root().to_path_buf())
            {
                self.editor.close_project(id, &root);
            }
            self.open.remove(id);
            self.files.retain(|scope, _| scope.project() != id);
            self.reviews.retain(|scope, _| scope.project() != id);
            self.trail.close_project(id);
            self.terminals.close(id);
            self.agents.close_project(id);
            self.sessions.close_project(id);
            self.drop_project_tabs(id);
            self.store();
            self.request_redraw();
            return;
        }
        if let Message::ActivateProject(id) = message {
            self.open.activate(id);
            self.select_checkout();
            self.store();
            self.request_redraw();
            return;
        }
        if let Message::ToggleEntry(id) = message {
            if let Some(scope) = self.scope()
                && let Some(files) = self.files.get_mut(&scope)
            {
                files.toggle(id);
            }
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
        self.setup.apply(message);
        if let Message::SetKeymap(base) = message {
            self.resolver.set_keymap(base.keymap());
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
            Message::RefreshChanges => self.reread_worktree(),
            Message::ToggleChangeStaged(index) => self.toggle_change_staged(index),
            Message::ToggleGroupStaged(group) => self.toggle_group_staged(group),
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
            Message::OpenChange(index) => self.open_change(index),
            Message::OpenChangeDiff(index) => self.open_change_diff(index),
            Message::PreviousHunk => self.step_hunk(false),
            Message::NextHunk => self.step_hunk(true),
            Message::OpenChangeFile(index) => self.open_change_file(index),
            Message::CopyChangePath(index) => self.copy_changed_path(index, false),
            Message::CopyChangeRelativePath(index) => self.copy_changed_path(index, true),
            Message::RevealChange(index) => self.reveal_change(index),
            Message::ExpandChange(index) => {
                if let Some(review) = self.review_mut() {
                    review.toggle(index);
                }
            }
            Message::ShowInputMenu => self.open_menu(MenuTarget::Input),
            Message::EditText(action) => self.act(action),
            Message::WriteCommit(phase, anchor, head) => {
                self.point_in(Writing::Commit, phase, anchor, head);
            }
            _ => return false,
        }
        true
    }

    /// Changes what the index holds, and reads the worktree again after it.
    fn change_by(&mut self, change: impl FnOnce(&mut Review)) {
        if let Some(review) = self.review_mut() {
            change(review);
        }
        self.reread_worktree();
    }

    /// Opens the menu for `target` where the pointer is.
    ///
    /// The menu is placed rather than anchored: the pointer is the one place
    /// every tab, however narrow and however far along the bar, agrees on.
    fn open_menu(&mut self, target: MenuTarget) {
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
                    self.terminals.stop_others(scope, id);
                }
            }
            Message::CloseAllTerminals => {
                if let Some(scope) = self.scope() {
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
        self.terminals.start(scope, &directory);
        self.bottom_panel_open = true;
        self.terminal_focused = true;
        self.editor_focused = false;
    }

    /// Asks for a repository and adds the one that comes back to the window.
    ///
    /// The picker is the platform's own, so there is nothing to do when it is
    /// dismissed, and nothing to say when the folder it answers with is not in
    /// a repository — the set of open projects simply does not change.
    fn open_project(&mut self) {
        let Some(root) = rfd::FileDialog::new()
            .set_title("Open a repository")
            .pick_folder()
        else {
            return;
        };
        let _ = self.open.find_or_open(root);
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
            setup: self.setup.clone(),
            projects: self.open.roots(),
            active: self
                .open
                .active()
                .map(|project| project.root().to_path_buf()),
            layout: self.layout(),
            panes: self.saved_panes(),
            window: self.window_state,
            language_servers: self.language_servers.clone(),
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
            Writing::Commit => self.review_mut().map(crate::review::Review::message_mut),
            Writing::Prompt(session) => self
                .agents
                .get_mut(session)
                .map(crate::agent::Talk::prompt_mut),
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
    /// word on the second and its line on the third.
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
        if still && !pressed {
            return;
        }
        let presses = match still {
            true => self.text_clicks.press(anchor),
            false => {
                self.text_clicks.clear();
                0
            }
        };
        if let Some(input) = self.written_in() {
            input.point(phase, anchor, head, presses);
        }
    }

    /// Writes the window's preferences, projects and layout down.
    fn store(&mut self) {
        self.remember_window();
        config::save(&self.state());
    }

    /// A handle the threads behind the window wake it with, sending `wake`.
    pub(super) fn waker(&self, wake: Wake) -> Arc<dyn Fn() + Send + Sync> {
        let proxy = Mutex::new(self.proxy.clone());
        Arc::new(move || {
            if let Ok(proxy) = proxy.lock() {
                let _ = proxy.send_event(wake);
            }
        })
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
    /// under the title bar, a completion list under the word it completes, a
    /// hint beside the cursor — because none of them takes room from the
    /// screen they cover.
    fn overlays(&self, theme: &Theme) -> Vec<workspace::Overlaid> {
        let mut overlays = Vec::new();

        let window = self.renderer.as_ref().map_or(Size::zero(), Renderer::size);

        if let Some(picker) = self.picker.as_ref() {
            let width = crate::picker::width(picker.kind());
            let at = self.branch_picker_at.filter(|_| {
                matches!(
                    picker.kind(),
                    crate::picker::Kind::Branches | crate::picker::Kind::NewBranch
                )
            });
            let point = at.map_or_else(
                || Point::new(window.width / 2.0 - PICKER_WIDTH / 2.0, crate::picker::TOP),
                |anchor| {
                    let height = crate::picker::height(picker);
                    Point::new(
                        (anchor.x - 24.0).clamp(8.0, (window.width - width - 8.0).max(8.0)),
                        (anchor.y - height - 8.0).max(8.0),
                    )
                },
            );
            overlays.push(workspace::Overlaid {
                at: point,
                content: Box::new(crate::picker::picker(theme, picker)),
                backdrop: Some(Message::DismissPopup),
            });
        }

        if let Some(asked) = self.prompt.as_ref() {
            let height = crate::prompt::height(asked);
            overlays.push(workspace::Overlaid {
                at: Point::new(
                    window.width / 2.0 - crate::prompt::WIDTH / 2.0,
                    (window.height - height) / 2.0,
                ),
                content: Box::new(crate::prompt::prompt(theme, asked)),
                backdrop: Some(Message::DismissPrompt),
            });
        }

        if let Some(completions) = self.completions.as_ref() {
            overlays.push(workspace::Overlaid {
                at: completions.at(),
                content: Box::new(editor::completion_list(theme, completions)),
                backdrop: None,
            });
        }

        if let Some(hint) = self.hint.as_ref().filter(|hint| !hint.is_empty()) {
            overlays.push(workspace::Overlaid {
                at: hint.at,
                content: Box::new(editor::hint(theme, &hint.text())),
                backdrop: None,
            });
        }
        overlays
    }

    /// Where on screen the cursor of the focused pane last came out.
    pub(super) fn cursor_point(&self) -> Point {
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
        self.refresh_annotations();
        let shell = self
            .bottom_panel_open
            .then(|| self.active_shell())
            .flatten();
        let shells = self
            .scope()
            .map(|scope| self.terminals.list(scope))
            .unwrap_or_default();
        let panel = Panel {
            shell,
            shells,
            focused: self.terminal_focused,
        };
        let showing = self.active_file();
        let drop = self.drop_highlight();
        let carried = self.carried_tab();
        let layout = self.layout();
        let theme = self.theme();
        let editor = self.pane_view(&theme);
        let menu = self.menu_items();
        let overlays = self.overlays(&theme);
        let scope = self.scope();
        let sidebar = self.sidebar_projects();
        let files = workspace::Worktree {
            tree: scope.and_then(|scope| self.files.get(&scope)),
            review: scope.and_then(|scope| self.reviews.get(&scope)),
            committing: self.writing == Some(Writing::Commit),
            branch_bounds: self.branch_bounds.clone(),
            remote_bounds: self.remote_bounds.clone(),
            remote_operation: self.remote_operation.map(|operation| {
                let rotation = self.remote_phase as f32 * std::f32::consts::TAU / 20.0;
                (operation.label(), rotation)
            }),
        };
        let (Some(renderer), Some(ui), Some(list)) =
            (self.renderer.as_mut(), self.ui.as_mut(), self.list.as_mut())
        else {
            return;
        };

        ui.set_theme(theme);

        let size = renderer.size();
        self.scroll.set_viewport(size);
        list.reset(size);
        list.quad(Quad::filled(
            Rect::from_xywh(0.0, 0.0, size.width, size.height),
            theme.colors.background,
        ));

        let page = if self.setup.finished {
            workspace::workspace(
                &theme,
                &self.open,
                &sidebar,
                files,
                layout,
                Panes {
                    editor,
                    showing,
                    drop,
                    carried,
                    terminal: panel,
                    agents: self
                        .open
                        .active()
                        .map_or(0, |project| self.agents.count(project.id())),
                    menu,
                    overlays,
                },
            )
        } else {
            onboarding::page(&theme, &self.setup)
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
    /// or for the caret to turn over.
    ///
    /// The window is otherwise woken only by something happening; a pointer
    /// holding still and a caret blinking are the two things it has to
    /// notice by the clock.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let remote_ticked = self.remote_operation.is_some() && now >= self.remote_tick;
        if remote_ticked {
            self.remote_tick = now + std::time::Duration::from_millis(100);
            self.remote_phase = self.remote_phase.wrapping_add(1);
        }
        if self.rested() || self.blinked() || remote_ticked {
            self.request_redraw();
        }
        let remote_tick = self.remote_operation.map(|_| self.remote_tick);
        let next = [self.next_rest(), self.next_blink(), remote_tick]
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
        match event {
            Wake::Terminal => {
                if self.terminals.pump() {
                    self.close_empty_panel();
                    self.request_redraw();
                }
            }
            Wake::Agent => {
                if self.agents.pump() {
                    self.follow_agents();
                    self.reread_worked_sessions();
                    self.request_redraw();
                }
                if self.agents.take_opened() {
                    self.store();
                }
            }
            Wake::Language => {
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
                    if let Some(review) = self.reviews.get_mut(&scope) {
                        review.report(said);
                    }
                }
                self.remote_operation = None;
                self.request_redraw();
            }
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
            .with_maximized(self.window_state.maximized);
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
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("window creation failed"),
        );

        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        self.renderer = Some(Renderer::new(
            window.clone(),
            size.width,
            size.height,
            scale,
        ));
        self.window = Some(window);
        self.resolver.set_keymap(self.setup.keymap.keymap());

        self.terminals.set_notify(self.waker(Wake::Terminal));
        self.agents.set_notify(self.waker(Wake::Agent));
        self.editor.set_notify(self.waker(Wake::Language));
        self.editor.set_language_servers(&self.language_servers);
        self.reread_changes();

        let saved = std::mem::take(&mut self.saved);
        self.restore_panes(&saved);

        self.ui = Some(Ui::new(self.theme()));
        self.list = Some(DrawList::new(Size::zero()));
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
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }

        if self.close_requested {
            self.store();
            event_loop.exit();
        }
    }
}
