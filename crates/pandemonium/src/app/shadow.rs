//! The shadow a floating window that draws its own frame casts outside itself, where the desktop does not count it as part of the window: on Wayland strips of surface laid under the edges of the window, and on X11 the extents of the band inside the window that holds it, told to the window manager.

#[cfg(target_os = "linux")]
mod extents;
#[cfg(target_os = "linux")]
mod strips;

use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

/// What shows the shadow of one window outside the frame the desktop sees.
pub(super) enum Shadow {
    /// Nothing does: the desktop frames the window itself, or cannot be told of its shadow.
    Absent,
    /// Strips of surface laid under the edges of a Wayland window, with the shadow drawn on them.
    #[cfg(target_os = "linux")]
    Strips(Box<strips::Strips>),
    /// The extents of the band an X11 window draws its shadow in, told to the window manager.
    #[cfg(target_os = "linux")]
    Extents(Box<extents::Extents>),
}

impl Shadow {
    /// What shows the shadow of `window` on the desktop it opened on; [`Shadow::Absent`] where there is no way to.
    pub(super) fn open(window: &Window) -> Self {
        #[cfg(target_os = "linux")]
        {
            use winit::raw_window_handle::{
                HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
            };

            let display = window.display_handle().map(|handle| handle.as_raw());
            let surface = window.window_handle().map(|handle| handle.as_raw());
            match (display, surface) {
                (Ok(RawDisplayHandle::Wayland(display)), Ok(RawWindowHandle::Wayland(surface))) => {
                    if let Some(strips) = strips::Strips::open(display.display, surface.surface) {
                        return Self::Strips(Box::new(strips));
                    }
                }
                (_, Ok(RawWindowHandle::Xlib(handle))) => {
                    return Self::Extents(Box::new(extents::Extents::new(handle.window as u32)));
                }
                (_, Ok(RawWindowHandle::Xcb(handle))) => {
                    return Self::Extents(Box::new(extents::Extents::new(handle.window.get())));
                }
                _ => {}
            }
        }
        let _ = window;
        Self::Absent
    }

    /// Fits the shadow to `window` as it is now: cast around its frame while it is `floating`, and gone while it is not.
    pub(super) fn fit(&mut self, window: &Window, floating: bool) {
        match self {
            Self::Absent => {}
            #[cfg(target_os = "linux")]
            Self::Strips(strips) => {
                let scale = window.scale_factor();
                let size = window.inner_size().to_logical::<f64>(scale);
                strips.fit(
                    (size.width.round() as u32, size.height.round() as u32),
                    scale,
                    floating,
                );
            }
            #[cfg(target_os = "linux")]
            Self::Extents(extents) => {
                let band = if floating {
                    (f64::from(pm_components::SHADOW_MARGIN) * window.scale_factor()).round() as u32
                } else {
                    0
                };
                extents.fit(band);
            }
        }
    }
}

/// Whether the shadow of a floating window is shown outside the window on the desktop `event_loop` runs on, so that the window leaves no band for it: it is on Wayland.
pub(super) fn cast_outside(event_loop: &ActiveEventLoop) -> bool {
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::ActiveEventLoopExtWayland;

        event_loop.is_wayland()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = event_loop;
        false
    }
}
