//! The settings window: a window of its own beside the main one, opened while
//! settings are open and closed with them, drawn from the same state.

use std::sync::Arc;

use pm_components::settings::is_settings;
use pm_components::state::SETTINGS;
use pm_input::PointerState;
use pm_pages::Screen;
use pm_renderer::Renderer;
use pm_ui::{Cursor, Input, Output};
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use super::{App, cursor_icon, fail_to_start};
use crate::scene;

/// How large the settings window opens, in logical pixels.
const OPENING_SIZE: LogicalSize<f32> = LogicalSize::new(960.0, 720.0);

/// The least the settings window can be made, in logical pixels.
const LEAST_SIZE: LogicalSize<f32> = LogicalSize::new(640.0, 420.0);

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
}

impl App {
    /// Opens the settings window when settings were opened, brings it forward
    /// when it is already open, and closes it when settings were closed.
    pub(super) fn sync_settings_window(&mut self, event_loop: &ActiveEventLoop) {
        let open = self.model.read(Screen::settings_open);
        match (open, self.settings_surface.is_some()) {
            (true, false) => {
                if let Some(main) = self.renderer.as_ref() {
                    self.settings_surface = Some(Self::open_settings_window(event_loop, main));
                }
            }
            (false, true) => self.settings_surface = None,
            _ => {}
        }
    }

    /// A new settings window, drawn into with the device of the `main` window's renderer.
    fn open_settings_window(event_loop: &ActiveEventLoop, main: &Renderer) -> Surface {
        let attributes = Self::attributes(OPENING_SIZE)
            .with_title("Settings")
            .with_min_inner_size(LEAST_SIZE);
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => fail_to_start(&format!("could not open the settings window: {error}")),
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
        }
    }

    /// Opens settings in their window, or brings the window forward when they are open.
    pub(super) fn open_settings(&mut self) {
        match self
            .settings_surface
            .as_ref()
            .and_then(|surface| surface.window.as_ref())
        {
            Some(window) => window.focus_window(),
            None if !self.on_settings => self.activate(SETTINGS),
            None => {}
        }
    }

    /// Whether `id` is the settings window.
    pub(super) fn is_settings_window(&self, id: WindowId) -> bool {
        self.settings_surface
            .as_ref()
            .and_then(|surface| surface.window.as_ref())
            .is_some_and(|window| window.id() == id)
    }

    /// Asks the settings window, when it is open, to be drawn again.
    pub(super) fn redraw_settings_window(&self) {
        if let Some(window) = self
            .settings_surface
            .as_ref()
            .and_then(|surface| surface.window.as_ref())
        {
            window.request_redraw();
        }
    }

    /// Carries out `event` of the settings window: closing it closes settings,
    /// taking the keyboard leaves any field of the main window, and the rest
    /// is handled as the main window's events are, against the settings
    /// window's own frame and pointer.
    pub(super) fn settings_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.model
                    .update(|state| *state = std::mem::take(state).without_settings());
                self.settings_surface = None;
            }
            WindowEvent::Focused(true) => {
                if self.focus.current().is_some_and(|id| !is_settings(id)) {
                    self.focus.clear();
                }
            }
            event => {
                if matches!(event, WindowEvent::KeyboardInput { .. }) {
                    self.focus.set_order(
                        self.settings_output()
                            .regions
                            .iter()
                            .map(|region| region.id)
                            .collect(),
                    );
                }
                self.in_settings_window(|app| app.handle_window_event(event_loop, event));
            }
        }
    }

    /// What the settings window's last frame found, or nothing when it is closed.
    fn settings_output(&self) -> Output {
        self.settings_surface
            .as_ref()
            .map(|surface| surface.output.clone())
            .unwrap_or_default()
    }

    /// Runs `act` with the settings window standing in for the main one: its
    /// window, renderer, frame, pointer and pointer shape are swapped in for
    /// the time `act` runs.
    fn in_settings_window(&mut self, act: impl FnOnce(&mut Self)) {
        let Some(mut surface) = self.settings_surface.take() else {
            return;
        };
        self.swap_surface(&mut surface);
        self.on_settings = true;
        act(self);
        self.on_settings = false;
        self.swap_surface(&mut surface);
        if self.model.read(Screen::settings_open) {
            self.settings_surface = Some(surface);
        }
    }

    /// Trades the main window's surface with `surface`.
    fn swap_surface(&mut self, surface: &mut Surface) {
        std::mem::swap(&mut self.window, &mut surface.window);
        std::mem::swap(&mut self.renderer, &mut surface.renderer);
        std::mem::swap(&mut self.output, &mut surface.output);
        std::mem::swap(&mut self.pointer, &mut surface.pointer);
        std::mem::swap(&mut self.cursor, &mut surface.cursor);
    }

    /// Draws one frame of the settings window, while it stands in for the main
    /// one, and asks for the next while anything is moving or the frame could
    /// not be presented.
    pub(super) fn redraw_settings(&mut self) {
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
        self.output = model.read(|state| {
            scene::paint_settings(
                width,
                height,
                state,
                &input,
                theme,
                &mut self.shaper,
                &mut self.list,
            )
        });
        if self.output.cursor != self.cursor {
            self.cursor = self.output.cursor;
            window.set_cursor(cursor_icon(self.cursor));
        }
        let presented = renderer.render(&self.list, &mut self.shaper, scene::ground(theme), || {
            window.pre_present_notify()
        });
        if self.output.animating || !presented {
            window.request_redraw();
        }
    }
}
