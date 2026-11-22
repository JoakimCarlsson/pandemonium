//! The application: the window, its renderer and the events that drive them.

use pm_components::areas::SettingsEvent;
use pm_components::areas::{
    ComposerEvent, EditorEvent, MenuEvent, TerminalEvent, TreeEvent, WindowEvent,
};

use pm_components::areas::GitEvent;

use pm_components::areas::DiffEvent;

use pm_components::areas::DialogEvent;

use pm_components::events::Event;
use pm_components::events::FieldEvent;
use pm_components::focus::FocusRequesters;
use pm_components::target::Target;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::model::Model;

use pm_components::events::Change;

use pm_components::git_menus::MenuKind;
use pm_components::line_edits::LineEdit;
use pm_components::open_files::STRIP;
use pm_components::sash::Grows;

use pm_buffer::Reach;
use pm_components::EditorCommand;
use pm_components::pointing::{CodePointer, Press, Stroke};
use pm_components::theme::{Appearance, ThemeFamily};
use pm_components::tree_actions::TreeTarget;
use pm_editor::Edit;
use pm_pages::{Screen, WorkspaceState};
use rustle::{Key, KeyEvent, Modifiers};

mod asides;
mod browser;
mod browser_keys;
mod capture;
mod code;
mod controller;
mod editing;
mod frame;
#[cfg(test)]
mod headless;
mod host;
#[cfg(test)]
mod interactions;
mod panes;
mod paste;
mod performance;
pub(crate) mod port;
mod select;
mod selecting;
mod terminal;
mod tree;
mod vim;

use self::selecting::{Hold, Selected};
use crate::keys;
use crate::keys::Clip;
use crate::view::View;
use crate::work::Work;

pub use self::host::run;

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

/// The file the editor's settings are kept in.
const EDITOR_SETTINGS: &str = "editor-settings.json";

/// The file the window's own settings are kept in.
const WINDOW_SETTINGS: &str = "window-settings.json";

/// How long the window waits before looking for a newer release again while it stays open.
const RELEASE_RECHECK: Duration = Duration::from_secs(6 * 60 * 60);

/// How long typing pauses before the view is written.
const VIEW_SAVE_PAUSE: Duration = Duration::from_millis(300);

/// How long the view waits at most, while it keeps changing, before it is written.
const VIEW_SAVE_BURST: Duration = Duration::from_secs(2);

/// The window and what draws into it; both exist once the platform is ready.
pub struct App {
    /// The interface on rustle, its windows and what its root reads.
    shell: host::Shell,
    /// The window standing as the main one, once it is open.
    window: Option<Rc<dyn port::Port>>,
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
    /// The rustle focus of every control, which says which holds keyboard focus.
    focus: FocusRequesters,
    /// Which modifier keys are held.
    modifiers: Modifiers,
    /// The control the primary button last went down or clicked on, or the secondary one
    /// went down on, where it was laid out and where the pointer was, in the window, which a
    /// menu it opens hangs from.
    anchor: Option<Press>,
    /// Whether the conversation said the primary button went down inside it, for the press
    /// being handled, so its selection stays.
    pressed_in_conversation: bool,
    /// The row of the project's files or of the changed files the primary button last went down on, while nothing else was pressed since, so Ctrl+A picks every row of its list.
    list_pressed: Option<Target>,
    /// The handle on the edge of a panel being dragged, how big the panel was when the drag began, and which way the handle grows it.
    sash_drag: Option<(Target, f32, Grows)>,
    /// The settings window and what it keeps, while settings are open and it is not standing in for the main window.
    settings_surface: Option<asides::Surface>,
    /// The folder picker's window and what it keeps, while the picker is open and it is not standing in for the main window.
    picker_surface: Option<asides::Surface>,
    /// The opening of the folder picker its window was last brought forward for.
    raised_picker: Option<u64>,
    /// The window beside the main one that stands in for it, while one of its events is handled.
    on_aside: Option<asides::Aside>,
    /// The field a press began in, while the button is held, so dragging selects.
    dragging: Option<Target>,
    /// What the press held in the code editor took, so dragging extends by the same amount.
    code_hold: Option<Hold<usize>>,
    /// What was last copied in the file shown: its text, the indentation of the lines it came from and whether it is whole lines copied with nothing selected.
    copied: Option<(String, String, bool)>,
    /// The byte of the file a press with Alt and Shift held began at, while the button is held, so dragging selects a rectangle of text.
    column_press: Option<usize>,
    /// The field or shell showing what an input method is composing, while it composes something.
    composing_in: Option<Target>,
    /// The project paths last written to disk, so only changes are written.
    saved: Option<Vec<String>>,
    /// The choices for the agents' settings last written to disk, so only changes are written.
    saved_choices: Vec<pm_store::Choice>,
    /// The editor's settings as they were last written to disk.
    saved_editor: pm_editor::EditorSettings,
    /// The agents that run, and the work done for them.
    work: Work,
    /// What was last written as the view, which is what the window opened with until it changes.
    saved_view: Option<View>,
    /// Serializes and writes the latest window state outside the event loop.
    view_writer: crate::view::Writer,
    /// Whether the view changed since it was last written.
    view_unsaved: bool,
    /// The text field that held the keyboard before the interface handled the
    /// current event, and what it held then.
    typed_before: Option<(Target, String)>,
    /// The field the reader was typing in when the window last closed, which takes the
    /// keyboard back as it appears, until the reader presses or types anything or it has it.
    refocus: Option<Target>,
    /// What held keyboard focus when the event loop last came to rest, for telling the screen which text field the reader left.
    focus_before: Option<Target>,
    /// Whether the window's own close button asked it to close and nothing is left to ask first.
    closing: bool,
    /// What the press held in the shell's window took, so dragging marks the same amount.
    terminal_hold: Option<Hold<(usize, usize)>>,
    /// The cell of the shell's window the pointer is over, as the window last told.
    terminal_hover: Option<(usize, usize)>,
    /// The part of a line the wheel has scrolled the shell that was not enough to move it.
    terminal_scroll_rest: f32,
    /// The word of the file shown the pointer rests on, by where it starts, while the language server is yet to be asked about it.
    hover_rest: Option<usize>,
    /// Whether the pointer left both the card about a word and the word, while the card is still showing.
    hover_leaving: bool,
    /// Whether the pointer has been on the card about a word since it opened, so leaving it takes it away at once.
    hover_entered: bool,
    /// The byte of the file shown whose drawn character the pointer is over, as the code last told.
    editor_hover: Option<usize>,
    /// Whether the pointer is over the card about a word, as the card last told.
    hover_card_over: bool,
    /// Whether the primary button went down on the card about a word and is still held.
    hover_card_held: bool,
    /// The chords pressed so far of a binding of more than one.
    pending: Vec<pm_editor::Chord>,
    /// The chords pressed so far while keys are recorded on the keymap page, as they are written.
    recorded: Vec<String>,
    /// The file, version and caret the language server was last asked about for where the thing at the caret is used.
    highlighted: Option<(String, i32, usize)>,
    /// The engine that paints pages of the web, and the pages it holds.
    web: browser::Web,
    /// The file, version, caret, other end of the selection and number of problems the language server was last asked about for the marker beside the caret's line.
    bulb_asked: Option<editing::BulbKey>,
    /// The byte of the selected text of the file shown that was pressed on, while the button is held and no drag has started from it.
    text_press: Option<usize>,
    /// Whether what the reader changed is written to disk, as it is unless the app runs headless.
    keeps_to_disk: bool,
    /// How large the main window's frame opens, in logical pixels.
    opening_frame: (f32, f32),
    /// Whether the main window is to be fitted to its screen once it is open, as no size was kept.
    fit_to_screen: bool,
    /// Whether the main window was taken in once rustle opened it.
    opened: bool,
    /// How many physical pixels a logical pixel of the main window is.
    scale: f32,
    /// The files sent with messages whose pictures were asked to be decoded, so each is asked once.
    previews_asked: std::collections::HashSet<String>,
    /// Whether the browser's pages are being shown or worked on, so they are pumped.
    browsing: bool,
    /// Whether a key or an input method's text was handed to the interface, which the app
    /// catches up with when it is next woken.
    event_pending: bool,
    /// Whether rustle drew a frame since the app was last woken.
    frame_pending: bool,
    /// Wakes rustle's loop to run the app's external handler, once the event being
    /// handled is done.
    loop_wake: pm_client::Wake,
}

impl App {
    /// An application with no window yet, whose clock starts now and whose work brings
    /// rustle's event loop out of its sleep through `wake`, rustle's external waker.
    pub fn new(wake: pm_client::Wake) -> Self {
        let mut work = Work::new(Arc::clone(&wake));
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
            .in_workspace(|state| {
                state
                    .with_choices(choices.clone())
                    .with_accounts(logins, in_use)
            });
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
                    Some(found) => screen.in_workspace(|state| state.with_discovery(index, found)),
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
        let screen = screen.in_workspace(|state| {
            state
                .with_editor_settings(editor.clone())
                .with_window_settings(window_settings.clone())
                .with_fonts(all_fonts, mono_fonts)
        });
        let mut app = Self::assemble(screen, work, window_settings, editor, wake);
        app.window = Some(Rc::new(port::NativePort::new(rustle::WindowHandle::MAIN)));
        let saved_size = Self::saved_size();
        app.fit_to_screen = saved_size.is_none();
        app.opening_frame = saved_size.unwrap_or((WIDTH, HEIGHT));
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
    /// the settings it starts with, and `loop_wake` wakes the event loop that
    /// runs its windows to run its external handler.
    fn assemble(
        screen: Screen,
        work: Work,
        window_settings: pm_config::WindowSettings,
        editor: pm_editor::EditorSettings,
        loop_wake: pm_client::Wake,
    ) -> Self {
        let model = Model::new(screen);
        let focus = FocusRequesters::new();
        Self {
            shell: host::Shell::new(model.clone(), focus.clone()),
            window: None,
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
            focus,
            modifiers: Modifiers::default(),
            anchor: None,
            pressed_in_conversation: false,
            list_pressed: None,
            sash_drag: None,
            settings_surface: None,
            picker_surface: None,
            raised_picker: None,
            on_aside: None,
            dragging: None,
            code_hold: None,
            column_press: None,
            copied: None,
            composing_in: None,
            saved: None,
            refocus: None,
            saved_view: None,
            view_writer: crate::view::Writer::default(),
            view_unsaved: false,
            typed_before: None,
            saved_choices: Vec::new(),
            saved_editor: editor,
            work,
            focus_before: None,
            closing: false,
            terminal_hold: None,
            terminal_hover: None,
            terminal_scroll_rest: 0.0,
            web: browser::Web::default(),
            hover_rest: None,
            hover_leaving: false,
            hover_entered: false,
            editor_hover: None,
            hover_card_over: false,
            hover_card_held: false,
            pending: Vec::new(),
            recorded: Vec::new(),
            highlighted: None,
            bulb_asked: None,
            text_press: None,
            keeps_to_disk: false,
            opening_frame: (WIDTH, HEIGHT),
            fit_to_screen: false,
            opened: false,
            scale: 1.0,
            previews_asked: std::collections::HashSet::new(),
            browsing: false,
            event_pending: false,
            frame_pending: false,
            loop_wake,
        }
    }
}

impl App {
    /// How large the frame of the window was when it was last closed, if that was kept
    /// and is large enough to be worth opening at.
    fn saved_size() -> Option<(f32, f32)> {
        pm_store::load_text(WINDOW_SIZE_FILE)
            .and_then(|text| {
                let (width, height) = text.trim().split_once(' ')?;
                Some((width.parse().ok()?, height.parse().ok()?))
            })
            .filter(|size: &(f32, f32)| size.0 >= MIN_SAVED && size.1 >= MIN_SAVED)
    }

    /// Keeps how large the frame of `window` is for the next launch, the window being
    /// `size` logical pixels across.
    fn remember_size(window: &dyn port::Port, size: (f32, f32)) {
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
            self.model.update(|state| {
                state.observe_conversation(visible, at_end, focused);
            });
        }
    }

    /// Asks the language server for the names of the lines of the file shown that are in view, when the file is long enough for those to be asked for first and they are not held, as the root's effect over the lines in view and the file's version has it do.
    pub(super) fn ask_names_in_view(&mut self) {
        if self.model.read(Screen::code_area).is_none() {
            return;
        }
        if self.model.read(|screen| screen.names_in_view_wanted()) {
            self.model.change(|state| state.annotations_requested());
            self.deliver();
        }
    }

    /// Hands `event` to the workspace's reducer, as a keymap command or a key of an open
    /// panel does, then does what it asks of the app.
    pub(super) fn handle(&mut self, event: Event) {
        self.update(Box::new(move |state| state.handle(event)));
    }

    /// Changes the workspace with `change`, then does what it asks of the app.
    pub(super) fn update(&mut self, change: Change) {
        self.model
            .update(|screen| *screen = std::mem::take(screen).changed(change));
        self.settle();
    }

    /// Does what every change of the workspace asks of the app once it is made: saves what
    /// is to be kept and sends what the agents and the folders are to be told.
    fn settle(&mut self) {
        self.persist();
        self.deliver();
    }

    /// Takes the row `row` of the list over the file, or its picked row: the command it
    /// holds when the list holds commands, and otherwise the row as the workspace takes it.
    pub(super) fn take_listing(&mut self, row: Option<usize>) {
        let id = row.map_or(Target::Editor(EditorEvent::ListingTake), |index| {
            Target::Editor(EditorEvent::ListingRow(index))
        });
        if !self.take_command(id) {
            self.handle(Event::pressing(id));
        }
    }

    /// Counts a click on the control `id` whose press made `clicks` presses in a row: a click
    /// whose press was the second or later in a row keeps the file its tab or row shows open.
    fn note_click(&mut self, id: Target, clicks: u32) {
        if clicks >= 2 {
            self.model.update(|state| {
                *state = std::mem::take(state).in_workspace(|state| state.with_file_kept(id))
            });
        }
    }

    /// Carries out what the reader asked of the agents and the folders, and copies what was asked to be copied.
    fn deliver(&mut self) {
        for text in self.work.deliver(&self.model) {
            if let Some(clipboard) = self.clipboard() {
                clipboard.set_text(&text);
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

    /// Whether the window looks for newer releases: what the root's effect that asks again
    /// every [`RELEASE_RECHECK`] is keyed on.
    fn release_checked(&self) -> bool {
        self.saved_window.general.check_updates
    }

    /// Asks for the newest release of the channel the reader follows, as the root's effect
    /// does every [`RELEASE_RECHECK`] while releases are looked for.
    fn release_rechecked(&mut self) {
        let general = &self.saved_window.general;
        if general.check_updates {
            self.work.ask_release(general.channel);
        }
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
            self.model.change(|state| state.without_figures());
        }
        let fonts = (settings.interface_font.clone(), settings.code_font.clone());
        if fonts != self.fonts_applied {
            crate::scene::set_fonts(&fonts.0, &fonts.1);
            self.fonts_applied = fonts;
        }
        let tokens = self.tokens();
        self.shell.set_tokens(tokens);
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

    /// Notes that the screen changed since the view was last written, so the root's effects
    /// write it once changes of the view pause, or at the latest [`VIEW_SAVE_BURST`] after
    /// the first change.
    fn defer_view_save(&mut self) {
        self.view_unsaved = true;
    }

    /// The fingerprint of the view as the screen shows it now, which the pause before the
    /// view is written waits on. The app says it each time it says what it waits on, before
    /// the windows are composed for what it handled, so a change of it is composed with the
    /// change of the screen that made it.
    fn view_fingerprint(&self) -> u64 {
        let work = &self.work;
        let focused = self.focus.focused();
        self.model.read(|screen| match screen {
            Screen::Workspace(state) => {
                View::capture(state, focused, |position| work.server_number(position)).fingerprint()
            }
        })
    }

    /// Writes the view once the root's effect says typing paused or the burst ran long, when
    /// it changed since it was last written.
    fn view_save_rested(&mut self) {
        if std::mem::take(&mut self.view_unsaved) {
            self.persist_view();
        }
    }

    /// Writes where the window is to disk when it differs from what was last written, once the window has finished opening where it was left.
    fn persist_view(&mut self) {
        if self.work.resuming() {
            return;
        }
        let work = &self.work;
        let focused = self.focus.focused();
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

    /// The field the reader was typing in when the window last closed, which the views give
    /// the keyboard back to as it appears (see [`pm_components::focus::restores_focus`]),
    /// once the view is no longer `resuming`; forgotten once rustle says the field holds it.
    fn restored_focus(
        refocus: &mut Option<Target>,
        focus: &FocusRequesters,
        resuming: bool,
    ) -> Option<Target> {
        if refocus.is_some_and(|field| focus.is_focused(field)) {
            *refocus = None;
        }
        refocus.filter(|_| !resuming)
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

    /// Notes what holds the keyboard and, when it is a text field, what it
    /// holds, before the interface handles an event, so [`App::follow_typing`]
    /// can tell whether the field changed while it did.
    pub(super) fn note_typing(&mut self) {
        self.typed_before = self.focus.focused().and_then(|id| {
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
            self.model.update(|state| {
                *state = std::mem::take(state)
                    .in_workspace(|state| state.handle(Event::Field(FieldEvent::Changed(id))))
            });
        }
    }

    /// Catches the screen up with what the reader scrolled in the interface, as the root's
    /// observer of the lists' scroll states asks once one moved, and asks for the diffs that
    /// a scroll of the diff brought into view.
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

    /// `at`, a point in the window, in the coordinates of the window's frame: past the band
    /// that holds the main window's shadow.
    pub(super) fn framed_point(&self, at: (f32, f32)) -> (f32, f32) {
        let corner = self.framed(rustle::Rect::new(
            rustle::Offset::new(at.0, at.1),
            rustle::Size::new(0.0, 0.0),
        ));
        (corner.origin.x, corner.origin.y)
    }

    /// Where the frame of the window events are handled for is laid out in that window:
    /// for the main window, where its frame told the screen it is, as `onGloballyPositioned`
    /// tells it, and before the frame was first laid out, the window inset by the band that
    /// holds its shadow; a window beside the main one is its own frame.
    fn frame_area(&self) -> rustle::Rect {
        let size = self
            .window
            .as_ref()
            .map_or((0.0, 0.0), |window| window.logical_size());
        let size = if size == (0.0, 0.0) {
            (WIDTH, HEIGHT)
        } else {
            size
        };
        let whole = rustle::Rect::new(
            rustle::Offset::new(0.0, 0.0),
            rustle::Size::new(size.0, size.1),
        );
        if self.on_aside.is_some() {
            return whole;
        }
        self.model
            .read(|screen| screen.places().get(Target::Window(WindowEvent::Frame)))
            .unwrap_or_else(|| {
                let band = pm_components::shadow_margin();
                rustle::Rect::new(
                    rustle::Offset::new(band, band),
                    rustle::Size::new(size.0 - 2.0 * band, size.1 - 2.0 * band),
                )
            })
    }

    /// `rect`, in the window, in the coordinates of the window's frame: from the corner
    /// [`frame_area`](Self::frame_area) says the frame is laid out at.
    pub(super) fn framed(&self, rect: rustle::Rect) -> rustle::Rect {
        let corner = self.frame_area().origin;
        rustle::Rect::new(
            rustle::Offset::new(rect.origin.x - corner.x, rect.origin.y - corner.y),
            rect.size,
        )
    }

    /// `rect`, in the coordinates of the window's frame, in the window: from the corner
    /// [`frame_area`](Self::frame_area) says the frame is laid out at.
    pub(super) fn unframed(&self, rect: rustle::Rect) -> rustle::Rect {
        let corner = self.frame_area().origin;
        rustle::Rect::new(
            rustle::Offset::new(rect.origin.x + corner.x, rect.origin.y + corner.y),
            rect.size,
        )
    }

    /// Where the area named `id` that tracks where it is drawn was drawn, in the
    /// coordinates of the window's frame, while it is shown.
    pub(super) fn place(&self, id: Target) -> Option<rustle::Rect> {
        let rect = self.model.read(|screen| screen.places().get(id))?;
        Some(self.framed(rect))
    }

    /// How wide and tall the frame of the window events are handled for is, in logical
    /// pixels, as [`frame_area`](Self::frame_area) says.
    pub(super) fn window_size(&self) -> (f32, f32) {
        let size = self.frame_area().size;
        (size.width, size.height)
    }

    /// Whether `id` is a handle the composables drag themselves: a panel's handle or the
    /// minimap, whose press is not a press of anything else.
    fn is_drag_handle(&self, id: Target) -> bool {
        let window = self.window_size();
        id == Target::Editor(EditorEvent::Minimap)
            || self.model.read(|screen| screen.sash(id, window)).is_some()
    }

    /// Makes the panel whose handle `id` is dragged as big as it was when the drag began
    /// plus how far the drag went, `by` logical pixels across and down: in the observable
    /// state that holds its size, without writing the screen, and through the workspace only
    /// for a split of panes laid out otherwise than the ones held.
    pub(super) fn sash_dragged(&mut self, id: Target, by: (f32, f32)) {
        let window = self.window_size();
        if self.sash_drag.is_none_or(|(held, ..)| held != id) {
            self.sash_drag = self
                .model
                .read(|screen| screen.sash(id, window))
                .map(|(size, grows)| (id, size, grows));
        }
        if let Some((id, size, grows)) = self.sash_drag {
            let size = size + grows.by((0.0, 0.0), by);
            if !self
                .model
                .read(|screen| screen.hold_sash_size(id, size, window))
            {
                self.model
                    .change(|state| state.with_sash_size(id, size, window));
            }
        }
    }

    /// Scrolls the file shown so its view is centred on the point of the minimap `down`
    /// logical pixels from its top, of its `height`.
    pub(super) fn minimap_dragged(&mut self, down: f32, height: f32) {
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.minimap_pressed(down, height))
        });
    }

    /// Goes to the line kept at the top of the file shown that the press is on, `on`, and says whether it is on one.
    fn press_sticky(&mut self, on: Option<Target>) -> bool {
        let Some(Target::Editor(EditorEvent::Sticky(above))) = on else {
            return false;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.sticky_pressed(above))
        });
        true
    }

    /// Presses the lens at the end of a line of the file shown that the press at `at`, in the
    /// window's frame, is on, `on`, and says whether it is on one.
    fn press_lens(&mut self, on: Option<Target>, at: (f32, f32)) -> bool {
        let Some(Target::Editor(EditorEvent::Lens(index))) = on else {
            return false;
        };
        self.model.update(|state| {
            *state = std::mem::take(state)
                .in_workspace(|state| state.lens_pressed(index, (at.0, at.1 + 12.0)))
        });
        true
    }

    /// Whether a press on the control `id` is taken by the control's own gesture, so its
    /// click does nothing: a handle, the minimap, a line kept at the top, a lens, a file of
    /// the excerpts that is not the one edited, and the bare title bar of a window.
    fn takes_its_press(&self, id: Target) -> bool {
        self.is_drag_handle(id)
            || matches!(
                id,
                Target::Editor(EditorEvent::Sticky(_) | EditorEvent::Lens(_))
                    | Target::Search(pm_components::areas::SearchEvent::ExcerptOwner(_))
            )
            || (id == Target::Window(WindowEvent::TitleBar) && self.window.is_some())
    }

    /// Follows a press of either button: the field the reader was typing in when the window
    /// last closed no longer takes the keyboard by itself.
    fn note_press(&mut self) {
        self.refocus = None;
    }

    /// Takes a press of the primary button on a control, as its view tells it while the
    /// interface handles the press: a press on the bare title bar moves the window, or
    /// maximizes it when it is the second in a row; anything else is pressed.
    pub(super) fn pressed(&mut self, press: Press) {
        self.anchor = Some(press);
        self.note_press();
        if press.id == Target::Window(WindowEvent::TitleBar) && self.title_bar_pressed(press.clicks)
        {
            return;
        }
        let at = self.framed_point(press.at);
        self.press_at(Some(press.id), at);
    }

    /// Takes a press of the primary button at `at` in the window that no control took.
    pub(super) fn pressed_elsewhere(&mut self, at: (f32, f32)) {
        self.note_press();
        let at = self.framed_point(at);
        self.press_at(None, at);
    }

    /// Takes a press of the secondary button on a control with a menu of its own, before the
    /// menu opens.
    pub(super) fn context_pressed(&mut self, press: Press) {
        self.anchor = Some(press);
        self.note_press();
    }

    /// Notes that the primary button went down inside the conversation, as the conversation
    /// tells before the press reaches what it landed on.
    pub(super) fn conversation_pressed(&mut self) {
        self.pressed_in_conversation = true;
    }

    /// Takes a press of the mouse's back button, or of its forward button when not `back`,
    /// where nothing else took it: the page of the browser that holds the keyboard steps
    /// through its history, or else the app through the places visited.
    pub(super) fn history_pressed(&mut self, back: bool) {
        self.note_press();
        if !(self.on_aside.is_none() && self.browser_history(back, false)) {
            self.step_through_history(back);
        }
    }

    /// Takes a press of the primary button on `on`, or on nothing, at `at` in the window's
    /// frame: a handle, minimap, line kept at the top or lens takes hold of it; anything
    /// else is pressed as [`press_on`](Self::press_on) presses it.
    fn press_at(&mut self, on: Option<Target>, at: (f32, f32)) {
        if on.is_some_and(|id| self.is_drag_handle(id))
            || self.press_sticky(on)
            || self.press_lens(on, at)
        {
            return;
        }
        self.press_on(on);
    }

    /// Presses `on`, or nothing: the press closes what a press elsewhere closes and holds a
    /// row of a list for the keys that pick every row. Keyboard focus is rustle's to move:
    /// a press on a text field or an area that takes the keyboard gives it focus, and a press
    /// anywhere else takes focus away.
    fn press_on(&mut self, on: Option<Target>) {
        self.list_pressed = on.filter(|id| {
            self.model.read(|screen| {
                screen.tree_row_of(*id).is_some()
                    || screen.change_row_of(*id).is_some()
                    || screen.change_folder_row_of(*id).is_some()
            })
        });
        self.text_press(on);
        if on != Some(Target::Tree(TreeEvent::EntryField))
            && self.model.read(|screen| screen.is_naming())
        {
            self.model.update(|state| {
                *state = std::mem::take(state).in_workspace(|state| state.without_naming())
            });
        }
        if on.is_none() && self.model.read(Screen::restore_offered) {
            self.model
                .update(|state| *state = std::mem::take(state).without_picker());
        }
    }

    /// Says whether `click`, a click of a control as its view tells it while the interface
    /// handles the release, counts: when the press was not taken by the control's own
    /// gesture and did not start a drag.
    pub(super) fn click_lands(&mut self, click: Press) -> bool {
        self.anchor = Some(click);
        let dragging = self.model.read(|screen| match screen {
            Screen::Workspace(state) => {
                state.project_drag.is_some() || state.pane_drag.is_some() || state.tree_dragging()
            }
        });
        if dragging || self.takes_its_press(click.id) {
            return false;
        }
        self.note_click(click.id, click.clicks);
        true
    }

    /// Closes the menu after its row `row` was picked, giving the field it is open over the
    /// keyboard back, and carries the row out on a code field. A text field's menu carries its
    /// rows out itself, through the field's own operations.
    pub(super) fn menu_action(&mut self, row: Target) {
        let Some(open) = self.model.read(|screen| screen.menu()) else {
            return;
        };
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.without_menu())
        });
        let field = open.field;
        self.focus.request_focus(field);
        if self.model.read(|screen| screen.text_field(field).is_some()) {
            return;
        }
        match row {
            Target::Menu(MenuEvent::MenuUndo) => self.edit_field(field, Edit::Undo),
            Target::Menu(MenuEvent::MenuRedo) => self.edit_field(field, Edit::Redo),
            Target::Menu(MenuEvent::MenuCut) => self.clip(field, Clip::Cut),
            Target::Menu(MenuEvent::MenuCopy) => self.clip(field, Clip::Copy),
            Target::Menu(MenuEvent::MenuPaste) => self.clip(field, Clip::Paste),
            Target::Menu(MenuEvent::MenuSelectAll) => self.edit_field(field, Edit::SelectAll),
            row => {
                if let Some(command) = EditorCommand::of(row) {
                    self.editor_command(command);
                }
            }
        }
    }

    /// Opens the menu of the `index`-th server of the settings page, hanging from its button.
    pub(super) fn open_settings_menu(&mut self, index: usize) {
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.with_settings_menu(index))
        });
    }

    /// Opens the menu on the tab of the open file at `index` where the pointer was, at `at`
    /// in the window's frame.
    pub(super) fn open_tab_menu(&mut self, index: usize, at: (f32, f32)) {
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.with_tab_menu(index, at))
        });
    }

    /// Opens the menu of the project at `project` where the pointer was, at `at` in the
    /// window's frame.
    pub(super) fn open_project_menu(&mut self, project: usize, at: Option<(f32, f32)>) {
        self.model.update(|state| {
            *state = state
                .clone()
                .in_workspace(|state| state.with_project_menu(project, at))
        });
    }

    /// Opens the menu on changed files where the pointer was, at `at` in the window's frame,
    /// over the file at `index`.
    pub(super) fn open_change_menu(&mut self, index: usize, at: (f32, f32)) {
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.with_change_menu(index, at));
        });
    }

    /// Opens the menu on changed files where the pointer was, at `at` in the window's frame,
    /// over the folder at `position` of their tree.
    pub(super) fn open_folder_change_menu(&mut self, position: usize, at: (f32, f32)) {
        self.model.update(|state| {
            *state = std::mem::take(state)
                .in_workspace(|state| state.with_folder_change_menu(position, at));
        });
    }

    /// Opens the menu on the project's files where the pointer was, at `at` in the window's
    /// frame, over `target`.
    pub(super) fn open_tree_menu(&mut self, target: TreeTarget, at: (f32, f32)) {
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.with_tree_menu(target, at));
        });
    }

    /// Opens the menu `kind` of the source control panel where the pointer was, at `at` in
    /// the window's frame, or, without a point, hanging from the control that opened it.
    pub(super) fn open_git_menu(&mut self, kind: MenuKind, at: Option<(f32, f32)>) {
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| match at {
                Some(at) => state.with_git_menu(kind, at),
                None => state.with_git_menu_under(kind),
            })
        });
    }

    /// Opens the menu of the session at `session` where the pointer was, at `at` in the
    /// window's frame.
    pub(super) fn open_session_menu(&mut self, session: usize, at: (f32, f32)) {
        self.model.update(|state| {
            *state = state
                .clone()
                .in_workspace(|state| state.with_session_menu(session, at))
        });
    }

    /// Follows the pointer to `at` while a project header is carried, over the project
    /// `over` when it is over one: that project is the one it would take the place of.
    fn drag_project(&mut self, at: (f32, f32), over: Option<usize>) {
        let Some(drag) = self.model.read(|screen| match screen {
            Screen::Workspace(state) => state.project_drag,
        }) else {
            return;
        };
        let over = over
            .filter(|project| *project != drag.from)
            .unwrap_or(drag.over);
        self.model.update(|state| {
            *state =
                std::mem::take(state).in_workspace(|state| state.dragged_over(drag.from, over, at))
        });
    }

    /// Follows a press and drag on the number of the line `id` of a diff: the press starts
    /// selecting lines for a comment there, which the numbers the pointer comes over then
    /// stretch.
    pub(super) fn comment_dragged(&mut self, id: Target) {
        if self.model.read(|screen| screen.comment_dragging()) {
            return;
        }
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.comment_drag_begun(id))
        });
    }

    /// Stretches the lines selected for a comment to the number of the line `id` of a diff,
    /// which the pointer came over, while they are being selected.
    pub(super) fn comment_hovered(&mut self, id: Target) {
        if !self.model.read(|screen| screen.comment_dragging()) {
            return;
        }
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.comment_drag_extended(id))
        });
    }

    /// Ends a selection of lines, which opens the box for a comment on them and gives it the keyboard.
    pub(super) fn end_comment_drag(&mut self) {
        if !self.model.read(|screen| screen.comment_dragging()) {
            return;
        }
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.comment_drag_ended())
        });
    }

    /// Ends a drag of a project header, putting the project where it was let go, and says whether there was a drag.
    pub(super) fn drop_project(&mut self) -> bool {
        if !self.model.read(|screen| match screen {
            Screen::Workspace(state) => state.project_drag.is_some(),
        }) {
            return false;
        }
        self.model
            .update(|state| *state = std::mem::take(state).in_workspace(|state| state.dropped()));
        self.persist();
        self.deliver();
        true
    }

    /// Opens the menu over the text field `id` at `at`, in the window's frame, the field
    /// having said whether it is `secure` and `multiline`.
    pub(super) fn open_text_menu(&mut self, id: Target, at: (f32, f32)) {
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.with_menu(id, at))
        });
    }

    /// Opens the menu over the code field `id` at `at`, in the window's frame, after putting
    /// the caret at the byte `offset` of the code text under the pointer unless it is inside
    /// the selection.
    pub(super) fn open_menu(&mut self, id: Target, (x, y): (f32, f32), offset: Option<usize>) {
        let Some(offset) = offset else {
            return;
        };
        self.focus.request_focus(id);
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
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| state.with_menu(id, (x, y)))
        });
    }

    /// Carries out what the primary button did on the code text of a field, as `pointer`
    /// tells: a press places the caret or selects, or makes a file of the excerpts the one
    /// edited; a move with the button held stretches the selection; the release ends it, and
    /// puts the caret where a press on the selection that never became a drag of it was.
    pub(super) fn code_pointer(&mut self, pointer: CodePointer) {
        match pointer.stroke {
            Stroke::Down { clicks, modifiers } => self.code_pressed(pointer, clicks, modifiers),
            Stroke::Move => {
                let Some(offset) = pointer.offset else {
                    return;
                };
                if self.dragging == Some(pointer.owner) {
                    self.drag_in_field(pointer.owner, offset);
                }
            }
            Stroke::Up => {
                self.text_released();
                self.dragging = None;
                self.column_press = None;
                self.code_hold = None;
            }
        }
    }

    /// Takes the press `pointer` on code text, the `clicks`-th in a row with `modifiers`
    /// held: on a file of the excerpts that is not the one edited, that file becomes the
    /// one edited and its caret goes where the press is; in the code editor the caret goes
    /// there and a selection starts that dragging extends, a second press soon after the
    /// first selecting the word under it and a third the line, which dragging then extends
    /// by words or lines. Alt adds a caret, Alt and Shift start a rectangle, Shift runs the
    /// selection out to the press, a press on the selection starts carrying it, and Control
    /// takes the reader to where the word under it is defined.
    fn code_pressed(&mut self, pointer: CodePointer, clicks: u32, modifiers: Modifiers) {
        let id = pointer.owner;
        self.code_hold = None;
        if let Some(index) = self.model.read(|screen| screen.excerpt_owner_of(id)) {
            self.press_excerpt(index, pointer.offset);
            return;
        }
        if id != Target::Editor(EditorEvent::EditorField) {
            return;
        }
        let Some(offset) = pointer.offset else {
            return;
        };
        self.focus.request_focus(id);
        self.press_in_field(id, offset, clicks, modifiers);
        if self.editor_reach_pressed(modifiers) {
            self.dragging = None;
        }
    }

    /// Takes a press on the lines of a file in the excerpts that is not the one being edited,
    /// at the byte `offset` of its text: the file becomes the one edited and its caret goes
    /// there.
    fn press_excerpt(&mut self, index: usize, offset: Option<usize>) {
        self.model.change(|state| state.excerpt_file_shown(index));
        self.focus
            .request_focus(Target::Editor(EditorEvent::EditorField));
        if let Some(offset) = offset {
            self.press_field_at(Target::Editor(EditorEvent::EditorField), offset, 1, false);
        }
    }

    /// Takes the press inside the code editor's field `id`, at the shown offset `offset`,
    /// the `clicks`-th in a row with `modifiers` held.
    fn press_in_field(&mut self, id: Target, offset: usize, clicks: u32, modifiers: Modifiers) {
        if modifiers.alt && modifiers.shift {
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
        if modifiers.alt {
            self.edit_field(id, Edit::AddSelection(offset, offset));
            return;
        }
        if clicks <= 1
            && !modifiers.shift
            && !modifiers.ctrl
            && !modifiers.meta
            && let Some(byte) = self
                .model
                .read(|screen| screen.field(id).map(|field| field.to_text(offset)))
            && self.text_pressed(byte)
        {
            return;
        }
        self.press_field_at(id, offset, clicks, modifiers.shift);
    }

    /// Starts the press in the field `id` at the shown offset `offset`, the `clicks`-th in a
    /// row, or running the selection there was out to it when `extending`.
    fn press_field_at(&mut self, id: Target, offset: usize, clicks: u32, extending: bool) {
        let Some((byte, anchor)) = self.model.read(|screen| {
            screen
                .field(id)
                .map(|field| (field.to_text(offset), field.anchor()))
        }) else {
            return;
        };
        let (hold, selected) = match extending {
            true => Hold::extending(anchor, byte),
            false => Hold::press(byte, clicks, |reach, at| self.code_reach(id, reach, at)),
        };
        self.select_code(id, selected);
        self.code_hold = Some(hold);
        self.dragging = Some(id);
    }

    /// The word or line `reach` takes around the byte `at` of the field `id`.
    fn code_reach(&self, id: Target, reach: Reach, at: usize) -> (usize, usize) {
        self.model
            .read(|screen| screen.field(id).map(|field| field.reach_around(reach, at)))
            .unwrap_or((at, at))
    }

    /// Puts the caret of the field `id` where `selected` says, or selects what it says.
    fn select_code(&mut self, id: Target, selected: Selected<usize>) {
        let (anchor, head) = match selected {
            Selected::Point(at) => (at, at),
            Selected::Span(anchor, head) => (anchor, head),
        };
        self.edit_field(id, Edit::Select(anchor, head));
    }

    /// Extends the press held in the field `id` to the shown offset `offset`.
    fn drag_in_field(&mut self, id: Target, offset: usize) {
        let Some(byte) = self
            .model
            .read(|screen| screen.field(id).map(|field| field.to_text(offset)))
        else {
            return;
        };
        if let Some(from) = self.column_press {
            self.model
                .change(|state| state.lines_edited(LineEdit::ColumnDrag(from, byte)));
            self.show_editor_caret();
            return;
        }
        let Some(hold) = self.code_hold else {
            return;
        };
        let selected = hold.drag(byte, |reach, at| self.code_reach(id, reach, at));
        self.select_code(id, selected);
    }

    /// Moves the caret of the multiline field `id` by `by` lines as they are
    /// drawn, to the start or the end of the text when there is no line there.
    fn move_vertically(&mut self, id: Target, by: isize, extend: bool) {
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
                pm_editor::Motion::LineUp
            } else {
                pm_editor::Motion::LineDown
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
    fn move_to_line_edge(&mut self, id: Target, end: bool, extend: bool) {
        let Some((caret, several)) = self.model.read(|screen| {
            screen
                .field(id)
                .map(|field| (field.to_shown(field.caret()), field.has_others()))
        }) else {
            return;
        };
        if several {
            let motion = if end {
                pm_editor::Motion::LineEnd
            } else {
                pm_editor::Motion::LineStart
            };
            self.edit_field(id, Edit::Move(motion, extend));
            return;
        }
        if let Some(target) = self.code_lines().line_edge(id, caret, end) {
            self.edit_field(id, Edit::Place(target, extend));
        }
    }

    /// Shows `text` an input method is composing in the field, shell or web page holding
    /// the keyboard, taking it away from the one that showed it before.
    fn compose(&mut self, text: String) {
        if self.compose_in_browser(&text) {
            return;
        }
        if text.is_empty() && self.composing_in.is_none() {
            return;
        }
        let target = self.focus.focused().filter(|id| {
            *id == Target::Terminal(TerminalEvent::TerminalBody)
                || *id == Target::Editor(EditorEvent::EditorField)
        });
        if let Some(held) = self.composing_in.filter(|held| Some(*held) != target) {
            self.show_composing(held, String::new());
        }
        self.composing_in = target.filter(|_| !text.is_empty());
        if let Some(id) = target {
            self.show_composing(id, text);
        }
    }

    /// Hands `text` an input method committed to the shell or the web page holding the
    /// keyboard, or types it into the code editor when it holds it.
    fn commit(&mut self, text: &str) {
        if self.commit_to_terminal(text) || self.commit_to_browser(text) {
            return;
        }
        if self.focus.focused() == Some(Target::Editor(EditorEvent::EditorField)) {
            self.edit_field(
                Target::Editor(EditorEvent::EditorField),
                Edit::Paste(text.to_owned()),
            );
        }
    }

    /// Shows `text` as being composed in the field or shell `id`.
    fn show_composing(&mut self, id: Target, text: String) {
        if id == Target::Terminal(TerminalEvent::TerminalBody) {
            self.model
                .update(|screen| screen.compose_in_terminal(&text));
        } else {
            self.edit_field(id, Edit::Compose(text));
        }
    }

    /// Makes `edit` to the code editor's field, which `id` names.
    fn edit_field(&mut self, id: Target, edit: Edit) {
        let looks = |app: &Self| {
            app.model.read(|screen| {
                screen
                    .field(id)
                    .map(|field| (field.caret(), field.selection(), field.text().len()))
            })
        };
        let before = looks(self);
        self.model.update(|state| {
            *state = std::mem::take(state).in_workspace(|state| {
                state.handle(Event::Field(FieldEvent::Edited { field: id, edit }))
            })
        });
        if id == Target::Editor(EditorEvent::EditorField) && looks(self) != before {
            self.show_editor_caret();
        }
    }

    /// Tells the screen which text field the reader left, when keyboard focus moved off one since the event loop last came to rest, so a field that saves on leaving does.
    fn tell_field_left(&mut self) {
        let focused = self.focus.focused();
        if focused == self.focus_before {
            return;
        }
        if let Some(left) = self
            .focus_before
            .filter(|id| WorkspaceState::saves_on_leaving(*id))
        {
            self.model.update(|state| {
                *state = std::mem::take(state)
                    .in_workspace(|state| state.handle(Event::Field(FieldEvent::Left(left))))
            });
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
            self.model.update(|state| {
                *state = std::mem::take(state).in_workspace(|state| state.with_caret_shown(height))
            });
        }
    }

    /// The clipboard rustle installed for the windows, or the one a test installed in its place.
    fn clipboard(&self) -> Option<Rc<dyn rustle::Clipboard>> {
        rustle::clipboard()
    }

    /// Carries out a clipboard shortcut in the code editor's field `id`.
    fn clip(&mut self, id: Target, action: Clip) {
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
                if id == Target::Editor(EditorEvent::EditorField)
                    && let Some((lines, indent)) = self.model.read(|screen| screen.editor_copied())
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
                if id == Target::Editor(EditorEvent::EditorField) {
                    self.copied = Some((selected.clone(), indentation, whole_lines));
                }
                if let Some(clipboard) = self.clipboard() {
                    clipboard.set_text(&selected);
                }
                if action == Clip::Cut {
                    if whole_lines {
                        self.model
                            .change(|state| state.lines_edited(LineEdit::Delete));
                        self.show_editor_caret();
                    } else {
                        self.edit_field(id, Edit::Backspace);
                    }
                }
            }
            Clip::Paste => {
                let pasted = self.clipboard().and_then(|clipboard| clipboard.text());
                if let Some(text) = pasted {
                    let adjusted = (id == Target::Editor(EditorEvent::EditorField))
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

    /// Whether a field the reader types in holds keyboard focus: a text field or the code
    /// editor's field.
    pub(super) fn typing(&self) -> bool {
        self.focus.focused().is_some_and(|id| {
            id == Target::Editor(EditorEvent::EditorField)
                || self.model.read(|screen| screen.text_field(id).is_some())
        })
    }

    /// Whether the prompt box holds keyboard focus.
    fn in_prompt(&self) -> bool {
        self.focus.focused() == Some(Target::Composer(ComposerEvent::Prompt))
    }

    /// Steps back or forward through what the window in front shows: the folders looked inside while the folder picker is in front, and the pages and views visited otherwise.
    fn step_through_history(&mut self, back: bool) {
        match self.on_aside {
            Some(asides::Aside::Picker) => self.handle(Event::pressing(if back {
                Target::Dialog(DialogEvent::PickerBack)
            } else {
                Target::Dialog(DialogEvent::PickerForward)
            })),
            _ => self.navigate(if back { -1 } else { 1 }),
        }
    }

    /// Moves `delta` pages or views along the ones visited, towards the past when negative.
    pub(super) fn navigate(&mut self, delta: isize) {
        self.model.change(|state| state.navigated(delta));
    }

    /// Carries out a key aimed at the list a row was last pressed in, and says whether it was one: the key that selects everything picks every row, and the project's files take the keys that cut, copy, paste, rename and delete.
    fn list_key(&mut self, event: &KeyEvent) -> bool {
        let Some(id) = self.list_pressed else {
            return false;
        };
        if self.typing() {
            return false;
        }
        if self.model.read(|screen| screen.tree_row_of(id).is_some()) && self.tree_key(event) {
            return true;
        }
        if !matches!(keys::edit(event), Some(Edit::SelectAll)) {
            return false;
        }
        self.model.change(|state| {
            if state.tree_row_of(id).is_some() {
                state.tree_all_picked()
            } else {
                state.changes_all_picked()
            }
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
            Key::Escape if self.on_aside == Some(asides::Aside::Picker) => {
                self.handle(Event::Dialog(DialogEvent::PickerClose))
            }
            Key::ArrowUp if self.on_aside == Some(asides::Aside::Picker) => {
                self.handle(Event::Dialog(DialogEvent::PickerUp))
            }
            Key::ArrowDown if self.on_aside == Some(asides::Aside::Picker) => {
                self.handle(Event::Dialog(DialogEvent::PickerDown))
            }
            Key::Enter if self.on_aside == Some(asides::Aside::Picker) => {
                if let Some(id) = self.model.read(|screen| screen.folder_picker_enter()) {
                    self.handle(Event::pressing(id));
                }
            }
            Key::Escape if self.model.read(|screen| screen.settings_open()) => {
                self.handle(Event::Settings(SettingsEvent::SettingsClose))
            }
            Key::Escape if self.model.read(|screen| screen.adder_open()) => {
                self.handle(Event::Dialog(DialogEvent::AdderEscape))
            }
            Key::ArrowUp if self.model.read(|screen| screen.adder_open()) => {
                self.handle(Event::Dialog(DialogEvent::AdderUp))
            }
            Key::ArrowDown if self.model.read(|screen| screen.adder_open()) => {
                self.handle(Event::Dialog(DialogEvent::AdderDown))
            }
            Key::Enter if self.model.read(|screen| screen.adder_open()) => {
                if let Some(id) = self.model.read(|screen| screen.adder_enter()) {
                    self.handle(Event::pressing(id));
                }
            }
            Key::Character(_)
                if let Some(digit) =
                    keys::character(event).and_then(|typed| typed.to_digit(10))
                    && !self.typing()
                    && let Some(id) = self
                        .model
                        .read(|screen| screen.inquiry_hotkey(digit as usize)) =>
            {
                self.handle(Event::pressing(id));
                if id == Target::Chat(pm_components::areas::ChatEvent::TypeOwn) {
                    self.focus
                        .request_focus(Target::Chat(pm_components::areas::ChatEvent::Other));
                }
            }
            Key::Tab if event.modifiers.shift && self.in_prompt() => {
                self.model.update(|state| {
                    *state = std::mem::take(state).in_workspace(|state| state.mode_cycled())
                });
                self.persist();
                self.deliver();
            }
            Key::Tab if self.in_prompt() && self.model.read(|screen| screen.slash_open()) => {
                self.model.update(|state| {
                    *state = std::mem::take(state).in_workspace(|state| state.slash_taken())
                });
            }
            Key::ArrowUp | Key::ArrowDown
                if self.in_prompt() && self.model.read(|screen| screen.slash_open()) =>
            {
                let delta = if event.key == Key::ArrowUp { -1 } else { 1 };
                self.model.update(|state| {
                    *state = std::mem::take(state).in_workspace(|state| state.slash_moved(delta))
                });
            }
            Key::Enter if self.in_prompt() && self.model.read(|screen| screen.slash_open()) => {
                self.model.update(|state| {
                    *state = std::mem::take(state).in_workspace(|state| state.slash_taken())
                });
            }
            Key::Escape if self.model.read(|screen| screen.review_overlay_open()) => {
                self.handle(Event::Git(GitEvent::ReviewDismiss))
            }
            Key::Escape if self.model.read(|screen| screen.commenting()) => {
                self.handle(Event::Diff(DiffEvent::CommentCancel))
            }
            Key::Escape if self.text_dragging() => self.text_drag_cancelled(),
            Key::Escape if self.model.read(Screen::restore_offered) => {
                self.model
                    .update(|state| *state = std::mem::take(state).without_picker());
            }
            Key::Escape if self.model.read(|screen| screen.is_naming()) => {
                self.model.update(|state| {
                    *state = std::mem::take(state).in_workspace(|state| state.without_naming())
                });
                self.focus.free_focus();
            }
            Key::Escape => self.focus.free_focus(),
            _ => return self.focused_key(event),
        }
        true
    }

    /// Carries out a key aimed at the code editor's field while it holds the keyboard, and
    /// says whether it took it: Enter breaks the line of a field of several lines unless
    /// Control or Command is held, and saves otherwise; the arrows, Home and End follow
    /// the lines as drawn, and the clipboard and editing keys edit the field. A rustle text
    /// field takes its own keys, and Enter or Space on a control goes on to the control.
    fn focused_key(&mut self, event: &KeyEvent) -> bool {
        let id = Target::Editor(EditorEvent::EditorField);
        if self.focus.focused() != Some(id) {
            return false;
        }
        let multiline = self
            .model
            .read(|screen| screen.field(id).is_some_and(pm_editor::Field::is_multiline));
        let shift = event.modifiers.shift;
        let hurried = event.modifiers.ctrl || event.modifiers.meta;
        if event.key == Key::Enter && (shift || !hurried) && multiline {
            self.edit_field(id, Edit::Insert('\n'));
        } else if event.key == Key::Enter {
            self.handle(Event::pressing(Target::Editor(EditorEvent::EditorSave)));
        } else if multiline && matches!(event.key, Key::ArrowUp | Key::ArrowDown) {
            self.move_vertically(id, if event.key == Key::ArrowUp { -1 } else { 1 }, shift);
        } else if multiline && matches!(event.key, Key::Home | Key::End) {
            self.move_to_line_edge(id, event.key == Key::End, shift);
        } else if self.move_along_screen(event) {
        } else if let Some(action) = keys::clip(event) {
            self.clip(id, action);
        } else if let Some(edit) = keys::edit(event) {
            self.edit_field(id, edit);
        }
        true
    }
}

/// The byte offset in the text of `field` that the offset `shown` of what is shown stands for.
fn field_text_offset(field: &pm_editor::Field, shown: usize) -> usize {
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

/// The appearance the desktop asks for: dark when it is `dark`, light otherwise.
fn appearance(dark: bool) -> Appearance {
    if dark {
        Appearance::Dark
    } else {
        Appearance::Light
    }
}
