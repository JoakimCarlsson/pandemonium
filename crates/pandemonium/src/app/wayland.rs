//! A connection of our own to the Wayland compositor a window is on, for what winit does not do itself.

use std::ffi::c_void;
use std::ptr::NonNull;

use wayland_client::backend::{Backend, ObjectId};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy};

/// A connection of our own on the Wayland display `display` the window is on, with its own queue of events.
///
/// The display must stay open for as long as the connection lives, which it does: winit keeps it open while its event loop runs, and the event loop outlives every window and so every connection made for one.
pub(super) fn share(display: NonNull<c_void>) -> Connection {
    let backend = unsafe { Backend::from_foreign_display(display.as_ptr().cast()) };
    Connection::from_backend(backend)
}

/// The window's `wl_surface` at `surface`, as an object of `connection`; `None` when the pointer is not a surface.
///
/// The surface must stay alive for as long as the object is used, which it does: it is only used while the window is open.
pub(super) fn beside(connection: &Connection, surface: NonNull<c_void>) -> Option<WlSurface> {
    let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.as_ptr().cast()) }.ok()?;
    WlSurface::from_id(connection, id).ok()
}
