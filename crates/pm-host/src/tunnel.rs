//! Scoped SSH reverse forwards for services belonging to the desktop editor.

use crate::{Host, remote::Remote};
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

/// A loopback listener on a remote host forwarding to one local service.
pub struct Tunnel {
    /// The local service receiving the forwarded connections.
    local: u16,
    /// The loopback port allocated on the remote host.
    remote: u16,
    /// The authenticated master owning this forwarding rule.
    control: PathBuf,
    /// Keeps the owning SSH master alive while the forwarding rule is used.
    host: Arc<Remote>,
}

impl Tunnel {
    /// The loopback port a remote process connects to.
    pub fn port(&self) -> u16 {
        self.remote
    }
}

impl Host {
    /// Forwards a local service to a remote loopback listener for its caller's lifetime.
    pub fn forward_local(&self, port: u16) -> io::Result<Arc<Tunnel>> {
        let host = self
            .0
            .as_ref()
            .ok_or_else(|| io::Error::other("Local services need no SSH forwarding"))?;
        let control = host.control.lock().unwrap().clone().ok_or_else(|| {
            io::Error::other("SSH forwarding needs an authenticated control master")
        })?;
        let mut forwards = host.forwards.lock().unwrap();
        if let Some(tunnel) = forwards.get(&port).and_then(std::sync::Weak::upgrade)
            && tunnel.control == control
        {
            return Ok(tunnel);
        }
        let output = Command::new("ssh")
            .arg("-S")
            .arg(&control)
            .args(["-O", "forward", "-R"])
            .arg(format!("127.0.0.1:0:127.0.0.1:{port}"))
            .arg(&host.name)
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let remote = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .map_err(io::Error::other)?;
        let tunnel = Arc::new(Tunnel {
            local: port,
            remote,
            control,
            host: host.clone(),
        });
        forwards.insert(port, Arc::downgrade(&tunnel));
        Ok(tunnel)
    }
}

impl Drop for Tunnel {
    /// Removes only this forwarding rule when its last conversation has closed.
    fn drop(&mut self) {
        let control = self.control.clone();
        let name = self.host.name.clone();
        let forward = format!("127.0.0.1:{}:127.0.0.1:{}", self.remote, self.local);
        std::thread::spawn(move || {
            let _ = Command::new("ssh")
                .arg("-S")
                .arg(control)
                .args(["-O", "cancel", "-R"])
                .arg(forward)
                .arg(name)
                .output();
        });
    }
}
