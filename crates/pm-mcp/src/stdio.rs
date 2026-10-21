//! A tool server started as a program and spoken to line by line over its standard streams.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio as Piped};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::listing::{Exchange, result_of};

/// How long a server has to answer, which is long enough for a runner to fetch it first.
const PATIENCE: Duration = Duration::from_secs(60);

/// A running server and what it wrote.
pub(crate) struct Stdio {
    /// The server's process, ended when this is dropped.
    child: Child,
    /// What the server is written to.
    input: ChildStdin,
    /// The messages the server wrote, one per line.
    lines: Receiver<Value>,
    /// The id the next request is sent under.
    next: u64,
}

impl Stdio {
    /// Starts `program` with `args`, and `env` over the environment it inherits.
    pub(crate) fn start(
        program: &str,
        args: &[String],
        env: &[(String, String)],
    ) -> Result<Self, String> {
        let found = pm_tools::installed(program);
        let mut command = Command::new(found.clone().unwrap_or_else(|| program.into()));
        if let Some(found) = &found {
            command.env("PATH", pm_tools::path_beside(found));
        }
        let mut child = command
            .args(args)
            .envs(env.iter().map(|(name, value)| (name, value)))
            .stdin(Piped::piped())
            .stdout(Piped::piped())
            .stderr(Piped::null())
            .spawn()
            .map_err(|error| format!("could not start {program}: {error}"))?;
        let input = child.stdin.take().ok_or("no input pipe")?;
        let output = child.stdout.take().ok_or("no output pipe")?;
        let (sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if let Ok(message) = serde_json::from_str::<Value>(&line)
                    && sender.send(message).is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input,
            lines,
            next: 1,
        })
    }

    /// Writes `message` to the server as one line.
    fn write(&mut self, message: &Value) -> Result<(), String> {
        writeln!(self.input, "{message}")
            .and_then(|()| self.input.flush())
            .map_err(|error| format!("the server stopped: {error}"))
    }
}

impl Exchange for Stdio {
    /// Sends the request and waits for the answer that carries its id.
    fn ask(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = self
                .lines
                .recv_timeout(left)
                .map_err(|_| "the server did not answer".to_owned())?;
            if message["id"].as_u64() == Some(id) {
                return result_of(&message);
            }
        }
    }

    /// Sends the notification.
    fn tell(&mut self, method: &str) -> Result<(), String> {
        self.write(&json!({ "jsonrpc": "2.0", "method": method }))
    }
}

impl Drop for Stdio {
    /// Ends the server's process.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
