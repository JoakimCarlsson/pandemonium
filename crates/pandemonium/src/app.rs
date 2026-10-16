//! The application: the window, its renderer and the events that drive them.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use pm_acp::{Login, Standing};
use pm_components::adder::{ADDER_DOWN, ADDER_ESCAPE, ADDER_UP};
use pm_components::arrange::{is_project_menu_item, is_session_menu_item};
use pm_components::assist::HOVER_SCROLL;
use pm_components::diff::gutter_of;
use pm_components::extensions::{EXTENSIONS_SCROLL, INSTALL_EXTENSION};
use pm_components::git_controls::{GRAPH_SASH, GRAPH_SCROLL, Pick, REVIEW_DISMISS, VIEWER_SCROLL};
use pm_components::git_menus::MenuKind;
use pm_components::message_menu::is_message_menu_item;
use pm_components::open_files::EDITOR_FIELD;
use pm_components::settings::{SETTINGS_CLOSE, SETTINGS_SCROLL, is_settings};
use pm_components::state::{COMMENT_CANCEL, COMMENT_FIELD};
use pm_components::tree_actions::{ENTRY_FIELD, TreeTarget};
use pm_components::{
    EDITOR_MENU_HEIGHT, EditorCommand, MENU_COPY, MENU_CUT, MENU_PASTE, MENU_REDO, MENU_SELECT_ALL,
    MENU_UNDO, is_menu_item,
};
use pm_input::{Button, Focus, FocusId, Key, KeyEvent, Modifiers, PointerEvent, PointerState};
use pm_pages::{
    ATTACH, AgentStatus, BROWSE, DIFF_SCROLL, MENU_SCROLL, PLAN_SCROLL, PROMPT, Screen,
    TERMINAL_BODY, TREE_SCROLL,
};
use pm_renderer::{DrawList, Renderer};
use pm_state::State;

use pm_text::{Edit, Shaper};
use pm_theme::{Appearance, ThemeFamily, default_family};
use pm_ui::{Cursor, Input, Output, TextPoint, TextSelection, Unit};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{CursorIcon, Window, WindowId};

mod code;
mod select;
mod settings_window;
mod terminal;

use crate::keys::Clip;
use crate::view::View;
use crate::work::Work;
use crate::{keys, scene};

/// How tall the strip of tabs above an open file is, in logical pixels.
const TABS: f32 = 48.0;

/// Logical width of the window when it opens on a machine that tells nothing of its screens.
const WIDTH: f32 = 1280.0;

/// Logical height of the window when it opens on a machine that tells nothing of its screens.
const HEIGHT: f32 = 800.0;

/// The name of the file the window's size is kept in.
const WINDOW_SIZE_FILE: &str = "window-size";

/// The least logical width or height of a size worth keeping, which leaves out a window that is minimized.
const MIN_SAVED: f32 = 320.0;

/// How wide, in logical pixels, the strip along the right edge of a scrolling box is where pressing takes hold of its bar.
const BAR_GRIP: f32 = 14.0;

/// How far one notch of a mouse wheel scrolls, in logical pixels.
const WHEEL_LINE: f32 = 48.0;

/// How soon after one press in a field a second one counts as a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// How far the pointer has to move, in logical pixels, with a project header held down for it to be a drag.
const DRAG_DISTANCE: f32 = 6.0;

/// The file the editor's settings are kept in.
const EDITOR_SETTINGS: &str = "editor-settings.json";

/// The window and what draws into it; both exist once the platform is ready.
pub struct App {
    /// The window, once it is open.
    window: Option<Arc<Window>>,
    /// What draws into the window, once it is open.
    renderer: Option<Renderer>,
    /// Shapes the text on screen.
    shaper: Shaper,
    /// The themes the app ships.
    family: ThemeFamily,
    /// Whether the desktop asks for light or dark.
    appearance: Appearance,
    /// The primitives of the frame being drawn, kept to reuse its allocation.
    list: DrawList,
    /// What the user can change on the screen.
    model: State<Screen>,
    /// Set by the model whenever it changes, so the screen is drawn again.
    dirty: Rc<Cell<bool>>,
    /// The pointer as the events so far left it.
    pointer: PointerState,
    /// Which element holds keyboard focus.
    focus: Focus,
    /// Which modifier keys are held.
    modifiers: Modifiers,
    /// What the last frame found on screen.
    output: Output,
    /// The element the primary button went down on.
    pressed_on: Option<FocusId>,
    /// The scrolling box whose bar is held down, and where the pointer last was along it.
    bar_drag: Option<(FocusId, f32)>,
    /// Where the pointer went down on the handle above the graph, and how tall the graph was.
    sash_drag: Option<(f32, f32)>,
    /// The settings window and what it keeps, while settings are open and it is not standing in for the main window.
    settings_surface: Option<settings_window::Surface>,
    /// Whether the settings window stands in for the main one, while one of its events is handled.
    on_settings: bool,
    /// When the application started, which animations run on.
    started: Instant,
    /// Where the threads that start agents report what they found.
    standings: Receiver<(usize, Standing)>,
    /// Handed to each thread that starts an agent.
    reporter: Sender<(usize, Standing)>,
    /// The shape the pointer was last set to.
    cursor: Cursor,
    /// The system clipboard, once something has needed it.
    clipboard: Option<arboard::Clipboard>,
    /// The field a press began in, while the button is held, so dragging selects.
    dragging: Option<FocusId>,
    /// The last press that landed in a field, for noticing a double click.
    last_press: Option<(Instant, FocusId, u8)>,
    /// The field or shell showing what an input method is composing, while it composes something.
    composing_in: Option<FocusId>,
    /// The project paths last written to disk, so only changes are written.
    saved: Option<Vec<String>>,
    /// The choices for the agents' settings last written to disk, so only changes are written.
    saved_choices: Vec<pm_store::Choice>,
    /// The editor's settings as they were last written to disk.
    saved_editor: pm_editor::EditorSettings,
    /// The file shown and its version while it waits to be saved by itself, and since when.
    autosave_wait: Option<((String, i32), Instant)>,
    /// The agents the server can start, which onboarding checks.
    agents: Vec<pm_protocol::AgentServer>,
    /// The logins each agent last offered, in the order of `agents`.
    logins: Vec<Vec<Login>>,
    /// The agents that run, and the work done for them.
    work: Work,
    /// What was last written as the view, which is what the window opened with until it changes.
    saved_view: Option<View>,
    /// The field the reader was typing in when the window last closed, which takes the keyboard once it is drawn.
    refocus: Option<FocusId>,
    /// The project header a press began on, by position, and where the pointer was.
    project_press: Option<(usize, (f32, f32))>,
    /// Whether the pointer moved far enough from `project_press` for it to be a drag.
    project_dragging: bool,
    /// Whether the prompt of the page for starting work was given keyboard focus on arriving there.
    autofocused: bool,
    /// Whether the tree was asking for a name when the last frame was drawn.
    naming: bool,
    /// The last press that activated a control, for noticing a file pressed twice.
    last_activation: Option<(FocusId, Instant)>,
    /// Whether the shell is to take the keyboard once it is drawn.
    focus_terminal: bool,
    /// The cell of the shell's window a press began in, while the button is held, so dragging marks text.
    terminal_anchor: Option<(usize, usize)>,
    /// The part of a line the wheel has scrolled the shell that was not enough to move it.
    terminal_scroll_rest: f32,
    /// The stretch of the conversation's text that is selected, if any.
    text_selection: Option<TextSelection>,
    /// The word of the file shown the pointer rests on, by where it starts, and since when.
    hover_rest: Option<(usize, Instant)>,
    /// When the pointer left both the card about a word and the word, while the card is still showing.
    hover_leaving: Option<Instant>,
    /// Whether the pointer has been on the card about a word since it opened, so leaving it takes it away at once.
    hover_entered: bool,
    /// The chords pressed so far of a binding of more than one.
    pending: Vec<pm_editor::Chord>,
    /// The chords pressed so far while keys are recorded on the keymap page, as they are written.
    recorded: Vec<String>,
    /// When recording keeps what was pressed so far, unless another chord comes first.
    record_deadline: Option<Instant>,
    /// The file, version and caret the language server was last asked about for where the thing at the caret is used.
    highlighted: Option<(String, i32, usize)>,
    /// The file, version and caret waiting to settle before the language server is asked about them, and since when.
    highlight_wait: Option<((String, i32, usize), Instant)>,
    /// What a press on selectable text took, from its start to its end, and
    /// how much it takes at a time, while the button is held, so dragging
    /// selects from it.
    text_held: Option<(TextPoint, TextPoint, Unit)>,
}

impl Default for App {
    /// An application with no window yet, whose clock starts now.
    fn default() -> Self {
        let mut work = Work::new();
        let opened = work.bootstrap();
        let connected = opened.is_some();
        let (saved, choices, logins, in_use, agents) = match opened {
            Some(held) => (
                held.projects,
                held.choices,
                held.logins,
                held.in_use,
                held.agents,
            ),
            None => {
                let (logins, in_use) = pm_store::load_accounts();
                (
                    pm_store::load_projects(),
                    pm_store::load_choices(),
                    logins,
                    in_use,
                    pm_protocol::builtin_agents(),
                )
            }
        };
        work.set_accounts(&logins, &in_use);
        let screen = saved
            .as_deref()
            .map_or_else(Screen::default, Screen::opened)
            .with_agent_list(agents.clone())
            .with_choices(choices.clone())
            .with_accounts(logins, in_use);
        let screen = agents
            .iter()
            .enumerate()
            .fold(
                screen,
                |screen, (index, agent)| match pm_store::load_text(&format!(
                    "discovery-{}",
                    agent.id
                ))
                .and_then(|text| pm_acp::Discovery::from_text(&text))
                {
                    Some(found) => screen.with_discovery(index, found),
                    None => screen,
                },
            );
        let saved_view = View::load();
        let screen = match (&saved_view, screen) {
            (Some(view), Screen::Workspace(state)) => Screen::Workspace(view.apply(state)),
            (_, screen) => screen,
        };
        if let Some(view) = saved_view
            .clone()
            .filter(|view| connected && view.home().is_none())
        {
            work.resume(view);
        }
        let editor = pm_store::load_text(EDITOR_SETTINGS)
            .map(|text| pm_editor::EditorSettings::read(&text))
            .unwrap_or_default();
        let model = State::new(screen.with_editor_settings(editor.clone()));
        let dirty = Rc::new(Cell::new(false));
        let flag = Rc::clone(&dirty);
        model.subscribe(move |_| flag.set(true));
        let (reporter, standings) = mpsc::channel();
        Self {
            window: None,
            renderer: None,
            shaper: scene::shaper(),
            family: default_family(),
            appearance: Appearance::Dark,
            list: DrawList::new(),
            model,
            dirty,
            pointer: PointerState::default(),
            focus: Focus::new(),
            modifiers: Modifiers::default(),
            output: Output::default(),
            pressed_on: None,
            bar_drag: None,
            sash_drag: None,
            settings_surface: None,
            on_settings: false,
            started: Instant::now(),
            standings,
            reporter,
            cursor: Cursor::Default,
            clipboard: None,
            dragging: None,
            last_press: None,
            composing_in: None,
            saved,
            refocus: saved_view.as_ref().and_then(View::typing),
            saved_view,
            project_press: None,
            project_dragging: false,
            saved_choices: choices,
            saved_editor: editor,
            autosave_wait: None,
            logins: vec![Vec::new(); agents.len()],
            agents,
            work,
            autofocused: false,
            naming: false,
            last_activation: None,
            focus_terminal: false,
            terminal_anchor: None,
            terminal_scroll_rest: 0.0,
            text_selection: None,
            text_held: None,
            hover_rest: None,
            hover_leaving: None,
            hover_entered: false,
            pending: Vec::new(),
            recorded: Vec::new(),
            record_deadline: None,
            highlighted: None,
            highlight_wait: None,
        }
    }
}

impl App {
    /// The attributes the window is opened with, `size` logical pixels across.
    fn attributes(size: LogicalSize<f32>) -> winit::window::WindowAttributes {
        let attributes = Window::default_attributes()
            .with_title("Pandemonium")
            .with_inner_size(size);
        #[cfg(target_os = "linux")]
        let attributes = {
            use winit::platform::wayland::WindowAttributesExtWayland;
            use winit::platform::x11::WindowAttributesExtX11;

            let app_id = pm_store::app_name();
            let attributes = WindowAttributesExtWayland::with_name(attributes, app_id, "");
            WindowAttributesExtX11::with_name(attributes, app_id, app_id)
        };
        attributes
    }

    /// How large the window opens: as large as it was when it was last closed, or as the screen it opens on is.
    fn opening_size(event_loop: &ActiveEventLoop) -> LogicalSize<f32> {
        let saved = pm_store::load_text(WINDOW_SIZE_FILE).and_then(|text| {
            let (width, height) = text.trim().split_once(' ')?;
            Some(LogicalSize::new(width.parse().ok()?, height.parse().ok()?))
        });
        let screen = || {
            let monitor = event_loop
                .primary_monitor()
                .or_else(|| event_loop.available_monitors().next())?;
            let size = monitor.size().to_logical::<f32>(monitor.scale_factor());
            Some(LogicalSize::new(size.width, size.height))
        };
        saved
            .filter(|size: &LogicalSize<f32>| size.width >= MIN_SAVED && size.height >= MIN_SAVED)
            .or_else(screen)
            .unwrap_or(LogicalSize::new(WIDTH, HEIGHT))
    }

    /// Keeps how large the window is for the next launch.
    fn remember_size(window: &Window) {
        let size = window.inner_size().to_logical::<f32>(window.scale_factor());
        if size.width >= MIN_SAVED && size.height >= MIN_SAVED {
            let _ = pm_store::save_text(
                WINDOW_SIZE_FILE,
                &format!("{} {}", size.width.round(), size.height.round()),
            );
        }
    }

    /// Draws one frame and asks for the next one while anything is moving or
    /// the frame could not be presented.
    fn redraw(&mut self) {
        if self.on_settings {
            self.redraw_settings();
            return;
        }
        self.fit_terminal();
        let reaching = self.reaching();
        let (Some(window), Some(renderer)) = (self.window.as_ref(), self.renderer.as_mut()) else {
            return;
        };
        let input = Input {
            pointer: self.pointer,
            focused: self.focus.current(),
            elapsed_ms: self.started.elapsed().as_secs_f32() * 1000.0,
            selection: self.text_selection,
        };
        let (width, height) = renderer.size();
        let model = self.model.clone();
        let theme = self.family.variant(self.appearance);
        let scroll_to = model.read(|state| {
            self.output = scene::paint(
                width,
                height,
                state,
                &input,
                theme,
                &mut self.shaper,
                &mut self.list,
            );
            self.focus
                .set_order(self.output.regions.iter().map(|region| region.id).collect());
            if std::mem::take(&mut self.focus_terminal) {
                self.focus.focus(TERMINAL_BODY);
            }
            let drawn = |id| self.output.regions.iter().any(|region| region.id == id);
            Self::give_back_focus(
                &mut self.refocus,
                &mut self.focus,
                self.work.resuming(),
                drawn,
            );
            let starting = state.is_starting();
            if starting && !self.autofocused {
                self.focus.focus(PROMPT);
            }
            self.autofocused = starting;
            let naming = state.is_naming();
            if naming && !self.naming {
                self.focus.focus(ENTRY_FIELD);
            }
            self.naming = naming;
            let mut scroll_to = None;
            if let Some(id) = self.focus.current()
                && let Some(field) = state.field(id)
                && field.is_multiline()
            {
                let caret = field.to_shown(field.caret());
                if let Some(held) = self
                    .output
                    .fields
                    .iter()
                    .rfind(|held| held.id == id && held.start <= caret)
                {
                    let caret_x = held.run.caret_x(caret - held.start);
                    window.set_ime_cursor_area(
                        LogicalPosition::new(held.x + caret_x, held.y),
                        LogicalSize::new(1.0, held.height),
                    );
                }
            } else if let Some(id) = self.focus.current()
                && let Some(field) = state.field(id)
                && let Some(held) = self.output.fields.iter().find(|held| held.id == id)
                && let Some(view) = held.view
            {
                let caret_x = held.run.caret_x(field.to_shown(field.caret()));
                let want = field.scroll_for(caret_x, held.run.width, view.width);
                if (want - field.scroll()).abs() > 0.5 {
                    scroll_to = Some((id, want));
                }
                window.set_ime_cursor_area(
                    LogicalPosition::new(held.x + caret_x, view.y),
                    LogicalSize::new(1.0, view.height),
                );
            }
            scroll_to
        });
        if let Some((id, want)) = scroll_to {
            self.model
                .update(|state| *state = std::mem::take(state).edited(id, Edit::ScrollTo(want)));
        }
        let cursor = if reaching {
            Cursor::Pointer
        } else {
            self.output.cursor
        };
        if cursor != self.cursor {
            self.cursor = cursor;
            window.set_cursor(cursor_icon(self.cursor));
        }
        let presented = renderer.render(&self.list, &mut self.shaper, scene::ground(theme), || {
            window.pre_present_notify()
        });
        if self.output.animating || !presented {
            window.request_redraw();
        }
        self.place_completion();
        self.place_signature();
    }

    /// Carries out what pressing the element `id` means.
    fn activate(&mut self, id: FocusId) {
        let again = self
            .last_activation
            .is_some_and(|(last, at)| last == id && at.elapsed() < DOUBLE_CLICK);
        self.last_activation = Some((id, Instant::now()));
        if again {
            self.model
                .update(|state| *state = std::mem::take(state).with_file_kept(id));
        }
        if is_menu_item(id) {
            self.menu_action(id);
            return;
        }
        if is_message_menu_item(id) {
            self.message_menu_action(id);
            return;
        }
        if let Some(index) = Screen::agent_of(id) {
            self.agent_action(index);
            return;
        }
        if id == BROWSE {
            self.browse();
            return;
        }
        if id == ATTACH {
            self.attach();
            return;
        }
        if id == INSTALL_EXTENSION {
            self.install_extension();
            return;
        }
        if let Some(index) = self.model.read(|screen| screen.settings_more_of(id)) {
            self.open_settings_menu(index);
            return;
        }
        if let Some(project) = self.model.read(|screen| screen.more_button_of(id)) {
            self.open_project_menu(project);
            return;
        }
        if let Some(kind) = Screen::menu_button_of(id) {
            self.open_git_menu(kind);
            return;
        }
        let picks = self.modifiers.shift || self.modifiers.control || self.modifiers.logo;
        if picks && let Some(index) = self.model.read(|screen| screen.change_row_of(id)) {
            let how = if self.modifiers.shift {
                Pick::Range
            } else {
                Pick::Toggle
            };
            self.model
                .update(|state| *state = std::mem::take(state).change_picked(index, how));
            return;
        }
        self.model
            .update(|state| *state = std::mem::take(state).after(id));
        if pm_components::find::is_find_part(id)
            || pm_components::listing::listing_row_of(id).is_some()
        {
            self.show_editor_caret();
        }
        if let Some(next) = Screen::refocus(id) {
            self.focus.focus(next);
        }
        self.focus_terminal = Screen::takes_keyboard(id);
        self.persist();
        self.deliver();
    }

    /// Carries out what the reader asked of the agents and the folders, and copies what was asked to be copied.
    fn deliver(&mut self) {
        for text in self.work.deliver(&self.model) {
            if let Some(clipboard) = self.clipboard() {
                let _ = clipboard.set_text(text);
            }
        }
    }

    /// Writes the editor's settings to disk when they differ from what was last written.
    fn persist_editor(&mut self) {
        let Some(settings) = self
            .model
            .read(Screen::editor_settings)
            .filter(|settings| *settings != self.saved_editor)
        else {
            return;
        };
        let _ = pm_store::save_text(EDITOR_SETTINGS, &settings.write());
        self.saved_editor = settings;
    }

    /// Writes what the reader chose for the agents' settings to disk when it differs from what was last written.
    fn persist_choices(&mut self) {
        let saved = &self.saved_choices;
        let Some(choices) = self.model.read(|screen| {
            screen
                .choices()
                .filter(|choices| *choices != saved.as_slice())
                .map(<[_]>::to_vec)
        }) else {
            return;
        };
        self.work.save_choices(choices.clone());
        self.saved_choices = choices;
    }

    /// Writes where the window is to disk when it differs from what was last written, once the window has finished opening where it was left.
    fn persist_view(&mut self) {
        if self.work.resuming() {
            return;
        }
        let work = &self.work;
        let focused = self.focus.current();
        let Some(view) = self.model.read(|screen| match screen {
            Screen::Workspace(state) => Some(View::capture(state, focused, |position| {
                work.server_number(position)
            })),
            Screen::Onboarding(_) => None,
        }) else {
            return;
        };
        if self.saved_view.as_ref() == Some(&view) {
            return;
        }
        view.save();
        self.saved_view = Some(view);
    }

    /// Gives `focus` to the field in `refocus`, the one the reader was typing in when the window last closed, once the view is no longer `resuming` and the field is `drawn`.
    fn give_back_focus(
        refocus: &mut Option<FocusId>,
        focus: &mut Focus,
        resuming: bool,
        drawn: impl Fn(FocusId) -> bool,
    ) {
        let Some(field) = *refocus else {
            return;
        };
        if resuming || !drawn(field) {
            return;
        }
        focus.focus(field);
        *refocus = None;
    }

    /// Writes the project list to disk when the workspace's differs from what was last written.
    fn persist(&mut self) {
        self.persist_choices();
        self.persist_editor();
        let Some(paths) = self.model.read(|screen| screen.project_paths()) else {
            return;
        };
        if self.saved.as_ref() == Some(&paths) {
            return;
        }
        self.work.save_projects(paths.clone());
        self.saved = Some(paths);
    }

    /// Finds out where every agent stands, each on a thread of its own, since
    /// starting one can take a while.
    fn check_agents(&self) {
        for (index, agent) in self
            .agents
            .iter()
            .enumerate()
            .filter_map(|(index, record)| Some((index, record.agent()?)))
        {
            let reporter = self.reporter.clone();
            std::thread::spawn(move || {
                let _ = reporter.send((index, agent.probe()));
            });
        }
    }

    /// Does what the row of the agent at `index` offers: signs it in,
    /// installs it or looks at it again, and reports where it ends up.
    fn agent_action(&mut self, index: usize) {
        let Some(agent) = self.agents.get(index).and_then(|record| record.agent()) else {
            return;
        };
        let Some(status) = self.model.read(|screen| screen.agent(index).cloned()) else {
            return;
        };
        let login = self.logins[index]
            .iter()
            .find(|login| !login.id.contains("cached"))
            .or(self.logins[index].first())
            .cloned();
        self.model
            .update(|state| *state = std::mem::take(state).with_agent(index, AgentStatus::Working));
        let reporter = self.reporter.clone();
        std::thread::spawn(move || {
            let outcome = match (status, login) {
                (AgentStatus::SignedOut, Some(login)) => agent.log_in(&login),
                (AgentStatus::SignedOut, None) => Err("no way to sign in".to_owned()),
                (AgentStatus::Missing, _) => agent.install(),
                _ => Ok(()),
            };
            let standing = match outcome {
                Ok(()) => agent.probe(),
                Err(reason) => Standing::Failed(reason),
            };
            let _ = reporter.send((index, standing));
        });
    }

    /// Takes in what the agent threads have found.
    fn take_standings(&mut self) {
        while let Ok((index, standing)) = self.standings.try_recv() {
            let status = match standing {
                Standing::Ready => AgentStatus::Ready,
                Standing::SignedOut(logins) => {
                    self.logins[index] = logins;
                    AgentStatus::SignedOut
                }
                Standing::Missing => AgentStatus::Missing,
                Standing::Failed(reason) => AgentStatus::Failed(reason),
            };
            self.model
                .update(|state| *state = std::mem::take(state).with_agent(index, status));
        }
    }

    /// Asks the desktop for a folder and adds it as a project, unless the
    /// dialog is dismissed.
    fn browse(&mut self) {
        let Some(folder) = rfd::FileDialog::new()
            .set_title("Choose a project folder")
            .pick_folder()
        else {
            return;
        };
        let path = folder.to_string_lossy().into_owned();
        self.model
            .update(|state| *state = std::mem::take(state).with_project_opened(&path));
        self.persist();
        self.work.relist();
    }

    /// Asks the desktop for a folder and installs the extension in it, unless the dialog is dismissed.
    fn install_extension(&mut self) {
        let Some(folder) = rfd::FileDialog::new()
            .set_title("Choose an extension folder")
            .pick_folder()
        else {
            return;
        };
        let path = folder.to_string_lossy().into_owned();
        self.model
            .update(|state| *state = std::mem::take(state).extension_picked(path));
        self.deliver();
    }

    /// Asks the desktop for files and mentions them in the prompt, unless the dialog is dismissed.
    fn attach(&mut self) {
        let Some(files) = rfd::FileDialog::new()
            .set_title("Attach files")
            .pick_files()
        else {
            return;
        };
        let paths: Vec<String> = files
            .iter()
            .map(|file| file.to_string_lossy().into_owned())
            .collect();
        self.model
            .update(|state| *state = std::mem::take(state).with_attachments(&paths));
        self.work.preview(paths);
        self.focus.focus(PROMPT);
    }

    /// Scrolls what is under the pointer by `pixels` logical pixels, downwards when positive.
    fn scroll(&mut self, pixels: f32) {
        if self.terminal_wheel(pixels) {
            return;
        }
        let (Some((x, y)), Some(renderer)) = (self.pointer.position, self.renderer.as_ref()) else {
            return;
        };
        if let Some(scroller) = self
            .output
            .scrollers
            .iter()
            .rev()
            .find(|scroller| scroller.rect.contains(x, y))
        {
            let (id, room) = (scroller.id, scroller.room);
            self.scroll_box(id, pixels, room);
            return;
        }
        let height = renderer.size().1;
        self.model
            .update(|state| *state = std::mem::take(state).scrolled(x, pixels, height));
    }

    /// Scrolls the box `id`, which has `room` pixels to scroll, by `pixels`, downwards when positive.
    fn scroll_box(&mut self, id: FocusId, pixels: f32, room: f32) {
        self.model.update(|state| {
            *state = match id {
                MENU_SCROLL => std::mem::take(state).scrolled_menu(pixels, room),
                PLAN_SCROLL => std::mem::take(state).scrolled_plan(pixels, room),
                DIFF_SCROLL => std::mem::take(state).scrolled_diff(pixels, room),
                TREE_SCROLL => std::mem::take(state).scrolled_tree(pixels, room),
                SETTINGS_SCROLL => std::mem::take(state).scrolled_settings(pixels, room),
                EXTENSIONS_SCROLL => std::mem::take(state).scrolled_extensions(pixels, room),
                HOVER_SCROLL => std::mem::take(state).scrolled_hover(pixels, room),
                VIEWER_SCROLL => std::mem::take(state).scrolled_viewer(pixels, room),
                GRAPH_SCROLL => std::mem::take(state).scrolled_graph(pixels, room),
                _ => std::mem::take(state).scrolled_chat(pixels, room),
            };
        });
    }

    /// Takes hold of the handle above the graph when the pointer is down on it, and says whether it did.
    fn grab_sash(&mut self) -> bool {
        let Some((_, y)) = self.pointer.position else {
            return false;
        };
        let on = self
            .pointer
            .position
            .and_then(|at| self.output.hit(at))
            .is_some_and(|id| id == GRAPH_SASH);
        if on {
            self.sash_drag = Some((y, self.model.read(|screen| screen.graph_height())));
        }
        on
    }

    /// Makes the graph as tall as the handle being held says, which is as far up as the pointer went.
    fn drag_sash(&mut self, y: f32) {
        if let Some((from, height)) = self.sash_drag {
            self.model.update(|state| {
                *state = std::mem::take(state).with_graph_height(height - (y - from))
            });
        }
    }

    /// Takes hold of the bar of the scrolling box under the pointer when it is down on the strip along its right edge, and says whether it did.
    fn grab_bar(&mut self) -> bool {
        let Some((x, y)) = self.pointer.position else {
            return false;
        };
        let grabbed = self.output.scrollers.iter().rev().find(|scroller| {
            scroller.room > 0.0
                && scroller.rect.contains(x, y)
                && x >= scroller.rect.x + scroller.rect.width - BAR_GRIP
        });
        self.bar_drag = grabbed.map(|scroller| (scroller.id, y));
        self.bar_drag.is_some()
    }

    /// Follows the pointer down to `y` while a bar is held: the box moves as far as the bar does along its track.
    fn drag_bar(&mut self, y: f32) {
        let Some((id, last)) = self.bar_drag else {
            return;
        };
        let Some(scroller) = self
            .output
            .scrollers
            .iter()
            .find(|scroller| scroller.id == id)
        else {
            return;
        };
        let height = scroller.rect.height;
        let track = (height - 6.0).max(0.0);
        let thumb = (track * height / (height + scroller.room)).clamp(24.0, track.max(24.0));
        let travel = (track - thumb).max(1.0);
        let (room, pixels) = (scroller.room, (y - last) * scroller.room / travel);
        self.bar_drag = Some((id, y));
        self.scroll_box(id, pixels, room);
    }

    /// Folds a pointer event in, takes focus from the focused element when
    /// a press lands anywhere else, and acts on a press that ends on the
    /// element it began on.
    fn pointer(&mut self, event: PointerEvent) {
        self.pointer.apply(event);
        match event {
            PointerEvent::Pressed(Button::Right) => {
                let on = self.pointer.position.and_then(|at| self.output.hit(at));
                let screen = self.model.get();
                match on {
                    Some(id) if Screen::is_field(id) => self.open_menu(id),
                    Some(id) if screen.session_row_of(id).is_some() => {
                        self.open_session_menu(screen.session_row_of(id).unwrap_or_default());
                    }
                    Some(id) if screen.header_of(id).is_some() => {
                        self.open_project_menu(screen.header_of(id).unwrap_or_default());
                    }
                    Some(id) if screen.change_row_of(id).is_some() => {
                        self.open_change_menu(screen.change_row_of(id).unwrap_or_default());
                    }
                    Some(id) if screen.change_folder_row_of(id).is_some() => {
                        let position = screen.change_folder_row_of(id).unwrap_or_default();
                        self.open_folder_change_menu(position);
                    }
                    Some(id) if screen.file_tab_of(id).is_some() => {
                        let index = screen.file_tab_of(id).unwrap_or_default();
                        self.open_tab_menu(index);
                    }
                    Some(id) if screen.tree_row_of(id).is_some() => {
                        let target = screen.tree_row_of(id).unwrap_or_else(TreeTarget::root);
                        self.open_tree_menu(target);
                    }
                    Some(id) if screen.history_row_of(id).is_some() => {
                        let row = screen.history_row_of(id).unwrap_or_default();
                        self.open_git_menu(MenuKind::History(row));
                    }
                    _ => self.open_message_menu(),
                }
            }
            PointerEvent::Pressed(Button::Left) if self.grab_sash() => {
                self.pressed_on = None;
            }
            PointerEvent::Pressed(Button::Left) if self.grab_bar() => {
                self.pressed_on = None;
            }
            PointerEvent::Pressed(Button::Left) => {
                self.terminal_press();
                let on = self.pointer.position.and_then(|at| self.output.hit(at));
                if let Some(id) = on.filter(|id| gutter_of(*id).is_some()) {
                    self.model
                        .update(|state| *state = std::mem::take(state).comment_drag_begun(id));
                }
                self.press_beside_code(on);
                let screen = self.model.get();
                self.project_press = on
                    .and_then(|id| screen.header_of(id))
                    .zip(self.pointer.position);
                self.project_dragging = false;
                if screen.list_menu_open()
                    && !on.is_some_and(|id| {
                        is_project_menu_item(id)
                            || is_session_menu_item(id)
                            || is_message_menu_item(id)
                            || screen.is_menu_row(id)
                    })
                {
                    self.model
                        .update(|state| *state = std::mem::take(state).without_menu());
                }
                if self.model.read(|screen| screen.menu().is_some())
                    && !on.is_some_and(is_menu_item)
                {
                    self.model
                        .update(|state| *state = std::mem::take(state).without_menu());
                }
                self.pressed_on = self.pointer.position.and_then(|at| self.output.hit(at));
                self.text_press();
                if self.pressed_on != Some(ENTRY_FIELD)
                    && self.model.read(|screen| screen.is_naming())
                {
                    self.model
                        .update(|state| *state = std::mem::take(state).without_naming());
                }
                if self.focus.current() != self.pressed_on {
                    self.focus.clear();
                }
                if self.pressed_on.is_none() && self.model.read(|screen| screen.picker_open()) {
                    self.model
                        .update(|state| *state = std::mem::take(state).without_picker());
                }
                if let Some(id) = self.pressed_on
                    && Screen::is_field(id)
                {
                    self.press_in_field(id);
                }
                if self.pressed_on == Some(EDITOR_FIELD) && self.editor_reach_pressed() {
                    self.dragging = None;
                }
            }
            PointerEvent::Moved(x, y) => {
                self.editor_pointer_moved(x, y);
                self.drag_bar(y);
                self.drag_sash(y);
                self.drag_comment((x, y));
                self.drag_project((x, y));
                self.terminal_drag((x, y));
                self.text_drag((x, y));
                if let Some(id) = self.dragging
                    && let Some(offset) = self.output.offset_in(id, x, y)
                {
                    self.edit_field(id, Edit::Place(offset, true));
                }
            }
            PointerEvent::Released(Button::Left) => {
                self.bar_drag = None;
                self.sash_drag = None;
                self.end_comment_drag();
                self.dragging = None;
                self.terminal_anchor = None;
                self.text_release();
                if self.drop_project() {
                    self.pressed_on = None;
                    return;
                }
                let released_on = self.pointer.position.and_then(|at| self.output.hit(at));
                if let (Some(pressed), Some(released)) = (self.pressed_on.take(), released_on)
                    && pressed == released
                {
                    if Screen::is_field(pressed) || pressed == TERMINAL_BODY {
                        self.focus.focus(pressed);
                    }
                    self.activate(pressed);
                }
            }
            _ => {}
        }
    }

    /// Carries out the menu row `row` on the field the menu is open over, and closes the menu.
    fn menu_action(&mut self, row: FocusId) {
        let Some(open) = self.model.read(|screen| screen.menu()) else {
            return;
        };
        self.model
            .update(|state| *state = std::mem::take(state).without_menu());
        let field = open.field;
        self.focus.focus(field);
        match row {
            MENU_UNDO => self.edit_field(field, Edit::Undo),
            MENU_REDO => self.edit_field(field, Edit::Redo),
            MENU_CUT => self.clip(field, Clip::Cut),
            MENU_COPY => self.clip(field, Clip::Copy),
            MENU_PASTE => self.clip(field, Clip::Paste),
            MENU_SELECT_ALL => self.edit_field(field, Edit::SelectAll),
            row => {
                if let Some(command) = EditorCommand::of(row) {
                    self.editor_command(command);
                }
            }
        }
    }

    /// Opens the menu of the `index`-th server of the settings page, just under the pointer and inside the window.
    fn open_settings_menu(&mut self, index: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).with_settings_menu(index, (x - 190.0, y + 14.0))
        });
    }

    /// Opens the menu on the tab of the open file at `index`, just under the pointer and inside the window.
    fn open_tab_menu(&mut self, index: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        let (width, height) = self
            .renderer
            .as_ref()
            .map_or((1280.0, 800.0), |renderer| renderer.size());
        let at = (
            (x - 8.0).min(width - 256.0).max(0.0),
            (y + 12.0)
                .min(height - pm_components::tab_menu::TAB_MENU_HEIGHT - 8.0)
                .max(0.0),
        );
        self.model
            .update(|state| *state = std::mem::take(state).with_tab_menu(index, at));
    }

    /// Opens the menu of the project at `project`, just under the pointer.
    fn open_project_menu(&mut self, project: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model.update(|state| {
            *state = state
                .clone()
                .with_project_menu(project, (x - 8.0, y + 12.0))
        });
    }

    /// Opens the menu on changed files, just under the pointer, over the file at `index`.
    fn open_change_menu(&mut self, index: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).with_change_menu(index, (x - 8.0, y + 12.0));
        });
    }

    /// Opens the menu on changed files, just under the pointer, over the folder at `position` of their tree.
    fn open_folder_change_menu(&mut self, position: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).with_folder_change_menu(position, (x - 8.0, y + 12.0));
        });
    }

    /// Opens the menu on the project's files, just under the pointer, over `target`.
    fn open_tree_menu(&mut self, target: TreeTarget) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).with_tree_menu(target, (x - 8.0, y + 12.0));
        });
    }

    /// Opens the menu `kind` of the source control panel, just under the pointer.
    fn open_git_menu(&mut self, kind: MenuKind) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        let at = match kind {
            MenuKind::History(_) => (x - 8.0, y + 12.0),
            _ => (x - 190.0, y + 14.0),
        };
        self.model
            .update(|state| *state = std::mem::take(state).with_git_menu(kind, at));
    }

    /// Opens the menu of the session at `session`, just under the pointer.
    fn open_session_menu(&mut self, session: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model.update(|state| {
            *state = state
                .clone()
                .with_session_menu(session, (x - 8.0, y + 12.0))
        });
    }

    /// Follows the pointer to `at` while a project header is held down: past a few pixels it is a drag, and the project under the pointer is the one it would take the place of.
    fn drag_project(&mut self, at: (f32, f32)) {
        let Some((from, origin)) = self.project_press else {
            return;
        };
        if !self.project_dragging && (at.0 - origin.0).hypot(at.1 - origin.1) < DRAG_DISTANCE {
            return;
        }
        self.project_dragging = true;
        let hit = self.output.hit(at);
        let over = self.model.read(|held| {
            let current = match held {
                Screen::Workspace(state) => state.project_drag.map_or(from, |drag| drag.over),
                Screen::Onboarding(_) => from,
            };
            hit.and_then(|id| held.project_at(id))
                .filter(|project| *project != from)
                .unwrap_or(current)
        });
        self.model
            .update(|state| *state = std::mem::take(state).dragged_over(from, over, at));
    }

    /// Stretches the lines being selected for a comment to the line number under `at`.
    fn drag_comment(&mut self, at: (f32, f32)) {
        if !self.model.read(|screen| screen.comment_dragging()) {
            return;
        }
        if let Some(id) = self.output.hit(at).filter(|id| gutter_of(*id).is_some()) {
            self.model
                .update(|state| *state = std::mem::take(state).comment_drag_extended(id));
        }
    }

    /// Ends a selection of lines, which opens the box for a comment on them and gives it the keyboard.
    fn end_comment_drag(&mut self) {
        if !self.model.read(|screen| screen.comment_dragging()) {
            return;
        }
        self.model
            .update(|state| *state = std::mem::take(state).comment_drag_ended());
        self.focus.focus(COMMENT_FIELD);
    }

    /// Ends a drag of a project header, putting the project where it was let go, and says whether there was a drag.
    fn drop_project(&mut self) -> bool {
        self.project_press = None;
        if !std::mem::take(&mut self.project_dragging) {
            return false;
        }
        self.model
            .update(|state| *state = std::mem::take(state).dropped());
        self.persist();
        self.deliver();
        true
    }

    /// Opens the menu over the field `id` at the pointer, after putting the
    /// caret where the pointer is unless it is inside the selection.
    fn open_menu(&mut self, id: FocusId) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        let Some(offset) = self.output.offset_in(id, x, y) else {
            return;
        };
        self.focus.focus(id);
        let inside = self.model.read(|screen| {
            screen.field(id).is_some_and(|field| {
                field
                    .selection()
                    .is_some_and(|range| range.contains(&field_text_offset(field, offset)))
            })
        });
        if !inside {
            self.edit_field(id, Edit::Place(offset, false));
        }
        let (x, y) = if id == EDITOR_FIELD {
            let (width, height) = self
                .renderer
                .as_ref()
                .map_or((1280.0, 800.0), |renderer| renderer.size());
            (
                x.min(width - 272.0).max(0.0),
                y.min(height - EDITOR_MENU_HEIGHT - 8.0).max(0.0),
            )
        } else {
            (x, y)
        };
        self.model
            .update(|state| *state = std::mem::take(state).with_menu(id, (x, y)));
    }

    /// Takes a press inside the field `id`: focuses it, puts the caret where
    /// the pointer is, and starts a selection that dragging extends. A second
    /// press soon after the first selects the word under it.
    fn press_in_field(&mut self, id: FocusId) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        let Some(offset) = self.output.offset_in(id, x, y) else {
            return;
        };
        self.focus.focus(id);
        if id == EDITOR_FIELD && self.modifiers.alt {
            self.edit_field(id, Edit::AddSelection(offset, offset));
            return;
        }
        let clicks = match self.last_press {
            Some((at, last, clicks)) if last == id && at.elapsed() < DOUBLE_CLICK => clicks + 1,
            _ => 1,
        };
        self.last_press = Some((Instant::now(), id, clicks));
        self.dragging = Some(id);
        let edit = match clicks {
            1 => Edit::Place(offset, self.modifiers.shift),
            2 => Edit::SelectWordAt(offset),
            _ => Edit::SelectAll,
        };
        self.edit_field(id, edit);
    }

    /// Moves the caret of the multiline field `id` by `by` lines as they are
    /// drawn, to the start or the end of the text when there is no line there.
    fn move_vertically(&mut self, id: FocusId, by: isize, extend: bool) {
        let Some((caret, length, several)) = self
            .model
            .get()
            .field(id)
            .map(|field| (field.caret(), field.text().len(), field.has_others()))
        else {
            return;
        };
        if several {
            let motion = if by < 0 {
                pm_text::Motion::LineUp
            } else {
                pm_text::Motion::LineDown
            };
            self.edit_field(id, Edit::Move(motion, extend));
            return;
        }
        let target = self
            .output
            .vertical(id, caret, by)
            .unwrap_or(if by < 0 { 0 } else { length });
        self.edit_field(id, Edit::Place(target, extend));
    }

    /// Moves the caret of the multiline field `id` to the start, or the end
    /// when `end` is set, of the line it is on as the line is drawn.
    fn move_to_line_edge(&mut self, id: FocusId, end: bool, extend: bool) {
        let Some((caret, several)) = self.model.read(|screen| {
            screen
                .field(id)
                .map(|field| (field.caret(), field.has_others()))
        }) else {
            return;
        };
        if several {
            let motion = if end {
                pm_text::Motion::LineEnd
            } else {
                pm_text::Motion::LineStart
            };
            self.edit_field(id, Edit::Move(motion, extend));
            return;
        }
        if let Some(target) = self.output.line_edge(id, caret, end) {
            self.edit_field(id, Edit::Place(target, extend));
        }
    }

    /// Shows `text` an input method is composing in the field or shell holding
    /// the keyboard, taking it away from the one that showed it before.
    fn compose(&mut self, text: String) {
        let target = self
            .focus
            .current()
            .filter(|id| *id == TERMINAL_BODY || Screen::is_field(*id));
        if let Some(held) = self.composing_in.filter(|held| Some(*held) != target) {
            self.show_composing(held, String::new());
        }
        self.composing_in = target.filter(|_| !text.is_empty());
        if let Some(id) = target {
            self.show_composing(id, text);
        }
    }

    /// Shows `text` as being composed in the field or shell `id`.
    fn show_composing(&mut self, id: FocusId, text: String) {
        if id == TERMINAL_BODY {
            self.model
                .update(|screen| screen.compose_in_terminal(&text));
        } else {
            self.edit_field(id, Edit::Compose(text));
        }
    }

    /// Makes `edit` to the field `id`.
    fn edit_field(&mut self, id: FocusId, edit: Edit) {
        let looks = |app: &Self| {
            app.model.read(|screen| {
                screen
                    .field(id)
                    .map(|field| (field.caret(), field.selection(), field.text().len()))
            })
        };
        let before = looks(self);
        self.model
            .update(|state| *state = std::mem::take(state).edited(id, edit));
        if id == EDITOR_FIELD && looks(self) != before {
            self.show_editor_caret();
        }
    }

    /// Scrolls the file shown just far enough to show its caret.
    fn show_editor_caret(&mut self) {
        if let Some(height) = self.renderer.as_ref().map(|renderer| renderer.size().1) {
            self.model
                .update(|state| *state = std::mem::take(state).with_caret_shown(height - TABS));
        }
    }

    /// The system clipboard, opened on first use.
    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()
    }

    /// Carries out a clipboard shortcut in the field `id`.
    fn clip(&mut self, id: FocusId, action: Clip) {
        match action {
            Clip::Copy | Clip::Cut => {
                let selected = self
                    .model
                    .get()
                    .field(id)
                    .map(|field| {
                        if field.has_others() {
                            field.selected_all()
                        } else {
                            field.selected().to_owned()
                        }
                    })
                    .unwrap_or_default();
                if selected.is_empty() {
                    return;
                }
                if let Some(clipboard) = self.clipboard() {
                    let _ = clipboard.set_text(selected);
                }
                if action == Clip::Cut {
                    self.edit_field(id, Edit::Backspace);
                }
            }
            Clip::Paste => {
                let pasted = self
                    .clipboard()
                    .and_then(|clipboard| clipboard.get_text().ok());
                if let Some(text) = pasted {
                    self.edit_field(id, Edit::Paste(text));
                }
            }
        }
    }

    /// Whether the prompt box holds keyboard focus.
    fn in_prompt(&self) -> bool {
        self.focus.current() == Some(PROMPT)
    }

    /// Moves `delta` pages or views along the ones visited, towards the past when negative.
    fn navigate(&mut self, delta: isize) {
        self.model
            .update(|screen| *screen = std::mem::take(screen).navigated(delta));
    }

    /// Acts on a key: Tab walks the focus, Enter and Space press the focused
    /// element, Escape lets go of focus.
    fn key(&mut self, event: KeyEvent) {
        if !event.pressed {
            return;
        }
        if event.key == Key::Character(',') && event.modifiers.control {
            self.open_settings();
            return;
        }
        if self.recording_key(event)
            || self.terminal_key(event)
            || self.text_key(event)
            || self.card_key(event)
            || self.rename_key(event)
            || self.find_key(event)
            || self.listing_key(event)
            || self.keymap_key(event)
            || self.editor_key(event)
        {
            return;
        }
        match event.key {
            Key::ArrowLeft if event.modifiers.alt => self.navigate(-1),
            Key::ArrowRight if event.modifiers.alt => self.navigate(1),
            Key::Escape if self.model.read(|screen| screen.settings_open()) => {
                self.activate(SETTINGS_CLOSE)
            }
            Key::Escape if self.model.read(|screen| screen.adder_open()) => {
                self.activate(ADDER_ESCAPE)
            }
            Key::ArrowUp if self.model.read(|screen| screen.adder_open()) => {
                self.activate(ADDER_UP)
            }
            Key::ArrowDown if self.model.read(|screen| screen.adder_open()) => {
                self.activate(ADDER_DOWN)
            }
            Key::Enter if self.model.read(|screen| screen.adder_open()) => {
                if let Some(id) = self.model.read(|screen| screen.adder_enter()) {
                    self.activate(id);
                }
            }
            Key::Character(digit)
                if digit.is_ascii_digit()
                    && !self.focus.current().is_some_and(Screen::is_field)
                    && let Some(id) = self
                        .model
                        .get()
                        .inquiry_hotkey(digit.to_digit(10).unwrap_or_default() as usize) =>
            {
                self.activate(id);
            }
            Key::Tab if event.modifiers.shift && self.in_prompt() => {
                self.model
                    .update(|state| *state = std::mem::take(state).mode_cycled());
                self.persist();
                self.deliver();
            }
            Key::Tab if self.in_prompt() && self.model.read(|screen| screen.slash_open()) => {
                self.model
                    .update(|state| *state = std::mem::take(state).slash_taken());
            }
            Key::ArrowUp | Key::ArrowDown
                if self.in_prompt() && self.model.read(|screen| screen.slash_open()) =>
            {
                let delta = if event.key == Key::ArrowUp { -1 } else { 1 };
                self.model
                    .update(|state| *state = std::mem::take(state).slash_moved(delta));
            }
            Key::Enter if self.in_prompt() && self.model.read(|screen| screen.slash_open()) => {
                self.model
                    .update(|state| *state = std::mem::take(state).slash_taken());
            }
            Key::Tab if event.modifiers.shift => self.focus.previous(),
            Key::Tab => self.focus.next(),
            Key::Escape if self.model.read(|screen| screen.review_overlay_open()) => {
                self.activate(REVIEW_DISMISS)
            }
            Key::Escape if self.model.read(|screen| screen.commenting()) => {
                self.activate(COMMENT_CANCEL)
            }
            Key::Escape if self.model.read(|screen| screen.menu().is_some()) => {
                self.model
                    .update(|state| *state = std::mem::take(state).without_menu());
            }
            Key::Escape if self.model.read(|screen| screen.picker_open()) => {
                self.model
                    .update(|state| *state = std::mem::take(state).without_picker());
            }
            Key::Escape if self.model.read(|screen| screen.is_naming()) => {
                self.model
                    .update(|state| *state = std::mem::take(state).without_naming());
                self.focus.clear();
            }
            Key::Escape => self.focus.clear(),
            _ => match self.focus.current() {
                Some(id) if Screen::is_field(id) => {
                    let multiline = self
                        .model
                        .read(|screen| screen.field(id).is_some_and(pm_text::Field::is_multiline));
                    let shift = event.modifiers.shift;
                    let hurried = event.modifiers.control || event.modifiers.logo;
                    let breaks = shift || (Screen::enter_breaks_line(id) && !hurried);
                    if event.key == Key::Enter && breaks && multiline {
                        self.edit_field(id, Edit::Insert('\n'));
                    } else if event.key == Key::Enter
                        && hurried
                        && let Some(submit) = Screen::hurried_submit_of(id)
                    {
                        self.activate(submit);
                    } else if event.key == Key::Enter {
                        self.activate(Screen::submit_of(id));
                    } else if multiline && matches!(event.key, Key::ArrowUp | Key::ArrowDown) {
                        self.move_vertically(
                            id,
                            if event.key == Key::ArrowUp { -1 } else { 1 },
                            shift,
                        );
                    } else if multiline && matches!(event.key, Key::Home | Key::End) {
                        self.move_to_line_edge(id, event.key == Key::End, shift);
                    } else if let Some(action) = keys::clip(&event) {
                        self.clip(id, action);
                    } else if let Some(edit) = keys::edit(&event) {
                        self.edit_field(id, edit);
                    }
                }
                Some(id) if matches!(event.key, Key::Enter | Key::Space) => self.activate(id),
                _ => {}
            },
        }
    }
}

/// The window's pointer for the shape a style asks for.
fn cursor_icon(cursor: Cursor) -> CursorIcon {
    match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::Move => CursorIcon::Move,
        Cursor::NotAllowed => CursorIcon::NotAllowed,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
        Cursor::Wait => CursorIcon::Wait,
        Cursor::Crosshair => CursorIcon::Crosshair,
        Cursor::ColResize => CursorIcon::ColResize,
        Cursor::RowResize => CursorIcon::RowResize,
    }
}

/// The byte offset in the text of `field` that the offset `shown` of what is shown stands for.
fn field_text_offset(field: &pm_text::Field, shown: usize) -> usize {
    (0..=field.text().len())
        .filter(|byte| field.text().is_char_boundary(*byte))
        .find(|byte| field.to_shown(*byte) >= shown)
        .unwrap_or(field.text().len())
}

/// The appearance the desktop asks for; dark when it does not say.
fn appearance(theme: Option<winit::window::Theme>) -> Appearance {
    match theme {
        Some(winit::window::Theme::Light) => Appearance::Light,
        _ => Appearance::Dark,
    }
}

/// Reports why the application cannot start and leaves.
fn fail_to_start(reason: &str) -> ! {
    eprintln!("pandemonium: {reason}");
    std::process::exit(1);
}

impl ApplicationHandler for App {
    /// Opens the window and builds its renderer once the platform is ready.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window =
            match event_loop.create_window(Self::attributes(Self::opening_size(event_loop))) {
                Ok(window) => Arc::new(window),
                Err(error) => fail_to_start(&format!("could not open a window: {error}")),
            };
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        match Renderer::new(window.clone(), size.width, size.height, scale) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => fail_to_start(&error.to_string()),
        }
        self.appearance = appearance(window.theme());
        if self
            .model
            .read(|screen| matches!(screen, Screen::Onboarding(_)))
        {
            self.check_agents();
        }
        window.set_ime_allowed(true);
        window.request_redraw();
        self.window = Some(window);
    }

    /// Draws again when the model changed while no event asked for it.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.take_standings();
        self.work.discover(&self.model);
        self.work.poll(&self.model);
        self.settle_rename_focus();
        if self.model.read(Screen::editor_settings_wanted) {
            let settings = self.saved_editor.clone();
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_editor_settings(settings));
        }
        if self.model.read(Screen::has_outbox) {
            self.deliver();
        }
        let waiting = self.model.read(|screen| screen.waiting()) || self.work.busy();
        let hover = [
            self.editor_hover_due(),
            self.editor_highlight_due(),
            self.editor_autosave_due(),
            self.recording_due(),
        ]
        .into_iter()
        .flatten()
        .min();
        if self.model.read(Screen::has_outbox) {
            self.deliver();
        }
        event_loop.set_control_flow(match (waiting, hover) {
            (true, _) => ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)),
            (false, Some(due)) => ControlFlow::WaitUntil(due),
            (false, None) => ControlFlow::Wait,
        });
        if self.model.read(Screen::place_unrecorded) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_place_recorded());
        }
        if self.dirty.get() {
            self.persist_view();
        }
        self.sync_settings_window(event_loop);
        if self.dirty.replace(false)
            && let Some(window) = self.window.as_ref()
        {
            window.request_redraw();
            self.redraw_settings_window();
        }
    }

    /// Routes window events to the renderer, the pointer and the keyboard.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.is_settings_window(id) {
            self.settings_window_event(event_loop, event);
            return;
        }
        if self.window.as_ref().is_none_or(|window| window.id() != id) {
            return;
        }
        if matches!(event, WindowEvent::Focused(true))
            && self.focus.current().is_some_and(is_settings)
        {
            self.focus.clear();
        }
        self.handle_window_event(event_loop, event);
    }
}

impl App {
    /// Routes an event of the window standing as the main one to the renderer, the pointer and the keyboard.
    fn handle_window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let scale = window.scale_factor() as f32;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ThemeChanged(theme) => self.appearance = appearance(Some(theme)),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height, scale);
                }
                if !self.on_settings {
                    Self::remember_size(&window);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = window.inner_size();
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height, scale);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer(PointerEvent::Moved(
                    position.x as f32 / scale,
                    position.y as f32 / scale,
                ));
            }
            WindowEvent::CursorLeft { .. } => self.pointer(PointerEvent::Left),
            WindowEvent::MouseWheel { delta, .. } => {
                let pixels = match delta {
                    MouseScrollDelta::LineDelta(_, lines) => -lines * WHEEL_LINE,
                    MouseScrollDelta::PixelDelta(moved) => -moved.y as f32 / scale,
                };
                self.scroll(pixels);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.refocus = None;
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    MouseButton::Back | MouseButton::Forward => {
                        if state == ElementState::Pressed {
                            self.navigate(if button == MouseButton::Back { -1 } else { 1 });
                        }
                        window.request_redraw();
                        return;
                    }
                    _ => return,
                };
                self.pointer(match state {
                    ElementState::Pressed => PointerEvent::Pressed(button),
                    ElementState::Released => PointerEvent::Released(button),
                });
            }
            WindowEvent::Ime(Ime::Preedit(composing, _)) => self.compose(composing),
            WindowEvent::Ime(Ime::Disabled) => self.compose(String::new()),
            WindowEvent::Ime(Ime::Commit(committed)) => {
                if self.commit_to_terminal(&committed) {
                } else if let Some(id) = self.focus.current()
                    && Screen::is_field(id)
                {
                    self.edit_field(id, Edit::Paste(committed));
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = keys::modifiers(modifiers.state());
                self.editor_modifiers_changed();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.refocus = None;
                if let Some(event) = keys::key_event(&event, self.modifiers) {
                    self.key(event);
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                return;
            }
            _ => return,
        }
        window.request_redraw();
    }
}
