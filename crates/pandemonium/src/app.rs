//! The application: the window, its renderer and the events that drive them.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::model::Model;
use pm_components::adder::{ADDER_DOWN, ADDER_ESCAPE, ADDER_UP};
use pm_components::arrange::{is_project_menu_item, is_session_menu_item};
use pm_components::diff::gutter_of;
use pm_components::events::Change;
use pm_components::focus::FocusId;
use pm_components::folder_picker::{
    PICKER_BACK, PICKER_CLOSE, PICKER_DOWN, PICKER_FORWARD, PICKER_UP,
};
use pm_components::git_controls::REVIEW_DISMISS;
use pm_components::git_menus::{CHANGE_MENU_HEIGHT, MenuKind};
use pm_components::line_edits::LineEdit;
use pm_components::message_menu::is_message_menu_item;
use pm_components::minimap::MINIMAP;
use pm_components::open_files::{EDITOR_FIELD, STRIP};
use pm_components::panes::split_row_of;
use pm_components::sash::Grows;
use pm_components::settings::SETTINGS_CLOSE;
use pm_components::state::{COMMENT_CANCEL, COMMENT_FIELD};
use pm_components::theme::{Appearance, ThemeFamily};
use pm_components::tree_actions::{ENTRY_FIELD, TREE_MENU_HEIGHT, TreeTarget};
use pm_components::{
    EDITOR_MENU_HEIGHT, EditorCommand, MENU_COPY, MENU_CUT, MENU_PASTE, MENU_REDO, MENU_SELECT_ALL,
    MENU_UNDO, is_menu_item,
};
use pm_field::{Edit, Gesture, Reach};
use pm_pages::{PROMPT, Screen, TERMINAL_BODY};
use rustle::{Key, KeyEvent, Modifiers};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};

mod asides;
mod browser;
mod capture;
mod code;
mod controller;
mod drops;
mod editing;
mod focus;
mod frame;
mod gesture;
#[cfg(test)]
mod headless;
mod host;
#[cfg(test)]
mod interactions;
mod panes;
mod paste;
mod performance;
mod pointer;
pub(crate) mod port;
mod select;
mod terminal;
mod tree;
mod vim;
#[cfg(target_os = "linux")]
mod wayland;

use self::gesture::CodeText;
use self::pointer::{Button, PointerEvent, PointerState};
use crate::keys;
use crate::keys::Clip;
use crate::view::View;
use crate::work::Work;

pub use self::host::Running;

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

/// How far one notch of a mouse wheel scrolls, in logical pixels.
const WHEEL_LINE: f32 = 48.0;

/// The file the editor's settings are kept in.
const EDITOR_SETTINGS: &str = "editor-settings.json";

/// The file the window's own settings are kept in.
const WINDOW_SETTINGS: &str = "window-settings.json";

/// How long the window waits before looking for a newer release again while it stays open.
const RELEASE_RECHECK: Duration = Duration::from_secs(6 * 60 * 60);

/// The window and what draws into it; both exist once the platform is ready.
pub struct App {
    /// The interface on rustle, its windows and what its root reads.
    shell: host::Shell,
    /// The window standing as the main one, once it is open.
    window: Option<Rc<dyn port::Port>>,
    /// Whether something changed outside the model that the windows must be composed again for.
    recompose: bool,
    /// The control the app kept focus on when the windows were last composed, so focus kept
    /// anywhere else since composes them again.
    composed_focus: Option<FocusId>,
    /// What the interface made of the event being handled.
    claimed: controller::Claims,
    /// Optional CPU frame timings and pointer response measurements.
    timings: performance::Timings,
    /// The light and the dark theme the window is drawn in.
    family: ThemeFamily,
    /// Whether the window is drawn light or dark.
    appearance: Appearance,
    /// Whether the desktop asks for light or dark.
    system_appearance: Appearance,
    /// The interface and code fonts the shaper was last set in, by family name.
    fonts_applied: (String, String),
    /// The window's own settings as they were last written to disk.
    saved_window: pm_config::WindowSettings,
    /// What the user can change on the screen.
    model: Model,
    /// Set by the model whenever it changes, so the screen is drawn again.
    dirty: Rc<Cell<bool>>,
    /// The pointer as the events so far left it.
    pointer: PointerState,
    /// Which element holds keyboard focus.
    focus: focus::KeptFocus,
    /// Which modifier keys are held.
    modifiers: Modifiers,
    /// The controls the pointer is over, the one it came over last at the end: what the
    /// pointer is over, as the controls' hover tells it.
    hovered: Vec<FocusId>,
    /// The control the primary button last went down or clicked on and where it was laid
    /// out, in the window, which a menu it opens hangs from.
    anchor: Option<(FocusId, rustle::Rect)>,
    /// The element the primary button went down on.
    pressed_on: Option<FocusId>,
    /// The row of the project's files or of the changed files the primary button last went down on, while nothing else was pressed since, so Ctrl+A picks every row of its list.
    list_pressed: Option<FocusId>,
    /// The handle on the edge of a panel being dragged, how big the panel was when the drag began, and which way the handle grows it.
    sash_drag: Option<(FocusId, f32, Grows)>,
    /// The settings window and what it keeps, while settings are open and it is not standing in for the main window.
    settings_surface: Option<asides::Surface>,
    /// The folder picker's window and what it keeps, while the picker is open and it is not standing in for the main window.
    picker_surface: Option<asides::Surface>,
    /// The opening of the folder picker its window was last brought forward for.
    raised_picker: Option<u64>,
    /// The window beside the main one that stands in for it, while one of its events is handled.
    on_aside: Option<asides::Aside>,
    /// The clipboard, once something has needed it.
    clipboard: Option<Box<dyn port::Board>>,
    /// The field a press began in, while the button is held, so dragging selects.
    dragging: Option<FocusId>,
    /// What the press held in a field took, so dragging extends by the same amount.
    field_gesture: Gesture<usize>,
    /// What was last copied in the file shown: its text, the indentation of the lines it came from and whether it is whole lines copied with nothing selected.
    copied: Option<(String, String, bool)>,
    /// The byte of the file a press with Alt and Shift held began at, while the button is held, so dragging selects a rectangle of text.
    column_press: Option<usize>,
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
    /// When the window last looked for a newer release.
    release_asked: Instant,
    /// The agents that run, and the work done for them.
    work: Work,
    /// What was last written as the view, which is what the window opened with until it changes.
    saved_view: Option<View>,
    /// Serializes and writes the latest window state outside the event loop.
    view_writer: crate::view::Writer,
    /// The next debounced capture of the window state.
    view_save_due: Option<Instant>,
    /// Where the input method was last told the caret is, as x, y, width and height in logical pixels, so that it is told again only when the caret moves.
    ime_area: Option<(f32, f32, f32, f32)>,
    /// The text field that held the keyboard before the interface handled the
    /// current event, and what it held then.
    typed_before: Option<(FocusId, String)>,
    /// What held the keyboard before the interface handled the current event.
    focus_before_event: Option<FocusId>,
    /// The start of the current burst, limiting how long continuous typing delays saving.
    view_save_started: Option<Instant>,
    /// The field the reader was typing in when the window last closed, which takes the keyboard once it is drawn.
    refocus: Option<FocusId>,
    /// The project header being carried, by position, from when its drag started.
    project_press: Option<usize>,
    /// Whether the carried project header has moved since its drag started.
    project_dragging: bool,
    /// The tab of a pane being carried, from when its drag started.
    pane_press: Option<pm_components::panes::Item>,
    /// Whether the carried tab has moved since its drag started.
    pane_dragging: bool,
    /// The row of the project's files being carried, from when its drag started.
    tree_press: Option<TreeTarget>,
    /// What is known of files dragged in from outside the window.
    outside: drops::Outside,
    /// Whether the prompt of the page for starting work was given keyboard focus on arriving there.
    autofocused: bool,
    /// Whether the tree was asking for a name when the last frame was drawn.
    naming: bool,
    /// What held keyboard focus when the event loop last came to rest, for telling the screen which text field the reader left.
    focus_before: Option<FocusId>,
    /// How many presses in a row the last press of a control made, as rustle counted them.
    press_clicks: u32,
    /// The view configuration in effect where the last pressed control was composed.
    press_configuration: rustle::ViewConfiguration,
    /// The edge of the window the pointer showed it resizes, when it did.
    edge_cursor: Option<winit::window::ResizeDirection>,
    /// Whether the window's own close button asked it to close and nothing is left to ask first.
    closing: bool,
    /// Whether the field narrowing the list over the file is to take the keyboard once it is drawn.
    focus_filter: bool,
    /// Whether the shell is to take the keyboard once it is drawn.
    focus_terminal: bool,
    /// What the press held in the shell's window took, so dragging marks the same amount.
    terminal_gesture: Gesture<(usize, usize)>,
    /// The part of a line the wheel has scrolled the shell that was not enough to move it.
    terminal_scroll_rest: f32,
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
    /// The engine that paints pages of the web, and the pages it holds.
    web: browser::Web,
    /// Whether the address of the browser is to take the keyboard once it is drawn.
    focus_browser_address: bool,
    /// The file, version, caret, other end of the selection and number of problems the language server was last asked about for the marker beside the caret's line.
    bulb_asked: Option<editing::BulbKey>,
    /// The same, waiting to settle before the language server is asked, and since when.
    bulb_wait: Option<(editing::BulbKey, Instant)>,
    /// The selected text of the file shown that was pressed on, while the button is held.
    text_drag: Option<editing::TextDrag>,
    /// Whether what the reader changed is written to disk, as it is unless the app runs headless.
    keeps_to_disk: bool,
}

impl App {
    /// An application with no window yet, whose clock starts now, whose event loop `wake` brings out of its sleep, and whose GPU is being opened as `opening`.
    pub fn new(wake: pm_client::Wake, proxy: EventLoopProxy) -> Self {
        let mut work = Work::new(wake);
        crate::startup::mark("app-new-begin");
        let saved = pm_store::load_projects();
        let choices = pm_store::load_choices();
        let (logins, in_use) = pm_store::load_accounts();
        let agents = pm_components::agent_list::AgentEntry::shipped();
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
                .and_then(|text| pm_harness::Discovery::from_text(&text))
                {
                    Some(found) => screen.with_discovery(index, found),
                    None => screen,
                },
            );
        let window_settings = pm_store::load_text(WINDOW_SETTINGS)
            .map(|text| pm_config::WindowSettings::read(&text))
            .unwrap_or_default();
        let saved_view =
            View::load().filter(|_| window_settings.general.launch == pm_config::Launch::Restore);
        let screen = match (&saved_view, screen) {
            (Some(view), Screen::Workspace(state)) => Screen::Workspace(view.apply(state)),
            (_, screen) => screen,
        };
        if let Some(view) = saved_view.clone().filter(|view| view.home().is_none()) {
            work.resume(view);
        }
        let editor = pm_store::load_text(EDITOR_SETTINGS)
            .map(|text| pm_editor::EditorSettings::read(&text))
            .unwrap_or_default();
        work.set_servers(&window_settings.servers);
        work.ask_machine_settings();
        if window_settings.general.check_updates {
            work.ask_release(window_settings.general.channel);
        }
        crate::startup::mark("settings-loaded");
        let (all_fonts, mono_fonts) = pm_components::theme::font_families();
        crate::scene::set_fonts(
            &window_settings.appearance.interface_font,
            &window_settings.appearance.code_font,
        );
        let screen = screen
            .with_editor_settings(editor.clone())
            .with_window_settings(window_settings.clone())
            .with_fonts(all_fonts, mono_fonts);
        let mut app = Self::assemble(screen, work, window_settings, editor, Some(proxy));
        app.keeps_to_disk = true;
        app.saved = saved;
        app.saved_choices = choices;
        app.refocus = saved_view.as_ref().and_then(View::typing);
        app.saved_view = saved_view;
        crate::startup::mark("fonts-model-ready");
        app
    }

    /// An application showing `screen`, with no window yet and nothing read
    /// from disk: `work` runs its agents, `window_settings` and `editor` are
    /// the settings it starts with, and `proxy` wakes the event loop that
    /// will open its windows, when there is one.
    fn assemble(
        screen: Screen,
        work: Work,
        window_settings: pm_config::WindowSettings,
        editor: pm_editor::EditorSettings,
        proxy: Option<EventLoopProxy>,
    ) -> Self {
        let model = Model::new(screen);
        let dirty = model.changes();
        let focus = focus::KeptFocus::new();
        Self {
            shell: host::Shell::new(model.clone(), focus.clone(), proxy),
            window: None,
            recompose: true,
            composed_focus: None,
            claimed: controller::Claims::default(),
            fonts_applied: (
                window_settings.appearance.interface_font.clone(),
                window_settings.appearance.code_font.clone(),
            ),
            family: chosen_family(&window_settings.appearance),
            saved_window: window_settings,
            timings: performance::Timings::default(),
            appearance: Appearance::Dark,
            system_appearance: Appearance::Dark,
            model,
            dirty,
            pointer: PointerState::default(),
            focus,
            modifiers: Modifiers::default(),
            hovered: Vec::new(),
            anchor: None,
            pressed_on: None,
            list_pressed: None,
            sash_drag: None,
            settings_surface: None,
            picker_surface: None,
            raised_picker: None,
            on_aside: None,
            clipboard: None,
            dragging: None,
            field_gesture: Gesture::default(),
            column_press: None,
            copied: None,
            composing_in: None,
            saved: None,
            refocus: None,
            saved_view: None,
            view_writer: crate::view::Writer::default(),
            view_save_due: None,
            ime_area: None,
            typed_before: None,
            focus_before_event: None,
            view_save_started: None,
            project_press: None,
            project_dragging: false,
            pane_press: None,
            pane_dragging: false,
            tree_press: None,
            outside: drops::Outside::default(),
            saved_choices: Vec::new(),
            saved_editor: editor,
            autosave_wait: None,
            release_asked: Instant::now(),
            work,
            autofocused: false,
            naming: false,
            focus_before: None,
            press_clicks: 1,
            press_configuration: rustle::ViewConfiguration::default(),
            edge_cursor: None,
            closing: false,
            focus_filter: false,
            focus_terminal: false,
            terminal_gesture: Gesture::default(),
            terminal_scroll_rest: 0.0,
            web: browser::Web::default(),
            focus_browser_address: false,
            hover_rest: None,
            hover_leaving: None,
            hover_entered: false,
            pending: Vec::new(),
            recorded: Vec::new(),
            record_deadline: None,
            highlighted: None,
            highlight_wait: None,
            bulb_asked: None,
            bulb_wait: None,
            text_drag: None,
            keeps_to_disk: false,
        }
    }
}

impl App {
    /// How large the frame of the window opens: as large as it was when the window was last closed, or as fits the screen it opens on.
    fn opening_size(event_loop: &dyn ActiveEventLoop) -> (f32, f32) {
        let saved = pm_store::load_text(WINDOW_SIZE_FILE).and_then(|text| {
            let (width, height) = text.trim().split_once(' ')?;
            Some((width.parse().ok()?, height.parse().ok()?))
        });
        let screen = || {
            let monitor = event_loop
                .primary_monitor()
                .or_else(|| event_loop.available_monitors().next())?;
            let size = monitor
                .current_video_mode()?
                .size()
                .to_logical::<f32>(monitor.scale_factor());
            Some(frame::without_band((size.width, size.height)))
        };
        saved
            .filter(|size: &(f32, f32)| size.0 >= MIN_SAVED && size.1 >= MIN_SAVED)
            .or_else(screen)
            .unwrap_or((WIDTH, HEIGHT))
    }

    /// Keeps how large the frame of the window is for the next launch.
    fn remember_size(window: &dyn port::Port) {
        let size = window.logical_size();
        let size = if window.floats() {
            frame::without_band(size)
        } else {
            size
        };
        if size.0 >= MIN_SAVED && size.1 >= MIN_SAVED {
            let _ = pm_store::save_text(
                WINDOW_SIZE_FILE,
                &format!("{} {}", size.0.round(), size.1.round()),
            );
        }
    }

    /// Marks updates read only after the end of the conversation has been presented in the focused window.
    fn note_conversation(&mut self, presented: bool) {
        let (visible, at_end) = self.model.read(|screen| {
            (
                screen.conversation_area().is_some(),
                screen.conversation_at_end(),
            )
        });
        let focused = presented
            && self
                .window
                .as_ref()
                .is_some_and(|window| window.has_focus())
            && !self.work.resuming();
        let pending = self.model.read(|screen| {
            matches!(screen, Screen::Workspace(state)
            if state.conversation_observation_pending(visible, at_end, focused))
        });
        if pending {
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.observe_conversation(visible, at_end, focused);
            });
        }
    }

    /// Tells the screen how wide the view of the file shown was drawn and how wide a character of code is, when that changed, so long lines wrap at its edge.
    fn note_code_view(&mut self) {
        let Some((width, height)) = self
            .model
            .read(Screen::code_area)
            .map(|view| (view.size.width, view.size.height))
        else {
            return;
        };
        if self.model.read(Screen::code_height) != Some(height) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_code_height(height));
        }
        if self.model.read(Screen::names_in_view_wanted) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).annotations_requested());
            self.deliver();
        }
        let Some(font) = self.model.read(|screen| {
            screen
                .editor_settings()
                .map(|settings| settings.font_size as f32)
        }) else {
            return;
        };
        let cell = rustle::layout_text(
            &"M".repeat(64),
            &pm_components::theme::code_style(font, 1.5),
            None,
        )
        .size()
        .width
            / 64.0;
        let known = self.model.read(Screen::code_view);
        let differs = known.is_none_or(|(held_width, held_cell)| {
            (held_width - width).abs() > 0.5 || (held_cell - cell).abs() > 0.01
        });
        if differs {
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_code_view(width, cell));
        }
    }

    /// Presses the control `id` through the workspace's reducer, as a keymap command or a
    /// key of an open panel does, then does what the press asks of the app.
    pub(super) fn after(&mut self, id: FocusId) {
        self.update(Box::new(move |state| state.after(id)));
    }

    /// Changes the workspace with `change`, then does what it asks of the app.
    pub(super) fn update(&mut self, change: Change) {
        self.model
            .update(|screen| *screen = std::mem::take(screen).changed(change));
        self.settle();
    }

    /// Does what every change of the workspace asks of the app once it is made: shows the
    /// caret, gives keyboard focus and the shell the keyboard as the press asked, then saves
    /// what is to be kept and sends what the agents and the folders are to be told.
    fn settle(&mut self) {
        let mut asked = None;
        self.model.update(|screen| asked = screen.take_asked());
        if let Some(asked) = asked {
            if asked.caret {
                self.show_editor_caret();
            }
            if let Some(next) = asked.focus {
                self.focus.focus(next);
            }
            self.focus_terminal = asked.keyboard_to_shell;
        }
        self.persist();
        self.deliver();
    }

    /// Hands in what the field submits, `id`, as Enter in the field does: the row picked
    /// of the list over the file when it is the list's, or the press of `id` otherwise.
    fn submit(&mut self, id: FocusId) {
        if id == pm_components::listing::LISTING_TAKE {
            self.take_listing(None);
        } else {
            self.after(id);
        }
    }

    /// Takes the row `row` of the list over the file, or its picked row: the command it
    /// holds when the list holds commands, and otherwise the row as the workspace takes it.
    pub(super) fn take_listing(&mut self, row: Option<usize>) {
        let id = row.map_or(pm_components::listing::LISTING_TAKE, |row| {
            FocusId(pm_components::listing::LISTING_ROW_BASE + row as u64)
        });
        if !self.take_command(id) {
            self.after(id);
        }
    }

    /// Counts a click on the control `id`: a click whose press was the second or later in a
    /// row keeps the file its tab or row shows open.
    fn note_click(&mut self, id: FocusId) {
        if self.press_clicks >= 2 {
            self.model
                .update(|state| *state = std::mem::take(state).with_file_kept(id));
        }
    }

    /// Carries out what the reader asked of the agents and the folders, and copies what was asked to be copied.
    fn deliver(&mut self) {
        for text in self.work.deliver(&self.model) {
            if let Some(clipboard) = self.clipboard() {
                clipboard.set_text(text);
            }
        }
        if chosen_family(&self.saved_window.appearance) != self.family {
            self.apply_appearance();
        }
    }

    /// Writes the editor's settings to disk when they differ from what was last written.
    fn persist_editor(&mut self) {
        self.follow_ligatures();
        let Some(settings) = self
            .model
            .read(Screen::editor_settings)
            .filter(|settings| *settings != self.saved_editor)
        else {
            return;
        };
        if self.keeps_to_disk {
            let _ = pm_store::save_text(EDITOR_SETTINGS, &settings.write());
        }
        self.saved_editor = settings;
    }

    /// Writes the window's own settings to disk when they differ from what was last written, and puts what changed of them to work.
    fn persist_window(&mut self) {
        let Some(settings) = self
            .model
            .read(Screen::window_settings)
            .filter(|settings| *settings != self.saved_window)
        else {
            return;
        };
        if self.keeps_to_disk {
            let _ = pm_store::save_text(WINDOW_SETTINGS, &settings.write());
        }
        self.work.set_servers(&settings.servers);
        self.saved_window = settings;
        self.apply_appearance();
    }

    /// When the window looks for a newer release again while updates are looked for, asking now once that time has come.
    fn release_due(&mut self) -> Option<Instant> {
        let general = &self.saved_window.general;
        if !general.check_updates {
            return None;
        }
        if self.release_asked.elapsed() >= RELEASE_RECHECK {
            self.work.ask_release(general.channel);
            self.release_asked = Instant::now();
        }
        Some(self.release_asked + RELEASE_RECHECK)
    }

    /// Draws the window light or dark, in the themes and fonts, as its own settings and the desktop say.
    fn apply_appearance(&mut self) {
        let settings = &self.saved_window.appearance;
        self.appearance = match settings.theme_mode {
            pm_config::ThemeMode::System => self.system_appearance,
            pm_config::ThemeMode::Light => Appearance::Light,
            pm_config::ThemeMode::Dark => Appearance::Dark,
        };
        self.family = chosen_family(settings);
        let palette = crate::work::picture::Palette::of(
            self.family.variant(self.appearance),
            self.appearance == Appearance::Dark,
        );
        if self.work.set_palette(palette) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).without_figures());
        }
        let fonts = (settings.interface_font.clone(), settings.code_font.clone());
        if fonts != self.fonts_applied {
            crate::scene::set_fonts(&fonts.0, &fonts.1);
            self.fonts_applied = fonts;
        }
        let tokens = self.tokens();
        self.shell.set_tokens(tokens);
        self.recompose = true;
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
        if self.keeps_to_disk {
            self.work.save_choices(choices.clone());
        }
        self.saved_choices = choices;
    }

    /// Defers view capture until typing pauses, while saving at least every two seconds.
    fn defer_view_save(&mut self) {
        let now = Instant::now();
        let started = *self.view_save_started.get_or_insert(now);
        self.view_save_due =
            Some((now + Duration::from_millis(300)).min(started + Duration::from_secs(2)));
    }

    /// Captures an expired pending save and returns the next deadline for the event loop.
    fn view_save_deadline(&mut self) -> Option<Instant> {
        if self.view_save_due.is_some_and(|due| due <= Instant::now()) {
            self.view_save_due = None;
            self.view_save_started = None;
            self.persist_view();
        }
        self.view_save_due
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
        }) else {
            return;
        };
        if self.saved_view.as_ref() == Some(&view) {
            return;
        }
        if self.keeps_to_disk {
            self.view_writer.queue(view.clone());
        }
        self.saved_view = Some(view);
    }

    /// Gives `focus` to the field in `refocus`, the one the reader was typing in when the
    /// window last closed, once the view is no longer `resuming`: focus is kept on it until
    /// rustle shows it holding focus, which is once the field is composed.
    fn give_back_focus(refocus: &mut Option<FocusId>, focus: &focus::KeptFocus, resuming: bool) {
        let Some(field) = *refocus else {
            return;
        };
        if resuming {
            return;
        }
        if focus.shown_on(field) {
            *refocus = None;
        } else {
            focus.focus(field);
        }
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
        if self.keeps_to_disk {
            self.work.save_projects(paths.clone());
        }
        self.saved = Some(paths);
    }

    /// Asks the desktop for files and mentions them in the prompt, unless the dialog is dismissed.
    pub(super) fn attach(&mut self) {
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
        self.attach_paths(paths);
    }

    /// Scrolls what is under the pointer by `pixels` logical pixels, downwards when
    /// positive, where the interface did not: the terminal or the sidebar's list.
    fn scroll(&mut self, pixels: f32) {
        if self.terminal_wheel(pixels) {
            return;
        }
        let Some((x, _)) = self.pointer.position else {
            return;
        };
        let height = self.window_size().1;
        self.model
            .update(|state| *state = std::mem::take(state).scrolled(x, pixels, height));
    }

    /// Notes what holds the keyboard and, when it is a text field, what it
    /// holds, before the interface handles an event, so [`App::follow_typing`]
    /// can tell whether the field changed while it did and the frame is
    /// composed again when the keyboard moved.
    pub(super) fn note_typing(&mut self) {
        self.focus_before_event = self.focus.current();
        self.typed_before = self
            .focus
            .current()
            .filter(|id| Screen::is_text_field(*id))
            .and_then(|id| {
                self.model
                    .read(|screen| screen.text_field(id).map(rustle::TextFieldState::text))
                    .map(|text| (id, text))
            });
    }

    /// Catches the screen up with what the reader typed in the text field that
    /// held the keyboard while the interface handled the event: what follows from
    /// the new text, such as a list narrowed or a search asked for.
    fn follow_typing(&mut self) {
        let Some((id, before)) = self.typed_before.take() else {
            return;
        };
        let now = self
            .model
            .read(|screen| screen.text_field(id).map(rustle::TextFieldState::text));
        if now.is_some_and(|now| now != before) {
            self.model
                .update(|state| *state = std::mem::take(state).field_typed(id));
        }
    }

    /// Catches the screen up with what the reader scrolled in the interface,
    /// and asks for the diffs that a scroll of the diff brought into view.
    pub(super) fn follow_scrolls(&mut self) {
        if !self.model.read(Screen::scrolls_moved) {
            return;
        }
        let mut moved = false;
        self.model.update(|screen| moved = screen.follow_scrolls());
        if moved {
            self.deliver();
        }
    }

    /// The control the pointer is over: the one it came over last of those it is still over.
    pub(super) fn under_pointer(&self) -> Option<FocusId> {
        self.hovered.last().copied()
    }

    /// `at`, a point in the window, in the coordinates of the window's frame: past the band
    /// that holds the main window's shadow.
    pub(super) fn framed_point(&self, at: (f32, f32)) -> (f32, f32) {
        let corner = self.framed(rustle::Rect::new(
            rustle::Offset::new(at.0, at.1),
            rustle::Size::new(0.0, 0.0),
        ));
        (corner.origin.x, corner.origin.y)
    }

    /// `rect`, in the window, in the coordinates of the window's frame: past the band that
    /// holds the main window's shadow.
    pub(super) fn framed(&self, rect: rustle::Rect) -> rustle::Rect {
        let band = if self.on_aside.is_none() {
            pm_components::shadow_margin()
        } else {
            0.0
        };
        rustle::Rect::new(
            rustle::Offset::new(rect.origin.x - band, rect.origin.y - band),
            rect.size,
        )
    }

    /// Where the area named `id` that tracks where it is drawn was drawn, in the
    /// coordinates of the window's frame, while it is shown.
    pub(super) fn place(&self, id: FocusId) -> Option<rustle::Rect> {
        let rect = self.model.read(|screen| screen.places().get(id))?;
        Some(self.framed(rect))
    }

    /// How wide and tall the window is, in logical pixels.
    pub(super) fn window_size(&self) -> (f32, f32) {
        let Some(window) = self.window.as_ref() else {
            return (WIDTH, HEIGHT);
        };
        let size = window.logical_size();
        let band = if self.on_aside.is_none() {
            2.0 * pm_components::shadow_margin()
        } else {
            0.0
        };
        (size.0 - band, size.1 - band)
    }

    /// Whether `on` is a handle the composables drag themselves: a panel's handle or the
    /// minimap, whose press is not a press of anything else.
    fn is_drag_handle(&self, on: Option<FocusId>) -> bool {
        let window = self.window_size();
        on.is_some_and(|id| {
            id == MINIMAP || self.model.read(|screen| screen.sash(id, window)).is_some()
        })
    }

    /// Makes the panel whose handle `id` is dragged as big as it was when the drag began
    /// plus how far the drag went, `by` logical pixels across and down.
    pub(super) fn sash_dragged(&mut self, id: FocusId, by: (f32, f32)) {
        let window = self.window_size();
        if self.sash_drag.is_none_or(|(held, ..)| held != id) {
            self.sash_drag = self
                .model
                .read(|screen| screen.sash(id, window))
                .map(|(size, grows)| (id, size, grows));
        }
        if let Some((id, size, grows)) = self.sash_drag {
            self.model.resize(|state| {
                *state = std::mem::take(state).with_sash_size(
                    id,
                    size + grows.by((0.0, 0.0), by),
                    window,
                )
            });
        }
    }

    /// Scrolls the file shown so its view is centred on the point of the minimap `down`
    /// logical pixels from its top, of its `height`.
    pub(super) fn minimap_dragged(&mut self, down: f32, height: f32) {
        self.model
            .update(|state| *state = std::mem::take(state).minimap_pressed(down, height));
    }

    /// Goes to the line kept at the top of the file shown that the press is on, `on`, and says whether it is on one.
    fn press_sticky(&mut self, on: Option<FocusId>) -> bool {
        let Some(above) = on.and_then(pm_components::sticky::sticky_of) else {
            return false;
        };
        self.model
            .update(|state| *state = std::mem::take(state).sticky_pressed(above));
        true
    }

    /// Presses the lens at the end of a line of the file shown that the press is on, `on`, and says whether it is on one.
    fn press_lens(&mut self, on: Option<FocusId>) -> bool {
        let Some(at) = self.pointer.position else {
            return false;
        };
        let Some(index) = on.and_then(pm_components::annotations::lens_of) else {
            return false;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).lens_pressed(index, (at.0, at.1 + 12.0))
        });
        true
    }

    /// Folds a pointer event in, takes focus from the focused element when
    /// a press lands anywhere else, and acts on a press that ends on the
    /// element it began on.
    fn pointer(&mut self, event: PointerEvent) {
        self.pointer.apply(event);
        match event {
            PointerEvent::Pressed(Button::Right) => {
                if !self.claimed.context {
                    self.open_message_menu();
                }
            }
            PointerEvent::Pressed(Button::Left) => {
                if self.model.read(Screen::scrollbar_held) {
                    self.pressed_on = None;
                } else if self.claimed.press.is_none() {
                    self.press_clicks = 1;
                    self.press_at(None);
                }
            }
            PointerEvent::Moved(x, y) => {
                self.editor_pointer_moved(x, y);
                self.terminal_drag((x, y));
                self.reach_terminal_link();
                self.drag_text_moved(x, y);
                if let Some(id) = self.dragging
                    && let Some(offset) = self.code_lines().offset_in(id, x, y)
                {
                    self.drag_in_field(id, offset);
                }
            }
            PointerEvent::Released(Button::Left) => {
                self.sash_drag = None;
                self.drag_text_released();
                self.dragging = None;
                self.column_press = None;
                self.field_gesture.release();
                if self.terminal_gesture.release() {
                    self.copy_terminal_selection();
                }
                if self.claimed.click.is_some() {
                    return;
                }
                let released_on = self.under_pointer();
                if let (Some(pressed), Some(released)) = (self.pressed_on.take(), released_on)
                    && pressed == released
                {
                    self.press_through(pressed);
                }
            }
            _ => {}
        }
    }

    /// Takes a press of the primary button on the control `id`, as its view
    /// tells it while the interface handles the press: a press on the bare
    /// title bar moves the window, or maximizes it when it is the second.
    pub(super) fn press(&mut self, id: FocusId) {
        if id == pm_components::title_bar::TITLE_BAR && self.title_bar_pressed() {
            return;
        }
        self.press_at(Some(id));
    }

    /// Takes a press of the primary button on `on`, or on nothing: a handle,
    /// minimap, line kept at the top or lens takes hold of it; anything
    /// else is pressed, closes what a press elsewhere closes, and takes the
    /// keyboard or starts a selection when it is a field.
    fn press_at(&mut self, on: Option<FocusId>) {
        if self.is_drag_handle(on) || self.press_sticky(on) || self.press_lens(on) {
            self.pressed_on = None;
            return;
        }
        self.terminal_press();
        self.pressed_on = on;
        self.list_pressed = self.pressed_on.filter(|id| {
            self.model.read(|screen| {
                screen.tree_row_of(*id).is_some()
                    || screen.change_row_of(*id).is_some()
                    || screen.change_folder_row_of(*id).is_some()
            })
        });
        self.press_excerpt();
        self.text_press();
        if self.pressed_on != Some(ENTRY_FIELD) && self.model.read(|screen| screen.is_naming()) {
            self.model
                .update(|state| *state = std::mem::take(state).without_naming());
        }
        if self.focus.current() != self.pressed_on {
            self.focus.clear();
        }
        if self.pressed_on.is_none() && self.model.read(Screen::restore_offered) {
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

    /// Closes what a press outside a popup closes, as the popup's `on_dismiss_request`
    /// asks while the interface handles the press, before the control pressed hears of it:
    /// a menu, unless the press is on one of its rows; the list over the file, the boxes
    /// for a new name, a colour and a line to go to, unless it is on them; and the setting
    /// menus and the list of branches when it is on no control at all.
    pub(super) fn dismiss_requested(&mut self) {
        let on = self.under_pointer();
        self.press_beside_code(on);
        if self.model.read(|screen| screen.list_menu_open())
            && !on.is_some_and(|id| {
                is_project_menu_item(id)
                    || is_session_menu_item(id)
                    || is_message_menu_item(id)
                    || split_row_of(id).is_some()
                    || self.model.read(|screen| screen.is_menu_row(id))
            })
        {
            self.model
                .update(|state| *state = std::mem::take(state).without_menu());
        }
        if self.model.read(|screen| screen.menu().is_some()) && !on.is_some_and(is_menu_item) {
            self.model
                .update(|state| *state = std::mem::take(state).without_menu());
        }
        if on.is_none() && self.model.read(|screen| screen.picker_open()) {
            self.model
                .update(|state| *state = std::mem::take(state).without_picker());
        }
    }

    /// Says whether a click on the control `id`, as its view tells it while the interface
    /// handles the release, counts: when the press began on it and was not the start of a
    /// drag. A click that counts gives the keyboard to the field, shell or page it is on.
    pub(super) fn click_lands(&mut self, id: FocusId) -> bool {
        let dragging =
            self.project_dragging || self.pane_dragging || self.model.read(Screen::tree_dragging);
        if dragging || self.pressed_on != Some(id) {
            return false;
        }
        self.pressed_on = None;
        self.take_keyboard_on_click(id);
        self.note_click(id);
        true
    }

    /// Gives the keyboard to `id` when it is a field, the shell or a page, which a click on
    /// it does.
    fn take_keyboard_on_click(&mut self, id: FocusId) {
        if Screen::is_field(id)
            || id == TERMINAL_BODY
            || id == pm_components::browser::BROWSER_BODY
            || id == pm_components::browser::BROWSER_TOOLS_BODY
        {
            self.focus.focus(id);
        }
    }

    /// Carries out a press that began and ended on `id` that no control's view took as a
    /// click, such as a press on the lines of a file in the excerpts: the field takes the
    /// keyboard and the workspace the press.
    fn press_through(&mut self, id: FocusId) {
        self.take_keyboard_on_click(id);
        self.after(id);
    }

    /// Carries out the menu row `row` on the field the menu is open over, and closes the menu.
    pub(super) fn menu_action(&mut self, row: FocusId) {
        let Some(open) = self.model.read(|screen| screen.menu()) else {
            return;
        };
        self.model
            .update(|state| *state = std::mem::take(state).without_menu());
        let field = open.field;
        self.focus.focus(field);
        let text = self.model.read(|screen| screen.text_field(field).cloned());
        match row {
            MENU_UNDO if let Some(text) = &text => text.undo_state().undo(),
            MENU_REDO if let Some(text) = &text => text.undo_state().redo(),
            MENU_SELECT_ALL if let Some(text) = &text => text.select_all(),
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
    pub(super) fn open_settings_menu(&mut self, index: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).with_settings_menu(index, (x - 190.0, y + 14.0))
        });
    }

    /// Opens the menu on the tab of the open file at `index`, just under the pointer and inside the window.
    pub(super) fn open_tab_menu(&mut self, index: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        let (width, height) = self.window_size();
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
    pub(super) fn open_project_menu(&mut self, project: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model.update(|state| {
            *state = state
                .clone()
                .with_project_menu(project, (x - 8.0, y + 12.0))
        });
    }

    /// Where a menu `menu_height` tall opens: just under the pointer and inside the window.
    fn menu_under_pointer(&self, menu_height: f32) -> Option<(f32, f32)> {
        let (x, y) = self.pointer.position?;
        let (width, height) = self.window_size();
        Some((
            (x - 8.0).min(width - 216.0).max(0.0),
            (y + 12.0).min(height - menu_height - 8.0).max(0.0),
        ))
    }

    /// Opens the menu on changed files, just under the pointer and inside the window, over the file at `index`.
    pub(super) fn open_change_menu(&mut self, index: usize) {
        let Some(at) = self.menu_under_pointer(CHANGE_MENU_HEIGHT) else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).with_change_menu(index, at);
        });
    }

    /// Opens the menu on changed files, just under the pointer and inside the window, over the folder at `position` of their tree.
    pub(super) fn open_folder_change_menu(&mut self, position: usize) {
        let Some(at) = self.menu_under_pointer(CHANGE_MENU_HEIGHT) else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).with_folder_change_menu(position, at);
        });
    }

    /// Opens the menu on the project's files, just under the pointer and inside the window, over `target`.
    pub(super) fn open_tree_menu(&mut self, target: TreeTarget) {
        let Some(at) = self.menu_under_pointer(TREE_MENU_HEIGHT) else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).with_tree_menu(target, at);
        });
    }

    /// Opens the menu `kind` of the source control panel from the control `trigger`, below it or above it, or just under the pointer when it comes from no control.
    pub(super) fn open_git_menu(&mut self, kind: MenuKind, trigger: Option<FocusId>) {
        let window = self.window_size();
        let control = self
            .anchor
            .filter(|(id, _)| Some(*id) == trigger)
            .map(|(_, area)| {
                let rect = self.framed(area);
                (
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                )
            });
        let at = if let Some(control) = control {
            self.model
                .read(|screen| screen.git_menu_below(kind, control, window))
        } else {
            let Some((x, y)) = self.pointer.position else {
                return;
            };
            match kind {
                MenuKind::History(_) => (x - 8.0, y + 12.0),
                _ => (x - 190.0, y + 14.0),
            }
        };
        self.model
            .update(|state| *state = std::mem::take(state).with_git_menu(kind, at));
    }

    /// Opens the menu of the session at `session`, just under the pointer.
    pub(super) fn open_session_menu(&mut self, session: usize) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        self.model.update(|state| {
            *state = state
                .clone()
                .with_session_menu(session, (x - 8.0, y + 12.0))
        });
    }

    /// Follows the pointer to `at` while a project header is carried: the project under the pointer is the one it would take the place of.
    fn drag_project(&mut self, at: (f32, f32)) {
        let Some(from) = self.project_press else {
            return;
        };
        self.project_dragging = true;
        let hit = self.under_pointer();
        let over = self.model.read(|held| {
            let current = match held {
                Screen::Workspace(state) => state.project_drag.map_or(from, |drag| drag.over),
            };
            hit.and_then(|id| held.project_at(id))
                .filter(|project| *project != from)
                .unwrap_or(current)
        });
        self.model
            .update(|state| *state = std::mem::take(state).dragged_over(from, over, at));
    }

    /// Follows a press and drag on the number of the line `id` of a diff: the press starts
    /// selecting lines for a comment there, and the drag stretches the lines to the number
    /// under the pointer.
    pub(super) fn comment_dragged(&mut self, id: FocusId) {
        if !self.model.read(|screen| screen.comment_dragging()) {
            self.model
                .update(|state| *state = std::mem::take(state).comment_drag_begun(id));
            return;
        }
        if let Some(id) = self.under_pointer().filter(|id| gutter_of(*id).is_some()) {
            self.model
                .update(|state| *state = std::mem::take(state).comment_drag_extended(id));
        }
    }

    /// Ends a selection of lines, which opens the box for a comment on them and gives it the keyboard.
    pub(super) fn end_comment_drag(&mut self) {
        if !self.model.read(|screen| screen.comment_dragging()) {
            return;
        }
        self.model
            .update(|state| *state = std::mem::take(state).comment_drag_ended());
        self.focus.focus(COMMENT_FIELD);
    }

    /// Ends a drag of a project header, putting the project where it was let go, and says whether there was a drag.
    pub(super) fn drop_project(&mut self) -> bool {
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
    pub(super) fn open_menu(&mut self, id: FocusId) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        if Screen::is_text_field(id) {
            self.focus.focus(id);
            self.model
                .update(|state| *state = std::mem::take(state).with_menu(id, (x, y)));
            return;
        }
        let Some(offset) = self.code_lines().offset_in(id, x, y) else {
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
            let (width, height) = self.window_size();
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

    /// Takes a press on the lines of a file in the excerpts that is not the one being edited: the file becomes the one edited and its caret goes where the pointer is.
    fn press_excerpt(&mut self) {
        let (Some(id), Some((x, y))) = (self.pressed_on, self.pointer.position) else {
            return;
        };
        let Some(index) = self.model.read(|screen| screen.excerpt_owner_of(id)) else {
            return;
        };
        let offset = self.code_lines().offset_in(id, x, y);
        self.model
            .update(|screen| *screen = std::mem::take(screen).excerpt_file_shown(index));
        self.pressed_on = Some(EDITOR_FIELD);
        self.focus.focus(EDITOR_FIELD);
        if let Some(offset) = offset {
            self.press_field_at(EDITOR_FIELD, offset, Reach::Point, false);
        }
    }

    /// Takes a press inside the field `id`: focuses it. A text field places
    /// its caret and selects by words and lines itself; in the code editor the
    /// caret goes where the pointer is and a selection starts that dragging
    /// extends, a second press soon after the first selecting the word under it
    /// and a third the line, which dragging then extends by words or lines.
    fn press_in_field(&mut self, id: FocusId) {
        if Screen::is_text_field(id) {
            self.focus.focus(id);
            return;
        }
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        let Some(offset) = self.code_lines().offset_in(id, x, y) else {
            return;
        };
        self.focus.focus(id);
        if id == EDITOR_FIELD && self.modifiers.alt && self.modifiers.shift {
            let byte = self
                .model
                .read(|screen| screen.field(id).map(|field| field.to_text(offset)));
            if let Some(byte) = byte {
                self.column_press = Some(byte);
                self.dragging = Some(id);
                self.edit_field(id, Edit::Select(byte, byte));
            }
            return;
        }
        if id == EDITOR_FIELD && self.modifiers.alt {
            self.edit_field(id, Edit::AddSelection(offset, offset));
            return;
        }
        let reach = Reach::of_clicks(u8::try_from(self.press_clicks).unwrap_or(u8::MAX));
        if id == EDITOR_FIELD
            && reach == Reach::Point
            && !self.modifiers.shift
            && !self.modifiers.ctrl
            && !self.modifiers.meta
            && let Some(byte) = self
                .model
                .read(|screen| screen.field(id).map(|field| field.to_text(offset)))
            && self.drag_text_begins(byte, (x, y))
        {
            return;
        }
        self.press_field_at(id, offset, reach, self.modifiers.shift);
    }

    /// Starts the press in the field `id` at the shown offset `offset`, taking `reach` of the text, or running the selection there was out to it when `extending`.
    fn press_field_at(&mut self, id: FocusId, offset: usize, reach: Reach, extending: bool) {
        let Some((byte, anchor)) = self.model.read(|screen| {
            screen
                .field(id)
                .map(|field| (field.to_text(offset), field.anchor()))
        }) else {
            return;
        };
        let mut gesture = std::mem::take(&mut self.field_gesture);
        let mut surface = CodeText { app: self, id };
        match extending {
            true => gesture.press_from(&mut surface, anchor, byte),
            false => gesture.press(&mut surface, byte, reach),
        }
        self.field_gesture = gesture;
        self.dragging = Some(id);
    }

    /// Extends the press held in the field `id` to the shown offset `offset`.
    fn drag_in_field(&mut self, id: FocusId, offset: usize) {
        let Some(byte) = self
            .model
            .read(|screen| screen.field(id).map(|field| field.to_text(offset)))
        else {
            return;
        };
        if let Some(from) = self.column_press {
            self.model.update(|screen| {
                *screen = std::mem::take(screen).lines_edited(LineEdit::ColumnDrag(from, byte))
            });
            self.show_editor_caret();
            return;
        }
        let gesture = std::mem::take(&mut self.field_gesture);
        gesture.drag(&mut CodeText { app: self, id }, byte);
        self.field_gesture = gesture;
    }

    /// Moves the caret of the multiline field `id` by `by` lines as they are
    /// drawn, to the start or the end of the text when there is no line there.
    fn move_vertically(&mut self, id: FocusId, by: isize, extend: bool) {
        let Some((caret, length, several)) = self.model.read(|screen| {
            screen.field(id).map(|field| {
                (
                    field.to_shown(field.caret()),
                    field.shown().len(),
                    field.has_others(),
                )
            })
        }) else {
            return;
        };
        if several {
            let motion = if by < 0 {
                pm_field::Motion::LineUp
            } else {
                pm_field::Motion::LineDown
            };
            self.edit_field(id, Edit::Move(motion, extend));
            return;
        }
        let target = self
            .code_lines()
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
                .map(|field| (field.to_shown(field.caret()), field.has_others()))
        }) else {
            return;
        };
        if several {
            let motion = if end {
                pm_field::Motion::LineEnd
            } else {
                pm_field::Motion::LineStart
            };
            self.edit_field(id, Edit::Move(motion, extend));
            return;
        }
        if let Some(target) = self.code_lines().line_edge(id, caret, end) {
            self.edit_field(id, Edit::Place(target, extend));
        }
    }

    /// Shows `text` an input method is composing in the field or shell holding
    /// the keyboard, taking it away from the one that showed it before.
    fn compose(&mut self, text: String) {
        if text.is_empty() && self.composing_in.is_none() {
            return;
        }
        let target = self
            .focus
            .current()
            .filter(|id| *id == TERMINAL_BODY || *id == EDITOR_FIELD);
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

    /// Makes `edit` to the code editor's field, which `id` names.
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

    /// Tells the screen which text field the reader left, when keyboard focus moved off one since the event loop last came to rest, so a field that saves on leaving does.
    fn tell_field_left(&mut self) {
        let focused = self.focus.current();
        if focused == self.focus_before {
            return;
        }
        if let Some(left) = self.focus_before.filter(|id| Screen::saves_on_leaving(*id)) {
            self.model
                .update(|state| *state = std::mem::take(state).field_left(left));
        }
        self.focus_before = focused;
    }

    /// Scrolls the file shown just far enough to show its caret.
    fn show_editor_caret(&mut self) {
        let drawn = self
            .model
            .read(Screen::code_area)
            .map(|view| view.size.height + STRIP);
        let window = self.window.as_ref().map(|_| self.window_size().1 - TABS);
        if let Some(height) = drawn.or(window) {
            self.model
                .update(|state| *state = std::mem::take(state).with_caret_shown(height));
        }
    }

    /// The clipboard, the system's opened on first use.
    fn clipboard(&mut self) -> Option<&mut (dyn port::Board + 'static)> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new()
                .ok()
                .map(|clipboard| Box::new(clipboard) as Box<dyn port::Board>);
        }
        self.clipboard.as_deref_mut()
    }

    /// Carries out a clipboard shortcut in the field `id`.
    fn clip(&mut self, id: FocusId, action: Clip) {
        if let Some(text) = self.model.read(|screen| screen.text_field(id).cloned()) {
            self.clip_text(id, &text, action);
            return;
        }
        match action {
            Clip::Copy | Clip::Cut => {
                let selected = self.model.read(|screen| {
                    screen
                        .field(id)
                        .map(|field| {
                            if field.has_others() {
                                field.selected_all()
                            } else {
                                field.selected().to_owned()
                            }
                        })
                        .unwrap_or_default()
                });
                let mut whole_lines = false;
                let mut indentation = String::new();
                let mut selected = selected;
                if id == EDITOR_FIELD
                    && let Some((lines, indent)) = self.model.read(Screen::editor_copied)
                {
                    indentation = indent;
                    if selected.is_empty()
                        && let Some(lines) = lines
                    {
                        selected = lines;
                        whole_lines = true;
                    }
                }
                if selected.is_empty() {
                    return;
                }
                if id == EDITOR_FIELD {
                    self.copied = Some((selected.clone(), indentation, whole_lines));
                }
                if let Some(clipboard) = self.clipboard() {
                    clipboard.set_text(selected);
                }
                if action == Clip::Cut {
                    if whole_lines {
                        self.model.update(|screen| {
                            *screen = std::mem::take(screen).lines_edited(LineEdit::Delete)
                        });
                        self.show_editor_caret();
                    } else {
                        self.edit_field(id, Edit::Backspace);
                    }
                }
            }
            Clip::Paste => {
                if id == PROMPT && self.paste_attachments() {
                    return;
                }
                let pasted = self.clipboard().and_then(|clipboard| clipboard.text());
                if let Some(text) = pasted {
                    let adjusted = (id == EDITOR_FIELD)
                        .then_some(self.copied.as_ref())
                        .flatten()
                        .filter(|(copied, ..)| *copied == text)
                        .and_then(|(_, indentation, whole)| {
                            self.model
                                .read(|screen| screen.editor_paste_edit(&text, *whole, indentation))
                        });
                    self.edit_field(id, adjusted.unwrap_or(Edit::Paste(text)));
                }
            }
        }
    }

    /// Carries out a clipboard shortcut in the text field `id`, which edits
    /// `field`: nothing of a secure field is copied, and what is pasted in a
    /// field of one line loses its line breaks.
    fn clip_text(&mut self, id: FocusId, field: &rustle::TextFieldState, action: Clip) {
        match action {
            Clip::Copy | Clip::Cut => {
                let selected = field.selected_text();
                if Screen::is_secure(id) || selected.is_empty() {
                    return;
                }
                if let Some(clipboard) = self.clipboard() {
                    clipboard.set_text(selected);
                }
                if action == Clip::Cut {
                    field.edit(|buffer| {
                        let selection = buffer.selection();
                        buffer.delete(selection.min(), selection.max());
                    });
                }
            }
            Clip::Paste => {
                if id == PROMPT && self.paste_attachments() {
                    return;
                }
                let Some(pasted) = self.clipboard().and_then(|clipboard| clipboard.text()) else {
                    return;
                };
                let multiline = Screen::is_multiline(id);
                let pasted: String = pasted
                    .chars()
                    .filter(|c| !c.is_control() || (multiline && (*c == '\n' || *c == '\t')))
                    .collect();
                field.edit(|buffer| {
                    let selection = buffer.selection();
                    buffer.replace(selection.min(), selection.max(), &pasted);
                    buffer.place_cursor_before_char_at(selection.min() + pasted.chars().count());
                });
            }
        }
    }

    /// Whether the prompt box holds keyboard focus.
    fn in_prompt(&self) -> bool {
        self.focus.current() == Some(PROMPT)
    }

    /// Steps back or forward through what the window in front shows: the folders looked inside while the folder picker is in front, and the pages and views visited otherwise.
    fn step_through_history(&mut self, back: bool) {
        match self.on_aside {
            Some(asides::Aside::Picker) => {
                self.after(if back { PICKER_BACK } else { PICKER_FORWARD })
            }
            _ => self.navigate(if back { -1 } else { 1 }),
        }
    }

    /// Moves `delta` pages or views along the ones visited, towards the past when negative.
    pub(super) fn navigate(&mut self, delta: isize) {
        self.model
            .update(|screen| *screen = std::mem::take(screen).navigated(delta));
    }

    /// Carries out a key aimed at the list a row was last pressed in, and says whether it was one: the key that selects everything picks every row, and the project's files take the keys that cut, copy, paste, rename and delete.
    fn list_key(&mut self, event: &KeyEvent) -> bool {
        let Some(id) = self.list_pressed else {
            return false;
        };
        if self.focus.current().is_some_and(Screen::is_field) {
            return false;
        }
        if self.model.read(|screen| screen.tree_row_of(id).is_some()) && self.tree_key(event) {
            return true;
        }
        if !matches!(keys::edit(event), Some(Edit::SelectAll)) {
            return false;
        }
        self.model.update(|screen| {
            let tree = screen.tree_row_of(id).is_some();
            let taken = std::mem::take(screen);
            *screen = if tree {
                taken.tree_all_picked()
            } else {
                taken.changes_all_picked()
            };
        });
        true
    }

    /// Acts on `event`, a key the window's root sees before whatever holds rustle's focus,
    /// and says whether the app took it, in which case nothing else in the window sees it.
    /// A key the app leaves goes on to what holds focus, then to rustle, whose Tab walks
    /// focus along the window's controls.
    pub(super) fn key_event(&mut self, event: &KeyEvent) -> bool {
        if !keys::is_app_key(event) {
            return false;
        }
        if !event.pressed {
            self.browser_key_released(event);
            return false;
        }
        self.keyed(event)
    }

    /// Carries out a pressed key, and says whether anything took it: Escape lets go of
    /// focus, and the panels and fields open take their own keys first. Tab is left to
    /// rustle unless the prompt takes it, and Enter and Space on a control to the control,
    /// which runs its own lambda.
    fn keyed(&mut self, event: &KeyEvent) -> bool {
        if self.recording_key(event)
            || self.browser_key(event)
            || self.terminal_key(event)
            || self.list_key(event)
            || self.vim_key(event)
            || self.text_key(event)
            || self.card_key(event)
            || self.rename_key(event)
            || self.find_key(event)
            || self.listing_key(event)
            || self.keymap_key(event)
            || self.editor_key(event)
        {
            return true;
        }
        match event.key {
            Key::Escape if self.on_aside == Some(asides::Aside::Picker) => self.after(PICKER_CLOSE),
            Key::ArrowUp if self.on_aside == Some(asides::Aside::Picker) => self.after(PICKER_UP),
            Key::ArrowDown if self.on_aside == Some(asides::Aside::Picker) => {
                self.after(PICKER_DOWN)
            }
            Key::Enter if self.on_aside == Some(asides::Aside::Picker) => {
                if let Some(id) = self.model.read(|screen| screen.folder_picker_enter()) {
                    self.after(id);
                }
            }
            Key::Escape if self.model.read(|screen| screen.settings_open()) => {
                self.after(SETTINGS_CLOSE)
            }
            Key::Escape if self.model.read(|screen| screen.adder_open()) => {
                self.after(ADDER_ESCAPE)
            }
            Key::ArrowUp if self.model.read(|screen| screen.adder_open()) => self.after(ADDER_UP),
            Key::ArrowDown if self.model.read(|screen| screen.adder_open()) => {
                self.after(ADDER_DOWN)
            }
            Key::Enter if self.model.read(|screen| screen.adder_open()) => {
                if let Some(id) = self.model.read(|screen| screen.adder_enter()) {
                    self.after(id);
                }
            }
            Key::Character(_)
                if let Some(digit) =
                    keys::character(event).and_then(|typed| typed.to_digit(10))
                    && !self.focus.current().is_some_and(Screen::is_field)
                    && let Some(id) = self
                        .model
                        .read(|screen| screen.inquiry_hotkey(digit as usize)) =>
            {
                self.after(id);
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
            Key::Escape if self.model.read(|screen| screen.review_overlay_open()) => {
                self.after(REVIEW_DISMISS)
            }
            Key::Escape if self.model.read(|screen| screen.commenting()) => {
                self.after(COMMENT_CANCEL)
            }
            Key::Escape if self.text_drag.is_some() => self.drag_text_cancelled(),
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
            _ => return self.focused_key(event),
        }
        true
    }

    /// Carries out a key aimed at whatever holds the keyboard, and says whether it took it:
    /// a text field's Enter and clipboard keys and the code editor's keys. Enter or Space
    /// on a control goes on to the control.
    fn focused_key(&mut self, event: &KeyEvent) -> bool {
        match self.focus.current() {
            Some(id) if Screen::is_text_field(id) => self.text_field_key(id, event),
            Some(id) if Screen::is_field(id) => {
                let multiline = self
                    .model
                    .read(|screen| screen.field(id).is_some_and(pm_field::Field::is_multiline));
                let shift = event.modifiers.shift;
                let hurried = event.modifiers.ctrl || event.modifiers.meta;
                let breaks = shift || (Screen::enter_breaks_line(id) && !hurried);
                if event.key == Key::Enter && breaks && multiline {
                    self.edit_field(id, Edit::Insert('\n'));
                } else if event.key == Key::Enter
                    && hurried
                    && let Some(submit) = Screen::hurried_submit_of(id)
                {
                    self.submit(submit);
                } else if event.key == Key::Enter {
                    self.submit(Screen::submit_of(id));
                } else if multiline && matches!(event.key, Key::ArrowUp | Key::ArrowDown) {
                    self.move_vertically(id, if event.key == Key::ArrowUp { -1 } else { 1 }, shift);
                } else if multiline && matches!(event.key, Key::Home | Key::End) {
                    self.move_to_line_edge(id, event.key == Key::End, shift);
                } else if id == EDITOR_FIELD && self.move_along_screen(event) {
                } else if let Some(action) = keys::clip(event) {
                    self.clip(id, action);
                } else if let Some(edit) = keys::edit(event) {
                    self.edit_field(id, edit);
                }
                true
            }
            _ => false,
        }
    }

    /// Carries out a key aimed at the text field `id` that the app keeps, and says
    /// whether it did: Enter presses what the field hands in, Control or Command with
    /// it what it hands in at once, and breaks the line instead in a field of several
    /// lines when Shift is held or the field breaks lines on Enter; the clipboard keys
    /// use the app's clipboard. The field edits with every other key itself.
    fn text_field_key(&mut self, id: FocusId, event: &KeyEvent) -> bool {
        let shift = event.modifiers.shift;
        let hurried = event.modifiers.ctrl || event.modifiers.meta;
        let breaks = shift || (Screen::enter_breaks_line(id) && !hurried);
        if event.key == Key::Enter && breaks && Screen::is_multiline(id) {
            false
        } else if event.key == Key::Enter
            && hurried
            && let Some(submit) = Screen::hurried_submit_of(id)
        {
            self.submit(submit);
            true
        } else if event.key == Key::Enter {
            self.submit(Screen::submit_of(id));
            true
        } else if let Some(action) = keys::clip(event) {
            self.clip(id, action);
            true
        } else {
            false
        }
    }
}

/// The byte offset in the text of `field` that the offset `shown` of what is shown stands for.
fn field_text_offset(field: &pm_field::Field, shown: usize) -> usize {
    field.to_text(shown)
}

/// The themes `settings` pick: the light one for the window drawn light, the dark one for it drawn dark.
fn chosen_family(settings: &pm_config::AppearanceSettings) -> ThemeFamily {
    let dark = pm_components::theme::family_named(&settings.dark_theme);
    ThemeFamily {
        name: dark.name,
        dark: dark.dark,
        light: pm_components::theme::family_named(&settings.light_theme).light,
    }
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

impl App {
    /// Does what waits for the loop to come to rest: the agents' work, the
    /// model's requests and saving. Says how the loop is to wait and whether
    /// the app is to quit.
    fn rest(&mut self) -> (ControlFlow, bool) {
        self.work.discover();
        self.work.poll(&self.model);
        self.pump_drops();
        self.settle_rename_focus();
        if self.model.read(Screen::editor_settings_wanted) {
            let settings = self.saved_editor.clone();
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_editor_settings(settings));
        }
        if self.model.read(Screen::window_settings_wanted) {
            let settings = self.saved_window.clone();
            let (all, mono) = pm_components::theme::font_families();
            self.model.update(|screen| {
                *screen = std::mem::take(screen)
                    .with_window_settings(settings)
                    .with_fonts(all, mono);
            });
            self.work.show_release(&self.model);
        }
        if self.model.read(Screen::has_outbox) {
            self.deliver();
        }
        if self.dirty.get() {
            self.defer_view_save();
            self.persist_window();
            self.persist_editor();
        }
        let browsing = self.browser_tick();
        let waiting = self.model.read(|screen| screen.waiting()) || self.work.busy();
        let hover = [
            self.editor_hover_due(),
            self.editor_highlight_due(),
            self.editor_bulb_due(),
            self.editor_autosave_due(),
            self.recording_due(),
            self.release_due(),
            self.view_save_deadline(),
            self.drop_deadline(),
        ]
        .into_iter()
        .flatten()
        .min();
        if self.model.read(Screen::has_outbox) {
            self.deliver();
        }
        let flow = match (waiting, hover) {
            _ if browsing => ControlFlow::WaitUntil(Instant::now() + browser::PUMP),
            _ if self.work.hearing() => {
                ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(2))
            }
            (true, _) => ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)),
            (false, Some(due)) => ControlFlow::WaitUntil(due),
            (false, None) => ControlFlow::Wait,
        };
        if self.model.read(Screen::place_unrecorded) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_place_recorded());
        }
        (flow, self.closing || self.model.read(Screen::quitting))
    }
}
