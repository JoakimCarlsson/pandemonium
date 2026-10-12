//! The application: the window, its renderer and the events that drive them.

use std::sync::Arc;

use pm_color::Color;
use pm_renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

/// The name the desktop knows the application by.
const APP_ID: &str = "pandemonium";

/// Logical width of the window when it opens.
const WIDTH: f32 = 1280.0;

/// Logical height of the window when it opens.
const HEIGHT: f32 = 800.0;

/// The window and what draws into it; both exist once the platform is ready.
#[derive(Default)]
pub struct App {
    /// The window, once it is open.
    window: Option<Arc<Window>>,
    /// What draws into the window, once it is open.
    renderer: Option<Renderer>,
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
        match Renderer::new(window.clone(), size.width, size.height) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => fail_to_start(&error.to_string()),
        }
        window.request_redraw();
        self.window = Some(window);
    }

    /// Routes window events to the renderer.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(renderer)) =
                    (self.window.as_ref(), self.renderer.as_mut())
                {
                    renderer.render(Color::Zinc950.srgba(), || window.pre_present_notify());
                }
            }
            _ => {}
        }
    }
}
