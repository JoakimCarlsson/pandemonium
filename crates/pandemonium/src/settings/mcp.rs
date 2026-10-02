//! The MCP Servers page: what every agent is started with, and where to find more.
//!
//! The page is laid out after VS Code's: a line on what MCP servers are, a
//! search box, the servers that are installed with the way to add one, and
//! under them the servers the public registry offers, each with a button
//! that installs it. Searching narrows both lists.

use pm_acp::{Listing, McpServer};
use pm_gfx::Rgba;
use pm_ui::{
    Div, IconName, Styled, Theme, button, h_flex, icon, icon_button, rule, switch, text, v_flex,
};

use crate::input::{Input, input_view};
use crate::message::Message;

/// The most characters of a description a row shows.
const DESCRIPTION: usize = 140;

/// What the registry last answered a search with.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    /// The servers the registry offered.
    pub listings: Vec<Listing>,
    /// Whether a search is out and has not come back.
    pub loading: bool,
    /// Why the last search failed, when it did.
    pub error: Option<String>,
}

/// One box of the form a server is described in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpField {
    /// What the server is called.
    Name,
    /// The command to run, or the address of the server.
    Target,
    /// The name of the `n`-th variable or header.
    VariableName(usize),
    /// The value of the `n`-th variable or header.
    VariableValue(usize),
}

/// A server being described, every part of it in one place: the boxes it is
/// written in, and which server it replaces when it is saved.
pub struct McpForm {
    /// The place in the list of the server being edited, when one is.
    pub index: Option<usize>,
    /// The server as it was, or as the registry lists it, whose way of being
    /// reached is kept while its address is left as it was.
    pub original: Option<McpServer>,
    /// What the server is called.
    pub name: Input,
    /// The command to run, or the address of the server.
    pub target: Input,
    /// The names and values of its environment or headers.
    pub variables: Vec<(Input, Input)>,
}

impl McpForm {
    /// A form for a server, holding what `server` holds, or nothing when
    /// there is none; it replaces the `index`-th server when it has one.
    pub fn new(index: Option<usize>, server: Option<&McpServer>, extra: &[String]) -> Self {
        let line = |value: &str| {
            let mut input = Input::one_line("MCP server");
            input.set(value);
            input
        };
        let mut variables = server
            .map(|server| server.reach.variables().to_vec())
            .unwrap_or_default()
            .into_iter()
            .map(|(name, value)| (line(&name), line(&value)))
            .collect::<Vec<_>>();
        variables.extend(extra.iter().map(|name| (line(name), line(""))));
        Self {
            index,
            original: server.cloned(),
            name: line(server.map_or("", |server| &server.name)),
            target: line(
                &server
                    .map(|server| server.reach.target())
                    .unwrap_or_default(),
            ),
            variables,
        }
    }

    /// The box `field` names.
    pub fn input(&self, field: McpField) -> Option<&Input> {
        match field {
            McpField::Name => Some(&self.name),
            McpField::Target => Some(&self.target),
            McpField::VariableName(at) => self.variables.get(at).map(|pair| &pair.0),
            McpField::VariableValue(at) => self.variables.get(at).map(|pair| &pair.1),
        }
    }

    /// The box `field` names, to write in.
    pub fn input_mut(&mut self, field: McpField) -> Option<&mut Input> {
        match field {
            McpField::Name => Some(&mut self.name),
            McpField::Target => Some(&mut self.target),
            McpField::VariableName(at) => self.variables.get_mut(at).map(|pair| &mut pair.0),
            McpField::VariableValue(at) => self.variables.get_mut(at).map(|pair| &mut pair.1),
        }
    }

    /// Every box, in the order the Tab key walks them.
    pub fn fields(&self) -> Vec<McpField> {
        [McpField::Name, McpField::Target]
            .into_iter()
            .chain(
                (0..self.variables.len())
                    .flat_map(|at| [McpField::VariableName(at), McpField::VariableValue(at)]),
            )
            .collect()
    }

    /// Adds a variable called `name`, empty.
    pub fn add_variable(&mut self, name: &str) {
        let mut label = Input::one_line("MCP server");
        label.set(name);
        self.variables.push((label, Input::one_line("MCP server")));
    }

    /// Takes the `at`-th variable out.
    pub fn remove_variable(&mut self, at: usize) {
        if at < self.variables.len() {
            self.variables.remove(at);
        }
    }

    /// What the variables are called, for a server that is reached over the
    /// network or one that is started.
    fn variables_label(&self) -> &'static str {
        match self.target.value().trim_start().starts_with("http") {
            true => "Headers",
            false => "Environment variables",
        }
    }

    /// The variables filled in, less the ones with no name or no value.
    pub fn filled(&self) -> Vec<(String, String)> {
        self.variables
            .iter()
            .map(|(name, value)| {
                (
                    name.value().trim().to_owned(),
                    value.value().trim().to_owned(),
                )
            })
            .filter(|(name, value)| !name.is_empty() && !value.is_empty())
            .collect()
    }
}

/// What the MCP Servers page is drawn from.
pub struct McpPage<'a> {
    /// The form a server is being described in, while one is.
    pub form: Option<&'a McpForm>,
    /// The box of the form that has the keyboard, while one does.
    pub focus: Option<McpField>,
    /// The servers every agent is started with.
    pub servers: &'a [McpServer],
    /// How many running agents were given each of the servers, in their order.
    pub usage: &'a [usize],
    /// What the registry last offered.
    pub catalog: &'a Catalog,
    /// The box the search is typed in.
    pub search: &'a Input,
    /// Whether the search box has the keyboard.
    pub typing: bool,
    /// Whether its caret is in the visible half of its blink.
    pub solid: bool,
    /// Whether the list of installed servers is open.
    pub installed_open: bool,
    /// Whether the list of servers on offer is open.
    pub available_open: bool,
}

/// Builds the page.
pub fn mcp_page(theme: &Theme, page: &McpPage<'_>) -> Div<Message> {
    let query = page.search.value().to_lowercase();
    let installed = page
        .servers
        .iter()
        .enumerate()
        .filter(|(_, server)| {
            query.is_empty()
                || server.name.to_lowercase().contains(&query)
                || server.reach.target().to_lowercase().contains(&query)
        })
        .collect::<Vec<_>>();
    let offered = page.catalog.listings.iter().enumerate().collect::<Vec<_>>();

    v_flex()
        .w_full()
        .gap(4)
        .child(
            v_flex()
                .w_full()
                .gap(1)
                .child(
                    text("An open standard that lets AI use external tools and services. MCP servers provide tools for file operations, databases, APIs, and more.")
                        .color(theme.colors.text_muted),
                )
                .child(
                    h_flex().child(
                        button("Learn more about MCP servers", Message::OpenMcpDocs).ghost(),
                    ),
                ),
        )
        .child(input_view(
            theme,
            page.search,
            page.typing,
            page.solid,
            1.0,
            Message::WriteMcpSearch,
            Message::ShowInputMenu,
        ))
        .child(installed_section(theme, page, &installed))
        .child(available_section(theme, page, &offered))
}

/// The servers that are installed, under a header that opens and folds them.
///
/// A server being edited is its row opened out into a form; one being added
/// is a form above the rows.
fn installed_section(
    theme: &Theme,
    page: &McpPage<'_>,
    installed: &[(usize, &McpServer)],
) -> Div<Message> {
    let editing = page.form.and_then(|form| form.index);
    let mut rows = installed
        .iter()
        .map(
            |(index, server)| match (page.form, editing == Some(*index)) {
                (Some(form), true) => form_row(theme, page, form),
                _ => {
                    let used = page.usage.get(*index).copied().unwrap_or_default();
                    installed_row(theme, *index, server, used)
                }
            },
        )
        .collect::<Vec<_>>();
    if let Some(form) = page.form.filter(|form| form.index.is_none()) {
        rows.insert(0, form_row(theme, page, form));
    }
    if rows.is_empty() {
        rows.push(match page.servers.is_empty() {
            true => note(theme, "No MCP servers are installed."),
            false => note(theme, "No installed servers match."),
        });
    }
    v_flex()
        .w_full()
        .gap(1.5)
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(header(
                    theme,
                    "Installed",
                    installed.len(),
                    page.installed_open || page.form.is_some(),
                    Message::ToggleMcpInstalled,
                ))
                .child(button("Add Server", Message::AddMcpServer).outlined()),
        )
        .when(page.installed_open || page.form.is_some(), |section| {
            section.child(card(theme, rows))
        })
}

/// The form a server is described in: every box at once, so that changing
/// one thing is one box and a press of Save.
fn form_row(theme: &Theme, page: &McpPage<'_>, form: &McpForm) -> Div<Message> {
    let present = form
        .variables
        .iter()
        .map(|(name, _)| name.value())
        .collect::<Vec<_>>();
    let known = form
        .original
        .as_ref()
        .and_then(|original| {
            page.catalog
                .listings
                .iter()
                .find(|listing| listing.server.name == original.name)
        })
        .map(|listing| listing.inputs.clone())
        .unwrap_or_default();
    let suggestions = known
        .into_iter()
        .enumerate()
        .filter(|(_, name)| !present.contains(name))
        .map(|(place, name)| {
            button(format!("+ {name}"), Message::SuggestMcpVariable(place)).outlined()
        })
        .collect::<Vec<_>>();
    let boxed = |label: &str, field: McpField| {
        v_flex()
            .w_full()
            .gap(0.5)
            .child(
                text(label.to_owned())
                    .text_sm()
                    .color(theme.colors.text_muted),
            )
            .child(field_view(theme, page, form, field))
    };
    v_flex()
        .w_full()
        .p(3)
        .gap(2.5)
        .child(boxed("Name", McpField::Name))
        .child(boxed(
            "Command, or the https:// address of the server",
            McpField::Target,
        ))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    text(form.variables_label())
                        .text_sm()
                        .color(theme.colors.text_muted),
                )
                .child(button("+ Add", Message::AddMcpVariable).outlined()),
        )
        .children((0..form.variables.len()).map(|at| {
            h_flex()
                .w_full()
                .gap(1.5)
                .items_center()
                .child(h_flex().flex_1().child(field_view(
                    theme,
                    page,
                    form,
                    McpField::VariableName(at),
                )))
                .child(h_flex().flex_1().child(field_view(
                    theme,
                    page,
                    form,
                    McpField::VariableValue(at),
                )))
                .child(
                    icon_button(theme, IconName::Close, Message::RemoveMcpVariable(at))
                        .tooltip("Remove"),
                )
        }))
        .when(!suggestions.is_empty(), |column| {
            column.child(h_flex().gap(1).children(suggestions))
        })
        .child(
            text("A name and a value, such as an API key. A row with an empty value is left out.")
                .text_xs()
                .color(theme.colors.text_subtle),
        )
        .child(
            h_flex()
                .gap(1.5)
                .child(button("Save", Message::SaveMcpForm).filled())
                .child(button("Cancel", Message::CancelMcpForm).outlined()),
        )
}

/// One box of the form, drawn as every box of text in the window is.
fn field_view(theme: &Theme, page: &McpPage<'_>, form: &McpForm, field: McpField) -> Div<Message> {
    match form.input(field) {
        Some(input) => input_view(
            theme,
            input,
            page.focus == Some(field),
            page.solid,
            1.0,
            move |phase, from, to| Message::WriteMcpField(field, phase, from, to),
            Message::ShowInputMenu,
        ),
        None => h_flex(),
    }
}

/// One installed server: its name and how it stands, what it is for and how
/// it is reached, and the switch and menu that change it.
///
/// A server stands as disabled while it is switched off, as used while
/// running agents were given it, and as idle otherwise.
fn installed_row(theme: &Theme, index: usize, server: &McpServer, used: usize) -> Div<Message> {
    let (standing, tone) = match (server.enabled, used) {
        (false, _) => ("Disabled".to_owned(), theme.colors.text_subtle),
        (true, 0) => ("Idle".to_owned(), theme.colors.text_muted),
        (true, 1) => ("In use by 1 agent".to_owned(), theme.colors.accent),
        (true, count) => (format!("In use by {count} agents"), theme.colors.accent),
    };
    let variables = server.reach.variables().len();
    let reached = match variables {
        0 => format!("{} · {}", server.reach.kind(), server.reach.target()),
        count => format!(
            "{} · {} · {count} variable{}",
            server.reach.kind(),
            server.reach.target(),
            if count == 1 { "" } else { "s" }
        ),
    };
    let name = match server.enabled {
        true => theme.colors.text,
        false => theme.colors.text_muted,
    };
    h_flex()
        .w_full()
        .px(3)
        .py(2.5)
        .gap(4)
        .items_center()
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .gap(0.5)
                .overflow_hidden()
                .child(
                    h_flex()
                        .gap(1.5)
                        .items_center()
                        .child(text(server.name.clone()).font_medium().color(name))
                        .child(badge(theme, &standing, tone)),
                )
                .child(
                    text(clipped(&server.description))
                        .text_sm()
                        .color(theme.colors.text_muted),
                )
                .child(
                    text(clipped(&reached))
                        .text_xs()
                        .font_mono()
                        .color(theme.colors.text_subtle),
                ),
        )
        .child(
            h_flex()
                .gap(2)
                .items_center()
                .child(switch(server.enabled, Message::ToggleMcpServer(index)))
                .child(
                    icon_button(theme, IconName::More, Message::ShowMcpServerMenu(index))
                        .tooltip("More actions"),
                ),
        )
}

/// A small label in a rounded wash, in `tone`.
fn badge(theme: &Theme, label: &str, tone: Rgba) -> Div<Message> {
    h_flex()
        .px(1.5)
        .rounded(theme.radius.lg)
        .bg(theme.colors.surface_selected)
        .child(text(label.to_owned()).text_xs().color(tone))
}

/// The servers the registry offers, under a header that opens and folds them.
fn available_section(
    theme: &Theme,
    page: &McpPage<'_>,
    offered: &[(usize, &Listing)],
) -> Div<Message> {
    let rows = match (
        &page.catalog.error,
        page.catalog.loading,
        offered.is_empty(),
    ) {
        (_, true, true) => vec![note(theme, "Loading marketplace MCP servers…")],
        (Some(error), _, true) => vec![note(
            theme,
            &format!("Marketplace results are unavailable: {error}"),
        )],
        (None, false, true) => vec![note(theme, "No marketplace MCP servers are available.")],
        _ => offered
            .iter()
            .map(|(index, listing)| available_row(theme, page, *index, listing))
            .collect(),
    };
    v_flex()
        .w_full()
        .gap(1.5)
        .child(header(
            theme,
            "Available",
            offered.len(),
            page.available_open,
            Message::ToggleMcpAvailable,
        ))
        .when(page.available_open, |section| {
            section
                .child(
                    text(match page.search.value().trim().is_empty() {
                        true => "Featured MCP servers from the marketplace. Search to find more.",
                        false => "Best matches from the marketplace, first-party servers first.",
                    })
                    .text_sm()
                    .color(theme.colors.text_muted),
                )
                .child(card(theme, rows))
        })
}

/// One server on offer: what it is called, what it does, and the button
/// that installs it, or says it already is.
fn available_row(
    theme: &Theme,
    page: &McpPage<'_>,
    index: usize,
    listing: &Listing,
) -> Div<Message> {
    let installed = page
        .servers
        .iter()
        .any(|server| server.name == listing.server.name);
    let action = match installed {
        true => button("Installed", Message::InstallMcpServer(index)).outlined(),
        false => button("Install", Message::InstallMcpServer(index)).filled(),
    };
    row(
        theme,
        &listing.title,
        &clipped(&listing.description),
        h_flex().child(action),
    )
}

/// The title of a list: a chevron that opens or folds it, its name, and how
/// many it holds.
fn header(theme: &Theme, title: &str, count: usize, open: bool, toggle: Message) -> Div<Message> {
    let chevron = match open {
        true => IconName::ChevronDown,
        false => IconName::ChevronRight,
    };
    h_flex()
        .h_px(theme.size.control)
        .gap(1.5)
        .items_center()
        .on_click(toggle)
        .child(icon(chevron).color(theme.colors.text_muted))
        .child(text(title).font_semibold())
        .child(
            h_flex()
                .px(1.5)
                .rounded(theme.radius.lg)
                .bg(theme.colors.surface_selected)
                .child(
                    text(count.to_string())
                        .text_xs()
                        .color(theme.colors.text_muted),
                ),
        )
}

/// Rows in a bordered box, a hairline between each and the next.
fn card(theme: &Theme, rows: Vec<Div<Message>>) -> Div<Message> {
    let mut body = v_flex()
        .w_full()
        .rounded(theme.radius.md)
        .border_1(theme.colors.border_variant)
        .bg(theme.colors.surface);
    for (place, row) in rows.into_iter().enumerate() {
        if place > 0 {
            body = body.child(rule(theme));
        }
        body = body.child(row);
    }
    body
}

/// One row of a card: a name over what it is, and `control` at the end.
fn row(theme: &Theme, title: &str, detail: &str, control: Div<Message>) -> Div<Message> {
    h_flex()
        .w_full()
        .px(3)
        .py(2.5)
        .gap(4)
        .items_center()
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .gap(0.5)
                .overflow_hidden()
                .child(text(title).font_medium())
                .child(text(detail).text_sm().color(theme.colors.text_muted)),
        )
        .child(control)
}

/// A row of a card that only says something.
fn note(theme: &Theme, message: &str) -> Div<Message> {
    h_flex()
        .w_full()
        .px(3)
        .py(3)
        .child(text(message).text_sm().color(theme.colors.text_muted))
}

/// `description` cut to its first line and to what a row has room for.
fn clipped(description: &str) -> String {
    let line = description.lines().next().unwrap_or_default().trim();
    match line.chars().count() > DESCRIPTION {
        true => format!("{}…", line.chars().take(DESCRIPTION).collect::<String>()),
        false => match line.is_empty() {
            true => "No description provided.".to_owned(),
            false => line.to_owned(),
        },
    }
}
