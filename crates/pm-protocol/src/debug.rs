//! What is asked and said about debugging: the programs a folder can be debugged as, the breakpoints a client keeps, and what a client does to a running session.

use serde::{Deserialize, Serialize};

/// One thing a folder can be debugged as, in the order the server lists them; a client chooses one by its place in the list.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DebugScenario {
    /// What it is called.
    pub label: String,
    /// The adapter that runs it, as a reader knows it, or the name the scenario gave it when the server has none.
    pub adapter: String,
    /// Whether the adapter is installed on the server's machine.
    pub installed: bool,
    /// Whether it joins a program that is running rather than starting one.
    pub attach: bool,
    /// The file it was read from, from the root of the folder, or nothing for one the server offers on its own.
    pub source: Option<String>,
    /// The task it runs, by name, and must see finish before it starts.
    #[serde(default)]
    pub task: Option<String>,
}

/// A program running on the server's machine that a debugger can attach to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DebugProcess {
    /// The operating system's number for it.
    pub pid: u32,
    /// The program's short name.
    pub name: String,
    /// Its command line.
    pub command: String,
}

/// A debug adapter the server's machine has.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DebugAdapter {
    /// The name `debug_attach` takes it by.
    pub id: String,
    /// The name a reader knows it by.
    pub name: String,
    /// The starts of the names of the programs it attaches to; an empty one stands for any program.
    #[serde(default)]
    pub attach_names: Vec<String>,
}

/// The breakpoints of one file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DebugFile {
    /// The file, from the root of the folder.
    pub path: String,
    /// The breakpoints, counted from line zero.
    pub breakpoints: Vec<pm_dap::Breakpoint>,
}

/// What the server knows about one debugging session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DebugInfo {
    /// The session's number.
    pub debug: u64,
    /// The folder the program is debugged in.
    pub directory: String,
    /// What the program is debugged as.
    pub label: String,
}

/// What a client does to a debugging session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DebugAction {
    /// Runs the paused program on.
    Continue,
    /// Pauses the running program.
    Pause,
    /// Runs to the next line of the frame the program paused in.
    StepOver,
    /// Runs into the call on the line the program paused on.
    StepIn,
    /// Runs out of the frame the program paused in.
    StepOut,
    /// Ends the session, and the program with it.
    Stop,
    /// Looks at another frame of the stack.
    SelectFrame {
        /// The frame, by the number the adapter gave it.
        frame: i64,
    },
    /// Asks what is behind a variable.
    Expand {
        /// The reference the adapter gave the variable.
        reference: i64,
    },
    /// Asks what an expression comes to in the frame being looked at, and writes the answer in the console.
    Evaluate {
        /// The expression.
        expression: String,
    },
    /// Replaces the watch expressions.
    SetWatches {
        /// The expressions.
        watches: Vec<String>,
    },
    /// Replaces the breakpoints of a file.
    SetBreakpoints {
        /// The file, from the root of the folder.
        path: String,
        /// The breakpoints, counted from line zero.
        breakpoints: Vec<pm_dap::Breakpoint>,
    },
}
