//! Same-user local control socket and terminal client for SSH access.

use std::sync::mpsc;

#[cfg(not(unix))]
use winit::event_loop::EventLoopProxy;

#[cfg(not(unix))]
use crate::app::Wake;

/// A command waiting for the window and the channel its answer returns on.
pub struct Request {
    /// One line of terminal command text.
    pub line: String,
    /// Where the window sends its answer.
    pub answer: mpsc::Sender<Result<String, String>>,
}

#[cfg(unix)]
mod unix {
    use std::fs::{self, DirBuilder};
    use std::io::{self, BufRead, BufReader, Read, Write};
    use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    use winit::event_loop::EventLoopProxy;

    use crate::app::Wake;
    use crate::config;
    use crate::control::Request;

    /// A private socket and commands waiting for the editor event loop.
    pub struct Server {
        /// The socket removed when this window closes.
        path: PathBuf,
        /// Commands received by the listener thread.
        pending: Arc<Mutex<Vec<Request>>>,
    }

    impl Server {
        /// Opens the private local socket and starts accepting control commands.
        pub fn start(proxy: EventLoopProxy<Wake>) -> io::Result<Self> {
            let path = socket_path()?;
            if path.exists() {
                if UnixStream::connect(&path).is_ok() {
                    return Err(io::Error::new(
                        io::ErrorKind::AddrInUse,
                        "editor already running",
                    ));
                }
                if !fs::symlink_metadata(&path)?.file_type().is_socket() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "control path is not a socket",
                    ));
                }
                fs::remove_file(&path)?;
            }
            let listener = UnixListener::bind(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            let pending = Arc::new(Mutex::new(Vec::new()));
            let queue = pending.clone();
            std::thread::spawn(move || {
                for connection in listener.incoming().flatten() {
                    let queue = queue.clone();
                    let proxy = proxy.clone();
                    std::thread::spawn(move || serve(connection, queue, proxy));
                }
            });
            Ok(Self { path, pending })
        }

        /// Takes commands for the window to run on its own event loop.
        pub fn take(&self) -> Vec<Request> {
            self.pending
                .lock()
                .map(|mut queue| std::mem::take(&mut *queue))
                .unwrap_or_default()
        }
    }

    impl Drop for Server {
        /// Removes this window's socket when the event loop exits.
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    /// Receives one bounded command and waits for its result from the window.
    fn serve(
        mut stream: UnixStream,
        pending: Arc<Mutex<Vec<Request>>>,
        proxy: EventLoopProxy<Wake>,
    ) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
        let mut line = String::new();
        let result = BufReader::new((&stream).take(65_537)).read_line(&mut line);
        let answer = match result {
            Ok(size) if size > 0 && size <= 65_536 && line.ends_with('\n') => {
                let (tx, rx) = mpsc::channel();
                if let Ok(mut queue) = pending.lock() {
                    queue.push(Request {
                        line: line.trim_end_matches(['\r', '\n']).to_owned(),
                        answer: tx,
                    });
                    let _ = proxy.send_event(Wake::Control);
                    rx.recv_timeout(Duration::from_secs(10))
                        .unwrap_or_else(|_| Err("editor did not answer".to_owned()))
                } else {
                    Err("control queue unavailable".to_owned())
                }
            }
            _ => Err("invalid or oversized command".to_owned()),
        };
        let _ = serde_json::to_writer(&mut stream, &answer);
        let _ = stream.write_all(b"\n");
    }

    /// Returns the private socket path, creating its owner-only directory.
    fn socket_path() -> io::Result<PathBuf> {
        let home = config::home()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "editor home unavailable"))?;
        let directory = home.join("control");
        if !directory.exists() {
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&directory)?;
        }
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.is_dir()
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.uid() != fs::metadata(&home)?.uid()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "control directory must be private",
            ));
        }
        Ok(directory.join("editor.sock"))
    }

    /// Sends a command to the running editor and returns its text or error.
    fn send(line: &str) -> Result<String, String> {
        let path = socket_path().map_err(|error| error.to_string())?;
        let mut stream =
            UnixStream::connect(path).map_err(|error| format!("editor unavailable: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(12)))
            .map_err(|error| error.to_string())?;
        stream
            .write_all(format!("{line}\n").as_bytes())
            .map_err(|error| error.to_string())?;
        let mut answer = String::new();
        BufReader::new(stream)
            .read_line(&mut answer)
            .map_err(|error| error.to_string())?;
        serde_json::from_str::<Result<String, String>>(&answer)
            .map_err(|error| error.to_string())?
    }

    /// Runs one command, or reads commands interactively from the SSH terminal.
    pub fn client(args: Vec<String>) {
        if !args.is_empty() {
            match send(&args.join(" ")) {
                Ok(text) => println!("{text}"),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
        println!("Pandemonium control. Type help for commands, quit to leave.");
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if matches!(line.trim(), "quit" | "exit") {
                break;
            }
            match send(&line) {
                Ok(text) => println!("{text}"),
                Err(error) => eprintln!("{error}"),
            }
        }
    }
}

#[cfg(unix)]
pub use unix::{Server, client};

#[cfg(not(unix))]
/// No Unix control socket exists on this platform.
pub struct Server;

#[cfg(not(unix))]
impl Server {
    /// Reports that the local SSH control socket is unavailable here.
    pub fn start(_proxy: EventLoopProxy<Wake>) -> std::io::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "SSH control requires Unix",
        ))
    }

    /// No commands can arrive on this platform.
    pub fn take(&self) -> Vec<Request> {
        Vec::new()
    }
}

#[cfg(not(unix))]
/// Reports that the terminal control client needs a Unix host.
pub fn client(_args: Vec<String>) {
    eprintln!("SSH control requires Unix");
    std::process::exit(1);
}
