//! A nonblocking editor-side transport to the reference Jupyter client.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::Scope;

/// The reference-client bridge bundled with the editor.
const BRIDGE: &str = include_str!("bridge.py");

/// Maximum events queued between the transport and the window.
const EVENT_CAPACITY: usize = 256;

/// Maximum events folded into one window update.
const EVENT_BATCH: usize = 128;

/// One asynchronous bridge owned by a notebook's project and worktree.
pub struct Kernel {
    /// The owning project and worktree, never inferred from the active window.
    pub scope: Scope,
    /// Commands queued for the transport thread.
    commands: Sender<Value>,
    /// Protocol events awaiting the window's next wake.
    events: Receiver<Value>,
    /// Requests another update when a bounded batch leaves events waiting.
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Kernel {
    /// Starts discovery in the worktree without starting or executing a kernel.
    pub fn discover(scope: Scope, root: &Path, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (commands, receive) = mpsc::channel();
        let (send, events) = mpsc::sync_channel(EVENT_CAPACITY);
        let notify = wake.clone();
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            if let Err(message) = bridge(&root, receive, &send, &notify) {
                let _ = send.send(json!({"type": "failure", "message": message}));
                notify();
            }
        });
        let kernel = Self {
            scope,
            commands,
            events,
            wake,
        };
        let _ = kernel.send(json!({"action": "discover"}));
        kernel
    }

    /// Queues a command without waiting for the child or a channel reply.
    pub fn send(&self, command: Value) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "Jupyter transport closed. Refresh kernels to reconnect.".into())
    }

    /// Drains events already received, never waiting for kernel work.
    pub fn drain(&self) -> Vec<Value> {
        let events = self.events.try_iter().take(EVENT_BATCH).collect::<Vec<_>>();
        if events.len() == EVENT_BATCH {
            (self.wake)();
        }
        events
    }
}

impl Drop for Kernel {
    /// Requests shutdown when the notebook's last tab or its project closes.
    fn drop(&mut self) {
        let _ = self.send(json!({"action": "quit"}));
    }
}

/// Chooses a worktree environment first, then an installed Python with Jupyter.
fn python(root: &Path) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    for environment in [".venv", "venv"] {
        candidates.push(root.join(environment).join(if cfg!(windows) {
            "Scripts/python.exe"
        } else {
            "bin/python"
        }));
    }
    candidates.extend([PathBuf::from("python3"), PathBuf::from("python")]);
    for candidate in candidates {
        if Command::new(&candidate)
            .args(["-c", "import jupyter_client"])
            .current_dir(root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return Ok(candidate);
        }
    }
    Err("Jupyter client is missing. Install it in this worktree: python -m pip install jupyter_client ipykernel; then refresh kernels.".into())
}

/// Launches the bridge and forwards commands and events on background threads.
fn bridge(
    root: &Path,
    commands: Receiver<Value>,
    events: &mpsc::SyncSender<Value>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> Result<(), String> {
    let mut child = Command::new(python(root)?)
        .args(["-u", "-c", BRIDGE])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not start Jupyter: {error}"))?;
    let mut input = child.stdin.take().ok_or("Jupyter has no input pipe")?;
    let output = child.stdout.take().ok_or("Jupyter has no output pipe")?;
    let errors = child
        .stderr
        .take()
        .ok_or("Jupyter has no diagnostic pipe")?;
    let diagnostics = Arc::new(Mutex::new(String::new()));
    let captured = diagnostics.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(errors).lines().map_while(Result::ok) {
            if let Ok(mut captured) = captured.lock() {
                if captured.len() > 8192 {
                    captured.clear();
                }
                captured.push_str(&line);
                captured.push('\n');
            }
        }
    });
    let received = events.clone();
    let notify = wake.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if let Ok(event) = serde_json::from_str(&line) {
                let _ = received.send(event);
                notify();
            }
        }
    });
    let mut quitting = None;
    let result = loop {
        match commands.recv_timeout(Duration::from_millis(50)) {
            Ok(command) => {
                if command["action"] == "quit" {
                    quitting = Some(Instant::now());
                }
                if let Err(error) = writeln!(input, "{command}").and_then(|()| input.flush()) {
                    break Err(format!(
                        "Jupyter transport failed: {error}. Refresh kernels to reconnect."
                    ));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if quitting.is_none() {
                    let _ = writeln!(input, "{}", json!({"action": "quit"}));
                    let _ = input.flush();
                    quitting = Some(Instant::now());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if quitting.is_some() {
                    break Ok(());
                }
                let detail = diagnostics
                    .lock()
                    .map(|text| text.clone())
                    .unwrap_or_default();
                break Err(format!(
                    "Jupyter exited ({status}). {detail} Refresh kernels to reconnect."
                ));
            }
            Err(error) => break Err(error.to_string()),
            _ => {}
        }
        if quitting.is_some_and(|since| since.elapsed() > Duration::from_secs(35)) {
            break Ok(());
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    result
}
