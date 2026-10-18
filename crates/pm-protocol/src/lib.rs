//! The messages between a window, or any other client, and the server that owns the agents.

mod address;
mod agents;
mod bytes;
mod credential;
mod debug;
mod extensions;
mod machine;
mod message;
mod request;
mod response;

pub use address::{DIRECT_SCHEME, SSH_SCHEME, direct_address, host_of, is_destination};
pub use agents::builtin_agents;
pub use credential::{Build, Credential, Target};
pub use debug::{DebugAction, DebugAdapter, DebugFile, DebugInfo, DebugProcess, DebugScenario};
pub use extensions::{ExtensionInfo, GrammarBundle};
pub use machine::{
    AgentPreferences, FETCH_MINUTES, GitPreferences, LanguagePreferences, MachineSettings,
    ShellPreferences,
};
pub use message::{ClientMessage, ServerMessage};
pub use pm_buffer::{Position, Span};
pub use pm_dap::{
    Breakpoint, Category as DebugCategory, Event as DebugEvent, Frame as DebugFrame,
    Line as DebugLine, Placed, Scope as DebugScope, Standing, Thread as DebugThread, Variable,
    View as DebugView, Watched as DebugWatched,
};
pub use pm_lsp::{
    CodeAction, Completion, CompletionKind, Diagnostic, FileEdit, FoldRange, Hover, Location,
    Problem, Seek, Severity, Signature, TextChange, Triggers,
};
pub use request::{AgentServer, Attachment, Login, McpServer, RegistryAgent, Request, ToolListing};
pub use response::{Response, SessionInfo, TerminalInfo};

/// The version of the protocol this build speaks; clients and servers refuse each other when they differ.
pub const VERSION: u32 = 1;

/// The port a server listens on when it is not told another.
pub const DEFAULT_PORT: u16 = 6868;
