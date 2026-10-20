//! The pages of the settings window that list what agents are given in a project: an overview, the skills, instructions, agent definitions and hooks found in the folders the agent reads, and the tools it may call, each with a switch that leaves it out of new sessions.

use std::path::Path;

use pm_icon::IconName;
use pm_input::FocusId;
use pm_protocol::library::{Item, Kind, Product, Scope, Source, ToolSet, locations};
use pm_style::{
    Role, flex, flex_1, flex_col, flex_none, gap, items_center, min_w_px, overflow_hidden, pl,
};
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::parts::{clipped, glyph, line};
use crate::settings::Section;
use crate::settings_row::{choices, switch};
use crate::settings_servers::{
    boxed, card, flat_row, heading_only, note, paragraph, search_box, wrapped_note,
};
use crate::size::Size;
use crate::state::{Outgoing, Page, WorkspaceState};
use crate::{Badge, Button};

/// The first of the choices of the agent the pages read for; the rest follow it.
const AGENT_PICK_BASE: u64 = 20_000_950;

/// One past the last choice of the agent the pages read for.
const AGENT_PICK_END: u64 = 20_001_000;

/// The first of the switches turning a customization on or off; the rest follow it.
const ITEM_SWITCH_BASE: u64 = 20_001_000;

/// The first of the buttons opening the file of a customization; the rest follow it.
const ITEM_OPEN_BASE: u64 = 20_006_500;

/// The first of the switches turning one tool on or off, counted across every set; the rest follow it.
const TOOL_SWITCH_BASE: u64 = 20_007_500;

/// The first of the switches turning a whole set of tools on or off; the rest follow it.
const SET_SWITCH_BASE: u64 = 20_009_500;

/// The first of the headings that fold a set of tools away or show it; the rest follow it.
const SET_FOLD_BASE: u64 = 20_009_750;

/// One past the last name the pages answer to.
const LIBRARY_END: u64 = 20_010_000;

/// What the pages hold: what was read for which project and agent, and which sets of tools are unfolded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    /// The project and the agent, by position, what is held was read for.
    pub of: Option<(usize, usize)>,
    /// The agent chosen on the pages, by position, in place of the one the prompt is for.
    pub agent: Option<usize>,
    /// The product the agent runs, as its server says.
    pub product: Option<Product>,
    /// The customizations found, or nothing while they are being read.
    pub items: Option<Vec<Item>>,
    /// The sets of tools, or nothing while they are being listed.
    pub sets: Option<Vec<ToolSet>>,
    /// The keys of what is turned off.
    pub withheld: Vec<String>,
    /// Why the server could not say, when it could not.
    pub error: Option<String>,
    /// The sets of tools unfolded, by id.
    pub open_sets: Vec<String>,
}

/// Whether `section` is one of the pages of what agents are given.
pub(crate) fn lists(section: Section) -> bool {
    section == Section::Customizations || section == Section::Tools || kind_of(section).is_some()
}

/// The kind of customization `section` lists, when it lists one.
fn kind_of(section: Section) -> Option<Kind> {
    match section {
        Section::Skills => Some(Kind::Skill),
        Section::Instructions => Some(Kind::Instruction),
        Section::AgentDefinitions => Some(Kind::Agent),
        Section::Hooks => Some(Kind::Hook),
        _ => None,
    }
}

/// The section that lists `kind`.
fn section_of(kind: Kind) -> Section {
    match kind {
        Kind::Skill => Section::Skills,
        Kind::Instruction => Section::Instructions,
        Kind::Agent => Section::AgentDefinitions,
        Kind::Hook => Section::Hooks,
    }
}

/// What one of the pages says about itself under its heading.
pub(crate) fn summary(section: Section) -> &'static str {
    match section {
        Section::Customizations => {
            "What agents are given in the project besides the tool servers: the skills, instructions, agent definitions and hooks in the folders the agent reads, and the tools it may call. Turning one off leaves it out of sessions started afterwards; the files are never changed."
        }
        Section::Skills => {
            "Skills the agent loads when a task calls for them, each a folder with a SKILL.md."
        }
        Section::Instructions => {
            "Files of instructions the agent reads for the folders they apply to."
        }
        Section::AgentDefinitions => {
            "Agent definitions the agent can hand work to, each with the model and tools it names."
        }
        Section::Hooks => {
            "Commands the agent runs when something happens, as its settings files list them."
        }
        Section::Tools => {
            "The tools the agent may call: its own, and each tool server's. A tool turned off is not offered to new sessions; turning a set off turns off every tool in it."
        }
        _ => "",
    }
}

/// What the card of one of the pages on the overview says it holds.
pub(crate) fn blurb(section: Section) -> &'static str {
    match section {
        Section::Skills => "Folders of instructions loaded when a task calls for them.",
        Section::Instructions => "Files read for the folders they apply to.",
        Section::AgentDefinitions => "Agents the agent can hand work to.",
        Section::Hooks => "Commands run when something happens.",
        Section::Tools => "Its own tools, and each tool server's.",
        _ => "",
    }
}

/// What a row says a set of tools from `source` is.
fn source_label(set: &ToolSet) -> String {
    match (set.source, &set.plugin) {
        (Source::BuiltIn, _) => "Built-in".to_owned(),
        (Source::Server, _) => "MCP server".to_owned(),
        (Source::Plugin, Some(plugin)) => format!("From the {plugin} extension"),
        (Source::Plugin, None) => "From an extension".to_owned(),
    }
}

impl WorkspaceState {
    /// The project and the agent the pages are about, by position: the project of the page showing, or the first project when the page has none, and the agent chosen on the pages, or the one the prompt is for until one is.
    fn library_target(&self) -> Option<(usize, usize)> {
        let project = match self.page {
            Page::Project(project) | Page::New(project) => project,
            Page::Session(index) => self.sessions.get(index)?.project,
            Page::History | Page::Extensions => 0,
        };
        self.projects.get(project)?;
        let agent = self
            .settings_page
            .library
            .agent
            .filter(|agent| *agent < self.agent_slots())
            .unwrap_or_else(|| self.agent_now());
        Some((project, agent))
    }

    /// Asks the server of the project the pages are about for what its agent is given there, and for the agent's tools.
    pub(crate) fn read_library(&mut self) {
        let Some((project, agent)) = self.library_target() else {
            self.settings_page.library = Library::default();
            return;
        };
        let Some(held) = self.projects.get(project) else {
            return;
        };
        let library = &mut self.settings_page.library;
        library.of = Some((project, agent));
        library.error = None;
        if library.items.is_none() || library.sets.is_none() {
            library.items = None;
            library.sets = None;
        }
        self.outbox.push(Outgoing::Library {
            agent,
            project,
            host: held.host(),
            directory: held.directory().to_string_lossy().into_owned(),
        });
        self.outbox.push(Outgoing::ToolSets {
            agent,
            project,
            host: held.host(),
        });
    }

    /// The state with what the agent at `agent` is given in the project at `project`, when that is still what the pages are about.
    pub fn with_library(
        mut self,
        project: usize,
        agent: usize,
        product: Option<Product>,
        items: Vec<Item>,
        withheld: Vec<String>,
    ) -> Self {
        let library = &mut self.settings_page.library;
        if library.of == Some((project, agent)) {
            library.product = product;
            library.items = Some(items);
            library.withheld = withheld;
        }
        self
    }

    /// The state with the tools the agent at `agent` is given, when that is still the agent the pages are about.
    pub fn with_tool_sets(
        mut self,
        project: usize,
        agent: usize,
        product: Option<Product>,
        sets: Vec<ToolSet>,
        withheld: Vec<String>,
    ) -> Self {
        let library = &mut self.settings_page.library;
        if library.of == Some((project, agent)) {
            library.product = product;
            library.sets = Some(sets);
            library.withheld = withheld;
        }
        self
    }

    /// The state after the server could not say what the agent is given, for `reason`.
    pub fn with_library_failed(mut self, reason: String) -> Self {
        let library = &mut self.settings_page.library;
        library.error = Some(reason);
        library.items.get_or_insert_with(Vec::new);
        library.sets.get_or_insert_with(Vec::new);
        self
    }

    /// How many things `section` lists, for the list of sections, once they were read; nothing for a section that is not one of the pages.
    pub(crate) fn library_count(&self, section: Section) -> Option<usize> {
        let library = &self.settings_page.library;
        let items = library.items.as_ref();
        let tools = || {
            library
                .sets
                .as_ref()
                .map(|sets| sets.iter().map(|set| set.tools.len()).sum::<usize>())
        };
        match section {
            Section::Customizations => Some(items?.len() + tools()?),
            Section::Tools => tools(),
            _ => {
                let kind = kind_of(section)?;
                Some(items?.iter().filter(|item| item.kind == kind).count())
            }
        }
    }

    /// Has the server of the project the pages are about keep `withheld` as what is turned off.
    fn keep_withheld(&mut self, withheld: Vec<String>) {
        let host = self
            .settings_page
            .library
            .of
            .and_then(|(project, _)| self.projects.get(project))
            .map(crate::project::Project::host);
        let Some(host) = host else {
            return;
        };
        self.settings_page.library.withheld = withheld.clone();
        self.outbox.push(Outgoing::SaveWithheld {
            host,
            keys: withheld,
        });
    }

    /// The tool `number` names, counted across every set, as the set's place and the tool's place in it.
    fn tool_at(&self, number: usize) -> Option<(usize, usize)> {
        let mut left = number;
        for (place, set) in self.settings_page.library.sets.as_ref()?.iter().enumerate() {
            if left < set.tools.len() {
                return Some((place, left));
            }
            left -= set.tools.len();
        }
        None
    }

    /// The state after `action`, which may name a switch, a button or a heading of the pages, was pressed; whether it named one.
    pub(crate) fn library_pressed(&mut self, action: FocusId) -> bool {
        let FocusId(number) = action;
        if (AGENT_PICK_BASE..AGENT_PICK_END).contains(&number) {
            let library = &mut self.settings_page.library;
            library.agent = Some((number - AGENT_PICK_BASE) as usize);
            library.items = None;
            library.sets = None;
            self.read_library();
            return true;
        }
        if !(ITEM_SWITCH_BASE..ITEM_SWITCH_BASE + 1000).contains(&number)
            && !(ITEM_OPEN_BASE..LIBRARY_END).contains(&number)
        {
            return false;
        }
        let library = &self.settings_page.library;
        let withheld = library.withheld.clone();
        let product = library.product;
        match number {
            _ if number < ITEM_SWITCH_BASE + 1000 => {
                let Some(item) = library
                    .items
                    .as_ref()
                    .and_then(|items| items.get((number - ITEM_SWITCH_BASE) as usize))
                    .filter(|item| item.kind.switchable(product))
                else {
                    return true;
                };
                let key = item.key();
                let mut kept: Vec<String> = withheld
                    .iter()
                    .filter(|held| **held != key)
                    .cloned()
                    .collect();
                if item.enabled(&withheld) {
                    kept.push(key);
                }
                self.keep_withheld(kept);
            }
            _ if number < TOOL_SWITCH_BASE => {
                self.open_library_item((number - ITEM_OPEN_BASE) as usize)
            }
            _ if number < SET_SWITCH_BASE => {
                let Some((place, tool)) = self.tool_at((number - TOOL_SWITCH_BASE) as usize) else {
                    return true;
                };
                let Some(set) = self
                    .settings_page
                    .library
                    .sets
                    .as_ref()
                    .and_then(|sets| sets.get(place))
                else {
                    return true;
                };
                if set.per_tool(product) {
                    let kept =
                        pm_protocol::library::toggled_tool(set, &set.tools[tool].name, &withheld);
                    self.keep_withheld(kept);
                }
            }
            _ if number < SET_FOLD_BASE => {
                let Some(set) = library
                    .sets
                    .as_ref()
                    .and_then(|sets| sets.get((number - SET_SWITCH_BASE) as usize))
                    .filter(|set| set.switchable(product))
                else {
                    return true;
                };
                let kept = pm_protocol::library::toggled_set(set, &withheld);
                self.keep_withheld(kept);
            }
            _ => {
                let Some(id) = library
                    .sets
                    .as_ref()
                    .and_then(|sets| sets.get((number - SET_FOLD_BASE) as usize))
                    .map(|set| set.id.clone())
                else {
                    return true;
                };
                let open = &mut self.settings_page.library.open_sets;
                match open.iter().position(|held| *held == id) {
                    Some(place) => {
                        open.remove(place);
                    }
                    None => open.push(id),
                }
            }
        }
        true
    }

    /// Opens the file of the `index`-th customization: one in the project's folder in the window, and one elsewhere in the program this machine opens it with, when the project is on this machine.
    fn open_library_item(&mut self, index: usize) {
        let library = &self.settings_page.library;
        let (Some((project, _)), Some(item)) = (
            library.of,
            library.items.as_ref().and_then(|items| items.get(index)),
        ) else {
            return;
        };
        let Some(held) = self.projects.get(project) else {
            return;
        };
        let path = item.path.clone();
        let inside = inside_project(&path, &held.directory());
        let local = held.host().is_none();
        match inside {
            Some(inside) if item.scope == Scope::Workspace => {
                self.close_settings();
                self.page = Page::Project(project);
                self.sync_context();
                self.open_viewer(inside, false);
            }
            _ if local => self.outbox.push(Outgoing::OpenPath { path }),
            _ => {}
        }
    }
}

/// `path`, a file on the project's machine, from the root of the project's folder `directory` when it is inside it; a folder written from `~` is matched by what follows it, as the machine's home is not known here.
fn inside_project(path: &str, directory: &Path) -> Option<String> {
    let directory = directory.to_string_lossy();
    let folder = directory.trim_start_matches('~').trim_end_matches('/');
    let marker = format!("{folder}/");
    let at = match directory.starts_with('~') {
        true => path.find(&marker)?,
        false => path.starts_with(&marker).then_some(0)?,
    };
    Some(path[at + marker.len()..].to_owned())
}

/// Whether the `index`-th customization's file can be opened from here: one in the project's folder always can, one elsewhere only when the project is on this machine.
fn openable(state: &WorkspaceState, item: &Item) -> bool {
    let Some((project, _)) = state.settings_page.library.of else {
        return false;
    };
    state.projects.get(project).is_some_and(|held| {
        held.host().is_none()
            || (item.scope == Scope::Workspace
                && inside_project(&item.path, &held.directory()).is_some())
    })
}

/// The choice of the agent the pages read for, and what they say they were read for: the project by name, and whether the app knows the agent's files.
fn context(state: &WorkspaceState, room: f32) -> Node {
    let library = &state.settings_page.library;
    let Some((project, agent)) = library.of else {
        return wrapped_note("Add a project to see what agents are given in it.", room);
    };
    let project = state
        .projects
        .get(project)
        .map_or("", |held| held.name.as_str());
    let names: Vec<String> = state
        .offered_agents()
        .into_iter()
        .map(|offered| state.agent_look(offered).name)
        .collect();
    let named: Vec<&str> = names.iter().map(String::as_str).collect();
    let said = match library.product {
        Some(_) => format!("Read for {project}."),
        None => {
            format!("Read for {project}; the app does not know how to read this agent's files.")
        }
    };
    node()
        .styles([flex(), flex_col(), gap(2), flex_none()])
        .child(
            choices(&named, agent, |place| {
                FocusId(AGENT_PICK_BASE + place as u64)
            })
            .styles([pm_style::self_start()]),
        )
        .child(wrapped_note(&said, room))
}

/// What the page of `section`, one of the pages of what agents are given, holds under its title, `room` logical pixels across.
pub(crate) fn body(state: &WorkspaceState, section: Section, page: Node, room: f32) -> Node {
    let page = page
        .child(paragraph(summary(section), room))
        .child(context(state, room));
    let library = &state.settings_page.library;
    let page = page.children(
        library
            .error
            .as_ref()
            .map(|reason| wrapped_note(&format!("The server could not say: {reason}"), room)),
    );
    match (section, kind_of(section)) {
        (Section::Customizations, _) => page.children(
            Kind::ALL
                .into_iter()
                .map(section_of)
                .chain(std::iter::once(Section::Tools))
                .map(|part| {
                    let count = state
                        .library_count(part)
                        .map_or_else(|| "Reading…".to_owned(), |count| format!("{count} found"));
                    card(part, count)
                }),
        ),
        (Section::Tools, _) => tools_page(state, page, room),
        (_, Some(kind)) => items_page(state, kind, page, room),
        _ => page,
    }
}

/// The page of one kind of customization: the search box, a note when the agent cannot leave this kind out, and the rows found, or where the files would go when none are.
fn items_page(state: &WorkspaceState, kind: Kind, page: Node, room: f32) -> Node {
    let library = &state.settings_page.library;
    let product = library.product;
    let needle = state.settings_servers.search.text().trim().to_lowercase();
    let mut page = page.child(search_box(state, section_of(kind)));
    if library.of.is_some() && !kind.switchable(product) {
        let name = product.map_or("This agent", Product::name);
        page = page.child(wrapped_note(
            &format!("{name} reads these itself and cannot be started without one, so they are shown but cannot be turned off here."),
            room,
        ));
    }
    let Some(items) = library.items.as_ref() else {
        return page.child(note(if library.of.is_some() {
            "Reading…"
        } else {
            ""
        }));
    };
    let rows: Vec<Node> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind == kind)
        .filter(|(_, item)| {
            needle.is_empty()
                || item.name.to_lowercase().contains(&needle)
                || item.description.to_lowercase().contains(&needle)
                || item.command.to_lowercase().contains(&needle)
        })
        .map(|(index, item)| item_row(state, index, item))
        .collect();
    let found = items.iter().filter(|item| item.kind == kind).count();
    page = page.child(heading_only("Found", rows.len()));
    if !rows.is_empty() {
        return page.child(boxed(rows));
    }
    if found > 0 {
        return page.child(note("Nothing found matches."));
    }
    let places = locations(product, kind);
    let said = match places.is_empty() {
        true => format!(
            "Nothing found. {} keeps none the app reads.",
            product.map_or("This agent", Product::name)
        ),
        false => format!("Nothing found. They would go in {}.", places.join(", ")),
    };
    page.child(wrapped_note(&said, room))
}

/// One customization as its row shows it: its name with its scope, what it is for, what it says about itself in the code font, its switch and the button that opens its file.
fn item_row(state: &WorkspaceState, index: usize, item: &Item) -> Node {
    let library = &state.settings_page.library;
    let on = item.enabled(&library.withheld);
    let detail = match item.kind {
        Kind::Skill => item.path.clone(),
        Kind::Instruction => format!("applies to {} · {}", item.applies.join(", "), item.path),
        Kind::Agent => {
            let model = if item.model.is_empty() {
                "the session's model"
            } else {
                item.model.as_str()
            };
            let tools = if item.tools.is_empty() {
                "every tool".to_owned()
            } else {
                item.tools.join(", ")
            };
            format!("{model} · {tools}")
        }
        Kind::Hook => item.command.clone(),
    };
    let description = match item.kind {
        Kind::Hook if !item.description.is_empty() => format!("Matches {}", item.description),
        _ => item.description.clone(),
    };
    let mut badges = vec![Badge::new(item.scope.label()).tone(Role::Subtle).build()];
    if !on {
        badges.push(Badge::new("Off").tone(Role::Subtle).build());
    }
    let text = node()
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
                        item.name.clone(),
                        13,
                        1.4,
                        if on { Role::Text } else { Role::Muted },
                    )
                    .styles([pm_style::font_medium()]),
                )
                .children(badges),
        )
        .children((!description.is_empty()).then(|| clipped(description, 12, Role::Muted)))
        .child(clipped(detail, 11, Role::Subtle).styles([pm_style::font_mono()]));
    let actions = node()
        .styles([flex(), items_center(), gap(2), flex_none()])
        .children(
            item.kind
                .switchable(library.product)
                .then(|| switch(on, FocusId(ITEM_SWITCH_BASE + index as u64))),
        )
        .child(
            Button::new("Open", FocusId(ITEM_OPEN_BASE + index as u64))
                .variant(Variant::Outline)
                .size(Size::Sm)
                .disabled(!openable(state, item))
                .build(),
        );
    flat_row().child(text).child(actions)
}

/// The page of tools: the search box, and each set with how many of its tools are on and its switch, its tools under it when it is unfolded or searched.
fn tools_page(state: &WorkspaceState, page: Node, room: f32) -> Node {
    let library = &state.settings_page.library;
    let product = library.product;
    let needle = state.settings_servers.search.text().trim().to_lowercase();
    let mut page = page.child(search_box(state, Section::Tools));
    if library.of.is_some() && product != Some(Product::Claude) {
        let name = product.map_or("This agent", Product::name);
        page = page.child(wrapped_note(
            &format!("{name} is given a tool server's tools whole, so only whole servers can be turned off here."),
            room,
        ));
    }
    let Some(sets) = library.sets.as_ref() else {
        return page.child(note(if library.of.is_some() {
            "Listing the tools of each server…"
        } else {
            ""
        }));
    };
    let mut first = 0;
    let mut shown = Vec::new();
    for (place, set) in sets.iter().enumerate() {
        let start = first;
        first += set.tools.len();
        let named = needle.is_empty() || set.name.to_lowercase().contains(&needle);
        let matching: Vec<(usize, &pm_protocol::library::ToolEntry)> = set
            .tools
            .iter()
            .enumerate()
            .filter(|(_, tool)| {
                named
                    || tool.name.to_lowercase().contains(&needle)
                    || tool.description.to_lowercase().contains(&needle)
            })
            .collect();
        if !named && matching.is_empty() {
            continue;
        }
        let open = !needle.is_empty() || library.open_sets.contains(&set.id);
        let mut rows = vec![set_row(set, place, open, product, &library.withheld)];
        if open {
            rows.extend(matching.into_iter().map(|(index, tool)| {
                tool_row(
                    set,
                    tool,
                    FocusId(TOOL_SWITCH_BASE + (start + index) as u64),
                    product,
                    &library.withheld,
                )
            }));
        }
        shown.push(boxed(rows));
    }
    if shown.is_empty() {
        return page.child(note(if sets.is_empty() {
            "The agent has no tools the app knows of, and no tool server is switched on."
        } else {
            "Nothing found matches."
        }));
    }
    page.children(shown)
}

/// The heading row of a set of tools: the chevron that folds it, its name and where it comes from, how many of its tools are on, and its switch.
fn set_row(
    set: &ToolSet,
    place: usize,
    open: bool,
    product: Option<Product>,
    withheld: &[String],
) -> Node {
    let on = set.enabled(withheld);
    let count = match &set.error {
        Some(_) => "?".to_owned(),
        None => format!("{}/{}", set.enabled_count(withheld), set.tools.len()),
    };
    let chevron = if open {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let about = match &set.error {
        Some(reason) => format!(
            "{} · its tools could not be listed: {reason}",
            source_label(set)
        ),
        None => source_label(set),
    };
    flat_row()
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(2),
                    flex_1(),
                    min_w_px(0),
                    overflow_hidden(),
                    pm_style::cursor_pointer(),
                ])
                .focusable(FocusId(SET_FOLD_BASE + place as u64))
                .child(glyph(chevron, 4.0, Role::Muted))
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
                        .child(
                            node()
                                .styles([flex(), items_center(), gap(2)])
                                .child(
                                    line(
                                        set.name.clone(),
                                        13,
                                        1.4,
                                        if on { Role::Text } else { Role::Muted },
                                    )
                                    .styles([pm_style::font_medium()]),
                                )
                                .child(Badge::new(count).build()),
                        )
                        .child(clipped(about, 12, Role::Muted)),
                ),
        )
        .children(
            set.switchable(product)
                .then(|| switch(on, FocusId(SET_SWITCH_BASE + place as u64))),
        )
}

/// One tool of a set, set in under the set's heading: its name, what it does and, where the agent can be started without it alone, its switch.
fn tool_row(
    set: &ToolSet,
    tool: &pm_protocol::library::ToolEntry,
    id: FocusId,
    product: Option<Product>,
    withheld: &[String],
) -> Node {
    let on = set.tool_enabled(&tool.name, withheld);
    flat_row()
        .styles([pl(10)])
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
                .child(
                    line(
                        tool.name.clone(),
                        12,
                        1.4,
                        if on { Role::Text } else { Role::Muted },
                    )
                    .styles([pm_style::font_mono()]),
                )
                .children(
                    tool.description
                        .lines()
                        .next()
                        .map(|first| clipped(first.to_owned(), 12, Role::Muted)),
                ),
        )
        .children(set.per_tool(product).then(|| switch(on, id)))
}
