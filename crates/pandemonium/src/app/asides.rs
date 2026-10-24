//! The windows that open beside the main one, the settings window and the
//! folder picker's: each is opened while what it shows is open and closed
//! with it, and is drawn from the same state as the main window.

use std::sync::Arc;

use pm_components::folder_picker::{PICKER_FIELD, is_picker};
use pm_components::settings::is_settings;
use pm_components::state::SETTINGS;
use pm_input::{FocusId, PointerState};
use pm_pages::Screen;
use pm_renderer::{DrawList, Renderer};
use pm_text::Shaper;
use pm_theme::Theme;
use pm_ui::{Cursor, Input, Output};
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{UserAttentionType, Window, WindowId, WindowLevel};

use super::{App, cursor_icon, fail_to_start};
use crate::scene;

/// Which of the windows beside the main one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Aside {
    /// The settings window.
    Settings,
    /// The folder picker's window.
    Picker,
}

/// What draws one of the windows beside the main one into a draw list.
type Painter = fn(f32, f32, &Screen, &Input, &Theme, &mut Shaper, &mut DrawList) -> Output;

impl Aside {
    /// Every window that can open beside the main one.
    const ALL: [Self; 2] = [Self::Settings, Self::Picker];

    /// Whether what the window shows is open on `screen`.
    fn is_open(self, screen: &Screen) -> bool {
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
    fn opening_size(self) -> LogicalSize<f32> {
        match self {
            Self::Settings => LogicalSize::new(960.0, 720.0),
            Self::Picker => LogicalSize::new(880.0, 560.0),
        }
    }

    /// The least the window can be made, in logical pixels.
    fn least_size(self) -> LogicalSize<f32> {
        match self {
            Self::Settings => LogicalSize::new(640.0, 420.0),
            Self::Picker => LogicalSize::new(560.0, 360.0),
        }
    }

    /// Whether `id` names something of the window.
    pub(super) fn owns(self, id: FocusId) -> bool {
        match self {
            Self::Settings => is_settings(id),
            Self::Picker => is_picker(id),
        }
    }

    /// Where the window sits among the others: the folder picker stays in front of the windows of the app.
    fn level(self) -> WindowLevel {
        match self {
            Self::Settings => WindowLevel::Normal,
            Self::Picker => WindowLevel::AlwaysOnTop,
        }
    }

    /// What draws the window.
    fn painter(self) -> Painter {
        match self {
            Self::Settings => scene::paint_settings,
            Self::Picker => scene::paint_picker,
        }
    }
}

/// Whether `id` names something of any window beside the main one.
pub(super) fn is_aside(id: FocusId) -> bool {
    Aside::ALL.iter().any(|aside| aside.owns(id))
}

/// A window and what it alone keeps: what draws into it, where the pointer is
/// over it, what its last frame found and the pointer shape it shows.
pub(super) struct Surface {
    /// The window.
    window: Option<Arc<Window>>,
    /// What draws into the window.
    renderer: Option<Renderer>,
    /// What the last frame of the window found on screen.
    output: Output,
    /// The pointer as the window's events so far left it.
    pointer: PointerState,
    /// The shape the pointer was last set to over the window.
    cursor: Cursor,
    /// The edge of the window the pointer last showed it resizes, when it did.
    edge_cursor: Option<winit::window::ResizeDirection>,
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
    fn window_of(&self, aside: Aside) -> Option<&Arc<Window>> {
        match aside {
            Aside::Settings => self.settings_surface.as_ref(),
            Aside::Picker => self.picker_surface.as_ref(),
        }
        .and_then(|surface| surface.window.as_ref())
    }

    /// Opens each window whose state was opened, brings none forward when it
    /// is open already, and closes each whose state was closed.
    pub(super) fn sync_asides(&mut self, event_loop: &ActiveEventLoop) {
        for aside in Aside::ALL {
            let open = self.model.read(|screen| aside.is_open(screen));
            let showing = self.surface_of(aside).is_some();
            match (open, showing) {
                (true, false) => {
                    if let Some(main) = self.renderer.as_ref() {
                        let surface = Self::open_aside_window(event_loop, main, aside);
                        *self.surface_of(aside) = Some(surface);
                    }
                }
                (false, true) => *self.surface_of(aside) = None,
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
        self.raised_picker = serial;
        if serial.is_none() {
            return;
        }
        self.focus.claim(PICKER_FIELD);
        if let Some(window) = self.window_of(Aside::Picker) {
            window.set_minimized(false);
            window.focus_window();
            window.request_user_attention(Some(UserAttentionType::Informational));
        }
    }

    /// A new window for `aside`, drawn into with the device of the `main` window's renderer.
    fn open_aside_window(event_loop: &ActiveEventLoop, main: &Renderer, aside: Aside) -> Surface {
        let attributes = Self::attributes(aside.opening_size())
            .with_title(aside.title())
            .with_resizable(true)
            .with_window_level(aside.level())
            .with_min_inner_size(aside.least_size());
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => fail_to_start(&format!(
                "could not open the {} window: {error}",
                aside.title()
            )),
        };
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        let renderer = match main.beside(window.clone(), size.width, size.height, scale) {
            Ok(renderer) => renderer,
            Err(error) => fail_to_start(&error.to_string()),
        };
        window.set_ime_allowed(true);
        window.request_redraw();
        Surface {
            window: Some(window),
            renderer: Some(renderer),
            output: Output::default(),
            pointer: PointerState::default(),
            cursor: Cursor::Default,
            edge_cursor: None,
        }
    }

    /// Opens settings in their window, or brings the window forward when they are open.
    pub(super) fn open_settings(&mut self) {
        match self.window_of(Aside::Settings) {
            Some(window) => window.focus_window(),
            None if self.on_aside.is_none() => self.activate(SETTINGS),
            None => {}
        }
    }

    /// Which window beside the main one `id` is, if it is one.
    pub(super) fn aside_of_window(&self, id: WindowId) -> Option<Aside> {
        Aside::ALL.into_iter().find(|aside| {
            self.window_of(*aside)
                .is_some_and(|window| window.id() == id)
        })
    }

    /// Asks every window beside the main one that is open to be drawn again.
    pub(super) fn redraw_asides(&self) {
        for aside in Aside::ALL {
            if let Some(window) = self.window_of(aside) {
                window.request_redraw();
            }
        }
    }

    /// Carries out `event` of the window of `aside`: closing it closes what it
    /// shows, taking the keyboard leaves any field of another window, and the
    /// rest is handled as the main window's events are, against the window's
    /// own frame and pointer.
    pub(super) fn aside_event(
        &mut self,
        aside: Aside,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.model
                    .update(|state| *state = aside.closed(std::mem::take(state)));
                *self.surface_of(aside) = None;
            }
            WindowEvent::Focused(true) => {
                if self.focus.current().is_some_and(|id| !aside.owns(id)) {
                    self.focus.clear();
                }
            }
            event => {
                self.in_aside(aside, |app| app.handle_window_event(event_loop, event));
            }
        }
    }

    /// Runs `act` with the window of `aside` standing in for the main one: its
    /// window, renderer, frame, pointer and pointer shape are swapped in for
    /// the time `act` runs.
    fn in_aside(&mut self, aside: Aside, act: impl FnOnce(&mut Self)) {
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
        std::mem::swap(&mut self.renderer, &mut surface.renderer);
        std::mem::swap(&mut self.output, &mut surface.output);
        std::mem::swap(&mut self.pointer, &mut surface.pointer);
        std::mem::swap(&mut self.cursor, &mut surface.cursor);
        std::mem::swap(&mut self.edge_cursor, &mut surface.edge_cursor);
    }

    /// Draws one frame of the window of `aside`, while it stands in for the
    /// main one, and asks for the next while anything is moving or the frame
    /// could not be presented.
    pub(super) fn redraw_aside(&mut self, aside: Aside) {
        let edge = self.frame_edge();
        let (Some(window), Some(renderer)) = (self.window.as_ref(), self.renderer.as_mut()) else {
            return;
        };
        let input = Input {
            pointer: self.pointer,
            focused: self.focus.current(),
            elapsed_ms: self.started.elapsed().as_secs_f32() * 1000.0,
            selection: self.text_selection,
            held_bar: self.bar_drag.map(|(id, _)| id),
        };
        let (width, height) = renderer.size();
        let theme = self.family.variant(self.appearance);
        let model = self.model.clone();
        let paint = aside.painter();
        self.output = model.read(|state| {
            paint(
                width,
                height,
                state,
                &input,
                theme,
                &mut self.shaper,
                &mut self.list,
            )
        });
        if self.output.cursor != self.cursor || edge != self.edge_cursor {
            self.cursor = self.output.cursor;
            self.edge_cursor = edge;
            window
                .set_cursor(edge.map_or_else(|| cursor_icon(self.cursor), super::frame::edge_icon));
        }
        let held_elsewhere = self.focus.current().filter(|id| !aside.owns(*id));
        self.focus.set_order(
            self.output
                .regions
                .iter()
                .map(|region| region.id)
                .chain(held_elsewhere)
                .collect(),
        );
        let presented = renderer.render(&self.list, &mut self.shaper, scene::ground(theme), || {
            window.pre_present_notify()
        });
        if self.output.animating || !presented {
            window.request_redraw();
        }
    }
}
