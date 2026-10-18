//! The messages between a window, or any other client, and the server that owns the agents.

mod address;
mod agents;
mod bytes;
mod credential;
mod debug;
mod extensions;
mod history;
mod machine;
mod message;
mod request;
mod response;

pub use address::{
    CONTAINER_SCHEME, DIRECT_SCHEME, Route, SSH_SCHEME, TUNNEL_SCHEME, WSL_SCHEME, escape_folder,
    host_of, is_destination, route_of, unescape_folder,
};
pub use agents::builtin_agents;
pub use credential::{Build, Credential, Target};
pub use debug::{DebugAction, DebugAdapter, DebugFile, DebugInfo, DebugProcess, DebugScenario};
pub use extensions::{
    CAPABILITIES, ExtensionAssets, ExtensionCommand, ExtensionIcon, ExtensionIconTheme,
    ExtensionInfo, ExtensionTheme, GrammarBundle,
};
pub use history::{ConflictFiles, LocalVersion};
pub use machine::{
    AgentPreferences, ExtensionPreferences, FETCH_MINUTES, GitPreferences, HISTORY_DAYS,
    HISTORY_MEGABYTES, HistoryPreferences, LanguagePreferences, MachineSettings, ShellPreferences,
};
pub use message::{ClientMessage, ServerMessage};
pub use pm_buffer::{Position, Span};
pub use pm_dap::{
    Breakpoint, Category as DebugCategory, Event as DebugEvent, Frame as DebugFrame,
    Line as DebugLine, Placed, Scope as DebugScope, Standing, Thread as DebugThread, Variable,
    View as DebugView, Watched as DebugWatched,
};
pub use pm_lsp::{
    CodeAction, CodeLens, Completion, CompletionKind, Diagnostic, FileEdit, FoldRange, HintKind,
    Hover, InlayHint, Location, Problem, Seek, SemanticToken, Severity, Signature, TextChange,
    TokenKind, Triggers,
};
pub use request::{AgentServer, Attachment, Login, McpServer, RegistryAgent, Request, ToolListing};
pub use response::{Response, SessionInfo, TerminalInfo};

/// The version of the protocol this build speaks; clients and servers refuse each other when they differ.
pub const VERSION: u32 = 1;

/// The port a server listens on when it is not told another.
pub const DEFAULT_PORT: u16 = 6868;
