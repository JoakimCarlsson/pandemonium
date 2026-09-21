//! The window, what it is showing, and the frame it draws each redraw.
//!
//! [`App`] is the binary's whole state: one window, the GPU resources bound to
//! it and the model the frame is built from. It wires the layers and
//! implements none of them — every frame is `pm-ui` elements built from that
//! model, submitted to `pm-gfx` as one draw list.

mod input;

use std::sync::Arc;
use std::time::Instant;

use pm_gfx::{DrawList, Point, Quad, Rect, Renderer, Size};
use pm_ui::{Appearance, Axis, ResizeEdge, ResizeState, Scroll, Ui, family};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::config;
use crate::keymap::Resolver;
use crate::onboarding::{self, Message, Setup};
use crate::workspace::{self, SidebarProject};

/// The conductor window, the GPU resources bound to it and what it is showing.
#[derive(Default)]
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
    /// Whether the event loop should close after the current event.
    close_requested: bool,
}

impl App {
    /// The app as the last launch left it.
    pub fn restored() -> Self {
        Self {
            setup: config::load(),
            sidebar: ResizeState::new(252.0, 160.0, 480.0),
            bottom_panel: ResizeState::new(220.0, 120.0, 600.0),
            secondary_sidebar: ResizeState::new(252.0, 160.0, 480.0),
            primary_sidebar_open: true,
            ..Self::default()
        }
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
        if let Message::ResizeSidebar(event) = message {
            self.sidebar
                .resize(event, Axis::Horizontal, ResizeEdge::End);
            self.request_redraw();
            return;
        }
        if let Message::ResizeBottomPanel(event) = message {
            self.bottom_panel
                .resize(event, Axis::Vertical, ResizeEdge::Start);
            self.request_redraw();
            return;
        }
        if let Message::ResizeSecondarySidebar(event) = message {
            self.secondary_sidebar
                .resize(event, Axis::Horizontal, ResizeEdge::Start);
            self.request_redraw();
            return;
        }
        if message == Message::TogglePrimarySidebar {
            self.primary_sidebar_open = !self.primary_sidebar_open;
            self.request_redraw();
            return;
        }
        if message == Message::ToggleBottomPanel {
            self.bottom_panel_open = !self.bottom_panel_open;
            self.request_redraw();
            return;
        }
        if message == Message::ToggleSecondarySidebar {
            self.secondary_sidebar_open = !self.secondary_sidebar_open;
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
        config::save(&self.setup);
        self.request_redraw();
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
                &self.projects,
                workspace::Layout {
                    primary_sidebar_open: self.primary_sidebar_open,
                    primary_sidebar_width: self.sidebar.extent(),
                    bottom_panel_open: self.bottom_panel_open,
                    bottom_panel_height: self.bottom_panel.extent(),
                    secondary_sidebar_open: self.secondary_sidebar_open,
                    secondary_sidebar_width: self.secondary_sidebar.extent(),
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

impl ApplicationHandler for App {
    /// Opens the window and builds its renderer once the platform is ready.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("Pandemonium")
            .with_inner_size(LogicalSize::new(1440.0, 900.0));
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
            event_loop.exit();
        }
    }
}
