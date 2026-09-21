//! Application entry point: app state, panes, keymaps and wiring.

mod onboarding;

use std::sync::Arc;

use pm_gfx::{DrawList, Point, Quad, Rect, Renderer, Size};
use pm_ui::{Appearance, Theme, Ui};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use onboarding::{Message, Setup};

/// Logical pixels one notch of a mouse wheel scrolls.
const WHEEL_STEP: f32 = 48.0;

/// The conductor window, the GPU resources bound to it and what it is showing.
#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    ui: Option<Ui<Message>>,
    list: Option<DrawList>,
    setup: Setup,
    modifiers: ModifiersState,
    scroll: f32,
    content_height: f32,
}

impl App {
    /// The appearance the desktop asks for, defaulting to dark.
    fn system_appearance(&self) -> Appearance {
        match self.window.as_ref().and_then(|window| window.theme()) {
            Some(winit::window::Theme::Light) => Appearance::Light,
            _ => Appearance::Dark,
        }
    }

    /// Folds a message in and redraws.
    fn apply(&mut self, message: Message) {
        self.setup.apply(message);
        self.request_redraw();
    }

    /// Asks the platform for another frame.
    fn request_redraw(&self) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    /// Scrolls the page by `delta` logical pixels, clamped to its height.
    fn scroll_by(&mut self, delta: f32) {
        let viewport = self
            .renderer
            .as_ref()
            .map_or(0.0, |renderer| renderer.size().height);
        let limit = (self.content_height - viewport).max(0.0);
        self.scroll = (self.scroll - delta).clamp(0.0, limit);
        self.request_redraw();
    }

    /// Builds the frame and hands it to the renderer.
    fn draw(&mut self) {
        let (Some(renderer), Some(ui), Some(list)) =
            (self.renderer.as_mut(), self.ui.as_mut(), self.list.as_mut())
        else {
            return;
        };

        let theme = Theme::for_appearance(self.setup.theme_mode.resolve(
            match self.window.as_ref().and_then(|window| window.theme()) {
                Some(winit::window::Theme::Light) => Appearance::Light,
                _ => Appearance::Dark,
            },
        ));
        ui.set_theme(theme);

        let size = renderer.size();
        list.reset(size);
        list.quad(Quad::filled(
            Rect::from_xywh(0.0, 0.0, size.width, size.height),
            theme.colors.background,
        ));

        let page = onboarding::page(&theme, &self.setup);
        let painted = ui.draw(
            renderer.text(),
            list,
            Size::new(size.width, size.height + self.scroll),
            Point::new(0.0, -self.scroll),
            page,
        );
        self.content_height = painted.height;

        renderer.render(list);
    }
}

impl ApplicationHandler for App {
    /// Opens the window and builds its renderer once the platform is ready.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("Pandemonium")
            .with_inner_size(LogicalSize::new(1440.0, 900.0));
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

        let appearance = self.setup.theme_mode.resolve(self.system_appearance());
        self.ui = Some(Ui::new(Theme::for_appearance(appearance)));
        self.list = Some(DrawList::new(Size::zero()));
    }

    /// Routes window events to the UI and the renderer.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height, scale);
                }
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
                if let Some(ui) = self.ui.as_mut() {
                    ui.pointer_moved(Point::new(
                        position.x as f32 / scale,
                        position.y as f32 / scale,
                    ));
                }
                self.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(ui) = self.ui.as_mut() {
                    ui.pointer_left();
                }
                self.request_redraw();
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                let message = match (self.ui.as_mut(), state) {
                    (Some(ui), ElementState::Pressed) => {
                        ui.pointer_pressed();
                        None
                    }
                    (Some(ui), ElementState::Released) => ui.pointer_released(),
                    (None, _) => None,
                };
                match message {
                    Some(message) => self.apply(message),
                    None => self.request_redraw(),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, lines) => lines * WHEEL_STEP,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / scale,
                };
                self.scroll_by(delta);
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                let Some(ui) = self.ui.as_mut() else {
                    return;
                };

                let message = match event.logical_key {
                    Key::Named(NamedKey::Tab) if self.modifiers.shift_key() => {
                        ui.focus_previous();
                        None
                    }
                    Key::Named(NamedKey::Tab) => {
                        ui.focus_next();
                        None
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        ui.activate_focused()
                    }
                    Key::Named(NamedKey::Escape) => {
                        ui.clear_focus();
                        None
                    }
                    Key::Named(NamedKey::PageDown) => {
                        self.scroll_by(-WHEEL_STEP * 4.0);
                        None
                    }
                    Key::Named(NamedKey::PageUp) => {
                        self.scroll_by(WHEEL_STEP * 4.0);
                        None
                    }
                    _ => None,
                };

                match message {
                    Some(message) => self.apply(message),
                    None => self.request_redraw(),
                }
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }
}

/// Starts the conductor window.
fn main() {
    let event_loop = EventLoop::new().expect("event loop creation failed");
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop
        .run_app(&mut App::default())
        .expect("event loop failed");
}
