//! The pages of the settings window that list what agents are given in a project: an overview, the skills, instructions, agent definitions and hooks found in the folders the agent reads, and the tools it may call, each with a switch that leaves it out of new sessions.

use crate::button;
use crate::events;
use crate::icons;
use crate::target::Target;
use crate::theme;
use rustle::{Alignment, Arrangement, TextOverflow};

use std::path::Path;

use pm_protocol::library::{Item, Kind, Product, Scope, Source, ToolSet, locations};
use rustle::Modifier;

use crate::theme::Role;

use crate::Badge;
use crate::button::Variant;
use crate::parts::glyph;
use crate::settings::Section;
use crate::settings_drafts::{draft_node, draftable, new_button};
use crate::settings_row::switch;
use crate::settings_rows::{ITEM_OPENS, ITEM_SWITCHES, SET_FOLDS, SET_SWITCHES, TOOL_SWITCHES};
use crate::settings_servers::{
    boxed, card, flat_row, heading_only, name_tone, note, paragraph, search_box, spread, titled,
    wrapped_note,
};
use crate::state::{Outgoing, Page, WorkspaceState};

/// What the pages hold: what was read for which project and agent, and which sets of tools are unfolded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    /// The project, by position or nothing when none is open, and the agent, by position, what is held was read for.
    pub of: Option<(Option<usize>, usize)>,
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
    /// The form for a new customization, while it is open.
    pub draft: Option<crate::settings_drafts::DraftForm>,
}

impl Library {
    /// The set of tools at `place`, once the sets were listed.
    fn set(&self, place: usize) -> Option<&ToolSet> {
        self.sets.as_ref()?.get(place)
    }
}

/// Whether `section` is one of the pages of what agents are given.
pub(crate) fn lists(section: Section) -> bool {
    section == Section::Customizations || section == Section::Tools || kind_of(section).is_some()
}

/// The kind of customization `section` lists, when it lists one.
pub(crate) fn kind_of(section: Section) -> Option<Kind> {
    Kind::ALL
        .into_iter()
        .find(|kind| section_of(*kind) == section)
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
    /// The project and the agent the pages are about, by position: the project of the page showing, or the first project when the page has none, and the agent the prompt is for.
    fn library_target(&self) -> Option<(Option<usize>, usize)> {
        let project = match self.page {
            Page::Project(project) | Page::New(project) => Some(project),
            Page::Session(index) => self
                .sessions
                .get(index)
                .map(|session| session.project.unwrap_or(0)),
            Page::Chat | Page::History | Page::Extensions => Some(0),
        }
        .filter(|project| self.projects.get(*project).is_some());
        Some((project, self.agent_now()))
    }

    /// The machine, or nothing for this one, and the folder of the project at `project`; this machine and no folder when no project is open.
    pub(crate) fn library_place(&self, project: Option<usize>) -> Option<(Option<String>, String)> {
        let Some(project) = project else {
            return Some((None, String::new()));
        };
        let held = self.projects.get(project)?;
        Some((held.host(), held.directory().to_string_lossy().into_owned()))
    }

    /// Asks the server of the project the pages are about for what its agent is given there, and for the agent's tools.
    pub(crate) fn read_library(&mut self) {
        let Some((project, agent)) = self.library_target() else {
            self.settings_page.library = Library::default();
            return;
        };
        let Some((host, directory)) = self.library_place(project) else {
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
            host: host.clone(),
            directory,
        });
        self.outbox.push(Outgoing::ToolSets {
            agent,
            project,
            host,
        });
    }

    /// The state with what the agent at `agent` is given in the project at `project`, when that is still what the pages are about.
    pub fn with_library(
        self,
        project: Option<usize>,
        agent: usize,
        product: Option<Product>,
        items: Vec<Item>,
        withheld: Vec<String>,
    ) -> Self {
        self.library_answered((project, agent), product, withheld, |library| {
            library.items = Some(items);
        })
    }

    /// The state with the tools the agent at `agent` is given, when that is still the agent the pages are about.
    pub fn with_tool_sets(
        self,
        project: Option<usize>,
        agent: usize,
        product: Option<Product>,
        sets: Vec<ToolSet>,
        withheld: Vec<String>,
    ) -> Self {
        self.library_answered((project, agent), product, withheld, |library| {
            library.sets = Some(sets);
        })
    }

    /// The state with what the server answered about the project and the agent `of`, by position, when that is still what the pages are about: the `product` the agent runs, what is turned off, `withheld`, and what `fill` puts in.
    fn library_answered(
        mut self,
        of: (Option<usize>, usize),
        product: Option<Product>,
        withheld: Vec<String>,
        fill: impl FnOnce(&mut Library),
    ) -> Self {
        let library = &mut self.settings_page.library;
        if library.of == Some(of) {
            library.product = product;
            library.withheld = withheld;
            fill(library);
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
        let Some((host, _)) = self
            .settings_page
            .library
            .of
            .and_then(|(project, _)| self.library_place(project))
        else {
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
    pub(crate) fn library_pressed(&mut self, action: Target) -> bool {
        if let Some(index) = ITEM_SWITCHES.index_of(action) {
            self.toggle_item(index);
        } else if let Some(index) = ITEM_OPENS.index_of(action) {
            self.open_library_item(index);
        } else if let Some(number) = TOOL_SWITCHES.index_of(action) {
            self.toggle_tool(number);
        } else if let Some(place) = SET_SWITCHES.index_of(action) {
            self.toggle_set(place);
        } else if let Some(place) = SET_FOLDS.index_of(action) {
            self.fold_set(place);
        } else {
            return false;
        }
        true
    }

    /// Turns the `index`-th customization read off, or on again, when the agent can be started without it.
    fn toggle_item(&mut self, index: usize) {
        let library = &self.settings_page.library;
        let product = library.product;
        let Some(item) = library
            .items
            .as_ref()
            .and_then(|items| items.get(index))
            .filter(|item| item.kind.switchable(product))
        else {
            return;
        };
        let withheld = &library.withheld;
        let key = item.key();
        let mut kept: Vec<String> = withheld
            .iter()
            .filter(|held| **held != key)
            .cloned()
            .collect();
        if item.enabled(withheld) {
            kept.push(key);
        }
        self.keep_withheld(kept);
    }

    /// Turns the tool `number`, counted across every set, off or on again, when its set lets its tools be switched one by one.
    fn toggle_tool(&mut self, number: usize) {
        let Some((place, tool)) = self.tool_at(number) else {
            return;
        };
        let library = &self.settings_page.library;
        let Some(set) = library.set(place) else {
            return;
        };
        if set.per_tool(library.product) {
            let kept =
                pm_protocol::library::toggled_tool(set, &set.tools[tool].name, &library.withheld);
            self.keep_withheld(kept);
        }
    }

    /// Turns the set of tools at `place` off or on again, when the agent can be started without it.
    fn toggle_set(&mut self, place: usize) {
        let library = &self.settings_page.library;
        let Some(set) = library
            .set(place)
            .filter(|set| set.switchable(library.product))
        else {
            return;
        };
        let kept = pm_protocol::library::toggled_set(set, &library.withheld);
        self.keep_withheld(kept);
    }

    /// Unfolds the set of tools at `place`, or folds it away again.
    fn fold_set(&mut self, place: usize) {
        let Some(id) = self
            .settings_page
            .library
            .set(place)
            .map(|set| set.id.clone())
        else {
            return;
        };
        let open = &mut self.settings_page.library.open_sets;
        let before = open.len();
        open.retain(|held| *held != id);
        if open.len() == before {
            open.push(id);
        }
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
        let (path, scope) = (item.path.clone(), item.scope);
        self.open_customization(project, path, scope);
    }

    /// Opens the customization file `path` of `scope` for the project at `project`: one in the project's folder in the window, and one elsewhere in the program this machine opens it with, when the project is on this machine or no project is open.
    pub(crate) fn open_customization(
        &mut self,
        project: Option<usize>,
        path: String,
        scope: Scope,
    ) {
        let Some(project) = project else {
            self.outbox.push(Outgoing::OpenPath { path });
            return;
        };
        let Some(held) = self.projects.get(project) else {
            return;
        };
        let inside = inside_project(&path, &held.directory());
        let local = held.host().is_none();
        match inside {
            Some(inside) if scope == Scope::Workspace => {
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
    let Some(project) = project else {
        return !item.path.is_empty();
    };
    !item.path.is_empty()
        && state.projects.get(project).is_some_and(|held| {
            held.host().is_none()
                || (item.scope == Scope::Workspace
                    && inside_project(&item.path, &held.directory()).is_some())
        })
}

/// Emits what the pages say they were read for: the project by name, and whether the app knows the agent's files.
#[track_caller]
fn context(state: &WorkspaceState, room: f32) {
    let library = &state.settings_page.library;
    let Some((project, _)) = library.of else {
        wrapped_note("Add an agent to see what it is given.", room);
        return;
    };
    let read_for = match project.and_then(|project| state.projects.get(project)) {
        Some(held) => held.name.clone(),
        None => "this machine; open a project to see its own too".to_owned(),
    };
    let said = match library.product {
        Some(_) => format!("Read for {read_for}."),
        None => {
            format!("Read for {read_for}; the app does not know how to read this agent's files.")
        }
    };
    rustle::column_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Start,
        || wrapped_note(&said, room),
    );
}

/// Emits what the page of `section`, one of the pages of what agents are given, holds under its
/// title, `room` logical pixels across: `page`, what the caller put first, then what the page is
/// for, what it was read for and the rows of the page.
#[track_caller]
pub(crate) fn body(state: &WorkspaceState, section: Section, page: impl FnOnce(), room: f32) {
    page();
    paragraph(summary(section), room);
    context(state, room);
    let library = &state.settings_page.library;
    if let Some(reason) = library.error.as_ref() {
        wrapped_note(&format!("The server could not say: {reason}"), room);
    }
    match (section, kind_of(section)) {
        (Section::Customizations, _) => {
            for part in Kind::ALL
                .into_iter()
                .map(section_of)
                .chain(std::iter::once(Section::Tools))
            {
                let count = state
                    .library_count(part)
                    .map_or_else(|| "Reading…".to_owned(), |count| format!("{count} found"));
                card(part, count);
            }
        }
        (Section::Tools, _) => tools_page(state),
        (_, Some(kind)) => items_page(state, kind, room),
        _ => {}
    }
}

/// Emits the page of one kind of customization: the search box, a note when the agent cannot leave this kind out, and the rows found, or where the files would go when none are.
fn items_page(state: &WorkspaceState, kind: Kind, room: f32) {
    let library = &state.settings_page.library;
    let product = library.product;
    let needle = state.needle();
    search_box(state, section_of(kind));
    if library.of.is_some() && !kind.switchable(product) {
        let name = product.map_or("This agent", Product::name);
        wrapped_note(
            &format!(
                "{name} reads these itself and cannot be started without one, so they are shown but cannot be turned off here."
            ),
            room,
        );
    }
    let Some(items) = library.items.as_ref() else {
        note(library.of.map_or("", |_| "Reading…"));
        return;
    };
    let rows: Vec<(usize, &Item)> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind == kind)
        .filter(|(_, item)| {
            needle.is_empty()
                || item.name.to_lowercase().contains(&needle)
                || item.description.to_lowercase().contains(&needle)
                || item.command.to_lowercase().contains(&needle)
        })
        .collect();
    let found = items.iter().filter(|item| item.kind == kind).count();
    if kind == Kind::Instruction {
        crate::settings_drafts::prompt_node(state, room);
    }
    spread(|| {
        heading_only("Found", rows.len());
        if draftable(kind) {
            new_button(state, kind);
        }
    });
    if let Some(form) = library.draft.as_ref().filter(|form| form.kind == kind) {
        draft_node(form, room);
    }
    if !rows.is_empty() {
        boxed(|| {
            for (index, item) in rows {
                rustle::key(index, || item_row(state, index, item));
            }
        });
        return;
    }
    if found > 0 {
        note("Nothing found matches.");
        return;
    }
    let places = locations(kind);
    let said = match places.is_empty() {
        true => format!(
            "Nothing found. {} keeps none the app reads.",
            product.map_or("This agent", Product::name)
        ),
        false => format!("Nothing found. They would go in {}.", places.join(", ")),
    };
    wrapped_note(&said, room);
}

/// Emits one customization as its row shows it: its name with its scope, what it is for, what it says about itself in the code font, its switch and the button that opens its file.
fn item_row(state: &WorkspaceState, index: usize, item: &Item) {
    let library = &state.settings_page.library;
    let on = item.enabled(&library.withheld);
    let detail = match item.kind {
        Kind::Skill => item.path.clone(),
        Kind::Instruction
            if item.applies.iter().any(|said| {
                said == pm_protocol::library::REPLACES_PROMPT
                    || said == pm_protocol::library::APPENDS_PROMPT
            }) =>
        {
            format!("{} · {}", item.applies.join(", "), item.path)
        }
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
    flat_row(3, || {
        titled(
            &item.name,
            name_tone(on),
            || {
                Badge::new(item.scope.label()).tone(Role::Subtle).build();
                if !item.tool.is_empty() {
                    Badge::new(&item.tool).tone(Role::Subtle).build();
                }
                if !on {
                    Badge::new("Off").tone(Role::Subtle).build();
                }
            },
            || {
                if !description.is_empty() {
                    note(&description);
                }
                rustle::basic_text(
                    Modifier::new().fill_max_width(),
                    detail,
                    theme::line(11, Role::Subtle),
                    TextOverflow::Ellipsis,
                );
            },
        );
        rustle::row_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            || {
                if item.kind.switchable(library.product) {
                    switch(on, ITEM_SWITCHES.id(index));
                }
                if let Some(open) = ITEM_OPENS.id(index) {
                    button::small_button("Open", open, Variant::Outline)
                        .disabled(!openable(state, item))
                        .build();
                }
            },
        );
    });
}

/// Emits the page of tools: the search box, and each set with how many of its tools are on and its switch, its tools under it when it is unfolded or searched.
#[track_caller]
fn tools_page(state: &WorkspaceState) {
    let library = &state.settings_page.library;
    let product = library.product;
    let needle = state.needle();
    search_box(state, Section::Tools);
    let Some(sets) = library.sets.as_ref() else {
        note(
            library
                .of
                .map_or("", |_| "Listing the tools of each server…"),
        );
        return;
    };
    let mut first = 0;
    let mut any = false;
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
            .map(|(index, tool)| (start + index, tool))
            .collect();
        if !named && matching.is_empty() {
            continue;
        }
        any = true;
        let open = !needle.is_empty() || library.open_sets.contains(&set.id);
        rustle::key(place, || {
            boxed(|| {
                set_row(set, place, open, product, &library.withheld);
                if open {
                    for &(index, tool) in &matching {
                        rustle::key(index, || {
                            tool_row(
                                set,
                                tool,
                                TOOL_SWITCHES.id(index),
                                product,
                                &library.withheld,
                            );
                        });
                    }
                }
            });
        });
    }
    if !any {
        note(if sets.is_empty() {
            "The agent has no tools the app knows of, and no tool server is switched on."
        } else {
            "Nothing found matches."
        });
    }
}

/// Emits the heading row of a set of tools: the chevron that folds it, its name and where it comes from, how many of its tools are on, and its switch.
fn set_row(set: &ToolSet, place: usize, open: bool, product: Option<Product>, withheld: &[String]) {
    let on = set.enabled(withheld);
    let count = match &set.error {
        Some(_) => "?".to_owned(),
        None => format!("{}/{}", set.enabled_count(withheld), set.tools.len()),
    };
    let about = match &set.error {
        Some(reason) => format!(
            "{} · its tools could not be listed: {reason}",
            source_label(set)
        ),
        None => source_label(set),
    };
    flat_row(3, || {
        rustle::row_arranged(
            events::pressable(Modifier::new().weight(1.0).clip(), SET_FOLDS.id(place)),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            || {
                glyph(icons::chevron(open), 4.0, Role::Muted);
                titled(
                    &set.name,
                    name_tone(on),
                    || Badge::new(count).build(),
                    || note(&about),
                );
            },
        );
        if set.switchable(product) {
            switch(on, SET_SWITCHES.id(place));
        }
    });
}

/// Emits one tool of a set, set in under the set's heading: its name, what it does and, where the agent can be started without it alone, its switch answering to `id`.
fn tool_row(
    set: &ToolSet,
    tool: &pm_protocol::library::ToolEntry,
    id: Option<Target>,
    product: Option<Product>,
    withheld: &[String],
) {
    let on = set.tool_enabled(&tool.name, withheld);
    flat_row(10, || {
        rustle::column_arranged(
            Modifier::new().weight(1.0).clip(),
            Arrangement::SpacedBy(theme::sp(1)),
            Alignment::Start,
            || {
                rustle::basic_text(
                    Modifier::new(),
                    tool.name.clone(),
                    theme::code(12, name_tone(on)),
                    TextOverflow::Ellipsis,
                );
                if let Some(first) = tool.description.lines().next() {
                    note(first);
                }
            },
        );
        if set.per_tool(product) {
            switch(on, id);
        }
    });
}

#[cfg(test)]
/// Tests for the pages of what agents are given.
mod tests {
    use super::*;
    use pm_protocol::library::ToolEntry;

    /// The state with a project open and one built-in set of two tools read for it.
    fn with_tools() -> WorkspaceState {
        let mut state = WorkspaceState::new(&[]).with_project("/work/app");
        state.read_library();
        state.take_outbox();
        let (project, agent) = state
            .settings_page
            .library
            .of
            .expect("read for the project");
        let tool = |name: &str| ToolEntry {
            name: name.to_owned(),
            description: String::new(),
        };
        let set = ToolSet {
            id: "built-in".to_owned(),
            name: "Built-in tools".to_owned(),
            tools: vec![tool("bash"), tool("read")],
            ..ToolSet::default()
        };
        state.with_tool_sets(project, agent, Some(Product::Claude), vec![set], Vec::new())
    }

    /// Pressing a set's heading through the settings window unfolds it, and pressing its switch or a tool's turns them off; no other control of the window claims their names first.
    #[test]
    fn sets_of_tools_fold_and_switch_from_the_window() {
        let state = with_tools().settings_pressed(SET_FOLDS.at(0));
        assert_eq!(state.settings_page.library.open_sets, ["built-in"]);
        let mut state = state.settings_pressed(TOOL_SWITCHES.at(1));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::SaveWithheld { keys, .. } if keys.iter().any(|key| key.contains("read"))
        )));
        let mut state = state.settings_pressed(SET_SWITCHES.at(0));
        assert!(
            state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::SaveWithheld { .. }))
        );
    }
}
