//! The Agent Servers page: the agents the editor can start, and where to find more.
//!
//! The page is laid out like the MCP Servers one: a search box, the agents
//! that are installed with the way to add one by hand, and under them the
//! agents the Agent Client Protocol's registry lists, each with a button that
//! installs it. Searching narrows both lists.

use pm_acp::{Agent, Available, Install, Source};
use pm_ui::{Div, IconName, Styled, Theme, button, h_flex, icon_button, text, v_flex};

use crate::input::{Input, input_view};
use crate::message::Message;
use crate::settings::form::{FormField, FormView, ServerForm, form_row};
use crate::settings::parts::{badge, card, clipped, header, note, row};

/// What the registry last answered, and what is being installed from it.
#[derive(Clone, Debug, Default)]
pub struct AgentCatalog {
    /// The agents the registry offered, the well known ones first.
    pub agents: Vec<Available>,
    /// Whether the registry has been asked and has not answered.
    pub loading: bool,
    /// Why the registry could not be read, when it could not.
    pub error: Option<String>,
    /// The ids of the agents being downloaded now.
    pub installing: Vec<String>,
}

/// What the Agent Servers page is drawn from.
pub struct AgentList<'a> {
    /// Every agent the editor offers, shipped and added.
    pub agents: &'a [Agent],
    /// The agents the reader added, in the order they are kept.
    pub custom: &'a [Agent],
    /// What the registry offers.
    pub catalog: &'a AgentCatalog,
    /// The form an agent is being described in, while one is.
    pub form: Option<&'a ServerForm>,
    /// The box of the form that has the keyboard, while one does.
    pub focus: Option<FormField>,
    /// The box the search is typed in.
    pub search: &'a Input,
    /// Whether the search box has the keyboard.
    pub typing: bool,
    /// Whether the caret of whichever box has it is in the visible half of its blink.
    pub solid: bool,
    /// Whether the list of installed agents is open.
    pub installed_open: bool,
    /// Whether the list of agents on offer is open.
    pub available_open: bool,
}

/// Builds the page.
pub fn agent_list(theme: &Theme, list: &AgentList<'_>) -> Div<Message> {
    let query = list.search.value().trim().to_lowercase();
    v_flex()
        .w_full()
        .gap(4)
        .child(
            text("Agents that speak the Agent Client Protocol. The editor starts them in a worktree and talks to them over it.")
                .color(theme.colors.text_muted),
        )
        .child(input_view(
            theme,
            list.search,
            list.typing,
            list.solid,
            1.0,
            Message::WriteAgentSearch,
            Message::ShowInputMenu,
        ))
        .child(installed_section(theme, list, &query))
        .child(available_section(theme, list, &query))
}

/// The agents that are installed, under a header that opens and folds them.
fn installed_section(theme: &Theme, list: &AgentList<'_>, query: &str) -> Div<Message> {
    let editing = list.form.and_then(|form| form.index);
    let shown = list
        .agents
        .iter()
        .filter(|agent| {
            query.is_empty()
                || agent.name.to_lowercase().contains(query)
                || agent.id.contains(query)
        })
        .collect::<Vec<_>>();
    let mut rows = shown
        .iter()
        .map(|agent| {
            let place = list.custom.iter().position(|custom| custom.id == agent.id);
            match (list.form, place) {
                (Some(form), Some(place)) if editing == Some(place) => form_for(theme, list, form),
                _ => agent_row(theme, agent, place),
            }
        })
        .collect::<Vec<_>>();
    if let Some(form) = list.form.filter(|form| form.index.is_none()) {
        rows.insert(0, form_for(theme, list, form));
    }
    if rows.is_empty() {
        rows.push(note(theme, "No installed agents match."));
    }
    let open = list.installed_open || list.form.is_some();
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
                    shown.len(),
                    open,
                    Message::ToggleAgentsInstalled,
                ))
                .child(button("Add Agent", Message::AddAgentServer).outlined()),
        )
        .when(open, |section| section.child(card(theme, rows)))
}

/// The agents the registry offers, under a header that opens and folds them.
fn available_section(theme: &Theme, list: &AgentList<'_>, query: &str) -> Div<Message> {
    let mut offered = list
        .catalog
        .agents
        .iter()
        .enumerate()
        .filter(|(_, agent)| !list.agents.iter().any(|installed| agent.is(installed)))
        .filter(|(_, agent)| {
            query.is_empty()
                || agent.name.to_lowercase().contains(query)
                || agent.id.contains(query)
                || agent.description.to_lowercase().contains(query)
        })
        .collect::<Vec<_>>();
    offered.sort_by_key(|(_, agent)| match agent.name.to_lowercase() {
        name if query.is_empty() => usize::from(name.is_empty()),
        name if name == query => 0,
        name if name.starts_with(query) => 1,
        name if name.contains(query) => 2,
        _ => 3,
    });
    let rows = match (
        &list.catalog.error,
        list.catalog.loading,
        offered.is_empty(),
    ) {
        (_, true, true) => vec![note(theme, "Loading the agent registry…")],
        (Some(error), _, true) => vec![note(
            theme,
            &format!("The agent registry is unavailable: {error}"),
        )],
        (None, false, true) => vec![note(theme, "No registry agents match.")],
        _ => offered
            .iter()
            .map(|(index, agent)| available_row(theme, list, *index, agent))
            .collect(),
    };
    v_flex()
        .w_full()
        .gap(1.5)
        .child(header(
            theme,
            "Available",
            offered.len(),
            list.available_open,
            Message::ToggleAgentsAvailable,
        ))
        .when(list.available_open, |section| {
            section
                .child(
                    text("Agents from the Agent Client Protocol registry that can run on this machine.")
                        .text_sm()
                        .color(theme.colors.text_muted),
                )
                .child(card(theme, rows))
        })
}

/// One agent on offer: what it is called, what it does and how it is run,
/// and the button that installs it, or says it is or is being.
fn available_row(
    theme: &Theme,
    list: &AgentList<'_>,
    index: usize,
    agent: &Available,
) -> Div<Message> {
    let installing = list.catalog.installing.contains(&agent.id);
    let action = match installing {
        true => button("Installing…", Message::InstallAgent(index)).outlined(),
        false => button("Install", Message::InstallAgent(index)).filled(),
    };
    let how = match &agent.install {
        Install::Run { program, .. } => format!("runs through {program}"),
        Install::Download(_) => "downloaded for this machine".to_owned(),
    };
    let version = match agent.version.is_empty() {
        true => how,
        false => format!("{} · {how}", agent.version),
    };
    row(
        theme,
        &agent.name,
        &clipped(&agent.description),
        h_flex()
            .gap(3)
            .items_center()
            .child(
                text(version)
                    .text_xs()
                    .font_mono()
                    .color(theme.colors.text_subtle),
            )
            .child(action),
    )
}

/// The form an agent is described in.
fn form_for(theme: &Theme, list: &AgentList<'_>, form: &ServerForm) -> Div<Message> {
    form_row(
        theme,
        &FormView {
            form,
            focus: list.focus,
            solid: list.solid,
            suggestions: Vec::new(),
        },
    )
}

/// One agent: its name and whether it can be started, the command that
/// starts it, and for an agent the reader added, the menu that changes it.
fn agent_row(theme: &Theme, agent: &Agent, custom: Option<usize>) -> Div<Message> {
    let (standing, tone) = match (agent.installed(), agent.source) {
        (true, _) => ("Installed", theme.colors.accent),
        (false, Source::Package(_)) => ("Fetched on first run", theme.colors.text_muted),
        (false, _) => ("Not installed", theme.colors.text_subtle),
    };
    let command =
        pm_acp::command_line(std::iter::once(agent.program).chain(agent.arguments.iter().copied()));
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
                        .child(text(agent.name.to_owned()).font_medium())
                        .child(badge(theme, standing, tone))
                        .when(custom.is_some(), |line| {
                            line.child(badge(theme, "Custom", theme.colors.text_muted))
                        }),
                )
                .child(
                    text(clipped(&command))
                        .text_xs()
                        .font_mono()
                        .color(theme.colors.text_subtle),
                ),
        )
        .when_some(custom, |row, place| {
            row.child(
                icon_button(theme, IconName::More, Message::ShowAgentServerMenu(place))
                    .tooltip("More actions"),
            )
        })
}
