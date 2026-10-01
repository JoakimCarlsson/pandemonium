//! The machine endpoint: filesystem controls and multiplexed byte streams.

use crate::wire::{self, Frame, Reply, Request};
use crate::{Child, Command, CommandBuilder, Host, PtyChild, PtyControl, Stdio, Watcher};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};

/// A serialized writer shared by endpoint workers.
type Output = Arc<Mutex<Box<dyn Write + Send>>>;

/// Holds output workers until the request has been acknowledged.
type Gate = Arc<(Mutex<bool>, Condvar)>;

/// Endpoint resources retained for the connection's lifetime.
#[derive(Default)]
struct State {
    /// Spawned pipe processes.
    children: Mutex<HashMap<u32, Arc<Mutex<Child>>>>,
    /// Credits for every outgoing stream.
    flows: Mutex<HashMap<u32, Arc<crate::flow::Flow>>>,
    /// Spawned terminal processes.
    ptys: Mutex<HashMap<u32, Arc<Mutex<PtyChild>>>>,
    /// Terminal resize handles.
    controls: Mutex<HashMap<u32, PtyControl>>,
    /// Live filesystem watchers.
    watchers: Mutex<HashMap<u32, Arc<Watcher>>>,
}

/// Serves host operations until the owning transport closes.
pub fn serve(mut input: impl Read, output: impl Write + Send + 'static) -> io::Result<()> {
    let output: Output = Arc::new(Mutex::new(Box::new(output)));
    let state = Arc::new(State::default());
    let mut inputs: HashMap<u32, SyncSender<Frame>> = HashMap::new();
    let result = (|| {
        while let Some(frame) = Frame::read(&mut input)? {
            if frame.kind == wire::REQUEST {
                let request: Request = serde_json::from_slice(&frame.payload)?;
                let streams = match request.op.as_str() {
                    "read" => vec![frame.channel],
                    "walk" | "watch" => vec![channel(&request.args, "channel")?],
                    "spawn" => vec![
                        channel(&request.args, "output")?,
                        channel(&request.args, "error")?,
                    ],
                    "pty" => vec![channel(&request.args, "output")?],
                    _ => Vec::new(),
                };
                for channel in streams {
                    state
                        .flows
                        .lock()
                        .unwrap()
                        .insert(channel, Arc::new(crate::flow::Flow::new()));
                }
                let raw = match request.op.as_str() {
                    "write" => Some(frame.channel),
                    "spawn" | "pty" => Some(channel(&request.args, "input")?),
                    _ => None,
                };
                let receiver = raw.map(|channel| {
                    let (sender, receiver) = mpsc::sync_channel(512);
                    inputs.insert(channel, sender);
                    receiver
                });
                let output = output.clone();
                let state = state.clone();
                std::thread::spawn(move || {
                    let gate = Arc::new((Mutex::new(false), Condvar::new()));
                    let result = dispatch(
                        &state,
                        &output,
                        frame.channel,
                        request,
                        receiver,
                        gate.clone(),
                    );
                    let reply = match result {
                        Ok(value) => Reply { value, error: None },
                        Err(error) => Reply {
                            value: Value::Null,
                            error: Some((format!("{:?}", error.kind()), error.to_string())),
                        },
                    };
                    if let Ok(frame) = Frame::json(wire::REPLY, frame.channel, &reply) {
                        let _ = send(&output, frame);
                    }
                    *gate.0.lock().unwrap() = true;
                    gate.1.notify_all();
                });
            } else if frame.kind == wire::CREDIT {
                if let Some(flow) = state.flows.lock().unwrap().get(&frame.channel) {
                    flow.give();
                }
            } else if matches!(frame.kind, wire::BYTES | wire::EOF) {
                let ended = frame.kind == wire::EOF;
                let channel = frame.channel;
                if let Some(sender) = inputs.get(&channel) {
                    let _ = sender.send(frame);
                }
                if ended {
                    inputs.remove(&channel);
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unexpected host frame",
                ));
            }
        }
        Ok(())
    })();
    drop(inputs);
    for flow in state.flows.lock().unwrap().values() {
        flow.close();
    }
    state.watchers.lock().unwrap().clear();
    for child in state.children.lock().unwrap().values() {
        let mut child = child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }
    for child in state.ptys.lock().unwrap().values() {
        let mut child = child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

/// Runs one operating system request without knowledge of editor features.
fn dispatch(
    state: &Arc<State>,
    output: &Output,
    request_channel: u32,
    request: Request,
    input: Option<Receiver<Frame>>,
    gate: Gate,
) -> io::Result<Value> {
    let args = request.args;
    let fs = Host::local().fs();
    match request.op.as_str() {
        "hello" => Ok(
            json!({"version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"home":std::env::home_dir()}),
        ),
        "which" => Ok(json!(
            Host::local().which(
                args["program"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("missing program"))?
            )
        )),
        "read" => {
            let file = std::fs::File::open(path(&args, "path")?)?;
            pipe_out(
                Box::new(file),
                output.clone(),
                request_channel,
                gate,
                state.clone(),
            );
            Ok(Value::Null)
        }
        "write" => {
            let mut file = std::fs::File::create(path(&args, "path")?)?;
            receive(
                input.ok_or_else(|| io::Error::other("missing file input"))?,
                &mut file,
                output,
            )?;
            Ok(Value::Null)
        }
        "metadata" => Ok(json!(fs.metadata(path(&args, "path")?)?)),
        "symlink_metadata" => Ok(json!(fs.symlink_metadata(path(&args, "path")?)?)),
        "canonicalize" => Ok(json!(fs.canonicalize(path(&args, "path")?)?)),
        "read_link" => Ok(json!(fs.read_link(path(&args, "path")?)?)),
        "read_dir" => Ok(json!(
            fs.read_dir(path(&args, "path")?)?
                .collect::<io::Result<Vec<_>>>()?
        )),
        "copy" => Ok(json!(fs.copy(path(&args, "path")?, path(&args, "to")?)?)),
        "create_file" => {
            fs.create_file(path(&args, "path")?)?;
            Ok(Value::Null)
        }
        "create_dir_all" | "create_dir" | "rename" | "remove" | "remove_file"
        | "remove_dir_all" | "symlink" => {
            let to = args
                .get("to")
                .filter(|value| !value.is_null())
                .map(|_| path(&args, "to"))
                .transpose()?;
            crate::filesystem::mutate(&request.op, &path(&args, "path")?, to.as_deref())?;
            Ok(Value::Null)
        }
        "spawn" => {
            let mut spec: crate::command::Spec = serde_json::from_value(args["spec"].clone())?;
            if matches!(spec.stdin, Stdio::Inherit) {
                spec.stdin = Stdio::Null;
            }
            if matches!(spec.stdout, Stdio::Inherit) {
                spec.stdout = Stdio::Null;
            }
            if matches!(spec.stderr, Stdio::Inherit) {
                spec.stderr = Stdio::Null;
            }
            let mut child = Command::from_spec(&spec).spawn()?;
            let channel = channel(&args, "channel")?;
            if let Some(pipe) = child.stdout.take() {
                pipe_out(
                    pipe,
                    output.clone(),
                    channel_value(&args, "output")?,
                    gate.clone(),
                    state.clone(),
                );
            }
            if let Some(pipe) = child.stderr.take() {
                pipe_out(
                    pipe,
                    output.clone(),
                    channel_value(&args, "error")?,
                    gate.clone(),
                    state.clone(),
                );
            }
            if let Some(mut pipe) = child.stdin.take() {
                let output = output.clone();
                let input = input.ok_or_else(|| io::Error::other("missing process input"))?;
                std::thread::spawn(move || {
                    let _ = receive(input, &mut pipe, &output);
                });
            }
            state
                .children
                .lock()
                .unwrap()
                .insert(channel, Arc::new(Mutex::new(child)));
            Ok(Value::Null)
        }
        "pty" => {
            let argv: Vec<String> = serde_json::from_value(args["argv"].clone())?;
            let mut command = CommandBuilder::new(
                argv.first()
                    .ok_or_else(|| io::Error::other("missing terminal program"))?,
            );
            command.args(&argv[1..]);
            let env: Vec<(String, String)> = serde_json::from_value(args["env"].clone())?;
            for (name, value) in env {
                command.env(name, value);
            }
            let pty = Host::local().pty(
                command,
                &path(&args, "cwd")?,
                channel_value(&args, "cols")? as usize,
                channel_value(&args, "rows")? as usize,
            )?;
            let channel = channel(&args, "channel")?;
            state.controls.lock().unwrap().insert(channel, pty.control);
            state
                .ptys
                .lock()
                .unwrap()
                .insert(channel, Arc::new(Mutex::new(pty.child)));
            pipe_out(
                pty.reader,
                output.clone(),
                channel_value(&args, "output")?,
                gate.clone(),
                state.clone(),
            );
            let input = input.ok_or_else(|| io::Error::other("missing terminal input"))?;
            let input_output = output.clone();
            std::thread::spawn(move || {
                let mut writer = pty.writer;
                let _ = receive(input, &mut writer, &input_output);
            });
            Ok(Value::Null)
        }
        "try_wait" => {
            let child = state
                .children
                .lock()
                .unwrap()
                .get(&channel(&args, "channel")?)
                .cloned()
                .ok_or_else(|| io::Error::other("unknown child"))?;
            let status = child
                .lock()
                .unwrap()
                .try_wait()?
                .map(|status| status.code().unwrap_or(1));
            Ok(json!(status))
        }
        "pty_status" => {
            let child = state
                .ptys
                .lock()
                .unwrap()
                .get(&channel(&args, "channel")?)
                .cloned()
                .ok_or_else(|| io::Error::other("unknown terminal"))?;
            let status = child
                .lock()
                .unwrap()
                .try_wait()?
                .map(|status| (status.exit_code(), status.signal().map(str::to_owned)));
            Ok(json!(status))
        }
        "kill" => {
            let channel = channel(&args, "channel")?;
            if let Some(child) = state.children.lock().unwrap().get(&channel) {
                child.lock().unwrap().kill()?;
            }
            if let Some(child) = state.ptys.lock().unwrap().get(&channel) {
                child.lock().unwrap().kill()?;
            }
            Ok(Value::Null)
        }
        "release" => {
            if let Some(child) = state
                .children
                .lock()
                .unwrap()
                .remove(&channel(&args, "channel")?)
            {
                let mut child = child.lock().unwrap();
                if child.try_wait()?.is_none() {
                    child.kill()?;
                }
                child.wait()?;
            }
            Ok(Value::Null)
        }
        "release_pty" => {
            let channel = channel(&args, "channel")?;
            state.controls.lock().unwrap().remove(&channel);
            if let Some(child) = state.ptys.lock().unwrap().remove(&channel) {
                let mut child = child.lock().unwrap();
                if child.try_wait()?.is_none() {
                    child.kill()?;
                }
                child.wait()?;
            }
            Ok(Value::Null)
        }
        "cancel_stream" => {
            if let Some(flow) = state
                .flows
                .lock()
                .unwrap()
                .remove(&channel(&args, "channel")?)
            {
                flow.close();
            }
            Ok(Value::Null)
        }
        "resize" => {
            let controls = state.controls.lock().unwrap();
            let control = controls
                .get(&channel(&args, "channel")?)
                .ok_or_else(|| io::Error::other("unknown terminal"))?;
            control.resize(
                channel_value(&args, "cols")? as usize,
                channel_value(&args, "rows")? as usize,
            )?;
            Ok(Value::Null)
        }
        "walk" => {
            let root = path(&args, "root")?;
            let channel = channel(&args, "channel")?;
            let output = output.clone();
            let state = state.clone();
            std::thread::spawn(move || {
                await_ack(&gate);
                crate::walk::walk_each(&root, |path| {
                    Frame::json(wire::EVENT, channel, &path)
                        .is_ok_and(|frame| send_stream(&state, &output, frame).is_ok())
                });
                let _ = send(
                    &output,
                    Frame {
                        kind: wire::EOF,
                        channel,
                        payload: Vec::new(),
                    },
                );
            });
            Ok(Value::Null)
        }
        "watch" => {
            let channel = channel(&args, "channel")?;
            let state_weak = Arc::downgrade(state);
            let output = output.clone();
            let wake = Arc::new(move || {
                if let Some(state) = state_weak.upgrade() {
                    let watcher = state.watchers.lock().unwrap().get(&channel).cloned();
                    if let Some(watcher) = watcher
                        && let Ok(frame) = Frame::json(wire::EVENT, channel, &watcher.take())
                    {
                        let _ = send_stream(&state, &output, frame);
                    }
                }
            });
            let watcher = Arc::new(Watcher::start(&path(&args, "root")?, wake));
            state.watchers.lock().unwrap().insert(channel, watcher);
            Ok(Value::Null)
        }
        "unwatch" => {
            let channel = channel(&args, "channel")?;
            if let Some(flow) = state.flows.lock().unwrap().remove(&channel) {
                flow.close();
            }
            state.watchers.lock().unwrap().remove(&channel);
            Ok(Value::Null)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown host request",
        )),
    }
}

/// Writes a serialized frame to the endpoint output.
fn send(output: &Output, frame: Frame) -> io::Result<()> {
    frame.write(&mut *output.lock().unwrap())
}
/// Parses a path parameter without involving shell expansion.
fn path(args: &Value, key: &str) -> io::Result<PathBuf> {
    serde_json::from_value(args[key].clone()).map_err(io::Error::other)
}
/// Parses a channel or dimension parameter.
fn channel_value(args: &Value, key: &str) -> io::Result<u32> {
    args[key]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| io::Error::other(format!("invalid {key}")))
}
/// Parses a stream identity.
fn channel(args: &Value, key: &str) -> io::Result<u32> {
    channel_value(args, key)
}
/// Copies a raw input stream until its explicit EOF.
fn receive(input: Receiver<Frame>, writer: &mut impl Write, output: &Output) -> io::Result<()> {
    for frame in input {
        if frame.kind == wire::EOF {
            return writer.flush();
        }
        writer.write_all(&frame.payload)?;
        writer.flush()?;
        send(
            output,
            Frame {
                kind: wire::CREDIT,
                channel: frame.channel,
                payload: Vec::new(),
            },
        )?;
    }
    Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "transport closed before input finished",
    ))
}
/// Copies raw bytes onto a channel and sends a clean EOF.
fn pump(reader: &mut impl Read, output: &Output, channel: u32, state: &State) -> io::Result<()> {
    let mut bytes = [0; 8192];
    loop {
        let read = reader.read(&mut bytes)?;
        if read == 0 {
            break;
        }
        send_stream(
            state,
            output,
            Frame {
                kind: wire::BYTES,
                channel,
                payload: bytes[..read].to_vec(),
            },
        )?;
    }
    send(
        output,
        Frame {
            kind: wire::EOF,
            channel,
            payload: Vec::new(),
        },
    )
}
/// Starts a pipe reader independently of control request handling.
fn pipe_out(
    mut reader: Box<dyn Read + Send>,
    output: Output,
    channel: u32,
    gate: Gate,
    state: Arc<State>,
) {
    std::thread::spawn(move || {
        await_ack(&gate);
        let _ = pump(&mut reader, &output, channel, &state);
    });
}

/// Waits until the control reply has been sent before producing stream data.
fn await_ack(gate: &Gate) {
    let mut acknowledged = gate.0.lock().unwrap();
    while !*acknowledged {
        acknowledged = gate.1.wait(acknowledged).unwrap();
    }
}

/// Sends stream data only when that channel's receiver has room.
fn send_stream(state: &State, output: &Output, frame: Frame) -> io::Result<()> {
    let flow = state
        .flows
        .lock()
        .unwrap()
        .get(&frame.channel)
        .cloned()
        .ok_or_else(|| io::Error::other("unknown output stream"))?;
    flow.acquire()?;
    send(output, frame)
}
