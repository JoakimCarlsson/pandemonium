//! The MCP Servers page: what every agent is started with, and where to find more.
//!
//! The page is laid out after VS Code's: a line on what MCP servers are, a
//! search box, the servers that are installed with the way to add one, and
//! under them the servers the public registry offers, each with a button
//! that installs it. Searching narrows both lists.

use pm_acp::{Listing, McpServer};
use pm_ui::{Div, IconName, Styled, Theme, button, h_flex, icon_button, switch, text, v_flex};

use crate::input::{Input, input_view};
use crate::message::Message;
use crate::settings::form::{FormField, FormView, ServerForm, form_row};
use crate::settings::parts::{badge, card, clipped, header, note, row};

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

/// What the MCP Servers page is drawn from.
pub struct McpPage<'a> {
    /// The form a server is being described in, while one is.
    pub form: Option<&'a ServerForm>,
    /// The box of the form that has the keyboard, while one does.
    pub focus: Option<FormField>,
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
                (Some(form), true) => mcp_form_row(theme, page, form),
                _ => {
                    let used = page.usage.get(*index).copied().unwrap_or_default();
                    installed_row(theme, *index, server, used)
                }
            },
        )
        .collect::<Vec<_>>();
    if let Some(form) = page.form.filter(|form| form.index.is_none()) {
        rows.insert(0, mcp_form_row(theme, page, form));
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

/// The form an MCP server is described in, with the variables the registry
/// lists for it as suggestions.
fn mcp_form_row(theme: &Theme, page: &McpPage<'_>, form: &ServerForm) -> Div<Message> {
    form_row(
        theme,
        &FormView {
            form,
            focus: page.focus,
            solid: page.solid,
            suggestions: suggestions(form, page.catalog),
        },
    )
}

/// The variables the registry lists for the server `form` was opened on,
/// less the ones the form already has.
pub fn suggestions(form: &ServerForm, catalog: &Catalog) -> Vec<String> {
    let present = form
        .variables
        .iter()
        .map(|(name, _)| name.value())
        .collect::<Vec<_>>();
    form.original
        .as_ref()
        .and_then(|original| {
            catalog
                .listings
                .iter()
                .find(|listing| listing.server.name == original.name)
        })
        .map(|listing| listing.inputs.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|name| !present.contains(name))
        .collect()
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
