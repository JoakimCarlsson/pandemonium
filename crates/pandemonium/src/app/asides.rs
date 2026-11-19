//! The windows that open beside the main one, the settings window and the
//! folder picker's: each is opened while what it shows is open and closed
//! with it, and is drawn from the same state as the main window.

use pm_components::areas::DialogEvent;
use pm_components::areas::SidebarEvent;

use pm_components::events::Event;
use pm_components::target::Target;
use std::rc::Rc;

use pm_components::folder_picker::is_picker;

use super::pointer::PointerState;
use pm_components::settings::is_settings;
use pm_pages::Screen;
use rustle::WindowOptions;

use super::App;
use super::port::Port;

/// Which of the windows beside the main one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Aside {
    /// The settings window.
    Settings,
    /// The folder picker's window.
    Picker,
}

impl Aside {
    /// Every window that can open beside the main one.
    pub(super) const ALL: [Self; 2] = [Self::Settings, Self::Picker];

    /// Whether what the window shows is open on `screen`.
    pub(super) fn is_open(self, screen: &Screen) -> bool {
        match self {
            Self::Settings => screen.settings_open(),
            Self::Picker => screen.folder_picker_open(),
        }
    }

    /// `screen` with what the window shows closed, which is what closing the window does.
    pub(super) fn closed(self, screen: Screen) -> Screen {
        match self {
            Self::Settings => screen.without_settings(),
            Self::Picker => screen.without_folder_picker(),
        }
    }

    /// What the window is called.
    fn title(self) -> &'static str {
        match self {
            Self::Settings => "Settings",
            Self::Picker => "Choose a folder",
        }
    }

    /// How large the window opens, in logical pixels.
    pub(super) fn opening_size(self) -> (f32, f32) {
        match self {
            Self::Settings => (960.0, 720.0),
            Self::Picker => (880.0, 560.0),
        }
    }

    /// The least the window can be made, in logical pixels.
    fn least_size(self) -> (f32, f32) {
        match self {
            Self::Settings => (640.0, 420.0),
            Self::Picker => (560.0, 360.0),
        }
    }

    /// The options the window opens with: its title, size and least size, framed as the main one.
    pub(super) fn options(self) -> WindowOptions {
        let framed = pm_components::title_bar::FRAMELESS;
        WindowOptions {
            title: self.title().to_owned(),
            size: super::frame::with_band(self.opening_size()),
            min_size: super::frame::with_band(self.least_size()),
            decorations: !framed,
            transparent: framed,
            resizable: true,
            ime_allowed: true,
            app_id: Some(pm_store::app_name().to_owned()),
            macos_titlebar_transparent: true,
            macos_fullsize_content: true,
            macos_title_hidden: true,
            always_on_top: self.kept_in_front(),
            ..WindowOptions::default()
        }
    }

    /// Whether `id` names something of the window.
    pub(super) fn owns(self, id: Target) -> bool {
        match self {
            Self::Settings => is_settings(id),
            Self::Picker => is_picker(id),
        }
    }

    /// Whether the window stays in front of the others: the folder picker stays in front of the windows of the app.
    fn kept_in_front(self) -> bool {
        matches!(self, Self::Picker)
    }
}

/// Whether `id` names something of any window beside the main one.
pub(super) fn is_aside(id: Target) -> bool {
    Aside::ALL.iter().any(|aside| aside.owns(id))
}

/// A window and what it alone keeps: its native window once open, the
/// controls the pointer is over and the pointer as its events left it.
pub(super) struct Surface {
    /// The window, once rustle opened it.
    window: Option<Rc<dyn Port>>,
    /// The controls the pointer is over in the window, the one it came over last at the end.
    hovered: Vec<Target>,
    /// The pointer as the window's events so far left it.
    pointer: PointerState,
}

impl App {
    /// The place that keeps the surface of `aside`.
    fn surface_of(&mut self, aside: Aside) -> &mut Option<Surface> {
        match aside {
            Aside::Settings => &mut self.settings_surface,
            Aside::Picker => &mut self.picker_surface,
        }
    }

    /// The window of `aside`, while it is open.
    fn window_of(&self, aside: Aside) -> Option<Rc<dyn Port>> {
        self.shell.port_of(aside)
    }

    /// Keeps a surface for each window whose state was opened, once rustle
    /// opened its window, and drops the surface of each whose state was closed.
    pub(super) fn sync_asides(&mut self) {
        for aside in Aside::ALL {
            let open = self.model.read(|screen| aside.is_open(screen));
            let showing = self.surface_of(aside).is_some();
            match (open, showing, self.shell.port_of(aside)) {
                (true, false, Some(window)) => {
                    *self.surface_of(aside) = Some(Surface {
                        window: Some(window),
                        hovered: Vec::new(),
                        pointer: PointerState::default(),
                    });
                }
                (false, true, _) => {
                    *self.surface_of(aside) = None;
                }
                _ => {}
            }
        }
        self.raise_picker();
    }

    /// Brings the folder picker's window to the front and focuses its filter when the picker was opened again while it showed, or was just opened.
    fn raise_picker(&mut self) {
        let serial = self.model.read(Screen::folder_picker_serial);
        if serial == self.raised_picker {
            return;
        }
        if serial.is_some() && self.window_of(Aside::Picker).is_none() {
            return;
        }
        self.raised_picker = serial;
        if serial.is_none() {
            return;
        }
        self.focus.focus(Target::Dialog(DialogEvent::PickerField));
        self.recompose = true;
        if let Some(window) = self.window_of(Aside::Picker) {
            window.set_minimized(false);
            window.focus_window();
            window.request_attention();
        }
    }

    /// Opens settings in their window, or brings the window forward when they are open.
    pub(super) fn open_settings(&mut self) {
        match self.window_of(Aside::Settings) {
            Some(window) => window.focus_window(),
            None if self.on_aside.is_none() => self.handle(Event::Sidebar(SidebarEvent::Settings)),
            None => {}
        }
    }

    /// Runs `act` with the window of `aside` standing in for the main one: its
    /// window, frame, pointer and edge are swapped in for the time `act` runs.
    pub(super) fn in_aside(&mut self, aside: Aside, act: impl FnOnce(&mut Self)) {
        let Some(mut surface) = self.surface_of(aside).take() else {
            return;
        };
        self.swap_surface(&mut surface);
        self.on_aside = Some(aside);
        act(self);
        self.on_aside = None;
        self.swap_surface(&mut surface);
        if self.model.read(|screen| aside.is_open(screen)) {
            *self.surface_of(aside) = Some(surface);
        }
    }

    /// Trades the main window's surface with `surface`.
    fn swap_surface(&mut self, surface: &mut Surface) {
        std::mem::swap(&mut self.window, &mut surface.window);
        std::mem::swap(&mut self.hovered, &mut surface.hovered);
        std::mem::swap(&mut self.pointer, &mut surface.pointer);
    }
}
