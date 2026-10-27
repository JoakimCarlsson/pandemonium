//! The commands an agent asks the app to run in the dev container of its session: the agent itself stays on this machine with its login, and what it executes runs inside the container.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::chat::Enclose;
use crate::client::Client;

/// The error code for a request whose parameters are wrong.
const INVALID_PARAMS: i64 = -32602;

/// Where the commands of a session run: the folder the agent works in on this machine, the folder that is in the container, and how a command is wrapped to run there.
pub(crate) struct Containment {
    /// The folder the agent works in on this machine.
    pub(crate) host: PathBuf,
    /// The same folder as the container sees it.
    pub(crate) workspace: PathBuf,
    /// Wraps a command so that it runs in the container.
    pub(crate) enclose: Enclose,
}

/// One command the agent started.
struct Run {
    /// The process that carries the command out.
    child: Child,
    /// What the command has written so far, its errors included.
    output: Arc<Mutex<Vec<u8>>>,
    /// How many bytes of the end of the output the agent wants to see, when it said.
    limit: Option<usize>,
}

/// The commands an agent started in the container, and the requests waiting for one to end.
pub(crate) struct Terminals {
    /// Where the commands run, or nothing when the session is not in a container.
    containment: Option<Containment>,
    /// The commands, by the name the agent was given for each.
    running: HashMap<String, Run>,
    /// The requests to wait for a command to end, as the id the agent asked under and the command's name.
    waiting: Vec<(Value, String)>,
    /// The number the next command is named by.
    next: u64,
}

impl Terminals {
    /// The commands of a session that runs them in `containment`, or on the agent's own when there is none.
    pub(crate) fn new(containment: Option<Containment>) -> Self {
        Self {
            containment,
            running: HashMap::new(),
            waiting: Vec::new(),
            next: 0,
        }
    }

    /// Whether this serves the terminal requests of the agent.
    pub(crate) fn serves(&self) -> bool {
        self.containment.is_some()
    }

    /// Carries out the terminal request `method` the agent made under `id`, answering it now or, for a wait, when the command ends.
    pub(crate) fn serve(
        &mut self,
        client: &mut Client,
        id: &Value,
        method: &str,
        params: &Value,
    ) -> Result<(), String> {
        let answered = match method {
            "terminal/create" => self.create(params),
            "terminal/output" => self.output(params),
            "terminal/wait_for_exit" => {
                let name = named(params)?;
                if !self.running.contains_key(&name) {
                    return client.refuse(id, INVALID_PARAMS, "no such terminal");
                }
                self.waiting.push((id.clone(), name));
                return Ok(());
            }
            "terminal/kill" => self.end(params, false),
            "terminal/release" => self.end(params, true),
            _ => return client.refuse(id, -32601, "not supported"),
        };
        match answered {
            Ok(result) => client.respond(id, &result),
            Err(reason) => client.refuse(id, INVALID_PARAMS, &reason),
        }
    }

    /// Answers the waits whose command has ended.
    pub(crate) fn poll(&mut self, client: &mut Client) -> Result<(), String> {
        let mut index = 0;
        while index < self.waiting.len() {
            let name = self.waiting[index].1.clone();
            let ended = match self.running.get_mut(&name) {
                Some(run) => exit_of(run),
                None => Some(json!({ "exitCode": null, "signal": null })),
            };
            match ended {
                Some(status) => {
                    let (id, _) = self.waiting.remove(index);
                    client.respond(&id, &status)?;
                }
                None => index += 1,
            }
        }
        Ok(())
    }

    /// Starts the command `params` describes in the container and names it.
    fn create(&mut self, params: &Value) -> Result<Value, String> {
        let containment = self.containment.as_ref().ok_or("not in a container")?;
        let program = params["command"].as_str().ok_or("no command")?;
        let mut command = Command::new(program);
        command.args(
            params["args"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        );
        for variable in params["env"].as_array().into_iter().flatten() {
            if let (Some(name), Some(value)) =
                (variable["name"].as_str(), variable["value"].as_str())
            {
                command.env(name, value);
            }
        }
        command.current_dir(inside(containment, params["cwd"].as_str()));
        let mut wrapped = (containment.enclose)(command);
        wrapped
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = wrapped
            .spawn()
            .map_err(|error| format!("could not run {program}: {error}"))?;
        let output = Arc::new(Mutex::new(Vec::new()));
        if let Some(stream) = child.stdout.take() {
            collect(stream, Arc::clone(&output));
        }
        if let Some(stream) = child.stderr.take() {
            collect(stream, Arc::clone(&output));
        }
        self.next += 1;
        let name = format!("container-{}", self.next);
        self.running.insert(
            name.clone(),
            Run {
                child,
                output,
                limit: params["outputByteLimit"]
                    .as_u64()
                    .map(|limit| limit as usize),
            },
        );
        Ok(json!({ "terminalId": name }))
    }

    /// What the command `params` names has written so far, and how it ended if it has.
    fn output(&mut self, params: &Value) -> Result<Value, String> {
        let run = self
            .running
            .get_mut(&named(params)?)
            .ok_or("no such terminal")?;
        let status = exit_of(run);
        let kept = run
            .output
            .lock()
            .map(|kept| kept.clone())
            .unwrap_or_default();
        let (text, truncated) = tail(&kept, run.limit);
        Ok(json!({
            "output": text,
            "truncated": truncated,
            "exitStatus": status,
        }))
    }

    /// Stops the command `params` names and, when `release` is set, forgets it.
    fn end(&mut self, params: &Value, release: bool) -> Result<Value, String> {
        let name = named(params)?;
        let run = self.running.get_mut(&name).ok_or("no such terminal")?;
        let _ = run.child.kill();
        if release {
            self.running.remove(&name);
        }
        Ok(json!({}))
    }
}

impl Drop for Terminals {
    /// Stops every command that is still running.
    fn drop(&mut self) {
        for run in self.running.values_mut() {
            let _ = run.child.kill();
        }
    }
}

/// The name of the command a request is about.
fn named(params: &Value) -> Result<String, String> {
    params["terminalId"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "no terminal".to_owned())
}

/// The folder in the container that the folder `cwd`, on this machine, is, or the workspace when it is none inside the agent's.
fn inside(containment: &Containment, cwd: Option<&str>) -> PathBuf {
    cwd.map(Path::new)
        .and_then(|cwd| cwd.strip_prefix(&containment.host).ok())
        .map_or_else(
            || containment.workspace.clone(),
            |relative| containment.workspace.join(relative),
        )
}

/// How the command ended, or nothing while it runs.
fn exit_of(run: &mut Run) -> Option<Value> {
    run.child
        .try_wait()
        .ok()
        .flatten()
        .map(|status| json!({ "exitCode": status.code(), "signal": null }))
}

/// Reads `stream` to its end on a thread of its own, adding what it holds to `output` as it comes.
fn collect(mut stream: impl Read + Send + 'static, output: Arc<Mutex<Vec<u8>>>) {
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(count) = stream.read(&mut chunk) {
            if count == 0 {
                break;
            }
            if let Ok(mut kept) = output.lock() {
                kept.extend_from_slice(&chunk[..count]);
            }
        }
    });
}

/// The end of `bytes` that fits in `limit`, as text, and whether any was cut.
fn tail(bytes: &[u8], limit: Option<usize>) -> (String, bool) {
    let text = String::from_utf8_lossy(bytes);
    let Some(limit) = limit.filter(|limit| text.len() > *limit) else {
        return (text.into_owned(), false);
    };
    let from = (text.len() - limit..=text.len())
        .find(|at| text.is_char_boundary(*at))
        .unwrap_or(text.len());
    (text[from..].to_owned(), true)
}
