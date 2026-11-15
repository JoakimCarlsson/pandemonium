//! What the dock of the sample workspace shows besides the shell: two shell
//! tabs, a few problems, the tasks of a Rust project and a paused debugging
//! session.

use crate::dock_model::{
    DebugBreakpoint, DebugFrameEntry, DebugModel, DebugScenarioEntry, DebugScopeEntry,
    DebugSession, DebugStanding, DebugVariableEntry, InlineValue, Problem, ProblemSeverity,
    TaskEntry, TaskList, TerminalTab,
};

/// Two shells and a task, the first shown.
pub fn sample_terminals() -> Vec<TerminalTab> {
    vec![
        TerminalTab {
            key: 1,
            title: "Terminal 1".to_owned(),
            exited: false,
            task: false,
        },
        TerminalTab {
            key: 2,
            title: "Terminal 2".to_owned(),
            exited: false,
            task: false,
        },
        TerminalTab {
            key: 3,
            title: "cargo test".to_owned(),
            exited: true,
            task: true,
        },
    ]
}

/// A problem of `severity` on `line` of `path` saying `message`.
fn problem(severity: ProblemSeverity, path: &str, line: usize, message: &str) -> Problem {
    Problem {
        folder: "~/repos/void".to_owned(),
        path: path.to_owned(),
        line,
        column: 4,
        severity,
        message: message.to_owned(),
        source: Some("rustc".to_owned()),
    }
}

/// An error, a warning and a hint.
pub fn sample_problems() -> Vec<Problem> {
    vec![
        problem(
            ProblemSeverity::Error,
            "src/search.rs",
            41,
            "mismatched types: expected `usize`, found `u32`",
        ),
        problem(
            ProblemSeverity::Warning,
            "src/index.rs",
            12,
            "unused variable: `limit`",
        ),
        problem(
            ProblemSeverity::Hint,
            "src/main.rs",
            3,
            "consider importing `std::sync::Arc`",
        ),
    ]
}

/// A task called `name` of `group` running `command`.
fn task(name: &str, group: &str, command: &str) -> TaskEntry {
    TaskEntry {
        name: name.to_owned(),
        command: command.to_owned(),
        group: group.to_owned(),
        source: "Cargo.toml".to_owned(),
    }
}

/// The tasks Cargo implies.
pub fn sample_tasks() -> TaskList {
    TaskList {
        loaded: true,
        available: true,
        tasks: vec![
            task("cargo build", "build", "cargo build"),
            task("cargo test", "test", "cargo test"),
            task("cargo run", "run", "cargo run"),
            task("cargo clippy", "", "cargo clippy --all-targets"),
        ],
        problem: None,
    }
}

/// A variable at `depth` called `name` holding `value`.
fn variable(name: &str, value: &str, kind: &str, depth: usize) -> DebugVariableEntry {
    DebugVariableEntry {
        name: name.to_owned(),
        value: value.to_owned(),
        kind: Some(kind.to_owned()),
        reference: 0,
        expanded: false,
        depth,
    }
}

/// A session paused at a breakpoint in `search`.
pub fn sample_debug() -> DebugModel {
    DebugModel {
        loaded: true,
        scenarios: vec![
            DebugScenarioEntry {
                label: "Debug void".to_owned(),
                adapter: "CodeLLDB".to_owned(),
                installed: true,
            },
            DebugScenarioEntry {
                label: "Debug tests".to_owned(),
                adapter: "CodeLLDB".to_owned(),
                installed: true,
            },
        ],
        session: Some(DebugSession {
            id: 1,
            label: "Debug void".to_owned(),
            standing: DebugStanding::Stopped,
            reason: Some("breakpoint".to_owned()),
            frames: vec![
                DebugFrameEntry {
                    id: 1,
                    name: "void::search::rank".to_owned(),
                    place: "search.rs:42".to_owned(),
                },
                DebugFrameEntry {
                    id: 2,
                    name: "void::main".to_owned(),
                    place: "main.rs:18".to_owned(),
                },
            ],
            frame: Some(1),
            scopes: vec![DebugScopeEntry {
                name: "Locals".to_owned(),
                reference: 1,
                variables: vec![
                    DebugVariableEntry {
                        reference: 7,
                        expanded: true,
                        ..variable("query", "Query { .. }", "Query", 0)
                    },
                    variable("text", "\"rust dock\"", "String", 1),
                    variable("limit", "10", "usize", 1),
                    variable("hits", "3", "usize", 0),
                ],
            }],
            watches: vec![("hits * 2".to_owned(), "6".to_owned())],
            console: vec![
                "Launching void".to_owned(),
                "Stopped at breakpoint 1".to_owned(),
            ],
            inline_values: vec![InlineValue {
                path: "src/search.rs".to_owned(),
                line: 41,
                text: "query = Query { .. }, hits = 3".to_owned(),
            }],
        }),
        breakpoints: vec![DebugBreakpoint {
            path: "src/search.rs".to_owned(),
            line: 41,
            verified: Some(true),
        }],
        error: None,
    }
}
