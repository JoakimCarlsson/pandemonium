//! The MCP Servers page: what every agent is started with, and where to find more.
//!
//! The page is laid out after VS Code's: a line on what MCP servers are, a
//! search box, the servers that are installed with the way to add one, and
//! under them the servers the public registry offers, each with a button
//! that installs it. Searching narrows both lists.

use pm_acp::{Listing, McpServer};
use pm_ui::{Div, IconName, Styled, Theme, button, h_flex, icon, icon_button, rule, text, v_flex};

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

/// What the MCP Servers page is drawn from.
pub struct McpPage<'a> {
    /// The servers every agent is started with.
    pub servers: &'a [McpServer],
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
fn installed_section(
    theme: &Theme,
    page: &McpPage<'_>,
    installed: &[(usize, &McpServer)],
) -> Div<Message> {
    let rows = match (installed.is_empty(), page.servers.is_empty()) {
        (true, true) => vec![note(theme, "No MCP servers are installed.")],
        (true, false) => vec![note(theme, "No installed servers match.")],
        (false, _) => installed
            .iter()
            .map(|(index, server)| installed_row(theme, *index, server))
            .collect(),
    };
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
                    page.installed_open,
                    Message::ToggleMcpInstalled,
                ))
                .child(button("Add Server", Message::AddMcpServer).outlined()),
        )
        .when(page.installed_open, |section| {
            section.child(card(theme, rows))
        })
}

/// One installed server: what it is called, how it is reached, and the ways
/// to change it.
fn installed_row(theme: &Theme, index: usize, server: &McpServer) -> Div<Message> {
    let variables = server.reach.variables().len();
    let detail = match variables {
        0 => format!("{} · {}", server.reach.kind(), server.reach.target()),
        count => format!(
            "{} · {} · {count} variable{}",
            server.reach.kind(),
            server.reach.target(),
            if count == 1 { "" } else { "s" }
        ),
    };
    row(
        theme,
        &server.name,
        &clipped(&detail),
        h_flex()
            .gap(1)
            .items_center()
            .child(button("Edit", Message::EditMcpServer(index)).outlined())
            .child(
                icon_button(theme, IconName::Close, Message::RemoveMcpServer(index))
                    .tooltip("Remove"),
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
                    text("Browse and install MCP servers from the marketplace.")
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
