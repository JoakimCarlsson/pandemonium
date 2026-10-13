//! The application: the window, its renderer and the events that drive them.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use pm_components::ShowcaseState;
use pm_input::{Button, Focus, FocusId, Key, KeyEvent, Modifiers, PointerEvent, PointerState};
use pm_renderer::{DrawList, Renderer};
use pm_state::State;
use pm_text::Shaper;
use pm_theme::{Appearance, ThemeFamily, default_family};
use pm_ui::{Input, Output};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::{keys, scene};

/// The name the desktop knows the application by.
const APP_ID: &str = "pandemonium";

/// Logical width of the window when it opens.
const WIDTH: f32 = 1280.0;

/// Logical height of the window when it opens.
const HEIGHT: f32 = 800.0;

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
    model: State<ShowcaseState>,
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
    /// When the application started, which animations run on.
    started: Instant,
}

impl Default for App {
    /// An application with no window yet, whose clock starts now.
    fn default() -> Self {
        let model = State::new(ShowcaseState::default());
        let dirty = Rc::new(Cell::new(false));
        let flag = Rc::clone(&dirty);
        model.subscribe(move |_| flag.set(true));
        Self {
            window: None,
            renderer: None,
            shaper: Shaper::new(),
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
            started: Instant::now(),
        }
    }
}

impl App {
    /// The attributes the window is opened with.
    fn attributes() -> winit::window::WindowAttributes {
        let attributes = Window::default_attributes()
            .with_title("Pandemonium")
            .with_inner_size(LogicalSize::new(WIDTH, HEIGHT));
        #[cfg(target_os = "linux")]
        let attributes = {
            use winit::platform::wayland::WindowAttributesExtWayland;
            use winit::platform::x11::WindowAttributesExtX11;

            let attributes = WindowAttributesExtWayland::with_name(attributes, APP_ID, "");
            WindowAttributesExtX11::with_name(attributes, APP_ID, APP_ID)
        };
        attributes
    }

    /// Milliseconds on the application clock.
    fn now_ms(&self) -> f32 {
        self.started.elapsed().as_secs_f32() * 1000.0
    }

    /// Draws one frame and asks for the next one while anything is moving or
    /// the frame could not be presented.
    fn redraw(&mut self) {
        let (Some(window), Some(renderer)) = (self.window.as_ref(), self.renderer.as_mut()) else {
            return;
        };
        let input = Input {
            pointer: self.pointer,
            focused: self.focus.current(),
            elapsed_ms: self.started.elapsed().as_secs_f32() * 1000.0,
        };
        let (width, height) = renderer.size();
        let state = self.model.get();
        let theme = self.family.variant(self.appearance);
        self.output = scene::paint(
            width,
            height,
            &state,
            &input,
            theme,
            &mut self.shaper,
            &mut self.list,
        );
        self.focus
            .set_order(self.output.regions.iter().map(|region| region.id).collect());
        let presented = renderer.render(&self.list, &mut self.shaper, scene::ground(theme), || {
            window.pre_present_notify()
        });
        if self.output.animating || !presented {
            window.request_redraw();
        }
    }

    /// Carries out what pressing the element `id` means.
    fn activate(&mut self, id: FocusId) {
        let now = self.now_ms();
        self.model
            .update(|state| *state = state.clone().after(id, now));
    }

    /// Folds a pointer event in, and acts on a press that ends on the
    /// element it began on.
    fn pointer(&mut self, event: PointerEvent) {
        self.pointer.apply(event);
        match event {
            PointerEvent::Pressed(Button::Left) => {
                self.pressed_on = self.pointer.position.and_then(|at| self.output.hit(at));
            }
            PointerEvent::Released(Button::Left) => {
                let released_on = self.pointer.position.and_then(|at| self.output.hit(at));
                if let (Some(pressed), Some(released)) = (self.pressed_on.take(), released_on)
                    && pressed == released
                {
                    self.focus.focus(pressed);
                    self.activate(pressed);
                }
            }
            _ => {}
        }
    }

    /// Acts on a key: Tab walks the focus, Enter and Space press the focused
    /// element, Escape lets go of focus.
    fn key(&mut self, event: KeyEvent) {
        if !event.pressed {
            return;
        }
        match event.key {
            Key::Tab if event.modifiers.shift => self.focus.previous(),
            Key::Tab => self.focus.next(),
            Key::Escape => self.focus.clear(),
            _ => match self.focus.current() {
                Some(id) if ShowcaseState::is_field(id) => {
                    if let Some(edit) = keys::edit(&event) {
                        self.model
                            .update(|state| *state = state.clone().edited(id, edit));
                    }
                }
                Some(id) if matches!(event.key, Key::Enter | Key::Space) => self.activate(id),
                _ => {}
            },
        }
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

impl ApplicationHandler for App {
    /// Opens the window and builds its renderer once the platform is ready.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = match event_loop.create_window(Self::attributes()) {
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
        window.request_redraw();
        self.window = Some(window);
    }

    /// Draws again when the model changed while no event asked for it.
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.dirty.replace(false)
            && let Some(window) = self.window.as_ref()
        {
            window.request_redraw();
        }
    }

    /// Routes window events to the renderer, the pointer and the keyboard.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => self.pointer(match state {
                ElementState::Pressed => PointerEvent::Pressed(Button::Left),
                ElementState::Released => PointerEvent::Released(Button::Left),
            }),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = keys::modifiers(modifiers.state());
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(event) =
                    keys::key_event(&event.logical_key, event.state, self.modifiers)
                {
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
