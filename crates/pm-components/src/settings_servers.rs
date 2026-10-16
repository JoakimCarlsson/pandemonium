//! The pages of the settings panel that list servers: the agents that can be started and the tool servers every agent is started with, each with the ones added, a form to add or change one, and what the public registry offers.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_protocol::{AgentServer, McpServer, RegistryAgent, ToolListing};
use pm_style::{
    Radius, Role, bg, border, border_b, border_color, cursor_pointer, flex, flex_1, flex_col,
    flex_none, gap, h, items_center, items_start, justify_between, justify_end, max_w_px, min_h,
    min_w_px, mx_auto, overflow_hidden, p, px, rounded, rounded_full, self_start, size, w_full,
    w_px,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::{Size, Variant};
use crate::input::{self, Input};
use crate::menu::Menu;
use crate::parts::{clipped, glyph, line, wrap};
use crate::settings::{
    SETTINGS_ADD, SETTINGS_CANCEL, SETTINGS_DOCS, SETTINGS_EDIT_BASE, SETTINGS_ENV, SETTINGS_HOME,
    SETTINGS_INSTALL_BASE, SETTINGS_INSTALLER, SETTINGS_MENU_COPY, SETTINGS_MENU_EDIT,
    SETTINGS_MENU_REMOVE, SETTINGS_MENU_TOGGLE, SETTINGS_MENU_WEBSITE, SETTINGS_MORE_BASE,
    SETTINGS_NAME, SETTINGS_PACKAGE, SETTINGS_REMOVE_BASE, SETTINGS_SAVE, SETTINGS_SCROLL,
    SETTINGS_SEARCH, SETTINGS_SEARCH_GO, SETTINGS_SWITCH_BASE, SETTINGS_TARGET,
    SETTINGS_TOGGLE_AVAILABLE, SETTINGS_TOGGLE_INSTALLED, Section,
};
use crate::state::{Outgoing, WorkspaceState};
use crate::tile::Tile;

/// How wide a paragraph of the pages may be, in logical pixels, so that it breaks before it reaches the button beside it.
const PARAGRAPH_WIDTH: f32 = 600.0;

/// How wide the content of a page may be, in logical pixels, however wide the panel is.
const PAGE_WIDTH: f32 = 960.0;

/// The page that explains tool servers.
const TOOL_SERVERS_DOCS: &str = "https://modelcontextprotocol.io/";

/// What a registry last answered: the entries, whether it is being asked, and why it could not answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Offers<T> {
    /// What the registry offered.
    pub items: Vec<T>,
    /// Whether it has been asked and has not answered.
    pub loading: bool,
    /// Why it could not answer, when it could not.
    pub error: Option<String>,
}

impl<T> Default for Offers<T> {
    /// Nothing offered, and nothing asked.
    fn default() -> Self {
        Self {
            items: Vec::new(),
            loading: false,
            error: None,
        }
    }
}

impl<T> Offers<T> {
    /// The offers that were just asked for.
    fn asking() -> Self {
        Self {
            loading: true,
            ..Self::default()
        }
    }

    /// What the registry answered, replacing what was offered.
    fn answered(&mut self, answer: Result<Vec<T>, String>) {
        self.loading = false;
        match answer {
            Ok(items) => {
                self.items = items;
                self.error = None;
            }
            Err(reason) => self.error = Some(reason),
        }
    }
}

/// What the server list pages keep: whether Agents is folded, the lists as the server holds them, the registries' offers, the search and the form being filled in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerSettings {
    /// Whether the sections inside Agents are hidden from the list.
    pub folded: bool,
    /// The agents that can be started.
    pub agents: Vec<AgentServer>,
    /// The agents taken out of the list that sessions still run, kept so those sessions can still be named.
    pub retired: Vec<AgentServer>,
    /// The tool servers every agent is started with.
    pub tools: Vec<McpServer>,
    /// The agents the registry offers.
    pub registry: Offers<RegistryAgent>,
    /// The tool servers the registry offered for the last search.
    pub listings: Offers<ToolListing>,
    /// The words the tool registry was last searched for.
    pub searched: String,
    /// The box the list showing is searched with.
    pub search: Field,
    /// The form for adding or changing a server, while one is open.
    pub form: Option<ServerForm>,
    /// How far the page showing is scrolled, in logical pixels.
    pub scroll: f32,
    /// Whether the list of what is installed is folded away.
    pub installed_folded: bool,
    /// Whether the list of what the registry offers is folded away.
    pub available_folded: bool,
    /// The menu open over a server, and where, if one is.
    pub menu: Option<ServerMenu>,
}

impl ServerSettings {
    /// What the pages hold as the panel opens: the `agents` the window already lists, Agents folded, and both registries being asked.
    pub fn opening(agents: Vec<AgentServer>, retired: Vec<AgentServer>) -> Self {
        Self {
            agents,
            retired,
            folded: true,
            registry: Offers::asking(),
            listings: Offers::asking(),
            ..Self::default()
        }
    }
}

/// The menu of a server of the list showing: which one, and where it opens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerMenu {
    /// The place of the server in its list.
    pub index: usize,
    /// Where its top-left corner is, in logical pixels.
    pub at: (f32, f32),
}

/// A server being described: which one of its list it replaces, and the boxes it is written in.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerForm {
    /// The place in its list of the server being changed, or nothing for a new one.
    index: Option<usize>,
    /// What the server is called.
    name: Field,
    /// The command that starts it, or the address it listens at.
    target: Field,
    /// The environment it is started with, as `NAME=value` pairs.
    env: Field,
    /// For an agent, the package the runner fetches when it is not installed.
    package: Field,
    /// For an agent, the shell command that installs it when it is missing.
    installer: Field,
    /// For an agent, the variable that moves its config and login to another folder.
    home: Field,
    /// What it is for, kept from the registry listing it was opened from.
    description: String,
    /// Where its publisher describes it, kept from the registry listing it was opened from.
    website: String,
}

impl ServerForm {
    /// A form holding `name`, `target` and `env`, replacing the server at `index` when there is one.
    fn holding(index: Option<usize>, name: &str, target: &str, env: &[(String, String)]) -> Self {
        Self {
            index,
            name: Field::new(name),
            target: Field::new(target),
            env: Field::new(written(env)),
            package: Field::default(),
            installer: Field::default(),
            home: Field::default(),
            description: String::new(),
            website: String::new(),
        }
    }

    /// The form with the parts only an agent has filled in from `agent`.
    fn with_agent(mut self, agent: &AgentServer) -> Self {
        self.package = Field::new(agent.package.as_deref().unwrap_or_default());
        self.installer = Field::new(agent.installer.as_deref().unwrap_or_default());
        self.home = Field::new(agent.home.as_deref().unwrap_or_default());
        self
    }

    /// Whether the form holds enough to be kept.
    fn complete(&self) -> bool {
        !self.name.text().trim().is_empty() && !self.target.text().trim().is_empty()
    }
}

/// `env` as one line of `NAME=value` pairs.
fn written(env: &[(String, String)]) -> String {
    env.iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The pairs written in `text`, which are `NAME=value` words; a word with no `=` is left out.
fn parsed(text: &str) -> Vec<(String, String)> {
    text.split_whitespace()
        .filter_map(|word| word.split_once('='))
        .filter(|(name, _)| !name.is_empty())
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

/// What an agent called `name` is kept under: its name in lower case with dashes between the words, made different from every id in `taken`.
fn identifier(name: &str, taken: &[&str]) -> String {
    let base = name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase();
    let base = if base.is_empty() {
        "agent".to_owned()
    } else {
        base
    };
    (1..)
        .map(|attempt| match attempt {
            1 => base.clone(),
            attempt => format!("{base}-{attempt}"),
        })
        .find(|candidate| !taken.contains(&candidate.as_str()))
        .unwrap_or(base)
}

impl WorkspaceState {
    /// The state with `servers` as the tool servers every agent is started with.
    pub fn with_mcp_servers(mut self, servers: Vec<McpServer>) -> Self {
        self.settings_servers.tools = servers;
        self
    }

    /// The state with what the registry answered when asked for its agents.
    pub fn with_registry_agents(mut self, answer: Result<Vec<RegistryAgent>, String>) -> Self {
        self.settings_servers.registry.answered(answer);
        self
    }

    /// The state with what the registry answered when searched for `query`, when that is still the search being waited for.
    pub fn with_tool_listings(
        mut self,
        query: &str,
        answer: Result<Vec<ToolListing>, String>,
    ) -> Self {
        if self.settings_servers.searched == query {
            self.settings_servers.listings.answered(answer);
        }
        self
    }

    /// Puts away what belongs to the section that was showing, and has the tool registry offer its well known servers again if a search had narrowed them.
    pub(crate) fn section_changed(&mut self) {
        self.settings_servers.form = None;
        self.settings_servers.scroll = 0.0;
        self.settings_servers.search = Field::default();
        if !self.settings_servers.searched.is_empty() {
            self.search_tools(String::new());
        }
    }

    /// The state after the page showing in the settings panel is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_settings(mut self, delta: f32, room: f32) -> Self {
        self.settings_servers.scroll = (self.settings_servers.scroll + delta).clamp(0.0, room);
        self
    }

    /// Asks the tool registry for what matches `query`.
    fn search_tools(&mut self, query: String) {
        self.settings_servers.searched = query.clone();
        self.settings_servers.listings.loading = true;
        self.outbox.push(Outgoing::SearchTools { query });
    }

    /// The state after `action`, which names a button of a server list or its form, was pressed.
    pub(crate) fn servers_pressed(mut self, action: FocusId) -> Self {
        let FocusId(number) = action;
        match action {
            SETTINGS_ADD => {
                self.settings_servers.form = Some(ServerForm::holding(None, "", "", &[]))
            }
            SETTINGS_CANCEL => self.settings_servers.form = None,
            SETTINGS_TOGGLE_INSTALLED => {
                self.settings_servers.installed_folded = !self.settings_servers.installed_folded;
            }
            SETTINGS_TOGGLE_AVAILABLE => {
                self.settings_servers.available_folded = !self.settings_servers.available_folded;
            }
            SETTINGS_SAVE => self.save_form(),
            SETTINGS_SEARCH_GO if self.settings == Some(Section::McpServers) => {
                let query = self.settings_servers.search.text().trim().to_owned();
                self.search_tools(query);
            }
            _ if (SETTINGS_EDIT_BASE..SETTINGS_REMOVE_BASE).contains(&number) => {
                let index = (number - SETTINGS_EDIT_BASE) as usize;
                self.settings_servers.form = self.form_for(index);
            }
            _ if (SETTINGS_REMOVE_BASE..SETTINGS_INSTALL_BASE).contains(&number) => {
                self.remove_server((number - SETTINGS_REMOVE_BASE) as usize);
            }
            _ if (SETTINGS_INSTALL_BASE..SETTINGS_SWITCH_BASE).contains(&number) => {
                self.install((number - SETTINGS_INSTALL_BASE) as usize);
            }
            _ if (SETTINGS_SWITCH_BASE..SETTINGS_MORE_BASE).contains(&number) => {
                self.toggle_server((number - SETTINGS_SWITCH_BASE) as usize);
            }
            SETTINGS_DOCS => self.outbox.push(Outgoing::Open {
                address: TOOL_SERVERS_DOCS.to_owned(),
            }),
            SETTINGS_MENU_TOGGLE
            | SETTINGS_MENU_EDIT
            | SETTINGS_MENU_COPY
            | SETTINGS_MENU_WEBSITE
            | SETTINGS_MENU_REMOVE => self.menu_pressed(action),
            _ => {}
        }
        self
    }

    /// A form holding the `index`-th server of the list showing, if there is one.
    fn form_for(&self, index: usize) -> Option<ServerForm> {
        match self.settings? {
            Section::AgentServers => {
                let agent = self.settings_servers.agents.get(index)?;
                Some(
                    ServerForm::holding(Some(index), &agent.name, &agent.command, &agent.env)
                        .with_agent(agent),
                )
            }
            Section::McpServers => {
                let tool = self.settings_servers.tools.get(index)?;
                Some(ServerForm::holding(
                    Some(index),
                    &tool.name,
                    &tool.target,
                    &tool.env,
                ))
            }
            _ => None,
        }
    }

    /// Adds `agent` to the list under an id made from its name, and has the server keep it.
    fn keep_agent(&mut self, mut agent: AgentServer) {
        let mut servers = self.settings_servers.agents.clone();
        let taken = servers
            .iter()
            .map(|held| held.id.as_str())
            .collect::<Vec<_>>();
        agent.id = identifier(&agent.name, &taken);
        servers.push(agent);
        self.outbox
            .push(Outgoing::SaveAgentServers(servers.clone()));
        self.set_agents(servers);
    }

    /// Keeps what the form holds in the list showing, and has the server keep the list.
    fn save_form(&mut self) {
        let Some(form) = self.settings_servers.form.take_if(|form| form.complete()) else {
            return;
        };
        let (name, target, env) = (
            form.name.text().trim().to_owned(),
            form.target.text().trim().to_owned(),
            parsed(form.env.text()),
        );
        let extras = (
            optional(form.package.text()),
            optional(form.installer.text()),
            optional(form.home.text()),
        );
        match self.settings {
            Some(Section::AgentServers) => {
                let servers = &mut self.settings_servers.agents;
                match form.index.filter(|index| *index < servers.len()) {
                    Some(index) => {
                        let (package, installer, home) = extras;
                        servers[index].name = name;
                        servers[index].home = home.or_else(|| known_home(&target, &package));
                        servers[index].command = target;
                        servers[index].env = env;
                        servers[index].package = package;
                        servers[index].installer = installer;
                        self.outbox
                            .push(Outgoing::SaveAgentServers(servers.clone()));
                    }
                    None => {
                        let (package, installer, home) = extras;
                        self.keep_agent(AgentServer {
                            id: String::new(),
                            name,
                            home: home.or_else(|| known_home(&target, &package)),
                            command: target,
                            env,
                            package,
                            installer,
                        });
                    }
                }
            }
            Some(Section::McpServers) => {
                let servers = &mut self.settings_servers.tools;
                match form.index.filter(|index| *index < servers.len()) {
                    Some(index) => {
                        servers[index].name = name;
                        servers[index].target = target;
                        servers[index].env = env;
                    }
                    None => servers.push(McpServer {
                        name,
                        target,
                        env,
                        description: form.description,
                        website: form.website,
                        enabled: true,
                    }),
                }
                self.outbox.push(Outgoing::SaveMcpServers(servers.clone()));
            }
            _ => {}
        }
    }

    /// Installs the `index`-th entry the registry of the list showing offers: an agent is kept at once, a tool server is kept at once unless it has to be asked for something, in which case its form opens filled in.
    fn install(&mut self, index: usize) {
        match self.settings {
            Some(Section::AgentServers) => {
                let Some(offered) = self.settings_servers.registry.items.get(index).cloned() else {
                    return;
                };
                self.keep_agent(AgentServer {
                    id: String::new(),
                    name: offered.name,
                    home: known_home(&offered.command, &None),
                    command: offered.command,
                    env: offered.env,
                    package: None,
                    installer: None,
                });
            }
            Some(Section::McpServers) => {
                let Some(offered) = self.settings_servers.listings.items.get(index).cloned() else {
                    return;
                };
                if offered.asks {
                    let mut form =
                        ServerForm::holding(None, &offered.name, &offered.target, &offered.env);
                    form.description = offered.description;
                    form.website = offered.website;
                    self.settings_servers.form = Some(form);
                    return;
                }
                self.settings_servers.tools.push(McpServer {
                    name: offered.name,
                    target: offered.target,
                    env: offered.env,
                    description: offered.description,
                    website: offered.website,
                    enabled: true,
                });
                self.outbox.push(Outgoing::SaveMcpServers(
                    self.settings_servers.tools.clone(),
                ));
            }
            _ => {}
        }
    }

    /// The state with the menu of the `index`-th server of the list showing open at `at`.
    pub fn with_settings_menu(mut self, index: usize, at: (f32, f32)) -> Self {
        self.settings_servers.menu = Some(ServerMenu { index, at });
        self
    }

    /// Which server's menu button `id` is, if it is one.
    pub fn settings_more_of(&self, id: FocusId) -> Option<usize> {
        let FocusId(number) = id;
        (self.settings.is_some()
            && (SETTINGS_MORE_BASE..SETTINGS_MORE_BASE + 100).contains(&number))
        .then(|| (number - SETTINGS_MORE_BASE) as usize)
    }

    /// Whether `id` names a row of the menu of a server that is open.
    pub(crate) fn settings_menu_row(&self, id: FocusId) -> bool {
        self.settings_servers.menu.is_some()
            && [
                SETTINGS_MENU_TOGGLE,
                SETTINGS_MENU_EDIT,
                SETTINGS_MENU_COPY,
                SETTINGS_MENU_WEBSITE,
                SETTINGS_MENU_REMOVE,
            ]
            .contains(&id)
    }

    /// Switches the `index`-th tool server on or off, and has the server keep the list.
    fn toggle_server(&mut self, index: usize) {
        if let Some(tool) = self.settings_servers.tools.get_mut(index) {
            tool.enabled = !tool.enabled;
            self.outbox.push(Outgoing::SaveMcpServers(
                self.settings_servers.tools.clone(),
            ));
        }
    }

    /// The state after the row `row` of the open menu of a server was pressed, and the menu closed.
    fn menu_pressed(&mut self, row: FocusId) {
        let Some(open) = self.settings_servers.menu.take() else {
            return;
        };
        match row {
            SETTINGS_MENU_TOGGLE => self.toggle_server(open.index),
            SETTINGS_MENU_EDIT => self.settings_servers.form = self.form_for(open.index),
            SETTINGS_MENU_REMOVE => self.remove_server(open.index),
            SETTINGS_MENU_COPY => {
                if let Some(tool) = self.settings_servers.tools.get(open.index) {
                    self.outbox.push(Outgoing::Copy {
                        text: configuration_of(tool),
                    });
                }
            }
            SETTINGS_MENU_WEBSITE => {
                if let Some(tool) = self.settings_servers.tools.get(open.index) {
                    self.outbox.push(Outgoing::Open {
                        address: tool.website.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    /// Takes the `index`-th server out of the list showing, and has the server keep the list.
    fn remove_server(&mut self, index: usize) {
        self.settings_servers.form = None;
        match self.settings {
            Some(Section::AgentServers) if index < self.settings_servers.agents.len() => {
                let mut servers = self.settings_servers.agents.clone();
                servers.remove(index);
                self.outbox
                    .push(Outgoing::SaveAgentServers(servers.clone()));
                self.set_agents(servers);
            }
            Some(Section::McpServers) if index < self.settings_servers.tools.len() => {
                self.settings_servers.tools.remove(index);
                self.outbox.push(Outgoing::SaveMcpServers(
                    self.settings_servers.tools.clone(),
                ));
            }
            _ => {}
        }
    }

    /// The state after `edit` was made to the box `id` of the page.
    pub(crate) fn server_form_edited(&mut self, id: FocusId, edit: Edit) {
        if id == SETTINGS_SEARCH {
            self.settings_servers.search.apply(edit);
            return;
        }
        let Some(form) = self.settings_servers.form.as_mut() else {
            return;
        };
        match id {
            SETTINGS_NAME => form.name.apply(edit),
            SETTINGS_TARGET => form.target.apply(edit),
            SETTINGS_ENV => form.env.apply(edit),
            SETTINGS_PACKAGE => form.package.apply(edit),
            SETTINGS_INSTALLER => form.installer.apply(edit),
            SETTINGS_HOME => form.home.apply(edit),
            _ => {}
        }
    }

    /// The box `id` of the page, if it is open and `id` names one.
    pub(crate) fn server_form_field(&self, id: FocusId) -> Option<&Field> {
        if id == SETTINGS_SEARCH {
            return Some(&self.settings_servers.search);
        }
        let form = self.settings_servers.form.as_ref()?;
        match id {
            SETTINGS_NAME => Some(&form.name),
            SETTINGS_TARGET => Some(&form.target),
            SETTINGS_ENV => Some(&form.env),
            SETTINGS_PACKAGE => Some(&form.package),
            SETTINGS_INSTALLER => Some(&form.installer),
            SETTINGS_HOME => Some(&form.home),
            _ => None,
        }
    }

    /// What the search box holds, in lower case.
    fn needle(&self) -> String {
        self.settings_servers.search.text().trim().to_lowercase()
    }
}

/// `tool` written the way other tools keep theirs: an `mcpServers` object holding it under its name, as JSON.
fn configuration_of(tool: &McpServer) -> String {
    let table = |pairs: &[(String, String)]| {
        pairs
            .iter()
            .filter(|(_, value)| !value.is_empty())
            .map(|(name, value)| (name.clone(), serde_json::json!(value)))
            .collect::<serde_json::Map<_, _>>()
    };
    let entry = if tool.target.starts_with("http://") || tool.target.starts_with("https://") {
        serde_json::json!({ "type": "http", "url": tool.target, "headers": table(&tool.env) })
    } else {
        let mut words = pm_acp::command_words(&tool.target).into_iter();
        serde_json::json!({
            "command": words.next().unwrap_or_default(),
            "args": words.collect::<Vec<_>>(),
            "env": table(&tool.env),
        })
    };
    serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": { tool.name.clone(): entry } }))
        .unwrap_or_default()
}

/// The menu of a server of the list showing, when one is open.
pub fn server_menu(state: &WorkspaceState) -> Option<Node> {
    let open = state.settings_servers.menu?;
    let menu = Menu::new(open.at.0, open.at.1);
    Some(match state.settings? {
        Section::McpServers => {
            let tool = state.settings_servers.tools.get(open.index)?;
            menu.item(
                if tool.enabled { "Disable" } else { "Enable" },
                None,
                SETTINGS_MENU_TOGGLE,
                false,
            )
            .item("Edit…", None, SETTINGS_MENU_EDIT, false)
            .separator()
            .item("Copy Configuration (JSON)", None, SETTINGS_MENU_COPY, false)
            .item(
                "Open Website",
                None,
                SETTINGS_MENU_WEBSITE,
                tool.website.is_empty(),
            )
            .separator()
            .item("Uninstall", None, SETTINGS_MENU_REMOVE, false)
            .build()
        }
        _ => menu
            .item("Edit…", None, SETTINGS_MENU_EDIT, false)
            .separator()
            .item("Remove", None, SETTINGS_MENU_REMOVE, false)
            .build(),
    })
}

/// Whether `id` names a box of the pages.
pub(crate) fn is_form_field(id: FocusId) -> bool {
    [
        SETTINGS_NAME,
        SETTINGS_TARGET,
        SETTINGS_ENV,
        SETTINGS_PACKAGE,
        SETTINGS_INSTALLER,
        SETTINGS_HOME,
        SETTINGS_SEARCH,
    ]
    .contains(&id)
}

/// What a section says about itself under its heading.
fn summary(section: Section) -> &'static str {
    match section {
        Section::General => "",
        Section::Keymap => {
            "Pick the editor your hands know, or press a binding to give an action keys of your own."
        }
        Section::Editor
        | Section::EditorAppearance
        | Section::EditorEditing
        | Section::EditorSaving
        | Section::EditorIntelligence
        | Section::EditorSearch => crate::settings_editor::summary(section),
        Section::Agents => {
            "Tailor how agents work in your projects: the agents that can be started, and the tools they are given."
        }
        Section::AgentServers => {
            "Agents that speak the Agent Client Protocol. The editor starts them in a worktree and talks to them over it."
        }
        Section::McpServers => {
            "An open standard that lets AI use external tools and services. MCP servers provide tools for file operations, databases, APIs, and more."
        }
    }
}

/// The words `content` breaks into, one line under another.
fn paragraph(content: &str) -> Node {
    node().styles([flex(), flex_col(), min_w_px(0)]).children(
        wrap(content, 13, PARAGRAPH_WIDTH)
            .into_iter()
            .map(|words| line(words, 13, 1.5, Role::Muted)),
    )
}

/// The breadcrumb over the title of `section`'s page, and the title.
fn page_title(section: Section) -> Node {
    node()
        .styles([flex(), flex_col(), gap(1), flex_none()])
        .children(
            section
                .parent()
                .map(|parent| line(parent.title(), 12, 1.4, Role::Muted)),
        )
        .child(line(section.title(), 22, 1.3, Role::Text).styles([pm_style::font_medium()]))
}

/// What the right side of the panel holds: the title of `section` and what it is for, then its settings.
pub(crate) fn body(state: &WorkspaceState, section: Section) -> Node {
    let page = node().styles([
        flex(),
        flex_col(),
        flex_1(),
        min_w_px(0),
        min_h(0),
        overflow_hidden(),
        px(8),
        pm_style::pb(6),
    ]);
    let inner = node().styles([
        flex(),
        flex_col(),
        gap(4),
        flex_none(),
        w_full(),
        max_w_px(PAGE_WIDTH),
        mx_auto(),
    ]);
    let inner = inner.child(page_title(section));
    match section {
        Section::General => page.child(inner),
        Section::Editor => page.child(
            inner
                .child(paragraph(crate::settings_editor::summary(section)))
                .child(line("Settings", 15, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .children(
                    [
                        Section::EditorAppearance,
                        Section::EditorEditing,
                        Section::EditorSaving,
                        Section::EditorIntelligence,
                        Section::EditorSearch,
                    ]
                    .map(|part| card(part, String::new())),
                ),
        ),
        Section::Keymap => page
            .child(
                inner
                    .child(paragraph(summary(section)))
                    .children(crate::settings_keymap::rows(state)),
            )
            .scroll_from_start(SETTINGS_SCROLL, state.settings_servers.scroll),
        Section::EditorAppearance
        | Section::EditorEditing
        | Section::EditorSaving
        | Section::EditorIntelligence
        | Section::EditorSearch => page
            .child(
                inner
                    .child(paragraph(crate::settings_editor::summary(section)))
                    .child(crate::settings_editor::rows(state, section)),
            )
            .scroll_from_start(SETTINGS_SCROLL, state.settings_servers.scroll),
        Section::Agents => page.child(
            inner
                .child(paragraph(summary(section)))
                .child(line("Explore", 15, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .children(
                    [Section::AgentServers, Section::McpServers]
                        .map(|part| card(part, count_of(state, part))),
                ),
        ),
        Section::AgentServers | Section::McpServers => page
            .child(list(state, section, inner))
            .scroll_from_start(SETTINGS_SCROLL, state.settings_servers.scroll),
    }
}

/// How many servers `section` lists, in words.
fn count_of(state: &WorkspaceState, section: Section) -> String {
    match section {
        Section::AgentServers => format!("{} available", state.offered_agents().len()),
        Section::McpServers => format!("{} installed", state.settings_servers.tools.len()),
        _ => String::new(),
    }
}

/// A box with an outline around rows that have a hairline between each and the next.
fn boxed(rows: Vec<Node>) -> Node {
    let last = rows.len().saturating_sub(1);
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
            overflow_hidden(),
        ])
        .children(rows.into_iter().enumerate().map(|(place, row)| {
            if place == last {
                row
            } else {
                row.styles([border_b(), border_color(Role::Border)])
            }
        }))
}

/// A card of the overview: what `section` is called and for, and how much is set in it; pressing it opens it.
fn card(section: Section, count: String) -> Node {
    Tile::new(section.id())
        .build()
        .styles([
            flex_col(),
            items_start(),
            flex_none(),
            h(20),
            p(4),
            gap(1),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(
            node()
                .styles([flex(), items_center(), justify_between(), w_full()])
                .child(line(section.title(), 14, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .child(line(count, 12, 1.4, Role::Muted)),
        )
        .child(line(
            match section {
                Section::AgentServers => "The agents that can be started, and the ones you add.",
                Section::McpServers => "Tool servers every agent is started with.",
                other => crate::settings_editor::summary(other),
            },
            12,
            1.4,
            Role::Muted,
        ))
}

/// A row of a boxed list: its contents side by side, with room around them.
fn flat_row() -> Node {
    node().styles([
        flex(),
        items_center(),
        justify_between(),
        gap(3),
        flex_none(),
        px(3),
        pm_style::py(3),
    ])
}

/// The page of one list: what it is for, the search box, the form while one is open, the servers that are installed and what the registry offers.
fn list(state: &WorkspaceState, section: Section, page: Node) -> Node {
    let settings = &state.settings_servers;
    let form = settings.form.as_ref();
    let adding = form.is_some_and(|form| form.index.is_none());
    let needle = state.needle();
    let mut page = page.child(paragraph(summary(section)));
    if section == Section::McpServers {
        page = page.child(
            Button::new("Learn more about MCP servers", SETTINGS_DOCS)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build()
                .styles([flex_none(), self_start()]),
        );
    }
    let mut page = page.child(search_box(state, section));
    if let (Some(form), true) = (form, adding) {
        page = page.child(form_node(form, section));
    }
    let installed: Vec<Node> = installed_rows(state, section, &needle)
        .into_iter()
        .map(|shown| {
            let editing =
                form.is_some_and(|form| shown.place.is_some() && form.index == shown.place);
            match (editing, form) {
                (true, Some(form)) => form_node(form, section),
                _ => row(shown),
            }
        })
        .collect();
    let offers = offered_rows(state, section, &needle);
    let add_label = match section {
        Section::AgentServers => "Add Agent",
        _ => "Add Server",
    };
    page = page.child(
        node()
            .styles([flex(), items_center(), justify_between(), flex_none()])
            .child(heading(
                "Installed",
                installed.len(),
                !settings.installed_folded,
                SETTINGS_TOGGLE_INSTALLED,
            ))
            .child(
                Button::new(add_label, SETTINGS_ADD)
                    .variant(Variant::Outline)
                    .size(Size::Sm)
                    .disabled(adding)
                    .build(),
            ),
    );
    if !settings.installed_folded {
        page = page.child(if installed.is_empty() {
            note("Nothing installed matches.")
        } else {
            boxed(installed)
        });
    }
    page = page
        .child(heading(
            "Available",
            offers.rows.len(),
            !settings.available_folded,
            SETTINGS_TOGGLE_AVAILABLE,
        ))
        .child(note(match section {
            Section::AgentServers => "Agents from the Agent Client Protocol registry.",
            _ => "Featured MCP servers from the marketplace. Search to find more.",
        }));
    if !settings.available_folded {
        if !offers.rows.is_empty() {
            page = page.child(boxed(offers.rows));
        }
        page = page.children(offers.status.map(|status| note(&status)));
    }
    page
}

/// The box the list is narrowed with, and, for tool servers, the registry searched with.
fn search_box(state: &WorkspaceState, section: Section) -> Node {
    let placeholder = match section {
        Section::AgentServers => "Search agents",
        _ => "Search MCP servers, then press Enter to search the registry",
    };
    Input::new(&state.settings_servers.search, SETTINGS_SEARCH)
        .placeholder(placeholder)
        .build()
        .styles([flex_none()])
}

/// The name of a list that opens and folds it, with how many rows it holds.
fn heading(title: &str, count: usize, open: bool, toggle: FocusId) -> Node {
    let chevron = if open {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            flex_none(),
            h(9),
            cursor_pointer(),
        ])
        .focusable(toggle)
        .child(glyph(chevron, 4.0, Role::Muted))
        .child(line(title, 15, 1.4, Role::Text).styles([pm_style::font_medium()]))
        .child(
            node()
                .styles([flex(), px(2), rounded_full(), bg(Role::SurfaceSelected)])
                .child(line(count.to_string(), 12, 1.4, Role::Muted)),
        )
}

/// A line that only says something.
fn note(message: &str) -> Node {
    line(message, 12, 1.4, Role::Muted).styles([flex_none()])
}

/// How a tool server is reached, in the words of its row: what kind it is, where, and how many variables it takes.
fn reach_of(target: &str, variables: usize) -> String {
    let kind = if target.starts_with("http://") || target.starts_with("https://") {
        "http"
    } else {
        "command"
    };
    match variables {
        0 => format!("{kind} · {target}"),
        1 => format!("{kind} · {target} · 1 variable"),
        many => format!("{kind} · {target} · {many} variables"),
    }
}

/// One server already there, as its row shows it.
struct InstalledRow {
    /// What it is called.
    title: String,
    /// How it stands, as small labels beside its name.
    badges: Vec<(&'static str, Role)>,
    /// What it is for.
    description: String,
    /// What runs it, in the code font.
    detail: String,
    /// Its place in the list of the ones that can be changed, when it can be.
    place: Option<usize>,
    /// Whether it is switched on, when it can be switched.
    enabled: Option<bool>,
}

/// The rows of the servers already there: for the agents the ones the app ships, which cannot be changed, then the ones added by hand.
fn installed_rows(state: &WorkspaceState, section: Section, needle: &str) -> Vec<InstalledRow> {
    let matching = |row: &InstalledRow| {
        needle.is_empty()
            || row.title.to_lowercase().contains(needle)
            || row.detail.to_lowercase().contains(needle)
            || row.description.to_lowercase().contains(needle)
    };
    match section {
        Section::AgentServers => state
            .settings_servers
            .agents
            .iter()
            .enumerate()
            .map(|(index, agent)| InstalledRow {
                title: agent.name.clone(),
                badges: Vec::new(),
                description: String::new(),
                detail: agent.command.clone(),
                place: Some(index),
                enabled: None,
            })
            .filter(matching)
            .collect(),
        _ => state
            .settings_servers
            .tools
            .iter()
            .enumerate()
            .map(|(index, tool)| InstalledRow {
                title: tool.name.clone(),
                badges: if tool.enabled {
                    Vec::new()
                } else {
                    vec![("Disabled", Role::Subtle)]
                },
                description: tool.description.clone(),
                detail: reach_of(&tool.target, tool.env.len()),
                place: Some(index),
                enabled: Some(tool.enabled),
            })
            .filter(matching)
            .collect(),
    }
}

/// What the registry of a list offers, as rows, and the line to show under them when it has something to say.
struct Offered {
    /// The rows, each with its button.
    rows: Vec<Node>,
    /// That the registry is being asked, could not answer, or has nothing.
    status: Option<String>,
}

/// The rows of what the registry offers that matches `needle`, with the ones already installed marked.
fn offered_rows(state: &WorkspaceState, section: Section, needle: &str) -> Offered {
    let settings = &state.settings_servers;
    let (rows, loading, error): (Vec<Node>, bool, &Option<String>) = match section {
        Section::AgentServers => (
            settings
                .registry
                .items
                .iter()
                .enumerate()
                .filter(|(_, offered)| {
                    needle.is_empty()
                        || offered.name.to_lowercase().contains(needle)
                        || offered.id.contains(needle)
                        || offered.description.to_lowercase().contains(needle)
                })
                .map(|(index, offered)| {
                    offer(
                        index,
                        &offered.name,
                        &offered.description,
                        &how_run(offered),
                        "Install",
                        installed_agent(settings, offered),
                    )
                })
                .collect(),
            settings.registry.loading,
            &settings.registry.error,
        ),
        _ => (
            settings
                .listings
                .items
                .iter()
                .enumerate()
                .map(|(index, offered)| {
                    let installed = settings
                        .tools
                        .iter()
                        .any(|tool| tool.name == offered.name && tool.target == offered.target);
                    let button = if offered.asks { "Configure" } else { "Install" };
                    offer(
                        index,
                        &offered.title,
                        &offered.description,
                        "",
                        button,
                        installed,
                    )
                })
                .collect(),
            settings.listings.loading,
            &settings.listings.error,
        ),
    };
    let status = match (loading, error, rows.is_empty()) {
        (true, _, _) => Some("Asking the registry...".to_owned()),
        (false, Some(reason), _) => Some(format!("The registry could not be read: {reason}")),
        (false, None, true) => Some("The registry has nothing to offer here.".to_owned()),
        _ => None,
    };
    Offered { rows, status }
}

/// Whether `offered` is already in the list of agents: its command is, or the agent has its name.
fn installed_agent(settings: &ServerSettings, offered: &RegistryAgent) -> bool {
    settings.agents.iter().any(|agent| {
        agent.command == offered.command
            || agent.name.eq_ignore_ascii_case(&offered.name)
            || agent
                .package
                .as_deref()
                .is_some_and(|package| offered.command.contains(package))
    })
}

/// One row of what a registry offers: its name over what it is, and the button that installs it, or says that it is installed.
fn offer(
    index: usize,
    title: &str,
    description: &str,
    how: &str,
    action: &str,
    installed: bool,
) -> Node {
    let button = if installed {
        Button::new("Installed", FocusId(SETTINGS_INSTALL_BASE + index as u64))
            .variant(Variant::Outline)
            .size(Size::Sm)
            .disabled(true)
    } else {
        Button::new(action, FocusId(SETTINGS_INSTALL_BASE + index as u64)).size(Size::Sm)
    };
    flat_row()
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    gap(1),
                    flex_1(),
                    min_w_px(0),
                    overflow_hidden(),
                ])
                .child(line(title, 13, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .child(clipped(description, 12, Role::Muted)),
        )
        .child(
            node()
                .styles([flex(), items_center(), gap(3), flex_none()])
                .children(
                    (!how.is_empty())
                        .then(|| line(how, 11, 1.5, Role::Subtle).styles([pm_style::font_mono()])),
                )
                .child(button.build()),
        )
}

/// What an agent of the registry is, and how it is run, in a few words: its version and the program that runs it.
fn how_run(offered: &RegistryAgent) -> String {
    let program = pm_acp::command_words(&offered.command)
        .into_iter()
        .next()
        .unwrap_or_default();
    match offered.version.is_empty() {
        true => format!("runs through {program}"),
        false => format!("{} · runs through {program}", offered.version),
    }
}

/// A small label in a rounded wash, in `tone`.
fn badge(label: &str, tone: Role) -> Node {
    node()
        .styles([
            flex(),
            flex_none(),
            px(2),
            rounded_full(),
            bg(Role::SurfaceSelected),
        ])
        .child(line(label, 11, 1.5, tone))
}

/// A switch that is on while `on` is true, answering to `id`.
pub(crate) fn switch(on: bool, id: FocusId) -> Node {
    let knob = node().styles([size(4), rounded_full(), bg(Role::Text)]);
    let track = node()
        .styles([
            flex(),
            items_center(),
            flex_none(),
            w_px(36),
            h(5),
            p(0.5),
            rounded_full(),
            cursor_pointer(),
            bg(if on { Role::Success } else { Role::Border }),
        ])
        .focusable(id);
    if on {
        track.styles([justify_end()]).child(knob)
    } else {
        track.child(knob)
    }
}

/// One row of a list: a name with its labels over what it is for and what runs it, and, for a server that can be changed, its switch and menu.
fn row(shown: InstalledRow) -> Node {
    let dim = shown.enabled == Some(false);
    let mut text_column = node()
        .styles([
            flex(),
            flex_col(),
            gap(1),
            flex_1(),
            min_w_px(0),
            overflow_hidden(),
        ])
        .child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(
                    line(
                        shown.title,
                        13,
                        1.4,
                        if dim { Role::Muted } else { Role::Text },
                    )
                    .styles([pm_style::font_medium()]),
                )
                .children(
                    shown
                        .badges
                        .into_iter()
                        .map(|(label, tone)| badge(label, tone)),
                ),
        );
    if !shown.description.is_empty() {
        text_column = text_column.child(clipped(shown.description, 12, Role::Muted));
    }
    let mut actions = node().styles([flex(), items_center(), gap(2), flex_none()]);
    if let (Some(place), Some(on)) = (shown.place, shown.enabled) {
        actions = actions.child(switch(on, FocusId(SETTINGS_SWITCH_BASE + place as u64)));
    }
    if let Some(place) = shown.place {
        actions = actions.child(
            Button::icon(IconName::More, FocusId(SETTINGS_MORE_BASE + place as u64))
                .size(Size::Icon)
                .build(),
        );
    }
    flat_row()
        .child(
            text_column
                .child(clipped(shown.detail, 11, Role::Subtle).styles([pm_style::font_mono()])),
        )
        .child(actions)
}

/// The form: a labelled box for each part of a server, and the buttons that keep it or put it away.
fn form_node(form: &ServerForm, section: Section) -> Node {
    let target = match section {
        Section::AgentServers => "Command",
        _ => "Command or address",
    };
    let environment = match section {
        Section::AgentServers => "Environment (NAME=value, separated by spaces)",
        _ => "Environment or headers (NAME=value, separated by spaces)",
    };
    node()
        .styles([
            flex(),
            flex_col(),
            gap(3),
            p(4),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
            flex_none(),
        ])
        .child(labelled("Name", &form.name, SETTINGS_NAME, "My server"))
        .child(labelled(
            target,
            &form.target,
            SETTINGS_TARGET,
            "npx my-server",
        ))
        .child(labelled(environment, &form.env, SETTINGS_ENV, "TOKEN=abc"))
        .children((section == Section::AgentServers).then(|| agent_extras(form)))
        .child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(
                    Button::new("Save", SETTINGS_SAVE)
                        .size(Size::Sm)
                        .disabled(!form.complete())
                        .build(),
                )
                .child(
                    Button::new("Cancel", SETTINGS_CANCEL)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build(),
                ),
        )
}

/// The boxes only an agent has: where it is fetched from when missing, and how it keeps logins apart.
fn agent_extras(form: &ServerForm) -> Node {
    node()
        .styles([flex(), flex_col(), gap(3)])
        .child(labelled(
            "Package to fetch when it is not installed (optional)",
            &form.package,
            SETTINGS_PACKAGE,
            "@scope/my-agent",
        ))
        .child(labelled(
            "Command that installs it (optional)",
            &form.installer,
            SETTINGS_INSTALLER,
            "curl -fsSL https://example.com/install.sh | bash",
        ))
        .child(labelled(
            "Variable that moves its login folder (optional)",
            &form.home,
            SETTINGS_HOME,
            "MY_AGENT_HOME",
        ))
}

/// `text` without the space around it, or nothing when that leaves nothing.
fn optional(text: &str) -> Option<String> {
    Some(text.trim().to_owned()).filter(|text| !text.is_empty())
}

/// The variable that moves the login folder of the product that `command` and `package` start, when it is one the app knows.
fn known_home(command: &str, package: &Option<String>) -> Option<String> {
    pm_acp::Product::of(&format!(
        "{command} {}",
        package.as_deref().unwrap_or_default()
    ))
    .map(|product| product.home_variable().to_owned())
}

/// A box under the words that say what is written in it.
fn labelled(label: &str, field: &Field, id: FocusId, placeholder: &str) -> Node {
    node()
        .styles([flex(), flex_col(), gap(1)])
        .child(line(label, 12, 1.4, Role::Muted))
        .child(
            Input::new(field, id)
                .size(input::Size::Sm)
                .placeholder(placeholder)
                .build(),
        )
}
