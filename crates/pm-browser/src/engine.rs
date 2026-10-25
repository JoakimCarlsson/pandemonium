//! Starting the browser helper and the connection to it.

use std::io::BufReader;
use std::path::PathBuf;
use std::process::Child;
use std::sync::Arc;

use crate::error::Error;
use crate::install;
use crate::link::Link;
use crate::wire::Request;

/// The running browser helper; pages are opened in it as [`crate::View`]s.
///
/// Dropping it closes every page and ends the helper.
pub struct Engine {
    /// The connection to the helper.
    link: Arc<Link>,
    /// The helper's process.
    child: Child,
    /// The folder the helper's control socket and shared files are in.
    folder: PathBuf,
}

impl Engine {
    /// Starts the helper; fails when the browser is not installed.
    #[cfg(unix)]
    pub fn start() -> Result<Self, Error> {
        use std::os::unix::fs::DirBuilderExt;
        use std::os::unix::net::UnixListener;
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        let helper = install::helper().ok_or(Error::NotInstalled)?;
        let fail = |what: &dyn std::fmt::Display| Error::Helper(what.to_string());
        let root = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .or_else(|| pm_store::data_directory().map(|data| data.join("run")))
            .ok_or_else(|| fail(&"this machine has no runtime folder"))?;
        let folder = root.join(format!("pandemonium-browser-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&folder)
            .map_err(|error| fail(&error))?;
        let socket = folder.join("control.sock");
        let listener = UnixListener::bind(&socket).map_err(|error| fail(&error))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| fail(&error))?;
        let profile = install::profile().ok_or_else(|| fail(&"this machine has no data folder"))?;
        let mut child = Command::new(&helper)
            .arg("--control")
            .arg(&socket)
            .arg("--surfaces")
            .arg(&folder)
            .arg("--profile")
            .arg(&profile)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|error| fail(&error))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(_) if child.try_wait().ok().flatten().is_some() => {
                    let _ = std::fs::remove_dir_all(&folder);
                    return Err(fail(&"the helper exited while starting"));
                }
                Err(_) if Instant::now() > deadline => {
                    let _ = child.kill();
                    let _ = std::fs::remove_dir_all(&folder);
                    return Err(fail(&"the helper did not start in time"));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        };
        stream
            .set_nonblocking(false)
            .map_err(|error| fail(&error))?;
        let reader = stream.try_clone().map_err(|error| fail(&error))?;
        let link = Link::new(Box::new(stream));
        let listening = link.clone();
        std::thread::spawn(move || listening.listen(Box::new(BufReader::new(reader))));
        Ok(Self {
            link,
            child,
            folder,
        })
    }

    /// Starts the helper; fails everywhere the helper is not built for.
    #[cfg(not(unix))]
    pub fn start() -> Result<Self, Error> {
        Err(Error::Helper(
            "the embedded browser is not available on this system yet".to_owned(),
        ))
    }

    /// The connection pages talk through.
    pub(crate) fn link(&self) -> Arc<Link> {
        self.link.clone()
    }

    /// Why the helper failed, if it did.
    pub fn failure(&self) -> Option<String> {
        self.link.failure()
    }
}

impl Drop for Engine {
    /// Ends the helper and removes its files.
    fn drop(&mut self) {
        self.link.send(&Request::Shutdown);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}
