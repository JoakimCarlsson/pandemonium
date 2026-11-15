//! The plain data the dock draws besides the shell's screen: the tabs of the
//! shells, the problems the language servers find, the tasks of the folder
//! and how debugging it stands.

/// A shell the dock lists as a tab.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalTab {
    /// The number the window keeps it by, which stays while it lives.
    pub key: u64,
    /// What its tab reads: what the program in it titled it, the task it runs, or the shell's place in the list.
    pub title: String,
    /// Whether it ended.
    pub exited: bool,
    /// Whether it runs a task rather than a plain shell.
    pub task: bool,
}

/// How bad a problem is, worst first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProblemSeverity {
    /// The file is wrong.
    #[default]
    Error,
    /// The file may be wrong.
    Warning,
    /// Something worth knowing.
    Information,
    /// A suggestion.
    Hint,
}

/// Something a language server finds wrong with a file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Problem {
    /// The folder the file was opened in.
    pub folder: String,
    /// The file, from the root of the folder.
    pub path: String,
    /// The line it starts on, counted from zero.
    pub line: usize,
    /// The column it starts at, counted from zero.
    pub column: usize,
    /// How bad it is.
    pub severity: ProblemSeverity,
    /// What it says.
    pub message: String,
    /// What said it, such as the compiler or a linter, when the server says.
    pub source: Option<String>,
}

/// A task the folder can be built, tested or run with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskEntry {
    /// What it is called.
    pub name: String,
    /// What it types into the shell.
    pub command: String,
    /// What kind of task it is: `build`, `test`, `run`, or empty.
    pub group: String,
    /// The manifest it was read from.
    pub source: String,
}

/// The tasks of the folder the dock follows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskList {
    /// Whether the server answered for the folder.
    pub loaded: bool,
    /// Whether there is a folder to list tasks for: a session is open.
    pub available: bool,
    /// Each task, those the project writes first.
    pub tasks: Vec<TaskEntry>,
    /// Why the project's own file of tasks could not be read, when it could not.
    pub problem: Option<String>,
}

/// Where a debugging session has got to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugStanding {
    /// The adapter is starting, or the program is being set up.
    #[default]
    Starting,
    /// The program is running.
    Running,
    /// The program is paused, and can be looked at.
    Stopped,
    /// The session is over.
    Ended,
}

/// One thing the folder can be debugged as.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugScenarioEntry {
    /// What it is called.
    pub label: String,
    /// The adapter that runs it.
    pub adapter: String,
    /// Whether the adapter is installed on the server's machine.
    pub installed: bool,
}

/// One frame of the paused thread's stack.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugFrameEntry {
    /// What the adapter calls it.
    pub id: i64,
    /// The function it is in.
    pub name: String,
    /// The file and line it is at, as a reader reads them, or empty.
    pub place: String,
}

/// One variable shown under a scope, or a member of one, indented by its depth.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugVariableEntry {
    /// What it is called.
    pub name: String,
    /// What it holds.
    pub value: String,
    /// Its type, when the adapter says.
    pub kind: Option<String>,
    /// What its members are asked for by, or zero when it has none.
    pub reference: i64,
    /// Whether its members are shown below it.
    pub expanded: bool,
    /// How many variables it is nested in.
    pub depth: usize,
}

/// One scope of the frame looked at, with its variables once asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugScopeEntry {
    /// What it is called.
    pub name: String,
    /// What its variables are asked for by.
    pub reference: i64,
    /// Its variables and the members shown of them, in order.
    pub variables: Vec<DebugVariableEntry>,
}

/// A breakpoint the reader set.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct DebugBreakpoint {
    /// The file, from the root of the folder.
    pub path: String,
    /// The line, counted from zero.
    pub line: usize,
    /// Whether the adapter placed it, while a session runs.
    pub verified: Option<bool>,
}

/// A value the debugger shows beside a line of a file while the program is paused there.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineValue {
    /// The file, as the adapter names it.
    pub path: String,
    /// The line, counted from zero.
    pub line: usize,
    /// What is shown, such as `count = 3, name = "a"`.
    pub text: String,
}

/// How the debugging session the dock follows stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugSession {
    /// The server's number for it.
    pub id: u64,
    /// What the program is debugged as.
    pub label: String,
    /// Where it has got to.
    pub standing: DebugStanding,
    /// Why the program last paused, while it is paused.
    pub reason: Option<String>,
    /// The paused thread's stack, innermost first.
    pub frames: Vec<DebugFrameEntry>,
    /// The frame looked at.
    pub frame: Option<i64>,
    /// That frame's scopes.
    pub scopes: Vec<DebugScopeEntry>,
    /// Each watch expression with what it came to.
    pub watches: Vec<(String, String)>,
    /// The last lines of the console, oldest first.
    pub console: Vec<String>,
    /// The values shown beside the line the program paused on.
    pub inline_values: Vec<InlineValue>,
}

/// What the Debug tab shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugModel {
    /// Whether the server answered with what the folder can be debugged as.
    pub loaded: bool,
    /// What the folder can be debugged as, in the server's order.
    pub scenarios: Vec<DebugScenarioEntry>,
    /// The session followed, once one started.
    pub session: Option<DebugSession>,
    /// The breakpoints set, by file and line.
    pub breakpoints: Vec<DebugBreakpoint>,
    /// Why the last start failed, when it did.
    pub error: Option<String>,
}

/// What the Debug tab's controls ask of the session followed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugCommand {
    /// Runs the paused program on.
    Continue,
    /// Pauses the running program.
    Pause,
    /// Runs to the next line.
    StepOver,
    /// Runs into the call on the line.
    StepIn,
    /// Runs out of the frame.
    StepOut,
    /// Ends the session.
    Stop,
    /// Looks at the frame with this number.
    SelectFrame(i64),
    /// Shows or hides the members of the variable with this reference.
    Expand(i64),
    /// Asks what an expression comes to.
    Evaluate(String),
    /// Adds this expression to the watches.
    AddWatch(String),
    /// Takes this expression out of the watches.
    RemoveWatch(String),
}
