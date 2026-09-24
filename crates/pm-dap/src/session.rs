//! One program being debugged: the adapter, what it has been told, and what
//! it says.
//!
//! Starting the adapter, reaching it and setting the program up all happen on
//! threads of their own, so nothing an adapter does — including taking a few
//! seconds to open its socket — ever holds a frame up. What comes back is
//! kept in one [`State`] the window reads whenever it is woken, and a short
//! queue of [`Event`]s it acts on.
//!
//! The protocol's order is followed to the letter, because adapters differ
//! in how much of it they forgive: the handshake, then the launch, then —
//! once the adapter says it is ready for them — the breakpoints and the word
//! that setting up is done. Everything after that is the window asking.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::adapter::Connect;
use crate::scenario::{Request, Scenario};
use crate::state::{
    Category, Event, Frame, Line, Placed, Scope, Sent, Standing, State, Thread, Variable,
};
use crate::wire::{self, Wire};

/// How long an adapter that listens on a socket is given to open it.
const CONNECT_WITHIN: Duration = Duration::from_secs(10);

/// How long the editor waits between two tries at reaching that socket.
const CONNECT_RETRY: Duration = Duration::from_millis(100);

/// How long an adapter told to end is given to end on its own before it is
/// made to.
const END_WITHIN: Duration = Duration::from_secs(2);

/// How many frames of a paused thread's stack are asked for.
const FRAMES: usize = 64;

/// How a session wakes the window once it has something to say.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// A program the editor is debugging.
pub struct Session {
    /// What the program is being debugged as.
    scenario: Scenario,
    /// The worktree it is being debugged in.
    root: PathBuf,
    /// The adapter's process, kept so that it can be ended.
    process: Arc<Mutex<Option<Child>>>,
    /// The way messages go to the adapter.
    wire: Wire,
    /// What the adapter has said and what it is owed.
    state: Arc<Mutex<State>>,
}

impl Session {
    /// Starts debugging `scenario` in `root`, with `breakpoints` set before
    /// the program runs, waking the window through `notify`.
    ///
    /// The adapter is started here and everything after is done on the
    /// threads it leaves behind: a session is startable in a frame because
    /// nothing of it is waited for.
    pub fn start(
        scenario: Scenario,
        root: &Path,
        breakpoints: BTreeMap<PathBuf, Vec<usize>>,
        notify: Notify,
    ) -> io::Result<Self> {
        let adapter = scenario.adapter.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                format!("no debug adapter for `{}`", scenario.kind),
            )
        })?;
        let program = adapter.program().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not installed", adapter.name),
            )
        })?;
        let port = match adapter.connect {
            Connect::Tcp => free_port()?,
            Connect::Stdio => 0,
        };

        let mut process = Command::new(&program)
            .args(adapter.arguments_for(port))
            .env("PATH", pm_text::program::path_beside(&program))
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stderr = process.stderr.take();
        let stdin = process.stdin.take();
        let stdout = process.stdout.take();
        let process = Arc::new(Mutex::new(Some(process)));
        let (outbox, queued) = mpsc::channel();
        let wire = Wire::new(outbox);
        let state = Arc::new(Mutex::new(State::new(breakpoints)));
        let reader = Reader {
            process: process.clone(),
            state: state.clone(),
            wire: wire.clone(),
            notify: notify.clone(),
            request: scenario.request,
            arguments: scenario.arguments(),
        };

        if let Some(stderr) = stderr {
            echo(stderr, state.clone(), notify.clone());
        }
        match (adapter.connect, stdin, stdout) {
            (Connect::Stdio, Some(stdin), Some(stdout)) => {
                thread::spawn(move || wire::write_all(queued, stdin));
                thread::spawn(move || reader.run(stdout));
            }
            (_, _, stdout) => {
                if let Some(stdout) = stdout {
                    echo(stdout, state.clone(), notify.clone());
                }
                thread::spawn(move || reach(port, queued, reader));
            }
        }

        let session = Self {
            scenario,
            root: root.to_path_buf(),
            process,
            wire,
            state,
        };
        session.ask(
            "initialize",
            json!({
                "clientID": "pandemonium",
                "clientName": "Pandemonium",
                "adapterID": adapter.id,
                "pathFormat": "path",
                "linesStartAt1": true,
                "columnsStartAt1": true,
                "supportsVariableType": true,
                "supportsRunInTerminalRequest": false,
                "locale": "en",
            }),
            Sent::Initialize,
        );
        Ok(session)
    }

    /// What the program is being debugged as.
    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    /// The worktree it is being debugged in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the session has got to.
    pub fn standing(&self) -> Standing {
        self.read(|state| state.standing)
    }

    /// Why the program last paused, while it is paused.
    pub fn reason(&self) -> Option<String> {
        self.read(|state| state.reason.clone())
    }

    /// The program's threads, as last listed.
    pub fn threads(&self) -> Vec<Thread> {
        self.read(|state| state.threads.clone())
    }

    /// The thread the session is looking at.
    pub fn thread(&self) -> Option<i64> {
        self.read(|state| state.thread)
    }

    /// That thread's stack, innermost frame first, while it is paused.
    pub fn frames(&self) -> Vec<Frame> {
        self.read(|state| state.frames.clone())
    }

    /// The frame the session is looking at.
    pub fn frame(&self) -> Option<Frame> {
        self.read(|state| {
            let id = state.frame?;
            state.frames.iter().find(|frame| frame.id == id).cloned()
        })
    }

    /// That frame's scopes.
    pub fn scopes(&self) -> Vec<Scope> {
        self.read(|state| state.scopes.clone())
    }

    /// The variables behind `reference`, once they have been asked for.
    pub fn variables(&self, reference: i64) -> Option<Vec<Variable>> {
        self.read(|state| state.variables.get(&reference).cloned())
    }

    /// The console, oldest line first.
    pub fn lines(&self) -> Vec<Line> {
        self.read(|state| state.lines.clone())
    }

    /// How many lines the console holds.
    pub fn line_count(&self) -> usize {
        self.read(|state| state.lines.len())
    }

    /// Where the adapter put the breakpoints of `path`.
    pub fn placed(&self, path: &Path) -> Vec<Placed> {
        self.read(|state| state.placed.get(path).cloned().unwrap_or_default())
    }

    /// What the window has to act on, oldest first.
    pub fn take_events(&self) -> Vec<Event> {
        self.change(|state| std::mem::take(&mut state.events))
    }

    /// Whether anything has arrived since this was last asked.
    pub fn take_fresh(&self) -> bool {
        self.change(|state| std::mem::take(&mut state.fresh))
    }

    /// Sets the breakpoints of `path` to `lines`, counted from zero.
    ///
    /// Before the adapter is ready for breakpoints they are only kept, and
    /// all of them go at once when it is; after, the file's go as they
    /// change.
    pub fn set_breakpoints(&self, path: &Path, lines: Vec<usize>) {
        let configured = self.change(|state| {
            state.breakpoints.insert(path.to_path_buf(), lines.clone());
            state.configured && state.standing != Standing::Ended
        });
        if configured {
            send_breakpoints(&self.wire, &self.state, path, &lines);
        }
    }

    /// Runs the program on from where it paused.
    pub fn resume(&self) {
        self.step("continue");
    }

    /// Runs to the next line of the frame it paused in.
    pub fn step_over(&self) {
        self.step("next");
    }

    /// Runs into the call on the line it paused on.
    pub fn step_in(&self) {
        self.step("stepIn");
    }

    /// Runs out of the frame it paused in.
    pub fn step_out(&self) {
        self.step("stepOut");
    }

    /// Pauses the program where it is.
    pub fn pause(&self) {
        let Some(thread) = self.read(|state| {
            let running = state.standing == Standing::Running;
            running
                .then(|| {
                    state
                        .thread
                        .or(state.threads.first().map(|thread| thread.id))
                })
                .flatten()
        }) else {
            return;
        };
        self.ask("pause", json!({ "threadId": thread }), Sent::Resume);
    }

    /// Looks at the frame `id` names: its scopes, and what they hold.
    pub fn select_frame(&self, id: i64) {
        self.change(|state| {
            state.frame = Some(id);
            state.scopes.clear();
            state.variables.clear();
        });
        self.ask("scopes", json!({ "frameId": id }), Sent::Scopes(id));
    }

    /// Asks what is behind `reference`, where it has not been asked yet.
    pub fn expand(&self, reference: i64) {
        if reference == 0 || self.variables(reference).is_some() {
            return;
        }
        ask_variables(&self.wire, &self.state, reference);
    }

    /// Asks what `expression` comes to in the frame the session is looking
    /// at, and writes the question and its answer into the console.
    pub fn evaluate(&self, expression: &str) {
        let frame = self.change(|state| {
            state.say(Category::Asked, expression);
            state.frame
        });
        self.ask(
            "evaluate",
            json!({ "expression": expression, "frameId": frame, "context": "repl" }),
            Sent::Evaluate,
        );
    }

    /// Ends the session, and the program with it.
    pub fn stop(&self) {
        if self.standing() == Standing::Ended {
            return;
        }
        self.ask(
            "disconnect",
            json!({ "restart": false, "terminateDebuggee": true }),
            Sent::Disconnect,
        );
    }

    /// Runs the paused thread by `command`: on, or one step of the way.
    fn step(&self, command: &str) {
        let thread = self.change(|state| {
            let thread = (state.standing == Standing::Stopped)
                .then_some(state.thread)
                .flatten()?;
            state.resumed();
            state.fresh = true;
            Some(thread)
        });
        if let Some(thread) = thread {
            self.ask(command, json!({ "threadId": thread }), Sent::Resume);
        }
    }

    /// Sends the request `command` with `arguments`, remembering it as `sent`.
    fn ask(&self, command: &str, arguments: Value, sent: Sent) {
        ask(&self.wire, &self.state, command, arguments, sent);
    }

    /// Reads something off the state.
    fn read<T>(&self, read: impl FnOnce(&State) -> T) -> T {
        read(&lock(&self.state))
    }

    /// Puts the state through `change`.
    fn change<T>(&self, change: impl FnOnce(&mut State) -> T) -> T {
        change(&mut lock(&self.state))
    }
}

impl Drop for Session {
    /// Tells the adapter to end, and makes it end if it will not.
    ///
    /// An adapter killed outright can leave the program it was debugging
    /// running with nobody attached, so it is asked first and given a moment
    /// to take the program down with it.
    fn drop(&mut self) {
        self.stop();
        let process = self.process.clone();
        thread::spawn(move || {
            let started = Instant::now();
            let Ok(mut held) = process.lock() else {
                return;
            };
            let Some(child) = held.as_mut() else {
                return;
            };
            while started.elapsed() < END_WITHIN {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(CONNECT_RETRY);
            }
            let _ = child.kill();
            let _ = child.wait();
        });
    }
}

/// The thread that reads what an adapter says, and answers it.
struct Reader {
    /// The adapter's process, gathered once it stops talking.
    process: Arc<Mutex<Option<Child>>>,
    /// What the adapter has said and what it is owed.
    state: Arc<Mutex<State>>,
    /// The way messages go back to it.
    wire: Wire,
    /// How the window is woken.
    notify: Notify,
    /// Whether the program is launched or attached to.
    request: Request,
    /// What it is launched or attached with.
    arguments: Value,
}

impl Reader {
    /// Reads until the adapter stops talking, then ends the session.
    fn run(self, source: impl Read) {
        wire::read_all(source, |message| {
            self.dispatch(message);
            lock(&self.state).fresh = true;
            (self.notify)();
        });
        let mut state = lock(&self.state);
        state.end();
        state.fresh = true;
        drop(state);
        self.gather();
        (self.notify)();
    }

    /// Gathers the adapter's process once it has closed its end, so an
    /// adapter that ended is not left behind as a zombie until the session
    /// is dropped.
    fn gather(&self) {
        let started = Instant::now();
        let Ok(mut held) = self.process.lock() else {
            return;
        };
        let Some(child) = held.as_mut() else {
            return;
        };
        while started.elapsed() < END_WITHIN {
            if matches!(child.try_wait(), Ok(Some(_))) {
                *held = None;
                return;
            }
            thread::sleep(CONNECT_RETRY);
        }
    }

    /// Acts on one message from the adapter.
    fn dispatch(&self, message: &Value) {
        match message.get("type").and_then(Value::as_str) {
            Some("response") => self.answered(message),
            Some("event") => self.happened(message),
            Some("request") => self.wire.refuse(
                message
                    .get("seq")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                message
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ),
            _ => {}
        }
    }

    /// Acts on the answer to one of the editor's requests.
    fn answered(&self, message: &Value) {
        let seq = message.get("request_seq").and_then(Value::as_i64);
        let Some(sent) = seq.and_then(|seq| lock(&self.state).sent.remove(&seq)) else {
            return;
        };
        let body = message.get("body").cloned().unwrap_or(Value::Null);
        if message.get("success").and_then(Value::as_bool) != Some(true) {
            return self.refused(&sent, message);
        }
        match sent {
            Sent::Initialize => {
                lock(&self.state).capabilities = body;
                self.ask(self.request.command(), self.arguments.clone(), Sent::Launch);
            }
            Sent::Breakpoints(path) => {
                let placed = body
                    .get("breakpoints")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(placed)
                    .collect();
                lock(&self.state).placed.insert(path, placed);
            }
            Sent::ConfigurationDone => {
                let mut state = lock(&self.state);
                if state.standing == Standing::Starting {
                    state.standing = Standing::Running;
                }
            }
            Sent::Threads => self.listed(&body),
            Sent::StackTrace(thread) => self.traced(thread, &body),
            Sent::Scopes(frame) => self.scoped(frame, &body),
            Sent::Variables(reference) => {
                let variables = body
                    .get("variables")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(variable)
                    .collect();
                lock(&self.state).variables.insert(reference, variables);
            }
            Sent::Evaluate => {
                let result = body.get("result").and_then(Value::as_str).unwrap_or("");
                if !result.trim().is_empty() {
                    lock(&self.state).say(Category::Answer, result);
                }
            }
            Sent::Launch | Sent::Exceptions | Sent::Resume | Sent::Disconnect => {}
        }
    }

    /// Writes down why the adapter refused a request.
    ///
    /// A refused launch is the one a reader most needs to see — the program
    /// that is not there, the module that is not installed — and it goes in
    /// the console like every other refusal rather than being lost.
    fn refused(&self, sent: &Sent, message: &Value) {
        let said = message
            .pointer("/body/error/format")
            .or_else(|| message.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("refused");
        let command = message
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut state = lock(&self.state);
        match sent {
            Sent::Evaluate => state.say(Category::Error, said),
            Sent::Disconnect => state.end(),
            _ => state.say(Category::Error, &format!("{command}: {said}")),
        }
    }

    /// Acts on something that happened in the adapter or the program.
    fn happened(&self, message: &Value) {
        let body = message.get("body").cloned().unwrap_or(Value::Null);
        match message.get("event").and_then(Value::as_str) {
            Some("initialized") => self.configure(),
            Some("stopped") => self.stopped(&body),
            Some("continued") => lock(&self.state).resumed(),
            Some("output") => {
                let category = match body.get("category").and_then(Value::as_str) {
                    Some("telemetry") => return,
                    Some("stdout") => Category::Stdout,
                    Some("stderr") => Category::Stderr,
                    _ => Category::Console,
                };
                let output = body.get("output").and_then(Value::as_str).unwrap_or("");
                lock(&self.state).write(category, output);
            }
            Some("exited") => {
                let code = body.get("exitCode").and_then(Value::as_i64).unwrap_or(0);
                lock(&self.state).say(Category::Console, &format!("Exited with code {code}"));
            }
            Some("terminated") => {
                lock(&self.state).end();
                self.ask(
                    "disconnect",
                    json!({ "restart": false, "terminateDebuggee": true }),
                    Sent::Disconnect,
                );
            }
            _ => {}
        }
    }

    /// Sets the adapter up once it says it is ready: every breakpoint, no
    /// exception breakpoints, and the word that setting up is done.
    fn configure(&self) {
        let (breakpoints, capabilities) = {
            let mut state = lock(&self.state);
            state.configured = true;
            (state.breakpoints.clone(), state.capabilities.clone())
        };
        for (path, lines) in &breakpoints {
            send_breakpoints(&self.wire, &self.state, path, lines);
        }
        if capabilities.get("exceptionBreakpointFilters").is_some() {
            self.ask(
                "setExceptionBreakpoints",
                json!({ "filters": [] }),
                Sent::Exceptions,
            );
        }
        match capabilities
            .get("supportsConfigurationDoneRequest")
            .and_then(Value::as_bool)
        {
            Some(true) => self.ask("configurationDone", json!({}), Sent::ConfigurationDone),
            _ => lock(&self.state).standing = Standing::Running,
        }
    }

    /// Takes in that the program paused, and asks where.
    fn stopped(&self, body: &Value) {
        let thread = body.get("threadId").and_then(Value::as_i64);
        let reason = body
            .get("description")
            .or_else(|| body.get("reason"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        {
            let mut state = lock(&self.state);
            state.resumed();
            state.standing = Standing::Stopped;
            state.reason = reason;
            state.thread = thread.or(state.thread);
        }
        self.ask("threads", json!({}), Sent::Threads);
        if let Some(thread) = thread {
            self.trace(thread);
        }
    }

    /// Takes in the program's threads, and traces the first where the pause
    /// named none.
    fn listed(&self, body: &Value) {
        let threads = body
            .get("threads")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|thread| Thread {
                id: thread.get("id").and_then(Value::as_i64).unwrap_or_default(),
                name: text(thread, "name"),
            })
            .collect::<Vec<_>>();
        let untraced = {
            let mut state = lock(&self.state);
            let first = threads.first().map(|thread| thread.id);
            state.threads = threads;
            let paused = state.standing == Standing::Stopped;
            match (paused, state.thread) {
                (true, None) => {
                    state.thread = first;
                    first
                }
                _ => None,
            }
        };
        if let Some(thread) = untraced {
            self.trace(thread);
        }
    }

    /// Asks for the stack of `thread`.
    fn trace(&self, thread: i64) {
        self.ask(
            "stackTrace",
            json!({ "threadId": thread, "startFrame": 0, "levels": FRAMES }),
            Sent::StackTrace(thread),
        );
    }

    /// Takes in the stack of `thread`, and looks into its innermost frame.
    fn traced(&self, thread: i64, body: &Value) {
        let frames = body
            .get("stackFrames")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(frame)
            .collect::<Vec<_>>();
        let top = {
            let mut state = lock(&self.state);
            if state.thread != Some(thread) || state.standing != Standing::Stopped {
                return;
            }
            let top = frames.first().cloned();
            state.frames = frames;
            state.frame = top.as_ref().map(|frame| frame.id);
            if let Some(top) = top.clone() {
                state.events.push(Event::Paused(top));
            }
            top
        };
        if let Some(top) = top {
            self.ask("scopes", json!({ "frameId": top.id }), Sent::Scopes(top.id));
        }
    }

    /// Takes in the scopes of `frame`, and asks what the cheap ones hold.
    fn scoped(&self, frame: i64, body: &Value) {
        let scopes = body
            .get("scopes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|scope| Scope {
                name: text(scope, "name"),
                reference: scope
                    .get("variablesReference")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                expensive: scope
                    .get("expensive")
                    .and_then(Value::as_bool)
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        let wanted = {
            let mut state = lock(&self.state);
            if state.frame != Some(frame) {
                return;
            }
            state.scopes = scopes.clone();
            scopes
        };
        for scope in wanted.iter().filter(|scope| !scope.expensive) {
            ask_variables(&self.wire, &self.state, scope.reference);
        }
    }

    /// Sends the request `command` with `arguments`, remembering it as `sent`.
    fn ask(&self, command: &str, arguments: Value, sent: Sent) {
        ask(&self.wire, &self.state, command, arguments, sent);
    }
}

/// Sends the request `command` with `arguments` down `wire`, remembering in
/// `state` that it was sent as `sent`.
///
/// The request is written down before it goes, under the lock, so an answer
/// that comes back quicker than the sender can let go of the lock still
/// finds what it answers.
fn ask(wire: &Wire, state: &Mutex<State>, command: &str, arguments: Value, sent: Sent) {
    let mut state = lock(state);
    let seq = wire.request(command, arguments);
    state.sent.insert(seq, sent);
}

/// Asks what is behind `reference`.
fn ask_variables(wire: &Wire, state: &Mutex<State>, reference: i64) {
    ask(
        wire,
        state,
        "variables",
        json!({ "variablesReference": reference }),
        Sent::Variables(reference),
    );
}

/// Tells the adapter the breakpoints of `path` are on `lines`.
fn send_breakpoints(wire: &Wire, state: &Mutex<State>, path: &Path, lines: &[usize]) {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    ask(
        wire,
        state,
        "setBreakpoints",
        json!({
            "source": { "path": path, "name": name },
            "breakpoints": lines.iter().map(|line| json!({ "line": line + 1 })).collect::<Vec<_>>(),
            "lines": lines.iter().map(|line| line + 1).collect::<Vec<_>>(),
            "sourceModified": false,
        }),
        Sent::Breakpoints(path.to_path_buf()),
    );
}

/// Reaches an adapter listening on `port`, then carries messages both ways.
///
/// An adapter opens its socket some time after it starts, and how long is its
/// own business: the editor tries until it is let in or has waited long
/// enough to say so.
fn reach(port: u16, queued: Receiver<Value>, reader: Reader) {
    let started = Instant::now();
    let stream = loop {
        match TcpStream::connect((Ipv4Addr::LOCALHOST, port)) {
            Ok(stream) => break stream,
            Err(_) if started.elapsed() < CONNECT_WITHIN => thread::sleep(CONNECT_RETRY),
            Err(error) => {
                let mut state = lock(&reader.state);
                state.say(
                    Category::Error,
                    &format!("could not reach the adapter: {error}"),
                );
                state.end();
                drop(state);
                return (reader.notify)();
            }
        }
    };
    if let Ok(sink) = stream.try_clone() {
        thread::spawn(move || wire::write_all(queued, sink));
    }
    reader.run(stream);
}

/// Copies what an adapter writes on `source` into the console, a line at a
/// time.
///
/// What an adapter says outside the protocol is what it says when the
/// protocol has failed it: a Python without debugpy, a gdb too old for
/// `--interpreter=dap`. It is shown rather than thrown away.
fn echo(source: impl Read + Send + 'static, state: Arc<Mutex<State>>, notify: Notify) {
    thread::spawn(move || {
        for line in BufReader::new(source).lines().map_while(Result::ok) {
            let mut held = lock(&state);
            held.say(Category::Console, &line);
            held.fresh = true;
            drop(held);
            notify();
        }
    });
}

/// A port on the loopback address nobody is listening on.
fn free_port() -> io::Result<u16> {
    Ok(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()?
        .port())
}

/// The state behind `state`, even where a thread panicked holding it.
fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The string `key` of `value` holds, or nothing.
fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// A number the protocol counts from one, counted from zero.
fn from_one(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .saturating_sub(1) as usize
}

/// One breakpoint as the adapter placed it.
fn placed(breakpoint: &Value) -> Option<Placed> {
    breakpoint.get("line")?;
    Some(Placed {
        line: from_one(breakpoint, "line"),
        verified: breakpoint
            .get("verified")
            .and_then(Value::as_bool)
            .unwrap_or_default(),
    })
}

/// One frame of a stack.
fn frame(frame: &Value) -> Frame {
    Frame {
        id: frame.get("id").and_then(Value::as_i64).unwrap_or_default(),
        name: text(frame, "name"),
        path: frame
            .pointer("/source/path")
            .and_then(Value::as_str)
            .map(PathBuf::from),
        line: from_one(frame, "line"),
        column: from_one(frame, "column"),
    }
}

/// One variable.
fn variable(variable: &Value) -> Variable {
    Variable {
        name: text(variable, "name"),
        value: text(variable, "value"),
        kind: variable
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_owned),
        reference: variable
            .get("variablesReference")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
    }
}
