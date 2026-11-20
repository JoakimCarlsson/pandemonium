//! The pages of the settings panel that list servers: the agents that can be started and the tool servers every agent is started with, each with the ones added, a form to add or change one, and what the public registry offers.

use crate::areas::SettingsEvent;
use crate::kit::{self, Frame};
use crate::target::Target;
use crate::theme;
use pm_protocol::{McpServer, ToolListing};
use rustle::TextFieldState;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{BoxAlignment, Modifier};

use crate::agent_list::AgentEntry;
use crate::button::Variant;
use crate::input::Input;
use crate::menu::Menu;
use crate::parts::{glyph, line, wrap};
use crate::settings::Section;
use crate::settings_machines::labelled;
use crate::settings_row::switch;
use crate::settings_rows::{INSTALLS, SERVER_MORES, SERVER_SWITCHES};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::theme::{Radius, Role};
use crate::tile::Tile;
use crate::update::Update;
use crate::{Badge, Button};

/// How wide a paragraph of the pages may be, in logical pixels, so that it breaks before it reaches the button beside it.
const PARAGRAPH_WIDTH: f32 = 600.0;

/// The page that explains tool servers.
const TOOL_SERVERS_DOCS: &str = "https://modelcontextprotocol.io/";

/// The rows of the menu of a server.
const MENU_ROWS: [Target; 5] = [
    Target::Settings(SettingsEvent::SettingsMenuToggle),
    Target::Settings(SettingsEvent::SettingsMenuEdit),
    Target::Settings(SettingsEvent::SettingsMenuCopy),
    Target::Settings(SettingsEvent::SettingsMenuWebsite),
    Target::Settings(SettingsEvent::SettingsMenuRemove),
];

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

/// What the server list pages keep: whether Agents is folded, the lists as the server holds them, the tool registry's offers, the search and the form being filled in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerSettings {
    /// Whether the sections inside Agents are hidden from the list.
    pub folded: bool,
    /// The agents that can be started.
    pub agents: Vec<AgentEntry>,
    /// The agents taken out of the list that sessions still run, kept so those sessions can still be named.
    pub retired: Vec<AgentEntry>,
    /// The tool servers every agent is started with.
    pub tools: Vec<McpServer>,
    /// The tool servers the registry offered for the last search.
    pub listings: Offers<ToolListing>,
    /// The words the tool registry was last searched for.
    pub searched: String,
    /// The box the list showing is searched with.
    pub search: TextFieldState,
    /// The form for adding or changing a server, while one is open.
    pub form: Option<ServerForm>,
    /// How far the page showing is scrolled, in logical pixels.
    pub scroll: rustle::ScrollState,
    /// Whether the list of what is installed is folded away.
    pub installed_folded: bool,
    /// Whether the list of what the registry offers is folded away.
    pub available_folded: bool,
    /// The menu open over a server, and where, if one is.
    pub menu: Option<ServerMenu>,
    /// The box for an agent's API key, while one is open.
    pub key_form: Option<crate::settings_agents::KeyForm>,
    /// The box for an added account's name, while one is open.
    pub name_form: Option<crate::settings_agents::NameForm>,
    /// Who each added account is signed in as, by the login's id: nothing while it is being asked, and nothing inside when it is signed out.
    pub login_identities: std::collections::BTreeMap<String, Option<Option<String>>>,
}

impl ServerSettings {
    /// What the pages hold as the panel opens: the `agents` the window already lists, Agents folded, and the tool registry being asked.
    pub fn opening(agents: Vec<AgentEntry>, retired: Vec<AgentEntry>) -> Self {
        Self {
            agents,
            retired,
            folded: true,
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
    name: TextFieldState,
    /// The command that starts it, or the address it listens at.
    target: TextFieldState,
    /// The environment it is started with, as `NAME=value` pairs.
    env: TextFieldState,
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
            name: TextFieldState::new(name),
            target: TextFieldState::new(target),
            env: TextFieldState::new(written(env)),
            description: String::new(),
            website: String::new(),
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

impl WorkspaceState {
    /// The state with `servers` as the tool servers every agent is started with.
    pub fn with_mcp_servers(mut self, servers: Vec<McpServer>) -> Self {
        self.settings_servers.tools = servers;
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

    /// Puts away what belongs to the section that was showing, its forms and boxes, and has the tool registry offer its well known servers again if a search had narrowed them.
    pub(crate) fn section_changed(&mut self) {
        while self.put_away_innermost() {}
        self.settings_servers.menu = None;
        self.settings_servers.scroll.jump_to(0.0);
        self.settings_servers.search = TextFieldState::default();
        if !self.settings_servers.searched.is_empty() {
            self.search_tools(String::new());
        }
    }

    /// Asks the tool registry for what matches `query`.
    fn search_tools(&mut self, query: String) {
        self.settings_servers.searched = query.clone();
        self.settings_servers.listings.loading = true;
        self.outbox.push(Outgoing::SearchTools { query });
    }

    /// Has the server keep the tool servers as the page holds them.
    fn keep_tools(&mut self) {
        self.outbox.push(Outgoing::SaveMcpServers(
            self.settings_servers.tools.clone(),
        ));
    }

    /// The state after `action`, which names a button of a server list or its form, was pressed.
    pub(crate) fn servers_pressed(mut self, action: Target) -> Self {
        match action {
            Target::Settings(SettingsEvent::SettingsAdd) => {
                self.settings_servers.form = Some(ServerForm::holding(None, "", "", &[]))
            }
            Target::Settings(SettingsEvent::SettingsCancel) => self.settings_servers.form = None,
            Target::Settings(SettingsEvent::SettingsToggleInstalled) => {
                self.settings_servers.installed_folded = !self.settings_servers.installed_folded;
            }
            Target::Settings(SettingsEvent::SettingsToggleAvailable) => {
                self.settings_servers.available_folded = !self.settings_servers.available_folded;
            }
            Target::Settings(SettingsEvent::SettingsSave) => self.save_form(),
            Target::Settings(SettingsEvent::SettingsSearchGo)
                if self.settings == Some(Section::McpServers) =>
            {
                let query = self.settings_servers.search.text().trim().to_owned();
                self.search_tools(query);
            }
            _ if let Some(index) = INSTALLS.index_of(action) => self.install(index),
            _ if let Some(index) = SERVER_SWITCHES.index_of(action) => self.toggle_server(index),
            Target::Settings(SettingsEvent::SettingsDocs) => self.outbox.push(Outgoing::Open {
                address: TOOL_SERVERS_DOCS.to_owned(),
            }),
            _ if MENU_ROWS.contains(&action) => self.menu_pressed(action),
            _ => {}
        }
        self
    }

    /// A form holding the `index`-th server of the list showing, if there is one.
    fn form_for(&self, index: usize) -> Option<ServerForm> {
        let tool = (self.settings? == Section::McpServers)
            .then(|| self.settings_servers.tools.get(index))??;
        Some(ServerForm::holding(
            Some(index),
            &tool.name,
            &tool.target,
            &tool.env,
        ))
    }

    /// Keeps what the form holds in the list showing, and has the server keep the list.
    fn save_form(&mut self) {
        let Some(form) = self.settings_servers.form.take_if(|form| form.complete()) else {
            return;
        };
        let (name, target, env) = (
            form.name.text().trim().to_owned(),
            form.target.text().trim().to_owned(),
            parsed(&form.env.text()),
        );
        if self.settings != Some(Section::McpServers) {
            return;
        }
        let servers = &mut self.settings_servers.tools;
        match form.index.filter(|index| *index < servers.len()) {
            Some(index) => {
                servers[index].name = name;
                servers[index].target = target;
                servers[index].env = env;
            }
            None => servers.push(tool(name, target, env, form.description, form.website)),
        }
        self.keep_tools();
    }

    /// Installs the `index`-th entry the tool registry offers: a tool server is kept at once unless it has to be asked for something, in which case its form opens filled in.
    fn install(&mut self, index: usize) {
        if self.settings != Some(Section::McpServers) {
            return;
        }
        let Some(offered) = self.settings_servers.listings.items.get(index).cloned() else {
            return;
        };
        if offered.asks {
            let mut form = ServerForm::holding(None, &offered.name, &offered.target, &offered.env);
            form.description = offered.description;
            form.website = offered.website;
            self.settings_servers.form = Some(form);
            return;
        }
        self.settings_servers.tools.push(tool(
            offered.name,
            offered.target,
            offered.env,
            offered.description,
            offered.website,
        ));
        self.keep_tools();
    }

    /// The state with the menu of the `index`-th server of the list showing open at `at`.
    pub fn with_settings_menu(mut self, index: usize, at: (f32, f32)) -> Self {
        self.settings_servers.menu = Some(ServerMenu { index, at });
        self
    }

    /// Whether `id` names a row of the menu of a server that is open.
    pub(crate) fn settings_menu_row(&self, id: Target) -> bool {
        self.settings_servers.menu.is_some() && MENU_ROWS.contains(&id)
    }

    /// Switches the `index`-th tool server on or off and has the server keep the list; one an extension brings is left as it is, as it has no switch.
    fn toggle_server(&mut self, index: usize) {
        if let Some(tool) = self
            .settings_servers
            .tools
            .get_mut(index)
            .filter(|tool| tool.extension.is_none())
        {
            tool.enabled = !tool.enabled;
            self.keep_tools();
        }
    }

    /// The state after the row `row` of the open menu of a server was pressed, and the menu closed.
    fn menu_pressed(&mut self, row: Target) {
        let Some(open) = self.settings_servers.menu.take() else {
            return;
        };
        match row {
            Target::Settings(SettingsEvent::SettingsMenuToggle) => self.toggle_server(open.index),
            Target::Settings(SettingsEvent::SettingsMenuEdit) => {
                self.settings_servers.form = self.form_for(open.index)
            }
            Target::Settings(SettingsEvent::SettingsMenuRemove) => self.remove_server(open.index),
            Target::Settings(SettingsEvent::SettingsMenuCopy) => {
                if let Some(tool) = self.settings_servers.tools.get(open.index) {
                    self.outbox.push(Outgoing::Copy {
                        text: configuration_of(tool),
                    });
                }
            }
            Target::Settings(SettingsEvent::SettingsMenuWebsite) => {
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
        let removable = self
            .settings_servers
            .tools
            .get(index)
            .is_some_and(|tool| tool.extension.is_none());
        if self.settings == Some(Section::McpServers) && removable {
            self.settings_servers.tools.remove(index);
            self.keep_tools();
        }
    }

    /// The box `id` of the page, if it is open and `id` names one.
    pub(crate) fn server_form_field(&self, id: Target) -> Option<&TextFieldState> {
        if id == Target::Settings(SettingsEvent::SettingsSearch) {
            return Some(&self.settings_servers.search);
        }
        let form = self.settings_servers.form.as_ref()?;
        match id {
            Target::Settings(SettingsEvent::SettingsName) => Some(&form.name),
            Target::Settings(SettingsEvent::SettingsTarget) => Some(&form.target),
            Target::Settings(SettingsEvent::SettingsEnv) => Some(&form.env),
            _ => None,
        }
    }

    /// What the search box holds, in lower case.
    pub(crate) fn needle(&self) -> String {
        self.settings_servers.search.text().trim().to_lowercase()
    }
}

/// A tool server called `name`, started with `target` and `env`, saying what it is for and where its publisher describes it, switched on.
fn tool(
    name: String,
    target: String,
    env: Vec<(String, String)>,
    description: String,
    website: String,
) -> McpServer {
    McpServer {
        name,
        target,
        env,
        description,
        website,
        enabled: true,
        extension: None,
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
        let mut words = pm_harness::command_words(&tool.target).into_iter();
        serde_json::json!({
            "command": words.next().unwrap_or_default(),
            "args": words.collect::<Vec<_>>(),
            "env": table(&tool.env),
        })
    };
    serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": { tool.name.clone(): entry } }))
        .unwrap_or_default()
}

/// Emits the menu of a server of the list showing, when one is open.
pub fn server_menu(state: &WorkspaceState) {
    if let Some(menu) = server_menu_of(state) {
        menu.build();
    }
}

/// The menu of a server of the list showing, described, when one is open.
fn server_menu_of(state: &WorkspaceState) -> Option<Menu> {
    let open = state.settings_servers.menu?;
    let entries: Vec<Option<(&str, SettingsEvent, bool)>> = match state.settings? {
        Section::McpServers => {
            let tool = state.settings_servers.tools.get(open.index)?;
            let shared = [
                Some((
                    "Copy Configuration (JSON)",
                    SettingsEvent::SettingsMenuCopy,
                    false,
                )),
                Some((
                    "Open Website",
                    SettingsEvent::SettingsMenuWebsite,
                    tool.website.is_empty(),
                )),
            ];
            if tool.extension.is_some() {
                shared.to_vec()
            } else {
                let toggle = if tool.enabled { "Disable" } else { "Enable" };
                [
                    Some((toggle, SettingsEvent::SettingsMenuToggle, false)),
                    Some(("Edit…", SettingsEvent::SettingsMenuEdit, false)),
                    None,
                ]
                .into_iter()
                .chain(shared)
                .chain([
                    None,
                    Some(("Uninstall", SettingsEvent::SettingsMenuRemove, false)),
                ])
                .collect()
            }
        }
        _ => vec![
            Some(("Edit…", SettingsEvent::SettingsMenuEdit, false)),
            None,
            Some(("Remove", SettingsEvent::SettingsMenuRemove, false)),
        ],
    };
    let menu = Menu::new(open.at.0, open.at.1);
    Some(entries.into_iter().fold(menu, |menu, entry| match entry {
        Some((label, event, off)) => menu.item(label, None, Target::Settings(event), off),
        None => menu.separator(),
    }))
}

/// Whether `id` names a box of the pages.
pub(crate) fn is_form_field(id: Target) -> bool {
    [
        Target::Settings(SettingsEvent::SettingsName),
        Target::Settings(SettingsEvent::SettingsTarget),
        Target::Settings(SettingsEvent::SettingsEnv),
        Target::Settings(SettingsEvent::SettingsSearch),
    ]
    .contains(&id)
}

/// What a section says about itself under its heading.
fn summary(section: Section) -> &'static str {
    match section {
        Section::General => "How the window opens, closes and keeps itself up to date.",
        Section::Appearance => {
            "The theme, the fonts of the interface and of code, and how closely things are set."
        }
        Section::Languages => {
            "How each language is indented, formatted and read by its language server, and the languages installed on this machine."
        }
        Section::Keymap => {
            "Pick the editor your hands know, or press a binding to give an action keys of your own."
        }
        Section::Terminal => {
            "The shell the terminal starts, and how its text and cursor are drawn."
        }
        Section::Browser => {
            "The browser beside your work: turn it on, have it open the dev servers you and your agents start, and limit where agents may take it."
        }
        Section::Git => {
            "How the server fetches and keeps the history of files, and what git marks in files."
        }
        Section::Worktrees => {
            "How the server names the branches of sessions, where it makes their worktrees and what it copies into them."
        }
        Section::Agents => {
            "Tailor how agents work in your projects: what new sessions start with, what happens when an agent finishes, the agents that can be started and the tools they are given."
        }
        Section::AgentServers => {
            "The agents that can be started in a worktree, and the accounts each one is signed in with."
        }
        Section::McpServers => {
            "An open standard that lets AI use external tools and services. MCP servers provide tools for file operations, databases, APIs, and more."
        }
        Section::ModelServers => crate::settings_models::SUMMARY,
        Section::ModelSlots => crate::settings_slots::SUMMARY,
        Section::Orchestration => crate::settings_orchestration::SUMMARY,
        Section::Usage => crate::settings_usage::SUMMARY,
        Section::Servers => {
            "The servers the window works with: the one on this machine, and ones reached at their address, over ssh, inside WSL or a dev container, or through a tunnel."
        }
        _ if section == Section::Editor || crate::settings_editor::PAGES.contains(&section) => {
            crate::settings_editor::summary(section)
        }
        _ => crate::settings_library::summary(section),
    }
}

/// The tone of the name of a row: the text role while what it names is `on`, quieter while it is not.
pub(crate) fn name_tone(on: bool) -> Role {
    if on { Role::Text } else { Role::Muted }
}

/// Emits the words `content` breaks into, one line under another, in no more than `room` logical pixels across.
#[track_caller]
pub(crate) fn paragraph(content: &str, room: f32) {
    kit::column(Modifier::new(), 0, || {
        for words in wrap(content, 13, room.min(PARAGRAPH_WIDTH)) {
            line(words, 13, 1.5, Role::Muted);
        }
    });
}

/// Emits the breadcrumb over the title of `section`'s page, and the title.
pub(crate) fn page_title(section: Section) {
    kit::column(Modifier::new(), 1, || {
        if let Some(parent) = section.parent() {
            line(parent.title(), 12, 1.4, Role::Muted);
        }
        kit::text(
            Modifier::new(),
            section.title(),
            theme::text_style(22, 1.3)
                .font_weight(rustle::FontWeight::MEDIUM)
                .color(theme::color(Role::Text))
                .max_lines(1),
        );
    });
}

/// Emits what the right side of the window holds under the title of `section`: what it is for, then its settings, `room` logical pixels across, scrolled as the page showing is.
#[track_caller]
pub(crate) fn body(state: &WorkspaceState, section: Section, room: f32) {
    kit::column(
        Modifier::new()
            .weight(1.0)
            .clip()
            .vertical_scroll(&state.settings_servers.scroll)
            .fill_max_width()
            .padding_each(theme::sp(8), 0.0, theme::sp(8), theme::sp(6)),
        0,
        || {
            kit::column(Modifier::new().fill_max_width(), 4, || {
                section_content(state, section, room);
            });
        },
    );
}

/// Emits what the page of `section` holds, one part under another, `room` logical pixels across.
fn section_content(state: &WorkspaceState, section: Section, room: f32) {
    use crate::settings_pages::page_rows;
    match section {
        Section::Results => crate::settings_pages::results(state, room),
        Section::AgentServers => crate::settings_agents::body(state, || {}, room),
        Section::McpServers => list(state, room),
        Section::ModelServers => crate::settings_models::body(state, || {}, room),
        Section::ModelSlots => crate::settings_slots::body(state, || {}, room),
        Section::Orchestration => crate::settings_orchestration::body(state, || {}, room),
        Section::Usage => crate::settings_usage::body(state, || {}, room),
        _ if crate::settings_library::lists(section) => {
            crate::settings_library::body(state, section, || {}, room)
        }
        _ => {
            paragraph(summary(section), room);
            match section {
                Section::Editor => {
                    for page in crate::settings_editor::PAGES {
                        crate::settings_pages::group(page.title(), || {
                            page_rows(state, page, room);
                        });
                    }
                }
                Section::Languages => crate::settings_languages::body(state, room),
                Section::Keymap => crate::settings_keymap::rows(state, room),
                Section::Servers => crate::settings_machines::body(state, room),
                _ => page_rows(state, section, room),
            }
            match section {
                Section::General => release_note(state, room),
                Section::Browser => {
                    if let Some(said) = state.settings_page.browser.as_deref() {
                        wrapped_note(said, room);
                    }
                }
                Section::Agents => explore(state),
                _ => {}
            }
        }
    }
}

/// Emits the overview of the Agents page: a card for each page inside it, with how much is set in it.
#[track_caller]
fn explore(state: &WorkspaceState) {
    kit::text(Modifier::new(), "Explore", kit::strong(15, Role::Text));
    for part in crate::settings::AGENT_PAGES {
        card(part, count_of(state, part));
    }
}

/// Emits what looking for a newer release on the channel followed found, beside the version running, once it was looked for; with the button that opens its page when one is newer.
fn release_note(state: &WorkspaceState, room: f32) {
    let running = crate::update::RUNNING;
    let Some(release) = state.settings_page.release.as_ref() else {
        return;
    };
    let (said, newer) = match release {
        Update::Newer { name, .. } => (format!("{name} is available; this is {running}."), true),
        Update::Current => (format!("Pandemonium {running} is up to date."), false),
        Update::Unknown => (
            "Could not check for a newer release on this channel.".to_owned(),
            false,
        ),
    };
    kit::row(Modifier::new().fill_max_width(), 3, || {
        kit::row(Modifier::new().weight(1.0), 0, || {
            wrapped_note(&said, (room - 160.0).max(120.0));
        });
        if newer {
            kit::action(
                "Open Release Page",
                Target::Settings(SettingsEvent::SettingsRelease),
                Variant::Outline,
            );
        }
    });
}

/// How many servers `section` lists, in words.
fn count_of(state: &WorkspaceState, section: Section) -> String {
    match section {
        Section::AgentServers => format!("{} available", state.offered_agents().len()),
        Section::McpServers => format!("{} installed", state.settings_servers.tools.len()),
        Section::ModelServers => crate::settings_models::count(state),
        Section::ModelSlots => crate::settings_slots::count(state),
        Section::Orchestration => crate::settings_orchestration::count(state),
        Section::Usage => crate::settings_usage::count(state),
        _ => String::new(),
    }
}

/// Emits a box with an outline around the rows `rows` emits, with a hairline between each and the next.
#[track_caller]
pub(crate) fn boxed(rows: impl FnOnce()) {
    let rows = rustle::collect(rows);
    let last = rows.len().saturating_sub(1);
    kit::column(
        Modifier::new().fill_max_width().panel(Radius::Lg).clip(),
        0,
        || {
            for (place, mut held) in rows.into_iter().enumerate() {
                let ruled = if place == last {
                    Modifier::new()
                } else {
                    Modifier::new().ruled(theme::Side::Bottom)
                };
                held.modifier = Modifier::new()
                    .fill_max_width()
                    .chain(ruled)
                    .chain(held.modifier);
                rustle::emit(held);
            }
        },
    );
}

/// Emits a card of the overview: what `section` is called and for, and how much is set in it; pressing it opens it.
#[track_caller]
pub(crate) fn card(section: Section, count: String) {
    let about = match section {
        Section::AgentServers => "The agents that can be started, and their accounts.",
        Section::McpServers => "Tool servers every agent is started with.",
        Section::ModelServers => "OpenAI-compatible servers offered as agents.",
        Section::ModelSlots => "The model each kind of work asks, with backups.",
        Section::Orchestration => "The work categories a lead agent hands work to.",
        Section::Usage => "Tokens and spend across every session.",
        other if crate::settings_library::lists(other) => crate::settings_library::blurb(other),
        other => crate::settings_editor::summary(other),
    };
    Tile::new(section.id())
        .height(20.0)
        .radius(Radius::Lg)
        .modifier(
            Modifier::new()
                .fill_max_width()
                .background_rounded(theme::color(Role::Surface), Radius::Lg.px()),
        )
        .inner(Modifier::new().outlined(Radius::Lg).padding(theme::sp(4)))
        .alignment(Alignment::Start)
        .build_with(|| {
            kit::column(Modifier::new().weight(1.0), 1, || {
                spread(|| {
                    kit::text(
                        Modifier::new(),
                        section.title(),
                        kit::strong(14, Role::Text),
                    );
                    note(&count);
                });
                note(about);
            });
        });
}

/// Emits a row of a boxed list: what `content` emits side by side, three theme steps apart, with three theme steps of room around it and `inset` theme steps before it.
#[track_caller]
pub(crate) fn flat_row(inset: u32, content: impl FnOnce()) {
    kit::row(
        Modifier::new().padding_each(theme::sp(inset), theme::sp(3), theme::sp(3), theme::sp(3)),
        3,
        content,
    );
}

/// Emits `content` in a row across its parent, its children pushed to both ends and centred across it: a heading and the button beside it.
#[track_caller]
pub(crate) fn spread(content: impl FnOnce()) {
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpaceBetween,
        Alignment::Center,
        content,
    );
}

/// Emits the words of a row of a list, taking the room its controls leave: `title` in `tone` followed by what `badges` emits, over what `under` emits.
#[track_caller]
pub(crate) fn titled(title: &str, tone: Role, badges: impl FnOnce(), under: impl FnOnce()) {
    kit::column(Modifier::new().weight(1.0).clip(), 1, || {
        kit::row(Modifier::new(), 2, || {
            kit::text(Modifier::new(), title, kit::strong(13, tone));
            badges();
        });
        under();
    });
}

/// Emits the page of the tool servers, `room` logical pixels across: what it is for, the search box, the form while one is open, the servers that are installed and what the registry offers.
#[track_caller]
fn list(state: &WorkspaceState, room: f32) {
    let settings = &state.settings_servers;
    let form = settings.form.as_ref();
    let adding = form.is_some_and(|form| form.index.is_none());
    let needle = state.needle();
    paragraph(summary(Section::McpServers), room);
    kit::small_button(
        "Learn more about MCP servers",
        Target::Settings(SettingsEvent::SettingsDocs),
        Variant::Ghost,
    )
    .modifier(Modifier::new().align(BoxAlignment::TopStart))
    .build();
    search_box(state, Section::McpServers);
    if let (Some(form), true) = (form, adding) {
        form_node(form);
    }
    let installed = installed_rows(state, &needle);
    add_heading(
        "Installed",
        installed.len(),
        Some((
            !settings.installed_folded,
            Target::Settings(SettingsEvent::SettingsToggleInstalled),
        )),
        Target::Settings(SettingsEvent::SettingsAdd),
        adding,
    );
    if !settings.installed_folded {
        if installed.is_empty() {
            note("Nothing installed matches.");
        } else {
            boxed(|| {
                for shown in installed {
                    match form.filter(|form| form.index == Some(shown.place)) {
                        Some(form) => form_node(form),
                        None => installed_row(shown),
                    }
                }
            });
        }
    }
    let offers = offered_rows(state);
    heading(
        "Available",
        offers.rows.len(),
        Some((
            !settings.available_folded,
            Target::Settings(SettingsEvent::SettingsToggleAvailable),
        )),
    );
    wrapped_note(
        "Featured MCP servers from the marketplace. Search to find more.",
        room,
    );
    if !settings.available_folded {
        if !offers.rows.is_empty() {
            boxed(|| {
                for shown in offers.rows {
                    offer(shown);
                }
            });
        }
        if let Some(status) = offers.status {
            note(&status);
        }
    }
}

/// Emits the box the list is narrowed with, and, for tool servers, the registry searched with.
#[track_caller]
pub(crate) fn search_box(state: &WorkspaceState, section: Section) {
    let placeholder = match section {
        Section::AgentServers | Section::AgentDefinitions => "Search agents",
        Section::Skills => "Search skills",
        Section::Instructions => "Search instructions",
        Section::Hooks => "Search hooks",
        Section::Tools => "Search tools",
        _ => "Search MCP servers; Enter searches the registry",
    };
    Input::search(
        &state.settings_servers.search,
        Target::Settings(SettingsEvent::SettingsSearch),
    )
    .size(Size::Lg)
    .placeholder(placeholder)
    .build();
}

/// Emits the name of a list with how many rows it holds; while `fold` holds whether the list is open and the toggle that opens and folds it, the name answers to the toggle after a chevron saying which it is.
#[track_caller]
fn heading(title: &str, count: usize, fold: Option<(bool, Target)>) {
    kit::row(
        kit::pressable(
            Modifier::new().height(theme::sp(9)),
            fold.map(|(_, toggle)| toggle),
        ),
        2,
        || {
            if let Some((open, _)) = fold {
                glyph(kit::chevron(open), 4.0, Role::Muted);
            }
            kit::text(Modifier::new(), title, kit::strong(15, Role::Text));
            Badge::new(count.to_string()).build();
        },
    );
}

/// Emits the name of a list that does not fold, with how many rows it holds.
#[track_caller]
pub(crate) fn heading_only(title: &str, count: usize) {
    heading(title, count, None);
}

/// Emits the name of a list as [`heading`] does, and at the other end the button `add` that opens the form adding a server, turned off while one is being added.
#[track_caller]
pub(crate) fn add_heading(
    title: &str,
    count: usize,
    fold: Option<(bool, Target)>,
    add: Target,
    adding: bool,
) {
    spread(|| {
        heading(title, count, fold);
        kit::small_button("Add Server", add, Variant::Outline)
            .disabled(adding)
            .build();
    });
}

/// Emits a line that only says something.
#[track_caller]
pub(crate) fn note(message: &str) {
    line(message, 12, 1.4, Role::Muted);
}

/// Emits a quiet note saying `message`, broken into lines no more than `room` logical pixels across.
#[track_caller]
pub(crate) fn wrapped_note(message: &str, room: f32) {
    kit::column(Modifier::new(), 0, || {
        for words in wrap(message, 12, room) {
            line(words, 12, 1.4, Role::Muted);
        }
    });
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
    /// What it is for.
    description: String,
    /// What runs it, in the code font.
    detail: String,
    /// Its place in the list.
    place: usize,
    /// Whether it is switched on.
    enabled: bool,
    /// Whether its switch is drawn; one an extension brings is switched with the extension, so it has none.
    switchable: bool,
}

/// The rows of the tool servers already there whose name, reach or description holds `needle`.
fn installed_rows(state: &WorkspaceState, needle: &str) -> Vec<InstalledRow> {
    let matching = |row: &InstalledRow| {
        needle.is_empty()
            || row.title.to_lowercase().contains(needle)
            || row.detail.to_lowercase().contains(needle)
            || row.description.to_lowercase().contains(needle)
    };
    state
        .settings_servers
        .tools
        .iter()
        .enumerate()
        .map(|(index, tool)| InstalledRow {
            title: tool.name.clone(),
            description: match &tool.extension {
                Some(extension) => {
                    format!("From the {extension} extension. {}", tool.description)
                }
                None => tool.description.clone(),
            },
            detail: reach_of(&tool.target, tool.env.len()),
            place: index,
            enabled: tool.enabled,
            switchable: tool.extension.is_none(),
        })
        .filter(matching)
        .collect()
}

/// One entry the registry offers, as its row shows it.
struct OfferedRow {
    /// Its place among what the registry offers, which names its install button.
    index: usize,
    /// What it is called.
    title: String,
    /// What it is for.
    description: String,
    /// What its button says.
    action: &'static str,
    /// Whether it is installed already.
    installed: bool,
}

/// What the registry of a list offers, as rows, and the line to show under them when it has something to say.
struct Offered {
    /// The rows, each with its button.
    rows: Vec<OfferedRow>,
    /// That the registry is being asked, could not answer, or has nothing.
    status: Option<String>,
}

/// The rows of what the tool registry offers, with the ones already installed marked.
fn offered_rows(state: &WorkspaceState) -> Offered {
    let settings = &state.settings_servers;
    let rows: Vec<OfferedRow> = settings
        .listings
        .items
        .iter()
        .enumerate()
        .map(|(index, offered)| OfferedRow {
            index,
            title: offered.title.clone(),
            description: offered.description.clone(),
            action: if offered.asks { "Configure" } else { "Install" },
            installed: settings
                .tools
                .iter()
                .any(|tool| tool.name == offered.name && tool.target == offered.target),
        })
        .collect();
    let (loading, error) = (settings.listings.loading, &settings.listings.error);
    let status = match (loading, error, rows.is_empty()) {
        (true, _, _) => Some("Asking the registry...".to_owned()),
        (false, Some(reason), _) => Some(format!("The registry could not be read: {reason}")),
        (false, None, true) => Some("The registry has nothing to offer here.".to_owned()),
        _ => None,
    };
    Offered { rows, status }
}

/// Emits one row of what the registry offers: its name over what it is, and the button that installs it, or says that it is installed; a row past the room for install buttons has none.
#[track_caller]
fn offer(shown: OfferedRow) {
    flat_row(3, || {
        titled(&shown.title, Role::Text, || {}, || note(&shown.description));
        kit::row(Modifier::new(), 3, || {
            if let Some(install) = INSTALLS.id(shown.index) {
                if shown.installed {
                    kit::small_button("Installed", install, Variant::Outline)
                        .disabled(true)
                        .build();
                } else {
                    kit::action(shown.action, install, Variant::Default);
                }
            }
        });
    });
}

/// Emits one row of the list: a name, marked when it is switched off, over what it is for and what runs it, with its switch and menu.
fn installed_row(shown: InstalledRow) {
    flat_row(3, || {
        titled(
            &shown.title,
            name_tone(shown.enabled),
            || {
                if !shown.enabled {
                    Badge::new("Disabled").tone(Role::Subtle).build();
                }
            },
            || {
                if !shown.description.is_empty() {
                    note(&shown.description);
                }
                kit::text(
                    Modifier::new(),
                    shown.detail.as_str(),
                    kit::line(11, Role::Subtle),
                );
            },
        );
        kit::row(Modifier::new(), 2, || {
            if shown.switchable {
                switch(shown.enabled, SERVER_SWITCHES.id(shown.place));
            }
            if let Some(more) = SERVER_MORES.id(shown.place) {
                let operations = crate::events::operations();
                let index = shown.place;
                Button::icon(phosphor::DOTS_THREE, more)
                    .on_click(move || operations.open_settings_menu(index))
                    .build();
            }
        });
    });
}

/// Emits the form: a labelled box for each part of a server, and the buttons that keep it or put it away.
#[track_caller]
fn form_node(form: &ServerForm) {
    kit::column(
        Modifier::new()
            .fill_max_width()
            .panel(Radius::Lg)
            .padding(theme::sp(4)),
        3,
        || {
            labelled(
                "Name",
                &form.name,
                Target::Settings(SettingsEvent::SettingsName),
                "My server",
            );
            labelled(
                "Command or address",
                &form.target,
                Target::Settings(SettingsEvent::SettingsTarget),
                "npx my-server",
            );
            labelled(
                "Environment or headers (NAME=value, separated by spaces)",
                &form.env,
                Target::Settings(SettingsEvent::SettingsEnv),
                "TOKEN=abc",
            );
            kit::confirm_row(
                "Save",
                Target::Settings(SettingsEvent::SettingsSave),
                form.complete(),
                Target::Settings(SettingsEvent::SettingsCancel),
            );
        },
    );
}

/// Emits the buttons that end a form, `cancel` then `save`, the second turned off unless the form is `ready`, two theme steps apart and set against the end of the box `modifier` makes.
#[track_caller]
pub(crate) fn form_buttons(modifier: Modifier, cancel: Target, save: Target, ready: bool) {
    row_arranged(
        modifier,
        Arrangement::SpacedByAligned(theme::sp(2), Alignment::End),
        Alignment::Center,
        || {
            kit::action("Cancel", cancel, Variant::Ghost);
            kit::small_button("Save", save, Variant::Default)
                .disabled(!ready)
                .build();
        },
    );
}
