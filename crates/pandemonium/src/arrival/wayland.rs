//! Files carried onto the window on Wayland, read off the seat's data device.
//!
//! winit keeps the display to itself and does not follow drags on Wayland,
//! so the window opens a queue of its own on that same display, asks the
//! seat for a data device, and takes what is let go as a `text/uri-list` on
//! a thread of its own.

use std::ffi::OsString;
use std::io::{self, Read};
use std::os::fd::AsFd;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pm_gfx::Point;
use wayland_backend::client::Backend;
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_registry;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};

use winit::event_loop::OwnedDisplayHandle;
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

use super::Arrival;

/// The kind of data a file manager offers the files it carries as.
const URI_LIST: &str = "text/uri-list";

/// How often the guest queue checks events read by winit and pending transfers.
const DISPATCH_INTERVAL: Duration = Duration::from_millis(10);

/// Owns the listener thread and keeps its borrowed display alive until it joins.
pub struct Listening {
    /// Cancellation shared with initialization, dispatch and file transfers.
    stop: Arc<AtomicBool>,
    /// The thread whose guest backend must be dropped before the display.
    thread: Option<JoinHandle<()>>,
    /// Winit's display owner, retained through errors and unwinding as well.
    _display: OwnedDisplayHandle,
}

impl Drop for Listening {
    /// Wakes the listener and waits for every guest Wayland object to be dropped.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

/// The data device's side of the drag it is following.
struct Listener {
    /// The display, flushed before what is let go is read.
    connection: Connection,
    /// Cancellation checked before dispatching and while reading files.
    stop: Arc<AtomicBool>,
    /// The first data-device manager advertised by the compositor.
    manager: Option<WlDataDeviceManager>,
    /// The first seat advertised by the compositor.
    seat: Option<WlSeat>,
    /// The data device, released before dropping the guest backend.
    device: Option<WlDataDevice>,
    /// What is being carried over the window, when it is files.
    offer: Option<WlDataOffer>,
    /// Where over the window it was last seen.
    at: Point,
    /// Where each arrival goes.
    send: Box<dyn Fn(Arrival) + Send>,
}

/// Follows drags on an owned winit display until the returned guard is dropped.
///
/// The foreign backend borrows the raw display held by `display`; the guard
/// joins its thread before releasing that owner, including during unwinding.
pub fn listen(
    display: OwnedDisplayHandle,
    send: impl Fn(Arrival) + Send + 'static,
) -> io::Result<Listening> {
    let handle = display.display_handle().map_err(io::Error::other)?;
    let RawDisplayHandle::Wayland(raw) = handle.as_raw() else {
        return Err(io::Error::other("the display is not Wayland"));
    };
    let backend = unsafe { Backend::from_foreign_display(raw.display.as_ptr().cast()) };
    let connection = Connection::from_backend(backend);
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let thread = thread::Builder::new()
        .name("wayland-arrivals".to_owned())
        .spawn(move || follow(connection, Box::new(send), stopping))?;
    Ok(Listening {
        stop,
        thread: Some(thread),
        _display: display,
    })
}

/// Dispatches the guest queue's events read by winit, without a second reader.
///
/// Registry discovery uses the same cancellable loop as normal dispatch, so
/// missing globals and an unresponsive compositor cannot block shutdown.
/// Parking is interrupted by the guard, and no libwayland read guard can hold
/// shutdown waiting for another display reader.
fn follow(connection: Connection, send: Box<dyn Fn(Arrival) + Send>, stop: Arc<AtomicBool>) {
    let mut queue = connection.new_event_queue::<Listener>();
    let handle = queue.handle();
    connection.display().get_registry(&handle, ());
    let mut listener = Listener {
        connection,
        stop,
        manager: None,
        seat: None,
        device: None,
        offer: None,
        at: Point::new(0.0, 0.0),
        send,
    };
    while !listener.stopped() {
        if queue.dispatch_pending(&mut listener).is_err() || listener.connection.flush().is_err() {
            break;
        }
        thread::park_timeout(DISPATCH_INTERVAL);
    }
    listener.leave();
    if let Some(device) = listener.device.take()
        && device.version() >= 2
    {
        device.release();
    }
    let _ = listener.connection.flush();
}

impl Listener {
    /// Whether the owning application has requested shutdown.
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

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
        if !self.stopped() {
            (self.send)(Arrival::Dropped {
                at: Some(self.at),
                paths,
            });
        }
    }

    /// Asks the program carrying `offer` for its files, and reads them.
    fn read_paths(&self, offer: &WlDataOffer) -> Vec<PathBuf> {
        let Ok((mut reader, writer)) = UnixStream::pair() else {
            return Vec::new();
        };
        if reader.set_nonblocking(true).is_err() {
            return Vec::new();
        }
        offer.receive(URI_LIST.to_owned(), writer.as_fd());
        drop(writer);
        let _ = self.connection.flush();
        let mut listed = Vec::new();
        let mut bytes = [0; 4096];
        while !self.stopped() {
            match reader.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => listed.extend_from_slice(&bytes[..count]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::park_timeout(DISPATCH_INTERVAL);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
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

impl Dispatch<WlRegistry, ()> for Listener {
    /// Binds the first manager and seat as their globals arrive, then their device.
    fn event(
        listener: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        if listener.stopped() {
            return;
        }
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_data_device_manager" if listener.manager.is_none() => {
                    listener.manager = Some(registry.bind(name, version.min(3), handle, ()));
                }
                "wl_seat" if listener.seat.is_none() => {
                    listener.seat = Some(registry.bind(name, 1, handle, ()));
                }
                _ => {}
            }
            if listener.device.is_none()
                && let (Some(manager), Some(seat)) = (&listener.manager, &listener.seat)
            {
                listener.device = Some(manager.get_data_device(seat, handle, ()));
            }
        }
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
