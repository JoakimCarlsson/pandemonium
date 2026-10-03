//! The shape the preferences take on disk.
//!
//! Distinct from the in-memory state so the file survives that shape changing:
//! the theme family is stored by name rather than by its index into
//! the families on offer, and every field is optional so an older file still
//! loads.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pm_acp::{Agent, McpServer, Reach};
use pm_core::Bootstrap;
use pm_text::Server;
use serde::{Deserialize, Serialize};

use super::ServerList;

use crate::config::fonts::Fonts;
use crate::config::keymap::StoredChanges;
use crate::config::languages::{Formatter, LanguageOverrides};
use crate::config::theme::StoredOverrides;
use crate::config::{
    AgentOptions, EditPredictions, InstallLanguageServers, Preferences, Restored, ThemeMode,
    VimBinding, WindowState,
};
use crate::editor::{CursorShape, Display};
use crate::panes::{Saved, SavedAxis, SavedKind, SavedNode, SavedTab, Tool};
use crate::terminal::SavedShell;
use crate::workspace::Layout;

/// Width of a sidebar in layouts saved before tools became pane tabs.
const LEGACY_SIDEBAR_WIDTH: f32 = 252.0;

/// Height of the bottom panel in layouts saved before tools became pane tabs.
const LEGACY_PANEL_HEIGHT: f32 = 220.0;

/// The preferences as they are written down.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct Stored {
    /// The last options chosen for each agent CLI.
    #[serde(skip_serializing_if = "Option::is_none")]
    agents: Option<BTreeMap<String, AgentOptions>>,
    /// Which theme the editor draws in.
    theme_mode: Option<ThemeMode>,
    /// The name of the theme family the editor draws in.
    theme_family: Option<String>,
    /// The colours repainted over whichever family is chosen.
    #[serde(skip_serializing_if = "Option::is_none")]
    theme_overrides: Option<StoredOverrides>,
    /// The family prose and labels are set in.
    #[serde(skip_serializing_if = "Option::is_none")]
    ui_font_family: Option<String>,
    /// The body size of the interface.
    ui_font_size: Option<f32>,
    /// The family code is set in.
    #[serde(skip_serializing_if = "Option::is_none")]
    buffer_font_family: Option<String>,
    /// The size code is set in.
    buffer_font_size: Option<f32>,
    /// The weight code is set in.
    buffer_font_weight: Option<u16>,
    /// The distance between two lines of code, as a multiple of its size.
    buffer_line_height: Option<f32>,
    /// The size a terminal is set in.
    terminal_font_size: Option<f32>,
    /// How many lines of scrollback a terminal keeps.
    terminal_scrollback: Option<usize>,
    /// The name of the keymap the editor starts from.
    keymap: Option<String>,
    /// The reader's own bindings, over that keymap.
    #[serde(skip_serializing_if = "Option::is_none")]
    keybindings: Option<StoredChanges>,
    /// Whether editing starts in vim mode.
    vim_mode: Option<bool>,
    /// How much vim's unnamed register shares with the system clipboard.
    vim_clipboard: Option<StoredClipboardUse>,
    /// The reader's own vim bindings.
    #[serde(skip_serializing_if = "Option::is_none")]
    vim_keymap: Option<Vec<StoredVimBinding>>,
    /// How wide a step of indentation is where a file does not say.
    tab_size: Option<usize>,
    /// The column prose is wrapped to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    line_length: Option<usize>,
    /// Whether a step of indentation is a tab where a file does not say.
    hard_tabs: Option<bool>,
    /// Whether the gutter numbers the lines.
    line_numbers: Option<bool>,
    /// Whether the numbers count away from the cursor's line.
    relative_line_numbers: Option<bool>,
    /// Whether the cursor's line is washed.
    current_line_highlight: Option<bool>,
    /// Whether the other places the word at the cursor appears are washed.
    occurrence_highlight: Option<bool>,
    /// Whether a line is drawn at every step of indentation.
    indent_guides: Option<bool>,
    /// Whether brackets are coloured by how many pairs they are inside.
    bracket_pair_colorization: Option<bool>,
    /// Whether the lines the view is inside stay pinned above it.
    sticky_scroll: Option<bool>,
    /// Whether the scrollbars are drawn.
    scrollbars: Option<bool>,
    /// Whether the whole file is drawn in miniature beside the text.
    minimap: Option<bool>,
    /// Whether the file's path and the blocks the cursor is in are named
    /// above the text.
    breadcrumbs: Option<bool>,
    /// Whether a diff sets its two sides beside each other.
    split_diff: Option<bool>,
    /// The column a guide is drawn down, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    wrap_guide: Option<usize>,
    /// Whether a language server's hints are written into the lines.
    inlay_hints: Option<bool>,
    /// Whether a language server's notes are written after declarations.
    code_lens: Option<bool>,
    /// Inline prediction settings and optional dedicated server.
    edit_predictions: Option<StoredEditPredictions>,
    /// How the caret is drawn.
    cursor_shape: Option<StoredCursorShape>,
    /// Whether the caret blinks.
    cursor_blink: Option<bool>,
    /// How far a notch of the wheel scrolls, against its usual distance.
    scroll_sensitivity: Option<f32>,
    /// Whether a file is laid out the way its formatter would when it is saved.
    format_on_save: Option<bool>,
    /// Whether a file's imports are put in order when it is saved.
    organize_imports_on_save: Option<bool>,
    /// Whether the fixes a server can make on their own are made when a file
    /// is saved.
    fix_on_save: Option<bool>,
    /// Whether the space at the ends of lines goes when a file is saved.
    remove_trailing_whitespace_on_save: Option<bool>,
    /// Whether a saved file always ends in a line break.
    ensure_final_newline_on_save: Option<bool>,
    /// Whether a new session's worktree is trusted without being asked about.
    trust_worktrees: Option<bool>,
    /// Whether failing checks are sent back to the session agent.
    health_feedback: Option<bool>,
    /// Maximum automatic repair prompts.
    health_retries: Option<usize>,
    /// How missing language servers are installed.
    install_language_servers: Option<InstallLanguageServers>,
    /// The servers to run for a language.
    ///
    /// A list replaces the servers that language names. An `add` list runs
    /// after them:
    ///
    /// ```yaml
    /// language_servers:
    ///   Rust:
    ///     - rust-analyzer
    ///   Python:
    ///     add:
    ///       - mypy
    /// ```
    language_servers: Option<BTreeMap<String, StoredLanguageServers>>,
    /// What each language changes about how a file is written, over the
    /// settings every language shares.
    ///
    /// ```yaml
    /// languages:
    ///   Go:
    ///     tab_size: 8
    ///     hard_tabs: true
    ///   Markdown:
    ///     formatter:
    ///       external:
    ///         command: prettier --parser markdown
    /// ```
    languages: Option<BTreeMap<String, StoredLanguageSettings>>,
    /// Agents the reader added, beside the ones the editor ships.
    ///
    /// The key is the agent's id. An id the editor already ships replaces
    /// that agent.
    ///
    /// ```yaml
    /// agent_servers:
    ///   my-agent:
    ///     command: my-agent
    ///     args: ["acp"]
    ///     env:
    ///       EXAMPLE: "1"
    /// ```
    #[serde(
        default,
        deserialize_with = "read_agents",
        skip_serializing_if = "Option::is_none"
    )]
    agent_servers: Option<BTreeMap<String, StoredAgent>>,
    /// Tool servers every agent is opened with.
    ///
    /// The key is the server's name. One with a `command` is started by the
    /// agent; one with a `url` is reached over HTTP, or over server-sent
    /// events when `type` is `sse`.
    ///
    /// ```yaml
    /// mcp_servers:
    ///   filesystem:
    ///     command: mcp-server-filesystem
    ///     args: ["/srv"]
    ///   docs:
    ///     url: https://example.com/mcp
    ///     headers:
    ///       Authorization: Bearer token
    /// ```
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mcp_servers: Option<BTreeMap<String, StoredMcp>>,
    /// Paths symlinked into a fresh worktree, relative to the repository.
    worktree_link: Option<Vec<PathBuf>>,
    /// Paths copied into it, relative to the repository.
    worktree_copy: Option<Vec<PathBuf>>,
    /// The variable a session's own port is handed to a program in.
    worktree_port: Option<String>,
    /// Whether the first run's setup has been finished.
    finished: Option<bool>,
    /// The roots of the projects the window had open.
    projects: Option<Vec<PathBuf>>,
    /// Named project groups and their membership.
    #[serde(skip_serializing_if = "Option::is_none")]
    project_groups: Option<Vec<crate::project_groups::ProjectGroup>>,
    /// The root of the project the window was pointed at.
    active_project: Option<PathBuf>,
    /// How the window was divided into panes, and what was open in them.
    panes: Option<Saved>,
    /// The shells the window had running, and what they were called.
    #[serde(skip_serializing_if = "Option::is_none")]
    shells: Option<Vec<SavedShell>>,
    /// Whether the primary sidebar was visible.
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_sidebar_open: Option<bool>,
    /// The projects edge in layouts saved before tools became pane tabs.
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_sidebar_side: Option<String>,
    /// Width of the primary sidebar.
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_sidebar_width: Option<f32>,
    /// Whether the bottom panel was visible.
    #[serde(skip_serializing_if = "Option::is_none")]
    bottom_panel_open: Option<bool>,
    /// Height of the bottom panel.
    #[serde(skip_serializing_if = "Option::is_none")]
    bottom_panel_height: Option<f32>,
    /// Whether the secondary sidebar was visible.
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_sidebar_open: Option<bool>,
    /// The worktree edge in layouts saved before tools became pane tabs.
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_sidebar_side: Option<String>,
    /// Width of the secondary sidebar.
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_sidebar_width: Option<f32>,
    /// Which of the worktree's two lists that sidebar was showing.
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_sidebar_view: Option<Tool>,
    /// Height of the Source Control graph.
    history_graph_height: Option<f32>,
    /// Whether the Source Control graph was visible.
    history_graph_open: Option<bool>,
    /// Whether the Source Control changes section was expanded.
    changes_section_open: Option<bool>,
    /// Whether the Graph includes every history reference.
    history_all: Option<bool>,
    /// Logical width of the window when it is not maximized.
    window_width: Option<f32>,
    /// Logical height of the window when it is not maximized.
    window_height: Option<f32>,
    /// Whether the window filled the screen it was on.
    window_maximized: Option<bool>,
}

/// The servers configured for one language, as they are written down.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum StoredLanguageServers {
    /// These run instead of the servers the language names.
    Replace(Vec<StoredServer>),
    /// These run after the servers the language names.
    Add {
        /// The servers to run after the language's own.
        add: Vec<StoredServer>,
    },
}

/// What one language overrides, as it is written down.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct StoredLanguageSettings {
    /// How wide a step of indentation and a tab are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tab_size: Option<usize>,
    /// The column prose is wrapped to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    line_length: Option<usize>,
    /// Whether indentation is written as tabs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hard_tabs: Option<bool>,
    /// Whether the file is laid out when it is saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    format_on_save: Option<bool>,
    /// Whether the imports are put in order when it is saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    organize_imports_on_save: Option<bool>,
    /// Whether the fixes a server can make on its own are made when it is saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fix_on_save: Option<bool>,
    /// Whether the space at the ends of lines goes when it is saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remove_trailing_whitespace_on_save: Option<bool>,
    /// Whether it always ends in a line break when it is saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ensure_final_newline_on_save: Option<bool>,
    /// What lays the file out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    formatter: Option<StoredFormatter>,
}

/// What lays a file out, as it is written down.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum StoredFormatter {
    /// `language_server` or `off`.
    Named(String),
    /// A program the file is piped through.
    External {
        /// The program and its arguments, as one command line.
        external: StoredExternalFormatter,
    },
}

/// The program a file is piped through, as it is written down.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredExternalFormatter {
    /// The program and its arguments, as one command line.
    command: String,
}

impl StoredLanguageSettings {
    /// What this stands for.
    fn into_overrides(self) -> LanguageOverrides {
        LanguageOverrides {
            tab_size: self.tab_size.filter(|width| *width > 0),
            line_length: self.line_length.filter(|width| *width > 0),
            hard_tabs: self.hard_tabs,
            format_on_save: self.format_on_save,
            organize_imports_on_save: self.organize_imports_on_save,
            fix_on_save: self.fix_on_save,
            trim_whitespace: self.remove_trailing_whitespace_on_save,
            final_newline: self.ensure_final_newline_on_save,
            formatter: self.formatter.and_then(StoredFormatter::into_formatter),
        }
    }

    /// How `overrides` are written down.
    fn of(overrides: &LanguageOverrides) -> Self {
        Self {
            tab_size: overrides.tab_size,
            line_length: overrides.line_length,
            hard_tabs: overrides.hard_tabs,
            format_on_save: overrides.format_on_save,
            organize_imports_on_save: overrides.organize_imports_on_save,
            fix_on_save: overrides.fix_on_save,
            remove_trailing_whitespace_on_save: overrides.trim_whitespace,
            ensure_final_newline_on_save: overrides.final_newline,
            formatter: overrides.formatter.as_ref().map(StoredFormatter::of),
        }
    }
}

impl StoredFormatter {
    /// The formatter this stands for, or `None` for one the editor does not know.
    fn into_formatter(self) -> Option<Formatter> {
        match self {
            Self::Named(name) => match name.as_str() {
                "language_server" => Some(Formatter::LanguageServer),
                "off" => Some(Formatter::Off),
                _ => None,
            },
            Self::External { external } => Some(Formatter::External(external.command)),
        }
    }

    /// How `formatter` is written down.
    fn of(formatter: &Formatter) -> Self {
        match formatter {
            Formatter::LanguageServer => Self::Named("language_server".to_owned()),
            Formatter::Off => Self::Named("off".to_owned()),
            Formatter::External(command) => Self::External {
                external: StoredExternalFormatter {
                    command: command.clone(),
                },
            },
        }
    }
}

/// One agent the reader added, as it is written down.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredAgent {
    /// What a tab and a menu call it, when that differs from its id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// The program to run.
    command: String,
    /// The arguments that put the program into protocol mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    /// The environment the program is started with, over the one it inherits.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
}

/// One tool server the reader added, as it is written down.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredMcp {
    /// The program the agent starts, for a server reached over its pipes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    /// The arguments to run the program with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    /// The environment the program is started with.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
    /// Where a server reached over the network listens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    /// `sse` for a server of server-sent events; anything else is HTTP.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    transport: Option<String>,
    /// The headers sent with every request to a network server.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    headers: BTreeMap<String, String>,
    /// What the server is for, where its publisher says.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    /// Where the server's publisher describes it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    website: String,
    /// Whether agents are given the server; written only when it is switched off.
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_enabled")]
    enabled: bool,
}

/// One language server as it is written down.
///
/// A server that takes no arguments is written as the command alone, which
/// is what nearly all of them are; one that takes arguments spells them out.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StoredServer {
    /// The command, run with no arguments.
    Command(String),
    /// The command, the arguments to run it with, and what to configure it
    /// with as it starts.
    Invocation {
        /// The program to run.
        command: String,
        /// The arguments to run it with.
        #[serde(default)]
        arguments: Vec<String>,
        /// The pinned installation recipe.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        install: Option<super::recipe::StoredRecipe>,
        /// What the server is configured with as it starts.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<serde_norway::Value>,
    },
}

/// Inline prediction settings as written to settings.yaml.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredEditPredictions {
    /// Whether requests are enabled.
    #[serde(default = "enabled_by_default")]
    enabled: bool,
    /// A server added to every language when named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    server: Option<StoredServer>,
}

/// The default value of the inline prediction switch.
fn enabled_by_default() -> bool {
    true
}

impl StoredLanguageServers {
    /// The servers this stands for.
    fn into_list(self) -> ServerList {
        match self {
            Self::Replace(servers) => {
                ServerList::Replace(servers.into_iter().map(StoredServer::into_server).collect())
            }
            Self::Add { add } => {
                ServerList::Add(add.into_iter().map(StoredServer::into_server).collect())
            }
        }
    }

    /// How `list` is written down.
    fn of(list: &ServerList) -> Self {
        match list {
            ServerList::Replace(servers) => {
                Self::Replace(servers.iter().map(StoredServer::of).collect())
            }
            ServerList::Add(servers) => Self::Add {
                add: servers.iter().map(StoredServer::of).collect(),
            },
        }
    }
}

impl StoredAgent {
    /// The agent this stands for, named `id` for as long as the editor runs.
    fn into_agent(self, id: String) -> Agent {
        let name = self
            .name
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| id.clone());
        Agent::custom(
            id,
            name,
            self.command,
            self.args,
            self.env.into_iter().collect(),
        )
    }

    /// How `agent` is written down.
    fn of(agent: &Agent) -> Self {
        Self {
            name: (agent.name != agent.id).then(|| agent.name.to_owned()),
            command: agent.program.to_owned(),
            args: agent
                .arguments
                .iter()
                .map(|arg| (*arg).to_owned())
                .collect(),
            env: agent
                .env
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }
}

/// Whether `enabled` is the way a server is written when it says nothing.
fn is_enabled(enabled: &bool) -> bool {
    *enabled
}

impl StoredMcp {
    /// The server this stands for, or `None` when it names neither a program nor an address.
    fn into_server(self, name: String) -> Option<McpServer> {
        let reach = match (self.command, self.url) {
            (Some(program), _) if !program.is_empty() => Reach::Command {
                program,
                arguments: self.args,
                env: self.env.into_iter().collect(),
            },
            (_, Some(url)) if !url.is_empty() => match self.transport.as_deref() {
                Some("sse") => Reach::Events {
                    url,
                    headers: self.headers.into_iter().collect(),
                },
                _ => Reach::Http {
                    url,
                    headers: self.headers.into_iter().collect(),
                },
            },
            _ => return None,
        };
        Some(McpServer {
            name,
            reach,
            description: self.description,
            website: self.website,
            enabled: self.enabled,
        })
    }

    /// How `server` is written down.
    fn of(server: &McpServer) -> Self {
        let empty = Self {
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            url: None,
            transport: None,
            headers: BTreeMap::new(),
            description: server.description.clone(),
            website: server.website.clone(),
            enabled: server.enabled,
        };
        match &server.reach {
            Reach::Command {
                program,
                arguments,
                env,
            } => Self {
                command: Some(program.clone()),
                args: arguments.clone(),
                env: env.iter().cloned().collect(),
                ..empty
            },
            Reach::Http { url, headers } => Self {
                url: Some(url.clone()),
                headers: headers.iter().cloned().collect(),
                ..empty
            },
            Reach::Events { url, headers } => Self {
                url: Some(url.clone()),
                transport: Some("sse".to_owned()),
                headers: headers.iter().cloned().collect(),
                ..empty
            },
        }
    }
}

/// The agents in `deserializer`, less any entry that does not name a program.
fn read_agents<'de, D>(deserializer: D) -> Result<Option<BTreeMap<String, StoredAgent>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(raw) = Option::<BTreeMap<String, serde_norway::Value>>::deserialize(deserializer)?
    else {
        return Ok(None);
    };
    let agents = raw
        .into_iter()
        .filter_map(|(id, value)| {
            if id.is_empty() {
                return None;
            }
            let agent = serde_norway::from_value::<StoredAgent>(value).ok()?;
            (!agent.command.is_empty()).then_some((id, agent))
        })
        .collect::<BTreeMap<_, _>>();
    Ok((!agents.is_empty()).then_some(agents))
}

/// `value` kept for as long as the editor runs.
fn leaked(value: String) -> &'static str {
    value.leak()
}

impl StoredServer {
    /// Validates an extension or user supplied server declaration.
    pub fn validate(&self) -> Result<(), String> {
        let command = match self {
            Self::Command(command) => command,
            Self::Invocation {
                command,
                install,
                options,
                ..
            } => {
                if let Some(recipe) = install {
                    recipe.validate()?;
                }
                if options.as_ref().is_some_and(|value| !value.is_mapping()) {
                    return Err("Server initialization options must be an object.".into());
                }
                command
            }
        };
        if command.trim().is_empty() || command.contains('\0') {
            return Err("A server needs an executable.".into());
        }
        Ok(())
    }

    /// The server this stands for, named for as long as the editor runs.
    pub fn into_server(self) -> Server {
        let (command, arguments, options, install) = match self {
            Self::Command(command) => (command, Vec::new(), None, None),
            Self::Invocation {
                command,
                arguments,
                options,
                install,
            } => (command, arguments, options, install),
        };
        let install = install
            .filter(|recipe| recipe.validate().is_ok())
            .map(super::recipe::StoredRecipe::into_recipe)
            .or_else(|| pm_text::install::recipe(&command));
        Server {
            command: leaked(command),
            arguments: arguments
                .into_iter()
                .map(|argument| &*argument.leak())
                .collect::<Vec<_>>()
                .leak(),
            options: options
                .and_then(|options| serde_json::to_string(&options).ok())
                .map_or(pm_text::NO_OPTIONS, |options| &*options.leak()),
            install,
        }
    }

    /// How `server` is written down.
    pub fn of(server: &Server) -> Self {
        let options = serde_json::from_str::<serde_norway::Value>(server.options).ok();
        let options = options.filter(|options| !matches!(options, serde_norway::Value::Null));
        if server.arguments.is_empty() && options.is_none() {
            return Self::Command(server.command.to_owned());
        }
        Self::Invocation {
            command: server.command.to_owned(),
            arguments: server
                .arguments
                .iter()
                .map(|&argument| argument.to_owned())
                .collect(),
            options,
            install: server.install.map(super::recipe::StoredRecipe::of),
        }
    }
}

impl Stored {
    /// What this file stands for, defaulting anything it leaves out.
    pub(super) fn into_restored(self) -> Restored {
        Restored {
            projects: self.projects.clone().unwrap_or_default(),
            project_groups: self.project_groups.clone().unwrap_or_default(),
            active: self.active_project.clone(),
            layout: self.layout(),
            window: self.window(),
            panes: self.pane_layout(),
            shells: self.shells.clone().unwrap_or_default(),
            language_servers: self.language_servers(),
            agent_servers: self.agent_servers(),
            mcp_servers: self.mcp_servers(),
            onboarded: self.finished.unwrap_or_default(),
            preferences: self.into_preferences(),
        }
    }

    /// The servers this file configures, keyed by the language's own name.
    fn language_servers(&self) -> BTreeMap<String, ServerList> {
        self.language_servers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(language, servers)| (canonical_language(&language), servers.into_list()))
            .collect()
    }

    /// The agents this file adds.
    fn agent_servers(&self) -> Vec<Agent> {
        self.agent_servers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(id, agent)| agent.into_agent(id))
            .collect()
    }

    /// The tool servers this file offers every agent, less any that name no way to reach them.
    fn mcp_servers(&self) -> Vec<McpServer> {
        self.mcp_servers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, server)| server.into_server(name))
            .collect()
    }

    /// What this file gives a fresh worktree, defaulting what it leaves out.
    fn bootstrap(&self) -> Bootstrap {
        let defaults = Bootstrap::default();
        Bootstrap {
            link: self.worktree_link.clone().unwrap_or(defaults.link),
            copy: self.worktree_copy.clone().unwrap_or(defaults.copy),
            port: self.worktree_port.clone().or(defaults.port),
        }
    }

    /// Restores pane layouts, migrating fixed sidebars into tool tabs once.
    fn pane_layout(&self) -> Saved {
        let mut saved = self.panes.clone().unwrap_or_default();
        if saved.version >= 1 {
            return saved;
        }
        if self.bottom_panel_open.unwrap_or(false) {
            let height = self
                .bottom_panel_height
                .unwrap_or(LEGACY_PANEL_HEIGHT)
                .max(120.0);
            saved.root = SavedNode::Split {
                axis: SavedAxis::Column,
                shares: vec![(self.window().height - height).max(320.0), height],
                children: vec![
                    saved.root,
                    tool_pane(
                        &[Tool::Problems, Tool::Debug, Tool::Terminal],
                        Tool::Terminal,
                    ),
                ],
            };
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut before_shares = Vec::new();
        let mut after_shares = Vec::new();
        for (tools, front, width, right, visible) in [
            (
                &[Tool::Projects][..],
                Tool::Projects,
                self.primary_sidebar_width.unwrap_or(LEGACY_SIDEBAR_WIDTH),
                self.primary_sidebar_side.as_deref() == Some("right"),
                self.primary_sidebar_open.unwrap_or(true),
            ),
            (
                &[Tool::Files, Tool::Changes][..],
                self.secondary_sidebar_view.unwrap_or(Tool::Files),
                self.secondary_sidebar_width.unwrap_or(LEGACY_SIDEBAR_WIDTH),
                self.secondary_sidebar_side.as_deref() != Some("left"),
                self.secondary_sidebar_open.unwrap_or(true),
            ),
        ] {
            if !visible {
                continue;
            }
            let node = tool_pane(tools, front);
            let width = width.max(160.0);
            if right {
                after.insert(0, node);
                after_shares.insert(0, width);
            } else {
                before.push(node);
                before_shares.push(width);
            }
        }
        let taken: f32 = before_shares.iter().chain(&after_shares).sum();
        let editor_width = (self.window().width - taken).max(320.0);
        saved.focus += before.len();
        let mut children = before;
        children.push(saved.root);
        children.extend(after);
        let mut shares = before_shares;
        shares.push(editor_width);
        shares.extend(after_shares);
        saved.root = if children.len() == 1 {
            children.remove(0)
        } else {
            SavedNode::Split {
                axis: SavedAxis::Row,
                shares,
                children,
            }
        };
        saved.version = 1;
        saved
    }

    /// The regions this file stands for, defaulting anything it leaves out.
    fn layout(&self) -> Layout {
        let defaults = Layout::default();
        Layout {
            history_graph_height: self
                .history_graph_height
                .unwrap_or(defaults.history_graph_height),
            history_graph_open: self
                .history_graph_open
                .unwrap_or(defaults.history_graph_open),
            changes_section_open: self
                .changes_section_open
                .unwrap_or(defaults.changes_section_open),
            history_all: self.history_all.unwrap_or(defaults.history_all),
        }
    }

    /// The window this file stands for, defaulting anything it leaves out.
    fn window(&self) -> WindowState {
        let defaults = WindowState::default();
        WindowState {
            width: self.window_width.unwrap_or(defaults.width),
            height: self.window_height.unwrap_or(defaults.height),
            maximized: self.window_maximized.unwrap_or(defaults.maximized),
        }
    }

    /// The preferences this file stands for, defaulting anything it leaves out.
    fn into_preferences(self) -> Preferences {
        let defaults = Preferences::default();
        let bootstrap = self.bootstrap();
        Preferences {
            agent_options: self.agents.unwrap_or_default(),
            theme_mode: self.theme_mode.unwrap_or(defaults.theme_mode),
            theme_family: self
                .theme_family
                .as_deref()
                .and_then(crate::theme::find)
                .unwrap_or(defaults.theme_family),
            theme_overrides: self
                .theme_overrides
                .map_or(defaults.theme_overrides, StoredOverrides::into_overrides),
            fonts: Fonts {
                interface_family: self.ui_font_family.or(defaults.fonts.interface_family),
                interface_size: self.ui_font_size.unwrap_or(defaults.fonts.interface_size),
                buffer_family: self.buffer_font_family.or(defaults.fonts.buffer_family),
                buffer_size: self.buffer_font_size.unwrap_or(defaults.fonts.buffer_size),
                buffer_weight: self
                    .buffer_font_weight
                    .unwrap_or(defaults.fonts.buffer_weight),
                buffer_line_height: self
                    .buffer_line_height
                    .unwrap_or(defaults.fonts.buffer_line_height),
                terminal_size: self
                    .terminal_font_size
                    .unwrap_or(defaults.fonts.terminal_size),
            },
            keymap: self
                .keymap
                .as_deref()
                .and_then(crate::keymap::find)
                .unwrap_or(defaults.keymap),
            keybindings: self
                .keybindings
                .map_or(defaults.keybindings, StoredChanges::into_changes),
            vim_mode: self.vim_mode.unwrap_or(defaults.vim_mode),
            vim_clipboard: self
                .vim_clipboard
                .map_or(defaults.vim_clipboard, StoredClipboardUse::into_use),
            vim_bindings: self.vim_keymap.as_ref().map_or_else(Vec::new, |bindings| {
                bindings.iter().map(StoredVimBinding::to_binding).collect()
            }),
            tab_size: self.tab_size.unwrap_or(defaults.tab_size),
            line_length: self
                .line_length
                .filter(|width| *width > 0)
                .unwrap_or(defaults.line_length),
            hard_tabs: self.hard_tabs.unwrap_or(defaults.hard_tabs),
            display: Display {
                line_numbers: self.line_numbers.unwrap_or(defaults.display.line_numbers),
                relative_line_numbers: self
                    .relative_line_numbers
                    .unwrap_or(defaults.display.relative_line_numbers),
                current_line: self
                    .current_line_highlight
                    .unwrap_or(defaults.display.current_line),
                occurrences: self
                    .occurrence_highlight
                    .unwrap_or(defaults.display.occurrences),
                indent_guides: self.indent_guides.unwrap_or(defaults.display.indent_guides),
                bracket_colors: self
                    .bracket_pair_colorization
                    .unwrap_or(defaults.display.bracket_colors),
                sticky_scroll: self.sticky_scroll.unwrap_or(defaults.display.sticky_scroll),
                scrollbars: self.scrollbars.unwrap_or(defaults.display.scrollbars),
                minimap: self.minimap.unwrap_or(defaults.display.minimap),
                breadcrumbs: self.breadcrumbs.unwrap_or(defaults.display.breadcrumbs),
                wrap_guide: self.wrap_guide.or(defaults.display.wrap_guide),
                cursor_shape: self
                    .cursor_shape
                    .map_or(defaults.display.cursor_shape, StoredCursorShape::into_shape),
                whole_lines: false,
            },
            split_diff: self.split_diff.unwrap_or(defaults.split_diff),
            inlay_hints: self.inlay_hints.unwrap_or(defaults.inlay_hints),
            code_lens: self.code_lens.unwrap_or(defaults.code_lens),
            edit_predictions: self
                .edit_predictions
                .map_or(defaults.edit_predictions, |stored| EditPredictions {
                    enabled: stored.enabled,
                    server: stored.server.map(StoredServer::into_server),
                }),
            cursor_blink: self.cursor_blink.unwrap_or(defaults.cursor_blink),
            scroll_sensitivity: self
                .scroll_sensitivity
                .unwrap_or(defaults.scroll_sensitivity),
            format_on_save: self.format_on_save.unwrap_or(defaults.format_on_save),
            organize_imports_on_save: self
                .organize_imports_on_save
                .unwrap_or(defaults.organize_imports_on_save),
            fix_on_save: self.fix_on_save.unwrap_or(defaults.fix_on_save),
            trim_whitespace: self
                .remove_trailing_whitespace_on_save
                .unwrap_or(defaults.trim_whitespace),
            final_newline: self
                .ensure_final_newline_on_save
                .unwrap_or(defaults.final_newline),
            terminal_scrollback: self
                .terminal_scrollback
                .unwrap_or(defaults.terminal_scrollback),
            trust_worktrees: self.trust_worktrees.unwrap_or(defaults.trust_worktrees),
            health_feedback: self.health_feedback.unwrap_or(defaults.health_feedback),
            health_retries: self.health_retries.unwrap_or(defaults.health_retries),
            install_language_servers: self
                .install_language_servers
                .unwrap_or(defaults.install_language_servers),
            bootstrap,
            languages: self
                .languages
                .unwrap_or_default()
                .into_iter()
                .map(|(language, settings)| {
                    (canonical_language(&language), settings.into_overrides())
                })
                .filter(|(_, overrides)| !overrides.is_empty())
                .collect(),
        }
    }
}

impl Stored {
    /// The file to write for the window as it stands.
    pub(super) fn of(restored: &Restored) -> Self {
        let Restored {
            preferences,
            onboarded,
            projects,
            project_groups,
            active,
            layout,
            panes,
            shells,
            window,
            language_servers,
            agent_servers,
            mcp_servers,
        } = restored;
        let bootstrap = &preferences.bootstrap;
        let (fonts, display) = (&preferences.fonts, &preferences.display);
        let overrides = StoredOverrides::of(&preferences.theme_overrides);

        Self {
            agents: (!preferences.agent_options.is_empty())
                .then(|| preferences.agent_options.clone()),
            theme_mode: Some(preferences.theme_mode),
            theme_family: Some(
                crate::theme::family(preferences.theme_family)
                    .name
                    .to_owned(),
            ),
            theme_overrides: (!overrides.is_empty()).then_some(overrides),
            ui_font_family: fonts.interface_family.clone(),
            ui_font_size: Some(fonts.interface_size),
            buffer_font_family: fonts.buffer_family.clone(),
            buffer_font_size: Some(fonts.buffer_size),
            buffer_font_weight: Some(fonts.buffer_weight),
            buffer_line_height: Some(fonts.buffer_line_height),
            terminal_font_size: Some(fonts.terminal_size),
            terminal_scrollback: Some(preferences.terminal_scrollback),
            keymap: Some(crate::keymap::name(preferences.keymap).to_owned()),
            keybindings: Some(StoredChanges::of(&preferences.keybindings))
                .filter(|written| !written.is_empty()),
            vim_mode: Some(preferences.vim_mode),
            vim_clipboard: Some(StoredClipboardUse::of(preferences.vim_clipboard)),
            vim_keymap: (!preferences.vim_bindings.is_empty()).then(|| {
                preferences
                    .vim_bindings
                    .iter()
                    .map(StoredVimBinding::of)
                    .collect()
            }),
            tab_size: Some(preferences.tab_size),
            line_length: Some(preferences.line_length),
            hard_tabs: Some(preferences.hard_tabs),
            line_numbers: Some(display.line_numbers),
            relative_line_numbers: Some(display.relative_line_numbers),
            current_line_highlight: Some(display.current_line),
            occurrence_highlight: Some(display.occurrences),
            indent_guides: Some(display.indent_guides),
            bracket_pair_colorization: Some(display.bracket_colors),
            sticky_scroll: Some(display.sticky_scroll),
            scrollbars: Some(display.scrollbars),
            minimap: Some(display.minimap),
            breadcrumbs: Some(display.breadcrumbs),
            split_diff: Some(preferences.split_diff),
            wrap_guide: display.wrap_guide,
            inlay_hints: Some(preferences.inlay_hints),
            code_lens: Some(preferences.code_lens),
            edit_predictions: Some(StoredEditPredictions {
                enabled: preferences.edit_predictions.enabled,
                server: preferences
                    .edit_predictions
                    .server
                    .as_ref()
                    .map(StoredServer::of),
            }),
            cursor_shape: Some(StoredCursorShape::of(display.cursor_shape)),
            cursor_blink: Some(preferences.cursor_blink),
            scroll_sensitivity: Some(preferences.scroll_sensitivity),
            format_on_save: Some(preferences.format_on_save),
            organize_imports_on_save: Some(preferences.organize_imports_on_save),
            fix_on_save: Some(preferences.fix_on_save),
            remove_trailing_whitespace_on_save: Some(preferences.trim_whitespace),
            ensure_final_newline_on_save: Some(preferences.final_newline),
            trust_worktrees: Some(preferences.trust_worktrees),
            health_feedback: Some(preferences.health_feedback),
            health_retries: Some(preferences.health_retries),
            install_language_servers: Some(preferences.install_language_servers),
            languages: (!preferences.languages.is_empty()).then(|| {
                preferences
                    .languages
                    .iter()
                    .map(|(language, overrides)| {
                        (language.clone(), StoredLanguageSettings::of(overrides))
                    })
                    .collect()
            }),
            language_servers: (!language_servers.is_empty()).then(|| {
                language_servers
                    .iter()
                    .map(|(language, servers)| {
                        (language.clone(), StoredLanguageServers::of(servers))
                    })
                    .collect()
            }),
            agent_servers: (!agent_servers.is_empty()).then(|| {
                agent_servers
                    .iter()
                    .map(|agent| (agent.id.to_owned(), StoredAgent::of(agent)))
                    .collect()
            }),
            mcp_servers: (!mcp_servers.is_empty()).then(|| {
                mcp_servers
                    .iter()
                    .map(|server| (server.name.clone(), StoredMcp::of(server)))
                    .collect()
            }),
            worktree_link: Some(bootstrap.link.clone()),
            worktree_copy: Some(bootstrap.copy.clone()),
            worktree_port: bootstrap.port.clone(),
            finished: Some(*onboarded),
            projects: Some(projects.clone()),
            project_groups: (!project_groups.is_empty()).then(|| project_groups.clone()),
            active_project: active.clone(),
            panes: Some(panes.clone()),
            shells: Some(shells.clone()),
            primary_sidebar_open: None,
            primary_sidebar_width: None,
            bottom_panel_open: None,
            bottom_panel_height: None,
            secondary_sidebar_open: None,
            secondary_sidebar_width: None,
            secondary_sidebar_view: None,
            primary_sidebar_side: None,
            secondary_sidebar_side: None,
            history_graph_height: Some(layout.history_graph_height),
            history_graph_open: Some(layout.history_graph_open),
            changes_section_open: Some(layout.changes_section_open),
            history_all: Some(layout.history_all),
            window_width: Some(window.width),
            window_height: Some(window.height),
            window_maximized: Some(window.maximized),
        }
    }
}

/// `name` as the language it asks for, or unchanged when no language is called that.
fn canonical_language(name: &str) -> String {
    pm_text::Language::called(name)
        .map_or_else(|| name.to_owned(), |language| language.name().to_owned())
}

/// How much vim's unnamed register shares with the system clipboard, as it
/// is written down.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum StoredClipboardUse {
    /// Every yank and delete.
    Always,
    /// Only yanks.
    OnYank,
    /// Nothing but `"+` and `"*`.
    Never,
}

impl StoredClipboardUse {
    /// The written name of `sharing`.
    fn of(sharing: pm_vim::ClipboardUse) -> Self {
        match sharing {
            pm_vim::ClipboardUse::Always => Self::Always,
            pm_vim::ClipboardUse::OnYank => Self::OnYank,
            pm_vim::ClipboardUse::Never => Self::Never,
        }
    }

    /// The choice the written name stands for.
    fn into_use(self) -> pm_vim::ClipboardUse {
        match self {
            Self::Always => pm_vim::ClipboardUse::Always,
            Self::OnYank => pm_vim::ClipboardUse::OnYank,
            Self::Never => pm_vim::ClipboardUse::Never,
        }
    }
}

/// One of the reader's vim bindings, as it is written down.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredVimBinding {
    /// The keystrokes.
    keys: String,
    /// The action.
    action: String,
    /// When it applies, normal mode when left out.
    #[serde(default = "normal_mode")]
    when: String,
}

impl StoredVimBinding {
    /// The written form of `binding`.
    fn of(binding: &VimBinding) -> Self {
        Self {
            keys: binding.keys.clone(),
            action: binding.action.clone(),
            when: binding.when.clone(),
        }
    }

    /// The binding the written form stands for.
    fn to_binding(&self) -> VimBinding {
        VimBinding {
            keys: self.keys.clone(),
            action: self.action.clone(),
            when: self.when.clone(),
        }
    }
}

/// The clause a binding written without one applies in.
fn normal_mode() -> String {
    "normal".to_owned()
}

/// How the caret is drawn, as it is written down.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum StoredCursorShape {
    /// A thin upright line between two characters.
    Bar,
    /// A cell-wide block over the character after the caret.
    Block,
    /// A line under the character after the caret.
    Underline,
}

impl StoredCursorShape {
    /// The written name of `shape`.
    fn of(shape: CursorShape) -> Self {
        match shape {
            CursorShape::Bar => Self::Bar,
            CursorShape::Block => Self::Block,
            CursorShape::Underline => Self::Underline,
        }
    }

    /// The shape the written name stands for.
    fn into_shape(self) -> CursorShape {
        match self {
            Self::Bar => CursorShape::Bar,
            Self::Block => CursorShape::Block,
            Self::Underline => CursorShape::Underline,
        }
    }
}

/// Describes a tab group of workspace tools in a migrated layout.
fn tool_pane(tools: &[Tool], front: Tool) -> SavedNode {
    SavedNode::Pane {
        tabs: tools
            .iter()
            .map(|tool| SavedTab {
                kind: SavedKind::Tool,
                tool: Some(*tool),
                front: *tool == front,
                ..SavedTab::default()
            })
            .collect(),
    }
}
