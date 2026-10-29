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
    /// The folder the helper's shared files are in.
    folder: PathBuf,
}

/// Where the helper's own messages go: a file beside its profile, so that Chromium's noise does not fill the terminal and is still there to read.
fn log_file(profile: &std::path::Path) -> std::process::Stdio {
    let path = profile.with_file_name("browser.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::File::create(path)
        .map_or_else(|_| std::process::Stdio::null(), std::process::Stdio::from)
}

/// Creates `folder` and its parents, readable by this user alone where the system has such permissions.
fn private_folder(folder: &std::path::Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(folder)
}

/// A secret the helper proves it is the helper with, as the first line it sends.
fn token() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Waits for the helper to connect to `listener` and send `token`, ignoring anything else that connects.
fn accept_helper(
    listener: &std::net::TcpListener,
    token: &str,
    child: &mut Child,
    folder: &std::path::Path,
) -> Result<(std::net::TcpStream, BufReader<std::net::TcpStream>), Error> {
    use std::io::BufRead;
    use std::time::{Duration, Instant};

    let fail = |what: &dyn std::fmt::Display| Error::Helper(what.to_string());
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .map_err(|error| fail(&error))?;
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .map_err(|error| fail(&error))?;
                let mut reader = BufReader::new(stream.try_clone().map_err(|error| fail(&error))?);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() && line.trim_end() == token {
                    stream
                        .set_read_timeout(None)
                        .map_err(|error| fail(&error))?;
                    return Ok((stream, reader));
                }
            }
            Err(_) if child.try_wait().ok().flatten().is_some() => {
                let _ = std::fs::remove_dir_all(folder);
                return Err(fail(&"the helper exited while starting"));
            }
            Err(_) if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = std::fs::remove_dir_all(folder);
                return Err(fail(&"the helper did not start in time"));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

impl Engine {
    /// Starts the helper and waits for it to connect over a loopback port that only it, holding the token, may use; fails when the browser is not installed.
    pub fn start() -> Result<Self, Error> {
        use std::net::{Ipv4Addr, TcpListener};
        use std::process::{Command, Stdio};

        let helper = install::helper().ok_or(Error::NotInstalled)?;
        let fail = |what: &dyn std::fmt::Display| Error::Helper(what.to_string());
        let root = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .or_else(|| pm_store::data_directory().map(|data| data.join("run")))
            .ok_or_else(|| fail(&"this machine has no runtime folder"))?;
        let folder = root.join(format!("pandemonium-browser-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        private_folder(&folder).map_err(|error| fail(&error))?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|error| fail(&error))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| fail(&error))?;
        let address = listener.local_addr().map_err(|error| fail(&error))?;
        let secret = token().map_err(|error| fail(&error))?;
        let profile = install::profile().ok_or_else(|| fail(&"this machine has no data folder"))?;
        let mut child = Command::new(&helper)
            .arg("--control")
            .arg(address.to_string())
            .arg("--surfaces")
            .arg(&folder)
            .arg("--profile")
            .arg(&profile)
            .env("PANDEMONIUM_BROWSER_TOKEN", &secret)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log_file(&profile))
            .spawn()
            .map_err(|error| fail(&error))?;
        let (stream, reader) = accept_helper(&listener, &secret, &mut child, &folder)?;
        let link = Link::new(Box::new(stream));
        let listening = link.clone();
        std::thread::spawn(move || listening.listen(Box::new(reader)));
        Ok(Self {
            link,
            child,
            folder,
        })
    }

    /// The connection pages talk through.
    pub(crate) fn link(&self) -> Arc<Link> {
        self.link.clone()
    }

    /// Tells the helper which origins the tools of agents may take a page to: when `restrict`, only local addresses and `origins`.
    pub fn set_agent_origins(&self, restrict: bool, origins: &[String]) {
        self.link.send(&Request::AgentOrigins {
            restrict,
            origins: origins.to_vec(),
        });
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
