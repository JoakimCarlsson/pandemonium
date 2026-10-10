//! Shared machine handles and the window's pool of SSH connections.

use crate::remote::Remote;
use crate::{Command, FileSystem, Location};
use std::collections::HashMap;
use std::io;
use std::sync::Arc;

/// Cheap machine identity shared by every project on the machine.
#[derive(Clone, Default)]
pub struct Host(pub(crate) Option<Arc<Remote>>);

impl Host {
    /// The current machine.
    pub fn local() -> Self {
        Self(None)
    }
    /// The SSH alias, for a remote machine.
    pub fn name(&self) -> Option<&str> {
        self.0.as_ref().map(|remote| remote.name.as_str())
    }
    /// Whether this machine is local.
    pub fn is_local(&self) -> bool {
        self.0.is_none()
    }
    /// Whether operations can currently reach the machine.
    pub fn connected(&self) -> bool {
        self.0.as_ref().is_none_or(|remote| remote.connected())
    }
    /// Whether SSH authentication or endpoint setup is currently running.
    pub fn connecting(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|remote| remote.connecting.load(std::sync::atomic::Ordering::Acquire))
    }

    /// Wakes the caller on connection-state changes without polling the transport.
    pub fn set_notify(&self, notify: Arc<dyn Fn() + Send + Sync>) {
        if let Some(remote) = &self.0 {
            *remote.notify.lock().unwrap() = Some(notify);
        }
    }

    /// Clears authentication progress after a login terminal fails or is cancelled.
    pub fn cancel_authentication(&self) {
        if let Some(remote) = &self.0 {
            remote
                .connecting
                .store(false, std::sync::atomic::Ordering::Release);
        }
    }

    /// Starts a command builder on this machine.
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        Command::on(self.clone(), program)
    }
    /// Borrows the filesystem boundary for this machine.
    pub fn fs(&self) -> FileSystem {
        FileSystem(self.clone())
    }
    /// Builds a local SSH terminal command for interactive authentication.
    pub fn authentication(&self) -> io::Result<crate::CommandBuilder> {
        self.0
            .as_ref()
            .ok_or_else(|| io::Error::other("Local hosts need no SSH login"))?
            .authentication()
    }
    /// Redials a disconnected remote, retaining all existing handles.
    pub fn reconnect(&self) -> io::Result<()> {
        match &self.0 {
            Some(remote) => remote.connect(),
            None => Ok(()),
        }
    }
    /// The operating system reported by the owning machine.
    pub fn os(&self) -> String {
        match &self.0 {
            Some(remote) => remote.information.lock().unwrap()["os"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned(),
            None => std::env::consts::OS.to_owned(),
        }
    }

    /// The home directory reported by the owning machine.
    pub fn home(&self) -> Option<std::path::PathBuf> {
        match &self.0 {
            Some(remote) => remote.information.lock().unwrap()["home"]
                .as_str()
                .map(std::path::PathBuf::from),
            None => std::env::home_dir(),
        }
    }

    /// Reads one environment variable on the owning machine.
    pub fn environment(&self, name: &str) -> Option<String> {
        match &self.0 {
            Some(remote) => remote
                .request("environment", serde_json::json!({"name":name}))
                .ok()?
                .as_str()
                .map(str::to_owned),
            None => std::env::var(name).ok(),
        }
    }

    /// Opens a loopback TCP stream on the owning machine.
    pub fn tcp(&self, port: u16) -> io::Result<(crate::Input, crate::command::Reader)> {
        match &self.0 {
            Some(remote) => remote.tcp(port),
            None => {
                let stream = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))?;
                Ok((Box::new(stream.try_clone()?), Box::new(stream)))
            }
        }
    }

    /// Finds a currently unused loopback port on the owning machine.
    pub fn free_port(&self) -> io::Result<u16> {
        match &self.0 {
            Some(remote) => {
                serde_json::from_value(remote.request("free_port", serde_json::json!({}))?)
                    .map_err(io::Error::other)
            }
            None => Ok(
                std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?
                    .local_addr()?
                    .port(),
            ),
        }
    }

    /// Finds an installed program on this machine.
    pub fn which(&self, program: &str) -> Option<std::path::PathBuf> {
        match &self.0 {
            Some(remote) => serde_json::from_value(
                remote
                    .request("which", serde_json::json!({"program": program}))
                    .ok()?,
            )
            .ok()?,
            None => crate::filesystem::which(program),
        }
    }
    /// Lists files on the machine, with the palette's ignore rules.
    pub fn walk(&self, root: &std::path::Path, found: impl FnMut(std::path::PathBuf) -> bool) {
        match &self.0 {
            None => crate::walk::walk_each(root, found),
            Some(remote) => {
                let _ = remote.walk(root, found);
            }
        }
    }
    /// Follows filesystem changes on the machine.
    pub fn watch(
        &self,
        root: &std::path::Path,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> crate::Watcher {
        match &self.0 {
            None => crate::Watcher::start(root, wake),
            Some(remote) => remote.watch(root, wake),
        }
    }
}

impl std::fmt::Debug for Host {
    /// Shows the machine identity without transport internals.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Host").field(&self.name()).finish()
    }
}
impl PartialEq for Host {
    /// Compares machine identities.
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}
impl Eq for Host {}

impl std::hash::Hash for Host {
    /// Hashes the same stable machine identity used by equality.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

/// One connection for every SSH alias held by the window.
#[derive(Default)]
pub struct Hosts {
    /// Reusable handles, including disconnected ones.
    hosts: HashMap<String, Host>,
}

impl Hosts {
    /// Reserves the shared host handle before its login terminal starts.
    pub fn prepare(&mut self, name: &str) -> io::Result<Host> {
        if let Some(host) = self.hosts.get(name) {
            return Ok(host.clone());
        }
        let host = Host(Some(Arc::new(Remote::new(name)?)));
        self.hosts.insert(name.to_owned(), host.clone());
        Ok(host)
    }
    /// Connects once to an SSH alias and performs the version handshake.
    pub fn connect(&mut self, name: &str) -> io::Result<Host> {
        let host = self.prepare(name)?;
        if !host.connected() {
            host.reconnect()?;
        }
        Ok(host)
    }
    /// Recovers machine identity and path even while the saved host is offline.
    pub fn location(&mut self, path: &std::path::Path) -> io::Result<Location> {
        let text = path.to_string_lossy();
        if let Some(remote) = text.strip_prefix("ssh://") {
            let (host, path) = remote
                .split_once('/')
                .ok_or_else(|| io::Error::other("invalid SSH location"))?;
            return Ok(Location::new(self.prepare(host)?, format!("/{path}")));
        }
        Ok(Location::local(path))
    }
    /// Restores a stored local path or SSH location.
    pub fn restore(&mut self, path: &std::path::Path) -> io::Result<Location> {
        let location = self.location(path)?;
        if !location.host.connected() {
            location.host.reconnect()?;
        }
        Ok(location)
    }
}
