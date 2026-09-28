//! Files carried onto the window on Wayland, read off the seat's data device.
//!
//! winit keeps the display to itself and does not follow drags on Wayland,
//! so the window opens a queue of its own on that same display, asks the
//! seat for a data device, and takes what is let go as a `text/uri-list` on
//! a thread of its own.

use std::ffi::{OsString, c_void};
use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::Mutex;

use pm_gfx::Point;
use wayland_backend::client::Backend;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};

use super::Arrival;

/// The kind of data a file manager offers the files it carries as.
const URI_LIST: &str = "text/uri-list";

/// The data device's side of the drag it is following.
struct Listener {
    /// The display, flushed before what is let go is read.
    connection: Connection,
    /// What is being carried over the window, when it is files.
    offer: Option<WlDataOffer>,
    /// Where over the window it was last seen.
    at: Point,
    /// Where each arrival goes.
    send: Box<dyn Fn(Arrival) + Send>,
}

/// Follows drags onto the windows of the Wayland `display`, on a thread of
/// its own, calling `send` with each arrival.
pub fn listen(display: NonNull<c_void>, send: impl Fn(Arrival) + Send + 'static) {
    // SAFETY: winit keeps the display open for as long as its event loop runs,
    // which is the life of the process.
    let backend = unsafe { Backend::from_foreign_display(display.as_ptr().cast()) };
    let connection = Connection::from_backend(backend);
    std::thread::spawn(move || follow(connection, Box::new(send)));
}

/// Binds the seat's data device on a queue of its own and dispatches it for
/// as long as the display stays open.
fn follow(connection: Connection, send: Box<dyn Fn(Arrival) + Send>) -> Option<()> {
    let (globals, mut queue) = registry_queue_init::<Listener>(&connection).ok()?;
    let handle = queue.handle();
    let manager: WlDataDeviceManager = globals.bind(&handle, 1..=3, ()).ok()?;
    let seat: WlSeat = globals.bind(&handle, 1..=1, ()).ok()?;
    manager.get_data_device(&seat, &handle, ());
    let mut listener = Listener {
        connection,
        offer: None,
        at: Point::new(0.0, 0.0),
        send,
    };
    while queue.blocking_dispatch(&mut listener).is_ok() {}
    Some(())
}

impl Listener {
    /// Takes up the offer carried onto the window at `serial`, when it is
    /// files, and turns down anything else.
    fn enter(&mut self, serial: u32, offer: WlDataOffer) {
        let files = offer
            .data::<Mutex<Vec<String>>>()
            .and_then(|kinds| kinds.lock().ok())
            .is_some_and(|kinds| kinds.iter().any(|kind| kind == URI_LIST));
        if !files {
            offer.accept(serial, None);
            return offer.destroy();
        }
        offer.accept(serial, Some(URI_LIST.to_owned()));
        if offer.version() >= 3 {
            offer.set_actions(DndAction::Copy, DndAction::Copy);
        }
        self.offer = Some(offer);
        (self.send)(Arrival::Hovering(Some(self.at)));
    }

    /// Lets go of the offer being followed, saying the files have left.
    fn leave(&mut self) {
        if let Some(offer) = self.offer.take() {
            offer.destroy();
            (self.send)(Arrival::Left);
        }
    }

    /// Reads the files of the offer let go over the window, and says where
    /// they were let go.
    fn drop_offer(&mut self) {
        let Some(offer) = self.offer.take() else {
            return;
        };
        let paths = self.read_paths(&offer);
        if offer.version() >= 3 {
            offer.finish();
        }
        offer.destroy();
        (self.send)(Arrival::Dropped {
            at: Some(self.at),
            paths,
        });
    }

    /// Asks the program carrying `offer` for its files, and reads them.
    fn read_paths(&self, offer: &WlDataOffer) -> Vec<PathBuf> {
        let Ok((mut reader, writer)) = std::io::pipe() else {
            return Vec::new();
        };
        offer.receive(URI_LIST.to_owned(), writer.as_fd());
        drop(writer);
        let _ = self.connection.flush();
        let mut listed = Vec::new();
        let _ = reader.read_to_end(&mut listed);
        paths_listed(&String::from_utf8_lossy(&listed))
    }
}

/// The local files a `text/uri-list` names, skipping its comments and any
/// URI that is not a file's.
fn paths_listed(list: &str) -> Vec<PathBuf> {
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.strip_prefix("file://"))
        .filter_map(|rest| rest.find('/').map(|path| &rest[path..]))
        .map(|path| PathBuf::from(OsString::from_vec(percent_decoded(path))))
        .collect()
}

/// The bytes `text` stands for once each `%` escape in it is read back.
fn percent_decoded(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| text.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                at += 3;
            }
            None => {
                decoded.push(bytes[at]);
                at += 1;
            }
        }
    }
    decoded
}

impl Dispatch<WlRegistry, GlobalListContents> for Listener {
    /// Globals coming and going after the start are of no interest.
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for Listener {
    /// The seat is only the way to its data device; winit reads its input.
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: <WlSeat as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlDataDeviceManager, ()> for Listener {
    /// The manager says nothing.
    fn event(
        _: &mut Self,
        _: &WlDataDeviceManager,
        _: <WlDataDeviceManager as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlDataDevice, ()> for Listener {
    /// Follows a drag onto the window, over it, off it and let go on it.
    ///
    /// The clipboard's offers come through the same device; the window
    /// reads its clipboard elsewhere, so they are let go of at once.
    fn event(
        listener: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Enter {
                serial, x, y, id, ..
            } => {
                listener.leave();
                listener.at = Point::new(x as f32, y as f32);
                if let Some(offer) = id {
                    listener.enter(serial, offer);
                }
            }
            wl_data_device::Event::Motion { x, y, .. } => {
                listener.at = Point::new(x as f32, y as f32);
                if listener.offer.is_some() {
                    (listener.send)(Arrival::Hovering(Some(listener.at)));
                }
            }
            wl_data_device::Event::Leave => listener.leave(),
            wl_data_device::Event::Drop => listener.drop_offer(),
            wl_data_device::Event::Selection { id: Some(offer) } => offer.destroy(),
            _ => {}
        }
    }

    event_created_child!(Listener, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, Mutex::new(Vec::new())),
    ]);
}

impl Dispatch<WlDataOffer, Mutex<Vec<String>>> for Listener {
    /// Notes each kind of data an offer can be read as.
    fn event(
        _: &mut Self,
        _: &WlDataOffer,
        event: wl_data_offer::Event,
        kinds: &Mutex<Vec<String>>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event
            && let Ok(mut kinds) = kinds.lock()
        {
            kinds.push(mime_type);
        }
    }
}
