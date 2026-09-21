//! The window, what it is showing, and the frame it draws each redraw.
//!
//! [`App`] is the binary's whole state: one window, the GPU resources bound to
//! it and the model the frame is built from. It wires the layers and
//! implements none of them — every frame is `pm-ui` elements built from that
//! model, submitted to `pm-gfx` as one draw list.

mod input;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use std::collections::BTreeMap;

use pm_core::{FileTree, ProjectId, Projects};
use pm_gfx::{DrawList, Point, Quad, Rect, Renderer, Size};
use pm_text::Position;
use pm_ui::{
    Appearance, Axis, ResizeEdge, ResizeEvent, ResizePhase, ResizeState, Scroll, Ui, family,
};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::app::input::DOUBLE_CLICK_INTERVAL;
use crate::config::{self, Restored, WindowState};
use crate::desktop;
use crate::editor::{self, Files};
use crate::keymap::Resolver;
use crate::onboarding::{self, Message, Setup};
use crate::terminal::{Shell, Terminals};
use crate::workspace::{
    self, BOTTOM_PANEL_RANGE, Layout, MenuTarget, PRIMARY_SIDEBAR_RANGE, Pane, Panel, Panes,
    SECONDARY_SIDEBAR_RANGE, SidebarProject, TabMenu,
};

/// What the window is woken up for from outside the event loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wake {
    /// A terminal's child has written something that is waiting to be read.
    Terminal,
    /// A language server has said something about a file that is open.
    Language,
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
    /// Projects and sessions presented by the workspace.
    projects: Vec<SidebarProject>,
    /// The projects this window holds open.
    open: Projects,
    /// One file tree per open project, so each keeps what it has expanded.
    files: BTreeMap<ProjectId, FileTree>,
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
    /// The size and state the window is written down with.
    window_state: WindowState,
    /// Whether the event loop should close after the current event.
    close_requested: bool,
    /// The files the window has open, and the servers behind them.
    editor: Files,
    /// Whether keystrokes go to the editor pane rather than to the window.
    editor_focused: bool,
    /// The tab menu that is open over the panes, if one is.
    menu: Option<TabMenu>,
    /// Time and place of the last press in the editor pane.
    last_editor_click: Option<(Instant, Position)>,
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

        let files = open
            .iter()
            .map(|project| (project.id(), FileTree::new(project.root())))
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
            projects: Vec::new(),
            open,
            files,
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
            window_state: restored.window,
            close_requested: false,
            editor: Files::default(),
            editor_focused: false,
            menu: None,
            last_editor_click: None,
            terminals: Terminals::default(),
            terminal_focused: false,
            terminal_scroll_origin: None,
            editor_scroll_origin: None,
            proxy,
        }
    }

    /// The shell of the active project, started in its worktree if need be.
    ///
    /// A shell belongs to the worktree the window is pointed at, the same one
    /// the file tree lists, and it is started the first time its pane is
    /// drawn rather than when the project is opened.
    fn active_shell(&mut self) -> Option<Shell> {
        let project = self.open.active()?;
        let (id, root) = (project.id(), project.root().to_path_buf());
        self.terminals.open(id, &root)
    }

    /// Closes the panel once the worktree's last shell has exited.
    fn close_empty_panel(&mut self) {
        let Some(project) = self.open.active().map(pm_core::Project::id) else {
            return;
        };
        if self.bottom_panel_open && self.terminals.count(project) == 0 {
            self.bottom_panel_open = false;
            self.terminal_focused = false;
        }
    }

    /// Takes the keyboard away from the panes, for a click elsewhere.
    ///
    /// The click that lands back in a pane brings it straight back, so a
    /// press is free to drop focus without knowing where it landed.
    pub(super) fn release_pane_focus(&mut self) {
        self.terminal_focused = false;
        self.editor_focused = false;
    }

    /// The file the editor pane is showing, if a project has one open.
    pub(super) fn active_file(&self) -> Option<editor::OpenFile> {
        let project = self.open.active()?;
        self.editor.active(project.id())
    }

    /// What the focused pane holds, for the keymap's `when` clauses.
    pub(super) fn focused_pane_kind(&self) -> Option<&'static str> {
        match (self.editor_focused, self.terminal_focused) {
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
    fn open_file(&mut self, id: pm_core::EntryId) {
        let Some(project) = self.open.active() else {
            return;
        };
        let (project, root) = (project.id(), project.root().to_path_buf());
        let Some(path) = self.files.get(&project).and_then(|tree| {
            tree.rows()
                .iter()
                .find(|row| row.entry.id() == id)
                .map(|row| row.entry.path().to_path_buf())
        }) else {
            return;
        };

        if self.editor.open(project, &root, &path).is_some() {
            self.editor_focused = true;
            self.terminal_focused = false;
        }
    }

    /// Places the cursor where a press landed, or selects to where it reached.
    ///
    /// A second press in the same place within the double-click interval
    /// takes the word under it instead, which is the one gesture the element
    /// tree cannot tell the window about on its own.
    fn select_text(&mut self, anchor: Position, head: Position) {
        let Some(project) = self.open.active().map(pm_core::Project::id) else {
            return;
        };
        self.editor_focused = true;
        self.terminal_focused = false;

        let now = Instant::now();
        let twice = self.last_editor_click.is_some_and(|(at, place)| {
            place == anchor && now.duration_since(at) <= DOUBLE_CLICK_INTERVAL
        });
        self.last_editor_click = (anchor == head).then_some((now, anchor));

        self.editor
            .edit(project, |buffer| match (twice, anchor == head) {
                (true, true) => buffer.select_word(head),
                (false, true) => buffer.place(head, false),
                (_, false) => {
                    buffer.place(anchor, false);
                    buffer.place(head, true);
                }
            });
    }

    /// Scrolls the editor by a drag on its scrollbar.
    fn drag_editor_scrollbar(&mut self, event: ResizeEvent, lines_per_pixel: f32) {
        let Some(file) = self.active_file() else {
            return;
        };
        let mut document = file.borrow_mut();
        let base = match event.phase {
            ResizePhase::Started => document.scroll(),
            _ => self
                .editor_scroll_origin
                .unwrap_or_else(|| document.scroll()),
        };
        self.editor_scroll_origin = match event.phase {
            ResizePhase::Ended => None,
            _ => Some(base),
        };

        let travelled = event.delta(Axis::Vertical) * lines_per_pixel;
        document.scroll_to((base as f32 + travelled).round().max(0.0) as usize);
    }

    /// Starts another shell in the active project's worktree.
    fn start_shell(&mut self) {
        let Some(project) = self.open.active() else {
            return;
        };
        let (id, root) = (project.id(), project.root().to_path_buf());
        self.terminals.start(id, &root);
        self.bottom_panel_open = true;
    }

    /// Ends one shell, closing the panel when it was the worktree's last.
    ///
    /// An empty panel is a panel with nothing to show, so it goes away the
    /// way it would have if the shell had exited on its own.
    fn stop_shell(&mut self, shell: crate::terminal::ShellId) {
        let Some(project) = self.open.active().map(pm_core::Project::id) else {
            return;
        };
        self.terminals.stop(project, shell);
        if self.terminals.count(project) == 0 {
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
        let Some(shell) = self
            .open
            .active()
            .and_then(|project| self.terminals.active(project.id()))
        else {
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
        let project = self.open.active()?;
        self.terminals.active(project.id())
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
        if let Message::ShowFileMenu(id) = message {
            self.open_menu(MenuTarget::File(id));
            return;
        }
        if let Message::ShowTerminalMenu(id) = message {
            self.open_menu(MenuTarget::Terminal(id));
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
        if let Message::SelectFile(id) = message {
            if let Some(project) = self.open.active().map(pm_core::Project::id) {
                self.editor.activate(project, id);
            }
            self.editor_focused = true;
            self.terminal_focused = false;
            self.request_redraw();
            return;
        }
        if let Message::CloseFile(id) = message {
            if let Some(project) = self.open.active().map(pm_core::Project::id) {
                self.editor.close(project, id);
            }
            self.request_redraw();
            return;
        }
        if let Message::SelectText(anchor, head) = message {
            self.select_text(anchor, head);
            self.request_redraw();
            return;
        }
        if let Message::ScrollEditor(event, lines_per_pixel) = message {
            self.drag_editor_scrollbar(event, lines_per_pixel);
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
            if let Some(project) = self.open.active().map(pm_core::Project::id) {
                self.terminals.activate(project, id);
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
            self.files.remove(&id);
            self.terminals.close(id);
            self.store();
            self.request_redraw();
            return;
        }
        if let Message::ActivateProject(id) = message {
            self.open.activate(id);
            self.store();
            self.request_redraw();
            return;
        }
        if matches!(message, Message::ProjectMenu(_)) {
            self.request_redraw();
            return;
        }
        if let Message::ToggleEntry(id) = message {
            if let Some(project) = self.open.active().map(pm_core::Project::id)
                && let Some(files) = self.files.get_mut(&project)
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

    /// Opens the menu for `target` where the pointer is.
    ///
    /// The menu is placed rather than anchored: the pointer is the one place
    /// every tab, however narrow and however far along the bar, agrees on.
    fn open_menu(&mut self, target: MenuTarget) {
        self.menu = self.pointer.map(|at| TabMenu { at, target });
        self.request_redraw();
    }

    /// Puts away the menu that is open, saying whether there was one.
    pub(super) fn dismiss_menu(&mut self) -> bool {
        self.menu.take().is_some()
    }

    /// Carries out a command from a tab menu, if `message` is one.
    ///
    /// The menu commands are collected here because they are one family:
    /// every one of them acts on the tabs of the project the window is
    /// pointed at, and none of them touches the window's own state.
    fn tab_command(&mut self, message: Message) -> bool {
        let Some(project) = self.open.active().map(pm_core::Project::id) else {
            return matches!(message, Message::DismissMenu);
        };

        match message {
            Message::DismissMenu => {}
            Message::CloseOtherFiles(id) => self.editor.close_others(project, id),
            Message::CloseFilesLeft(id) => self.editor.close_left(project, id),
            Message::CloseFilesRight(id) => self.editor.close_right(project, id),
            Message::CloseSavedFiles => self.editor.close_saved(project),
            Message::CloseAllFiles => self.editor.close_all(project),
            Message::CopyFilePath(id) => {
                if let Some(path) = self.editor.path(project, id) {
                    desktop::copy(path.display().to_string());
                }
            }
            Message::CopyFileRelativePath(id) => {
                if let Some(path) = self.relative_path(project, id) {
                    desktop::copy(path);
                }
            }
            Message::RevealFile(id) => {
                if let Some(path) = self.editor.path(project, id) {
                    desktop::reveal(&path);
                }
            }
            Message::OpenFileInTerminal(id) => self.start_shell_beside(project, id),
            Message::CloseOtherTerminals(id) => self.terminals.stop_others(project, id),
            Message::CloseAllTerminals => {
                self.terminals.stop_all(project);
                self.close_empty_panel();
            }
            _ => return false,
        }
        true
    }

    /// The path of the file `id` names, from the project's worktree down.
    fn relative_path(&self, project: ProjectId, id: crate::editor::FileId) -> Option<String> {
        let root = self.open.get(project)?.root().to_path_buf();
        let path = self.editor.path(project, id)?;
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        Some(relative.display().to_string())
    }

    /// Starts a shell in the directory the file `id` names sits in.
    fn start_shell_beside(&mut self, project: ProjectId, id: crate::editor::FileId) {
        let Some(directory) = self
            .editor
            .path(project, id)
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        else {
            return;
        };
        self.terminals.start(project, &directory);
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

    /// Reads the worktree of any project that does not have a tree yet.
    fn read_new_worktrees(&mut self) {
        let missing = self
            .open
            .iter()
            .filter(|project| !self.files.contains_key(&project.id()))
            .map(|project| (project.id(), project.root().to_path_buf()))
            .collect::<Vec<_>>();
        for (id, root) in missing {
            self.files.insert(id, FileTree::new(root));
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
            window: self.window_state,
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

    /// Writes the window's preferences, projects and layout down.
    fn store(&mut self) {
        self.remember_window();
        config::save(&self.state());
    }

    /// A handle the threads behind the window wake it with, sending `wake`.
    fn waker(&self, wake: Wake) -> Arc<dyn Fn() + Send + Sync> {
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

    /// Builds the frame and hands it to the renderer.
    fn draw(&mut self) {
        let appearance = self.setup.theme_mode.resolve(self.system_appearance());
        let shell = self
            .bottom_panel_open
            .then(|| self.active_shell())
            .flatten();
        let shells = self
            .open
            .active()
            .map(|project| self.terminals.list(project.id()))
            .unwrap_or_default();
        let panel = Panel {
            shell,
            shells,
            focused: self.terminal_focused,
        };
        let pane = Pane {
            file: self.active_file(),
            files: self
                .open
                .active()
                .map(|project| self.editor.list(project.id()))
                .unwrap_or_default(),
            focused: self.editor_focused,
        };
        let files = self.open.active().map(pm_core::Project::id);
        let files = files.and_then(|id| self.files.get(&id));
        let layout = self.layout();
        let (Some(renderer), Some(ui), Some(list)) =
            (self.renderer.as_mut(), self.ui.as_mut(), self.list.as_mut())
        else {
            return;
        };

        let theme = family(self.setup.theme_family).variant(appearance);
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
                &self.projects,
                files,
                layout,
                Panes {
                    editor: pane,
                    terminal: panel,
                    menu: self.menu,
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
    }
}

impl ApplicationHandler<Wake> for App {
    /// Applies what the shells have written and draws the result.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: Wake) {
        match event {
            Wake::Terminal => {
                if self.terminals.pump() {
                    self.close_empty_panel();
                    self.request_redraw();
                }
            }
            Wake::Language => {
                if self.editor.refresh() {
                    self.request_redraw();
                }
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
        self.editor.set_notify(self.waker(Wake::Language));

        let appearance = self.setup.theme_mode.resolve(self.system_appearance());
        self.ui = Some(Ui::new(family(self.setup.theme_family).variant(appearance)));
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
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, lines) => lines * input::WHEEL_STEP,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / scale,
                };
                self.scroll_by(delta);
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
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
