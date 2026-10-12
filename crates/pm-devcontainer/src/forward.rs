//! Desktop loopback listeners for the ports a declaration forwards.

use crate::event::Forward;
use pm_host::Host;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// How often an idle listener looks for a request to stop.
const POLL: Duration = Duration::from_millis(40);

/// The listeners of one environment, released when it is dropped.
///
/// Each accepted connection is carried to the container's loopback through
/// the environment's own transport, so a service bound to `127.0.0.1` inside
/// the container is reachable without publishing it on the engine's network.
pub struct Forwarder {
    /// Set to ask every listener to stop.
    stopping: Arc<AtomicBool>,
    /// The accept loops.
    listeners: Vec<JoinHandle<()>>,
    /// Open desktop sockets, shut down on release so their pumps end.
    connections: Arc<Mutex<Vec<TcpStream>>>,
    /// What each declared port turned into.
    forwards: Vec<Forward>,
}

impl Forwarder {
    /// Opens a desktop listener for each container port, remapping a busy one.
    pub fn start(host: &Host, ports: &[u16]) -> Self {
        let stopping = Arc::new(AtomicBool::new(false));
        let connections = Arc::new(Mutex::new(Vec::new()));
        let mut listeners = Vec::new();
        let mut forwards = Vec::new();
        for &container in ports {
            let (listener, local) = match bind(container) {
                Ok(bound) => bound,
                Err(error) => {
                    forwards.push(Forward {
                        container,
                        requested: container,
                        local: Err(error.to_string()),
                    });
                    continue;
                }
            };
            let accept = Accept {
                host: host.clone(),
                container,
                stopping: stopping.clone(),
                connections: connections.clone(),
            };
            listeners.push(std::thread::spawn(move || accept.run(listener)));
            forwards.push(Forward {
                container,
                requested: container,
                local: Ok(local),
            });
        }
        Self {
            stopping,
            listeners,
            connections,
            forwards,
        }
    }

    /// What each declared port turned into.
    pub fn forwards(&self) -> &[Forward] {
        &self.forwards
    }

    /// Closes every listener and connection, returning once the listeners are gone.
    pub fn release(&mut self) {
        self.stopping.store(true, Ordering::Release);
        for stream in self.connections.lock().unwrap().drain(..) {
            let _ = stream.shutdown(Shutdown::Both);
        }
        for listener in self.listeners.drain(..) {
            let _ = listener.join();
        }
        self.forwards.clear();
    }
}

impl Drop for Forwarder {
    /// Releases the listeners with the environment.
    fn drop(&mut self) {
        self.release();
    }
}

/// Binds the requested loopback port, or any free one when it is taken.
fn bind(requested: u16) -> io::Result<(TcpListener, u16)> {
    let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, requested)) {
        Ok(listener) => listener,
        Err(error)
            if error.kind() == io::ErrorKind::AddrInUse
                || error.kind() == io::ErrorKind::PermissionDenied =>
        {
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        }
        Err(error) => return Err(error),
    };
    listener.set_nonblocking(true)?;
    let local = listener.local_addr()?.port();
    Ok((listener, local))
}

/// One listener's accept loop and what it needs to carry a connection.
struct Accept {
    /// The environment's machine.
    host: Host,
    /// The container port connections are carried to.
    container: u16,
    /// Set when the loop must end.
    stopping: Arc<AtomicBool>,
    /// Where accepted sockets are kept so release can close them.
    connections: Arc<Mutex<Vec<TcpStream>>>,
}

impl Accept {
    /// Accepts connections until asked to stop.
    fn run(self, listener: TcpListener) {
        while !self.stopping.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => self.carry(stream),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(POLL),
                Err(_) => break,
            }
        }
    }

    /// Joins an accepted socket to a loopback stream inside the container.
    fn carry(&self, stream: TcpStream) {
        let _ = stream.set_nonblocking(false);
        let Ok(kept) = stream.try_clone() else {
            return;
        };
        self.connections.lock().unwrap().push(kept);
        let host = self.host.clone();
        let container = self.container;
        std::thread::spawn(move || {
            let Ok((mut remote_in, mut remote_out)) = host.tcp(container) else {
                let _ = stream.shutdown(Shutdown::Both);
                return;
            };
            let Ok(mut local_in) = stream.try_clone() else {
                return;
            };
            let outgoing = std::thread::spawn(move || {
                let _ = pump(&mut local_in, &mut remote_in);
            });
            let mut local_out = stream;
            let _ = pump(&mut remote_out, &mut local_out);
            let _ = local_out.shutdown(Shutdown::Both);
            let _ = outgoing.join();
        });
    }
}

/// Copies `from` to `to` until either ends.
fn pump(from: &mut dyn Read, to: &mut dyn Write) -> io::Result<()> {
    let mut bytes = [0; 8192];
    loop {
        let count = from.read(&mut bytes)?;
        if count == 0 {
            return Ok(());
        }
        to.write_all(&bytes[..count])?;
        to.flush()?;
    }
}
