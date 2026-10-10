//! Process startup and bounded recovery of Claude's npm-cached adapter.

use pm_host::{Child, Command, Input as ChildStdin};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::agent::{Agent, Source};
use crate::process::Containment;

/// The Claude package whose cached executable can be repaired.
const CLAUDE_PACKAGE: &str = "@agentclientprotocol/claude-agent-acp";

/// The maximum error-pipe tail retained for a session.
const TROUBLE: usize = 8 * 1024;

/// How long a failed startup may take to close its pipes and exit.
const EXIT_WITHIN: Duration = Duration::from_secs(1);

/// A launch that can recover once without holding up the window thread.
pub(super) struct Recovery {
    /// The configured command, reused with the same environment and worktree.
    command: Command,
    /// The session's current child process.
    process: Arc<Mutex<Option<Child>>>,
    /// The container owned by the session.
    containment: Arc<Mutex<Containment>>,
    /// The session's error-pipe tail.
    trouble: Arc<Mutex<String>>,
    /// Completes stderr capture before recognizing a startup failure.
    watcher: Receiver<()>,
    /// The original permission failure, retained if the retry also fails.
    original: Option<String>,
}

impl Recovery {
    /// Watches stderr and retains restart state only for Claude's npm fallback.
    pub(super) fn new(
        agent: Agent,
        command: Command,
        process: Arc<Mutex<Option<Child>>>,
        containment: Arc<Mutex<Containment>>,
        trouble: Arc<Mutex<String>>,
        stderr: Box<dyn std::io::Read + Send>,
    ) -> Option<Self> {
        let watcher = capture(stderr, trouble.clone());
        let recoverable = command.is_local()
            && cfg!(unix)
            && agent.source == Source::Package(CLAUDE_PACKAGE)
            && agent.program == "claude-agent-acp"
            && command.get_args().next() == Some(std::ffi::OsStr::new("--yes"));
        recoverable.then_some(Self {
            command,
            process,
            containment,
            trouble,
            watcher,
            original: None,
        })
    }

    /// Repairs a recognized startup failure and returns fresh pipes for one retry.
    pub(super) fn restart(
        &mut self,
    ) -> Option<(ChildStdin, BufReader<Box<dyn std::io::Read + Send>>)> {
        let restarted = self.retry();
        if restarted.is_none()
            && let Some(original) = &self.original
            && let Ok(mut trouble) = self.trouble.lock()
            && !trouble.ends_with(original)
        {
            trouble.push_str(original);
        }
        restarted
    }

    /// Waits briefly for startup to end, validates the failure, and replaces the child.
    fn retry(&mut self) -> Option<(ChildStdin, BufReader<Box<dyn std::io::Read + Send>>)> {
        self.watcher.recv_timeout(EXIT_WITHIN).ok()?;
        if self.original.is_some() {
            return None;
        }
        let failure = self.trouble.lock().ok()?.clone();
        let deadline = Instant::now() + EXIT_WITHIN;
        let status = loop {
            if let Some(status) = self.process.lock().ok()?.as_mut()?.try_wait().ok()? {
                break status;
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let mut process = self.process.lock().ok()?;
        process.as_ref()?;
        if status.code() != Some(126) || !repair(&failure) {
            return None;
        }
        self.original = Some(failure);
        let mut containment = self.containment.lock().ok()?;
        containment.kill();
        let mut child = self.command.spawn().ok()?;
        let group = match Containment::new(&child) {
            Ok(group) => group,
            Err(_) => {
                let _ = child.kill();
                std::thread::spawn(move || child.wait());
                return None;
            }
        };
        let stdin = child.stdin.take().expect("stdin was piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout was piped"));
        let stderr = child.stderr.take().expect("stderr was piped");
        *process = Some(child);
        *containment = group;
        self.watcher = capture(stderr, self.trouble.clone());
        Some((stdin, stdout))
    }

    /// Clears the repaired startup failure once the adapter speaks the protocol.
    pub(super) fn connected(&mut self) {
        if let Some(original) = self.original.take()
            && let Ok(mut trouble) = self.trouble.lock()
            && trouble.starts_with(&original)
        {
            trouble.drain(..original.len());
        }
    }
}

/// Restores execute bits only on the exact package entry point a cache symlink names.
#[cfg(unix)]
fn repair(failure: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    failure.lines().any(|line| {
        let Some(path) = line.strip_suffix(": Permission denied") else {
            return false;
        };
        let Some((_, path)) = path.rsplit_once(": ") else {
            return false;
        };
        let bin = Path::new(path);
        if !bin.is_absolute() || !bin.ends_with("node_modules/.bin/claude-agent-acp") {
            return false;
        }
        let Some(modules) = bin.parent().and_then(Path::parent) else {
            return false;
        };
        let Some(cache) = modules.parent().and_then(Path::parent) else {
            return false;
        };
        if cache.file_name() != Some(std::ffi::OsStr::new("_npx")) {
            return false;
        }
        let entry = modules.join(CLAUDE_PACKAGE).join("dist/index.js");
        let Ok(target) = bin.canonicalize() else {
            return false;
        };
        if !bin.is_symlink() || target != entry || !target.is_file() {
            return false;
        }
        let Ok(file) = std::fs::File::open(&target) else {
            return false;
        };
        let Ok(metadata) = file.metadata() else {
            return false;
        };
        let mode = metadata.permissions().mode();
        if mode & 0o111 != 0 {
            return false;
        }
        file.set_permissions(std::fs::Permissions::from_mode(
            mode | ((mode & 0o444) >> 2),
        ))
        .is_ok()
    })
}

/// Leaves platforms without Unix execute bits unchanged.
#[cfg(not(unix))]
fn repair(_failure: &str) -> bool {
    false
}

/// Captures stderr independently and signals when the pipe closes.
fn capture(stderr: Box<dyn std::io::Read + Send>, trouble: Arc<Mutex<String>>) -> Receiver<()> {
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        watch(BufReader::new(stderr), &trouble);
        let _ = done.send(());
    });
    finished
}

/// Keeps a bounded tail of the agent's error pipe.
fn watch(stderr: impl BufRead, trouble: &Mutex<String>) {
    for line in stderr.lines().map_while(Result::ok) {
        let Ok(mut trouble) = trouble.lock() else {
            return;
        };
        trouble.push_str(&line);
        trouble.push('\n');
        if trouble.len() > TROUBLE {
            let over = trouble.len() - TROUBLE;
            let from = trouble
                .char_indices()
                .map(|(at, _)| at)
                .find(|at| *at >= over)
                .unwrap_or(trouble.len());
            *trouble = trouble.split_off(from);
        }
    }
}
