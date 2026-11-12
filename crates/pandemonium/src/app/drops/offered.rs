//! Files dragged from another program over a Wayland window, which winit does not report.
//!
//! A data device of our own, on a connection that shares the window's display, hears where the files are held over the window and reads their addresses when they are let go.

use std::ffi::c_void;
use std::io::Read;
use std::os::fd::AsFd;
use std::ptr::NonNull;

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};

use crate::app::wayland::{beside, share};

/// The kind of data a list of files is offered as: their addresses, one a line.
const URI_LIST: &str = "text/uri-list";

/// What happened to files held over the window, with places in logical pixels from the top left corner of its surface.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Offer {
    /// They are held at this place.
    Over(f32, f32),
    /// They left the window, or the drag was given up.
    Left,
    /// They were let go at this place: the paths of the files on this machine.
    Dropped(Vec<String>, (f32, f32)),
}

/// What the events of the connection change.
struct Hold {
    /// The surface of the window.
    window: WlSurface,
    /// The offers the compositor has announced that hold files.
    listed: Vec<WlDataOffer>,
    /// The offer held over the window now, when it holds files.
    over: Option<WlDataOffer>,
    /// Where it was last held.
    at: (f32, f32),
    /// What happened since it was last asked.
    offers: Vec<Offer>,
}

impl Hold {
    /// Lets go of `offer`, which is used no more.
    fn forget(&mut self, offer: &WlDataOffer) {
        self.listed.retain(|listed| listed != offer);
        offer.destroy();
    }
}

/// The data device that hears of files dragged over one Wayland window, and the connection it is made on.
pub(crate) struct Offered {
    /// Our connection to the compositor, on the window's display.
    connection: Connection,
    /// The queue the events of our objects arrive on.
    queue: EventQueue<Hold>,
    /// What those events change.
    hold: Hold,
    /// The device drags are told to.
    device: WlDataDevice,
}

impl Offered {
    /// Listens for files dragged over the Wayland surface `surface` on the display `display`, both as the window hands them out; `None` when the compositor lacks what that needs.
    ///
    /// The window's event loop must keep `display` open, and the window must keep `surface` alive, for as long as this lives: winit does both, as this is dropped before its window and every window before the event loop.
    pub(crate) fn open(display: NonNull<c_void>, surface: NonNull<c_void>) -> Option<Self> {
        let connection = share(display);
        let window = beside(&connection, surface)?;
        let (globals, mut queue) = registry_queue_init::<Hold>(&connection).ok()?;
        let handle = queue.handle();
        let seat: WlSeat = globals.bind(&handle, 1..=1, ()).ok()?;
        let manager: WlDataDeviceManager = globals.bind(&handle, 3..=3, ()).ok()?;
        let device = manager.get_data_device(&seat, &handle, ());
        let mut hold = Hold {
            window,
            listed: Vec::new(),
            over: None,
            at: (0.0, 0.0),
            offers: Vec::new(),
        };
        let _ = queue.roundtrip(&mut hold);
        Some(Self {
            connection,
            queue,
            hold,
            device,
        })
    }

    /// What happened to files held over the window since this was last asked, oldest first.
    pub(crate) fn take(&mut self) -> Vec<Offer> {
        let _ = self.queue.dispatch_pending(&mut self.hold);
        let _ = self.connection.flush();
        std::mem::take(&mut self.hold.offers)
    }
}

impl Drop for Offered {
    /// Takes the device off the compositor.
    fn drop(&mut self) {
        self.device.release();
        let _ = self.connection.flush();
    }
}

/// The paths of the files `offer` holds, read from the program that offers them; none when they cannot be read.
fn paths_of(offer: &WlDataOffer, connection: &Connection) -> Vec<String> {
    let Ok((mut reader, writer)) = std::io::pipe() else {
        return Vec::new();
    };
    offer.receive(URI_LIST.to_owned(), writer.as_fd());
    let _ = connection.flush();
    drop(writer);
    let mut text = String::new();
    let _ = reader.read_to_string(&mut text);
    text.lines().filter_map(local_path).collect()
}

/// The path on this machine the address `line` of a list of files names; `None` for a comment and for an address that is not a file's.
fn local_path(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix("file://")?;
    let path = &rest[rest.find('/')?..];
    let mut bytes = Vec::with_capacity(path.len());
    let mut left = path.bytes();
    while let Some(byte) = left.next() {
        if byte == b'%' {
            let high = char::from(left.next()?).to_digit(16)?;
            let low = char::from(left.next()?).to_digit(16)?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

impl Dispatch<WlDataDevice, ()> for Hold {
    /// Follows a drag over the window: accepts it while it holds files, and reads them when they are let go. An offer that is not used, such as the clipboard's, is let go of.
    fn event(
        hold: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        connection: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Enter {
                serial,
                surface,
                x,
                y,
                id: Some(offer),
            } => {
                if surface == hold.window && hold.listed.contains(&offer) {
                    offer.accept(serial, Some(URI_LIST.to_owned()));
                    offer.set_actions(DndAction::Copy, DndAction::Copy);
                    hold.at = (x as f32, y as f32);
                    hold.offers.push(Offer::Over(hold.at.0, hold.at.1));
                    hold.over = Some(offer);
                } else {
                    hold.forget(&offer);
                }
            }
            wl_data_device::Event::Motion { x, y, .. } if hold.over.is_some() => {
                hold.at = (x as f32, y as f32);
                hold.offers.push(Offer::Over(hold.at.0, hold.at.1));
            }
            wl_data_device::Event::Leave => {
                if let Some(offer) = hold.over.take() {
                    hold.forget(&offer);
                    hold.offers.push(Offer::Left);
                }
            }
            wl_data_device::Event::Drop => {
                if let Some(offer) = hold.over.take() {
                    let paths = paths_of(&offer, connection);
                    offer.finish();
                    hold.forget(&offer);
                    hold.offers.push(Offer::Dropped(paths, hold.at));
                }
            }
            wl_data_device::Event::Selection { id: Some(offer) } => hold.forget(&offer),
            _ => {}
        }
    }

    wayland_client::event_created_child!(Hold, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
    ]);
}

impl Dispatch<WlDataOffer, ()> for Hold {
    /// Notes an offer that holds files.
    fn event(
        hold: &mut Self,
        offer: &WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, wl_data_offer::Event::Offer { mime_type } if mime_type == URI_LIST) {
            hold.listed.push(offer.clone());
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for Hold {
    /// Ignores globals coming and going: the device binds what it needs once.
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(Hold: ignore WlSeat);
delegate_noop!(Hold: ignore WlDataDeviceManager);

/// Checks how the addresses of dropped files are read.
#[cfg(test)]
mod tests {
    use super::local_path;

    /// An address of a file gives its path with escapes undone; anything else gives nothing.
    #[test]
    fn addresses_become_paths() {
        assert_eq!(
            local_path("file:///home/me/My%20Files/a%C3%A5.txt\r"),
            Some("/home/me/My Files/aå.txt".to_owned())
        );
        assert_eq!(local_path("file://host/srv/x"), Some("/srv/x".to_owned()));
        assert_eq!(local_path("# a comment"), None);
        assert_eq!(local_path("https://example.com/x"), None);
        assert_eq!(local_path("file:///bad%zz"), None);
    }
}
