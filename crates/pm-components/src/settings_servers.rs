//! The pages of the settings panel that list servers: the agents that can be started and the tool servers every agent is started with, each with the ones added, a form to add or change one, and what the public registry offers.

use pm_acp::{AGENTS, Source};
use pm_icon::IconName;
use pm_input::FocusId;
use pm_protocol::{AgentServer, McpServer, RegistryAgent, ToolListing};
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_1, flex_col, flex_none, gap, h,
    items_center, items_start, justify_between, min_h, min_w_px, overflow_hidden, p, rounded,
    w_full,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::{Size, Variant};
use crate::input::{self, Input};
use crate::parts::{clipped, line, wrap};
use crate::settings::{
    SETTINGS_ADD, SETTINGS_CANCEL, SETTINGS_EDIT_BASE, SETTINGS_ENV, SETTINGS_INSTALL_BASE,
    SETTINGS_NAME, SETTINGS_REMOVE_BASE, SETTINGS_SAVE, SETTINGS_SCROLL, SETTINGS_SEARCH,
    SETTINGS_SEARCH_GO, SETTINGS_TARGET, Section,
};
use crate::state::{Outgoing, WorkspaceState};
use crate::tile::Tile;

/// How wide a paragraph of the pages may be, in logical pixels, so that it breaks before it reaches the button beside it.
const PARAGRAPH_WIDTH: f32 = 600.0;

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
    /// The agents added by hand.
    pub agents: Vec<AgentServer>,
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
}

impl ServerSettings {
    /// What the pages hold as the panel opens: nothing yet, Agents folded, and both registries being asked.
    pub fn opening() -> Self {
        Self {
            folded: true,
            registry: Offers::asking(),
            listings: Offers::asking(),
            ..Self::default()
        }
    }
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
}

impl ServerForm {
    /// A form holding `name`, `target` and `env`, replacing the server at `index` when there is one.
    fn holding(index: Option<usize>, name: &str, target: &str, env: &[(String, String)]) -> Self {
        Self {
            index,
            name: Field::new(name),
            target: Field::new(target),
            env: Field::new(written(env)),
        }
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
    /// The state with `servers` as the agents added by hand.
    pub fn with_agent_servers(mut self, servers: Vec<AgentServer>) -> Self {
        self.settings_servers.agents = servers;
        self
    }

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
            _ if number >= SETTINGS_INSTALL_BASE => {
                self.install((number - SETTINGS_INSTALL_BASE) as usize);
            }
            _ => {}
        }
        self
    }

    /// A form holding the `index`-th server of the list showing, if there is one.
    fn form_for(&self, index: usize) -> Option<ServerForm> {
        match self.settings? {
            Section::AgentServers => {
                let agent = self.settings_servers.agents.get(index)?;
                Some(ServerForm::holding(
                    Some(index),
                    &agent.name,
                    &agent.command,
                    &agent.env,
                ))
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
            Section::General | Section::Agents => None,
        }
    }

    /// Adds the agent `name` run by `command` to the ones added by hand, and has the server keep them.
    fn keep_agent(&mut self, name: String, command: String, env: Vec<(String, String)>) {
        let servers = &mut self.settings_servers.agents;
        let taken = AGENTS
            .iter()
            .map(|agent| agent.id)
            .chain(servers.iter().map(|agent| agent.id.as_str()))
            .collect::<Vec<_>>();
        let id = identifier(&name, &taken);
        servers.push(AgentServer {
            id,
            name,
            command,
            env,
        });
        self.outbox
            .push(Outgoing::SaveAgentServers(servers.clone()));
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
        match self.settings {
            Some(Section::AgentServers) => {
                let servers = &mut self.settings_servers.agents;
                match form.index.filter(|index| *index < servers.len()) {
                    Some(index) => {
                        servers[index].name = name;
                        servers[index].command = target;
                        servers[index].env = env;
                        self.outbox
                            .push(Outgoing::SaveAgentServers(servers.clone()));
                    }
                    None => self.keep_agent(name, target, env),
                }
            }
            Some(Section::McpServers) => {
                let servers = &mut self.settings_servers.tools;
                let tool = McpServer { name, target, env };
                match form.index.filter(|index| *index < servers.len()) {
                    Some(index) => servers[index] = tool,
                    None => servers.push(tool),
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
                self.keep_agent(offered.name, offered.command, offered.env);
            }
            Some(Section::McpServers) => {
                let Some(offered) = self.settings_servers.listings.items.get(index).cloned() else {
                    return;
                };
                if offered.asks {
                    self.settings_servers.form = Some(ServerForm::holding(
                        None,
                        &offered.name,
                        &offered.target,
                        &offered.env,
                    ));
                    return;
                }
                self.settings_servers.tools.push(McpServer {
                    name: offered.name,
                    target: offered.target,
                    env: offered.env,
                });
                self.outbox.push(Outgoing::SaveMcpServers(
                    self.settings_servers.tools.clone(),
                ));
            }
            _ => {}
        }
    }

    /// Takes the `index`-th server out of the list showing, and has the server keep the list.
    fn remove_server(&mut self, index: usize) {
        self.settings_servers.form = None;
        match self.settings {
            Some(Section::AgentServers) if index < self.settings_servers.agents.len() => {
                self.settings_servers.agents.remove(index);
                self.outbox.push(Outgoing::SaveAgentServers(
                    self.settings_servers.agents.clone(),
                ));
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
            _ => None,
        }
    }

    /// What the search box holds, in lower case.
    fn needle(&self) -> String {
        self.settings_servers.search.text().trim().to_lowercase()
    }
}

/// Whether `id` names a box of the pages.
pub(crate) fn is_form_field(id: FocusId) -> bool {
    [
        SETTINGS_NAME,
        SETTINGS_TARGET,
        SETTINGS_ENV,
        SETTINGS_SEARCH,
    ]
    .contains(&id)
}

/// What a section says about itself under its heading.
fn summary(section: Section) -> &'static str {
    match section {
        Section::General => "",
        Section::Agents => {
            "Tailor how agents work in your projects: the agents that can be started, and the tools they are given."
        }
        Section::AgentServers => {
            "The agents that can be started, and the ones you add. Any program that speaks the Agent Client Protocol can be added."
        }
        Section::McpServers => {
            "Tool servers every new session is started with. Give a command to run, or the address of one that is already running."
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

/// What the right side of the panel holds under the heading of `section`.
pub(crate) fn body(state: &WorkspaceState, section: Section) -> Node {
    let page = node().styles([
        flex(),
        flex_col(),
        flex_1(),
        min_w_px(0),
        min_h(0),
        overflow_hidden(),
        gap(4),
        p(6),
        bg(Role::Background),
    ]);
    match section {
        Section::General => page,
        Section::Agents => page.child(paragraph(summary(section))).children(
            [Section::AgentServers, Section::McpServers]
                .map(|inner| card(inner, count_of(state, inner))),
        ),
        Section::AgentServers | Section::McpServers => list(state, section, page)
            .scroll_from_start(SETTINGS_SCROLL, state.settings_servers.scroll),
    }
}

/// How many servers `section` lists, in words.
fn count_of(state: &WorkspaceState, section: Section) -> String {
    match section {
        Section::AgentServers => format!(
            "{} available",
            AGENTS.len() + state.settings_servers.agents.len()
        ),
        Section::McpServers => format!("{} installed", state.settings_servers.tools.len()),
        Section::General | Section::Agents => String::new(),
    }
}

/// A card of the overview: what `section` is called and for, and how much is set in it; pressing it opens it.
fn card(section: Section, count: String) -> Node {
    Tile::new(section.id())
        .build()
        .styles([
            flex_col(),
            items_start(),
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
                _ => "Tool servers every agent is started with.",
            },
            12,
            1.4,
            Role::Muted,
        ))
}

/// The page of one list: what it is for, the button that adds to it, the search box, the form while one is open, the servers that are installed and what the registry offers.
fn list(state: &WorkspaceState, section: Section, page: Node) -> Node {
    let form = state.settings_servers.form.as_ref();
    let adding = form.is_some_and(|form| form.index.is_none());
    let mut page = page
        .child(
            node()
                .styles([flex(), items_center(), justify_between()])
                .child(
                    node()
                        .styles([flex_1(), min_w_px(0)])
                        .child(paragraph(summary(section))),
                )
                .child(
                    Button::new(
                        match section {
                            Section::AgentServers => "Add agent",
                            _ => "Add server",
                        },
                        SETTINGS_ADD,
                    )
                    .variant(Variant::Outline)
                    .size(Size::Sm)
                    .with_icon(IconName::Plus)
                    .disabled(adding)
                    .build(),
                ),
        )
        .child(search_box(state, section));
    if let (Some(form), true) = (form, adding) {
        page = page.child(form_node(form, section));
    }
    let needle = state.needle();
    let installed: Vec<Node> = installed_rows(state, section, &needle)
        .into_iter()
        .map(|(title, detail, editable)| {
            let editing = form.is_some_and(|form| form.index == editable);
            match (editable, editing, form) {
                (Some(_), true, Some(form)) => form_node(form, section),
                _ => row(title, detail, editable),
            }
        })
        .collect();
    let offers = offered_rows(state, section, &needle);
    page.child(heading("Installed", installed.len()))
        .children(installed)
        .child(heading("Available", offers.rows.len()))
        .children(offers.rows)
        .children(offers.status.map(|status| note(&status)))
}

/// The box the list is narrowed with, and, for tool servers, the registry searched with.
fn search_box(state: &WorkspaceState, section: Section) -> Node {
    let placeholder = match section {
        Section::AgentServers => "Search agents",
        _ => "Search MCP servers, then press Enter to search the registry",
    };
    Input::new(&state.settings_servers.search, SETTINGS_SEARCH)
        .size(input::Size::Sm)
        .placeholder(placeholder)
        .build()
        .styles([flex_none()])
}

/// The name of a list over its rows, with how many there are.
fn heading(title: &str, count: usize) -> Node {
    node()
        .styles([flex(), items_center(), gap(2), flex_none()])
        .child(line(title, 15, 1.4, Role::Text).styles([pm_style::font_medium()]))
        .child(line(count.to_string(), 12, 1.4, Role::Muted))
}

/// A line that only says something.
fn note(message: &str) -> Node {
    line(message, 12, 1.4, Role::Muted)
}

/// The rows of the servers already there: for the agents the ones the app ships, which cannot be changed, then the ones added by hand; each as its name, what runs it and the place of the ones that can be changed.
fn installed_rows(
    state: &WorkspaceState,
    section: Section,
    needle: &str,
) -> Vec<(String, String, Option<usize>)> {
    let matching = |name: &str, detail: &str| {
        needle.is_empty()
            || name.to_lowercase().contains(needle)
            || detail.to_lowercase().contains(needle)
    };
    match section {
        Section::AgentServers => AGENTS
            .iter()
            .map(|agent| (agent.name.to_owned(), "Built in".to_owned(), None))
            .chain(
                state
                    .settings_servers
                    .agents
                    .iter()
                    .enumerate()
                    .map(|(index, agent)| (agent.name.clone(), agent.command.clone(), Some(index))),
            )
            .filter(|(name, detail, _)| matching(name, detail))
            .collect(),
        _ => state
            .settings_servers
            .tools
            .iter()
            .enumerate()
            .map(|(index, tool)| (tool.name.clone(), tool.target.clone(), Some(index)))
            .filter(|(name, detail, _)| matching(name, detail))
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

/// The rows of what the registry offers that is not installed yet and matches `needle`.
fn offered_rows(state: &WorkspaceState, section: Section, needle: &str) -> Offered {
    let settings = &state.settings_servers;
    let (rows, loading, error): (Vec<Node>, bool, &Option<String>) = match section {
        Section::AgentServers => (
            settings
                .registry
                .items
                .iter()
                .enumerate()
                .filter(|(_, offered)| !installed_agent(settings, offered))
                .filter(|(_, offered)| {
                    needle.is_empty()
                        || offered.name.to_lowercase().contains(needle)
                        || offered.id.contains(needle)
                        || offered.description.to_lowercase().contains(needle)
                })
                .map(|(index, offered)| {
                    offer(index, &offered.name, &offered.description, "Install")
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
                .filter(|(_, offered)| {
                    !settings
                        .tools
                        .iter()
                        .any(|tool| tool.name == offered.name && tool.target == offered.target)
                })
                .map(|(index, offered)| {
                    let button = if offered.asks { "Configure" } else { "Install" };
                    offer(index, &offered.title, &offered.description, button)
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

/// Whether `offered` is already an agent of the app: one it ships, or one added from the registry before.
fn installed_agent(settings: &ServerSettings, offered: &RegistryAgent) -> bool {
    settings
        .agents
        .iter()
        .any(|agent| agent.command == offered.command) || AGENTS.iter().any(|agent| {
        agent.name.eq_ignore_ascii_case(&offered.name)
            || matches!(agent.source, Source::Package(package) if offered.command.contains(package))
    })
}

/// One row of what a registry offers: its name over what it is, and the button that installs it.
fn offer(index: usize, title: &str, description: &str, action: &str) -> Node {
    Tile::strip(14)
        .build()
        .styles([justify_between(), gap(3), flex_none(), bg(Role::Surface)])
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(title, 13, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .child(clipped(description, 12, Role::Muted)),
        )
        .child(
            Button::new(action, FocusId(SETTINGS_INSTALL_BASE + index as u64))
                .variant(Variant::Outline)
                .size(Size::Sm)
                .build(),
        )
}

/// One row of a list: a name over what it runs, and, for a server that can be changed, the buttons that change it.
fn row(title: String, detail: String, editable: Option<usize>) -> Node {
    let strip = Tile::strip(14)
        .build()
        .styles([justify_between(), gap(3), flex_none(), bg(Role::Surface)])
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(title, 13, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .child(clipped(detail, 12, Role::Muted)),
        );
    let Some(index) = editable else {
        return strip;
    };
    strip.child(
        node()
            .styles([flex(), items_center(), gap(1), flex_none()])
            .child(
                Button::icon(IconName::Pencil, FocusId(SETTINGS_EDIT_BASE + index as u64))
                    .size(Size::Icon)
                    .build(),
            )
            .child(
                Button::icon(
                    IconName::Trash,
                    FocusId(SETTINGS_REMOVE_BASE + index as u64),
                )
                .size(Size::Icon)
                .build(),
            ),
    )
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
