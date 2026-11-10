//! The extents of the band an X11 window draws its shadow in, told to the window manager through `_GTK_FRAME_EXTENTS` so that it places, snaps and tiles the frame alone.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// The name of the property that holds the extents.
const PROPERTY: &[u8] = b"_GTK_FRAME_EXTENTS";

/// The band one X11 window draws its shadow in, as last told to the window manager.
pub(crate) struct Extents {
    /// The window.
    window: u32,
    /// A connection of our own to the X server and the atom of [`PROPERTY`] on it, once made; `None` when it could not be.
    link: Option<(RustConnection, u32)>,
    /// Whether a connection was tried yet.
    tried: bool,
    /// The width of the band last told, in physical pixels.
    told: Option<u32>,
}

impl Extents {
    /// The band of `window`, not yet told.
    pub(crate) fn new(window: u32) -> Self {
        Self {
            window,
            link: None,
            tried: false,
            told: None,
        }
    }

    /// Tells the window manager the band is `band` physical pixels wide on every side, or that there is none when it is zero, unless that is what it was last told.
    pub(crate) fn fit(&mut self, band: u32) {
        if self.told == Some(band) {
            return;
        }
        if !self.tried {
            self.tried = true;
            self.link = connect();
        }
        let Some((connection, atom)) = self.link.as_ref() else {
            return;
        };
        let sent = if band == 0 {
            connection
                .delete_property(self.window, *atom)
                .map(|cookie| cookie.ignore_error())
        } else {
            connection
                .change_property32(
                    PropMode::REPLACE,
                    self.window,
                    *atom,
                    AtomEnum::CARDINAL,
                    &[band; 4],
                )
                .map(|cookie| cookie.ignore_error())
        };
        if sent.is_ok() && connection.flush().is_ok() {
            self.told = Some(band);
        }
    }
}

/// A connection of our own to the X server the windows are on, with the atom of [`PROPERTY`] on it; `None` when either cannot be had.
fn connect() -> Option<(RustConnection, u32)> {
    let (connection, _) = x11rb::connect(None).ok()?;
    let atom = connection
        .intern_atom(false, PROPERTY)
        .ok()?
        .reply()
        .ok()?
        .atom;
    Some((connection, atom))
}
