//! What an adapter has said, as the window reads it.
//!
//! The protocol answers in fragments — a thread stopped, then its frames,
//! then the scopes of one of them, then what each scope holds — and a window
//! drawing a debugger wants the whole picture as it now stands. [`State`] is
//! that picture, kept up by the reader thread; the rest of this module is
//! the shapes it is drawn in.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use serde_json::Value;

/// Most lines the console keeps before it forgets the oldest.
const KEPT_LINES: usize = 10_000;

/// Where a debugging session has got to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Standing {
    /// The adapter is starting, or the program is being set up.
    Starting,
    /// The program is running.
    Running,
    /// The program is paused, and can be looked at.
    Stopped,
    /// The session is over.
    Ended,
}

/// Something the window acts on rather than only draws.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The program paused here, which the window brings into view.
    Paused(Frame),
    /// The adapter refused to launch or attach to the program.
    StartRefused(String),
    /// The session is over.
    Ended,
}

/// One thread of the program.
#[derive(Clone, Debug, PartialEq)]
pub struct Thread {
    /// What the adapter calls it.
    pub id: i64,
    /// What it is named.
    pub name: String,
}

/// One frame of a paused thread's stack.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// What the adapter calls it.
    pub id: i64,
    /// The function it is in.
    pub name: String,
    /// The file it is in, when the adapter knows one.
    pub path: Option<PathBuf>,
    /// The line it is at, counted from zero.
    pub line: usize,
    /// The column it is at, counted from zero.
    pub column: usize,
}

/// One scope of a frame: its locals, its arguments, its registers.
#[derive(Clone, Debug, PartialEq)]
pub struct Scope {
    /// What the scope is called.
    pub name: String,
    /// What its variables are asked for by.
    pub reference: i64,
    /// Whether the adapter says it is costly to look into.
    pub expensive: bool,
}

/// One variable, or one member of one.
#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    /// What it is called.
    pub name: String,
    /// What it holds, as the adapter writes it.
    pub value: String,
    /// Its type, when the adapter says.
    pub kind: Option<String>,
    /// What its members are asked for by, or zero when it has none.
    pub reference: i64,
}

/// A source breakpoint, with optional adapter-specific behavior.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Breakpoint {
    /// The source line, counted from zero.
    pub line: usize,
    /// An expression that must hold before stopping.
    pub condition: Option<String>,
    /// The adapter's hit-count expression.
    pub hits: Option<String>,
    /// A message to print without stopping.
    pub log: Option<String>,
}

/// The latest result of evaluating a watch expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Watched {
    /// The expression being watched.
    pub expression: String,
    /// Its value, or the adapter's reason it could not be evaluated.
    pub value: Result<Variable, String>,
}

/// Where a breakpoint came to rest, as the adapter placed it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placed {
    /// The line it stops on, counted from zero.
    pub line: usize,
    /// Whether the adapter could put it there at all.
    pub verified: bool,
    /// The adapter's explanation when it could not place the breakpoint.
    pub message: Option<String>,
}

/// Who a line of the console is from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    /// The adapter, about itself or the session.
    Console,
    /// The program's standard output.
    Stdout,
    /// The program's standard error.
    Stderr,
    /// An expression the reader asked about.
    Asked,
    /// What it came to.
    Answer,
    /// Something that went wrong.
    Error,
}

/// One line of the console.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// Who it is from.
    pub category: Category,
    /// What it says.
    pub text: String,
}

/// What one request was sent to find out.
#[derive(Clone, Debug)]
pub(crate) enum Sent {
    /// The handshake.
    Initialize,
    /// The program being launched or attached to.
    Launch,
    /// The breakpoints of one file.
    Breakpoints(PathBuf),
    /// The exception breakpoints, which are none.
    Exceptions,
    /// The end of the setting up.
    ConfigurationDone,
    /// The program's threads.
    Threads,
    /// The stack of one thread.
    StackTrace(i64),
    /// The scopes of one frame.
    Scopes(i64),
    /// The variables behind one reference.
    Variables(i64),
    /// Running on, or one step of it.
    Resume,
    /// An expression the reader asked about.
    Evaluate,
    /// A watch expression, identified by its position in the current list.
    Watch(usize),
    /// The end of the session.
    Disconnect,
}

/// What the adapter has said, and what it has not been told yet.
pub(crate) struct State {
    /// The adapter's name in unsupported-feature messages.
    pub adapter_name: &'static str,
    /// What the adapter said it can do, once the handshake has said it.
    pub capabilities: Value,
    /// Where the session has got to.
    pub standing: Standing,
    /// Why the program last paused, as the adapter put it.
    pub reason: Option<String>,
    /// Whether the adapter has been set up, and so takes breakpoints as they
    /// change rather than all at once.
    pub configured: bool,
    /// The lines each file has breakpoints on, as the window last set them.
    pub breakpoints: BTreeMap<PathBuf, Vec<Breakpoint>>,
    /// Where the adapter put each of them.
    pub placed: BTreeMap<PathBuf, Vec<Placed>>,
    /// Unsupported breakpoint messages already written to the console.
    pub reported_breakpoints: BTreeSet<String>,
    /// The requests sent and not yet answered, and what each was for.
    pub sent: HashMap<i64, Sent>,
    /// The program's threads, as last listed.
    pub threads: Vec<Thread>,
    /// The thread the session is looking at.
    pub thread: Option<i64>,
    /// That thread's stack, innermost frame first.
    pub frames: Vec<Frame>,
    /// The frame the session is looking at.
    pub frame: Option<i64>,
    /// That frame's scopes.
    pub scopes: Vec<Scope>,
    /// The variables behind every reference asked about since the last pause.
    pub variables: HashMap<i64, Vec<Variable>>,
    /// Expressions retained across pauses.
    pub watches: Vec<String>,
    /// The last result for each expression.
    pub watched: Vec<Watched>,
    /// The console, oldest line first.
    pub lines: Vec<Line>,
    /// Whether the last line of the console is still being written.
    pub open: bool,
    /// What the window has to act on and has not yet taken.
    pub events: Vec<Event>,
    /// Whether anything has arrived since the window last looked.
    pub fresh: bool,
}

impl State {
    /// The state of a session about to start, with `breakpoints` to set.
    pub(crate) fn new(
        breakpoints: BTreeMap<PathBuf, Vec<Breakpoint>>,
        adapter_name: &'static str,
    ) -> Self {
        Self {
            adapter_name,
            capabilities: Value::Null,
            standing: Standing::Starting,
            reason: None,
            configured: false,
            breakpoints,
            placed: BTreeMap::new(),
            reported_breakpoints: BTreeSet::new(),
            sent: HashMap::new(),
            threads: Vec::new(),
            thread: None,
            frames: Vec::new(),
            frame: None,
            scopes: Vec::new(),
            variables: HashMap::new(),
            watches: Vec::new(),
            watched: Vec::new(),
            lines: Vec::new(),
            open: false,
            events: Vec::new(),
            fresh: true,
        }
    }

    /// Writes `text` into the console as `category`.
    ///
    /// The program's output arrives in whatever pieces it was flushed in, so
    /// a piece that does not end a line is held open and the next piece from
    /// the same place carries on from it.
    pub(crate) fn write(&mut self, category: Category, text: &str) {
        let mut pieces = text.split('\n').peekable();
        while let Some(piece) = pieces.next() {
            let last = pieces.peek().is_none();
            if last && piece.is_empty() {
                self.open = false;
                break;
            }
            let piece = piece.trim_end_matches('\r');
            let continues = self.open
                && self
                    .lines
                    .last()
                    .is_some_and(|line| line.category == category);
            match continues {
                true => {
                    if let Some(line) = self.lines.last_mut() {
                        line.text.push_str(piece);
                    }
                }
                false => self.lines.push(Line {
                    category,
                    text: piece.to_owned(),
                }),
            }
            self.open = last;
        }
        let over = self.lines.len().saturating_sub(KEPT_LINES);
        self.lines.drain(..over);
    }

    /// Writes one whole line into the console as `category`.
    pub(crate) fn say(&mut self, category: Category, text: &str) {
        self.open = false;
        self.write(
            category,
            &format!("{}\n", text.trim_end_matches(['\n', '\r'])),
        );
    }

    /// Forgets everything that was true only while the program was paused.
    pub(crate) fn resumed(&mut self) {
        self.standing = Standing::Running;
        self.reason = None;
        self.frames.clear();
        self.frame = None;
        self.scopes.clear();
        self.variables.clear();
    }

    /// Marks the session over, telling the window once.
    pub(crate) fn end(&mut self) {
        if self.standing == Standing::Ended {
            return;
        }
        self.resumed();
        self.standing = Standing::Ended;
        self.events.push(Event::Ended);
    }
}
