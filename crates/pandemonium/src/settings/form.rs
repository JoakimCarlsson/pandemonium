//! The form a server is described in, for the two pages that have one.
//!
//! An MCP server and an agent are described alike: a name, the command that
//! starts it or the address it listens at, and the variables it is given. The
//! form holds every part of either in boxes at once, so that changing one
//! thing is one box and a press of Save; a page says which it is for and
//! what the registry suggests adding, and the app writes down what is saved.

use pm_acp::{Agent, McpServer};
use pm_ui::{Div, IconName, Styled, Theme, button, h_flex, icon_button, text, v_flex};

use crate::input::{Input, input_view};
use crate::message::Message;

/// One box of the form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormField {
    /// What the server is called.
    Name,
    /// The command to run, or the address of the server.
    Target,
    /// The name of the `n`-th variable or header.
    VariableName(usize),
    /// The value of the `n`-th variable or header.
    VariableValue(usize),
}

/// What a form describes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Subject {
    /// A tool server every agent is started with.
    McpServer,
    /// An agent the reader runs.
    Agent,
}

/// Something being described, every part of it in one place: the boxes it
/// is written in, and which one it replaces when it is saved.
pub struct ServerForm {
    /// What is being described.
    pub subject: Subject,
    /// The place in its list of the one being edited, when one is.
    pub index: Option<usize>,
    /// The MCP server as it was, or as the registry lists it, whose way of
    /// being reached is kept while its address is left as it was.
    pub original: Option<McpServer>,
    /// What it is called.
    pub name: Input,
    /// The command to run, or the address of the server.
    pub target: Input,
    /// The names and values of its environment or headers.
    pub variables: Vec<(Input, Input)>,
}

/// A box holding `value`.
fn line(value: &str) -> Input {
    let mut input = Input::one_line("Server");
    input.set(value);
    input
}

impl ServerForm {
    /// A form for an MCP server, holding what `server` holds, or nothing
    /// when there is none; it replaces the `index`-th server when it has one.
    pub fn for_mcp(index: Option<usize>, server: Option<&McpServer>) -> Self {
        let variables = server
            .map(|server| server.reach.variables().to_vec())
            .unwrap_or_default()
            .into_iter()
            .map(|(name, value)| (line(&name), line(&value)))
            .collect();
        Self {
            subject: Subject::McpServer,
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

    /// A form for an agent, holding what `agent` holds, or nothing when
    /// there is none; it replaces the `index`-th custom agent when it has one.
    pub fn for_agent(index: Option<usize>, agent: Option<&Agent>) -> Self {
        let variables = agent
            .map(|agent| agent.env.to_vec())
            .unwrap_or_default()
            .into_iter()
            .map(|(name, value)| (line(name), line(value)))
            .collect();
        let command = agent.map(|agent| {
            pm_acp::command_line(
                std::iter::once(agent.program).chain(agent.arguments.iter().copied()),
            )
        });
        Self {
            subject: Subject::Agent,
            index,
            original: None,
            name: line(agent.map_or("", |agent| agent.name)),
            target: line(&command.unwrap_or_default()),
            variables,
        }
    }

    /// The box `field` names.
    pub fn input(&self, field: FormField) -> Option<&Input> {
        match field {
            FormField::Name => Some(&self.name),
            FormField::Target => Some(&self.target),
            FormField::VariableName(at) => self.variables.get(at).map(|pair| &pair.0),
            FormField::VariableValue(at) => self.variables.get(at).map(|pair| &pair.1),
        }
    }

    /// The box `field` names, to write in.
    pub fn input_mut(&mut self, field: FormField) -> Option<&mut Input> {
        match field {
            FormField::Name => Some(&mut self.name),
            FormField::Target => Some(&mut self.target),
            FormField::VariableName(at) => self.variables.get_mut(at).map(|pair| &mut pair.0),
            FormField::VariableValue(at) => self.variables.get_mut(at).map(|pair| &mut pair.1),
        }
    }

    /// Every box, in the order the Tab key walks them.
    pub fn fields(&self) -> Vec<FormField> {
        [FormField::Name, FormField::Target]
            .into_iter()
            .chain(
                (0..self.variables.len())
                    .flat_map(|at| [FormField::VariableName(at), FormField::VariableValue(at)]),
            )
            .collect()
    }

    /// Adds a variable called `name`, empty.
    pub fn add_variable(&mut self, name: &str) {
        self.variables.push((line(name), line("")));
    }

    /// Takes the `at`-th variable out.
    pub fn remove_variable(&mut self, at: usize) {
        if at < self.variables.len() {
            self.variables.remove(at);
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

    /// What the target box asks for.
    fn target_label(&self) -> &'static str {
        match self.subject {
            Subject::McpServer => "Command, or the https:// address of the server",
            Subject::Agent => "Command that starts the agent in protocol mode, with its arguments",
        }
    }

    /// What the variables are called, for a server that is reached over the
    /// network, one that is started, or an agent.
    fn variables_label(&self) -> &'static str {
        match (
            self.subject,
            self.target.value().trim_start().starts_with("http"),
        ) {
            (Subject::McpServer, true) => "Headers",
            _ => "Environment variables",
        }
    }
}

/// What a form is drawn with besides its boxes.
pub struct FormView<'a> {
    /// The form.
    pub form: &'a ServerForm,
    /// The box that has the keyboard, while one does.
    pub focus: Option<FormField>,
    /// Whether its caret is in the visible half of its blink.
    pub solid: bool,
    /// The variables the registry lists for what is being described that the
    /// form does not have yet, each a press away from being added.
    pub suggestions: Vec<String>,
}

/// Builds the form: every box at once, and the presses that save it or let
/// it go.
pub fn form_row(theme: &Theme, view: &FormView<'_>) -> Div<Message> {
    let form = view.form;
    let suggestions = view
        .suggestions
        .iter()
        .enumerate()
        .map(|(place, name)| {
            button(format!("+ {name}"), Message::SuggestFormVariable(place)).outlined()
        })
        .collect::<Vec<_>>();
    let boxed = |label: &str, field: FormField| {
        v_flex()
            .w_full()
            .gap(0.5)
            .child(
                text(label.to_owned())
                    .text_sm()
                    .color(theme.colors.text_muted),
            )
            .child(field_view(theme, view, field))
    };
    v_flex()
        .w_full()
        .p(3)
        .gap(2.5)
        .child(boxed("Name", FormField::Name))
        .child(boxed(form.target_label(), FormField::Target))
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
                .child(button("+ Add", Message::AddFormVariable).outlined()),
        )
        .children((0..form.variables.len()).map(|at| {
            h_flex()
                .w_full()
                .gap(1.5)
                .items_center()
                .child(h_flex().flex_1().child(field_view(
                    theme,
                    view,
                    FormField::VariableName(at),
                )))
                .child(h_flex().flex_1().child(field_view(
                    theme,
                    view,
                    FormField::VariableValue(at),
                )))
                .child(
                    icon_button(theme, IconName::Close, Message::RemoveFormVariable(at))
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
                .child(button("Save", Message::SaveServerForm).filled())
                .child(button("Cancel", Message::CancelServerForm).outlined()),
        )
}

/// One box of the form, drawn as every box of text in the window is.
fn field_view(theme: &Theme, view: &FormView<'_>, field: FormField) -> Div<Message> {
    match view.form.input(field) {
        Some(input) => input_view(
            theme,
            input,
            view.focus == Some(field),
            view.solid,
            1.0,
            move |phase, from, to| Message::WriteFormField(field, phase, from, to),
            Message::ShowInputMenu,
        ),
        None => h_flex(),
    }
}
