//! The welcome page of a new session or a new chat: pick an agent, then
//! describe the work.

use rustle::prelude::*;

use pm_components::{
    Chip, Composer, ComposerChoice, ComposerMenu, ComposerPick, ComposerVariant, IconButton, Label,
    LabelVariant, Size,
};
use pm_shell::{AgentChoice, BASE_SETTING, BrowserAction, ComposerAction, Start, Workspace};

use crate::actions::Actions;
use crate::browser_pane::browser_pane;
use crate::prompt_box::{choice_of, with_attaching, with_attachments};

/// Width of the welcome page's column.
const COLUMN_WIDTH: f32 = 680.0;

/// The glyphs of the providers that have one of their own, by provider id;
/// every other agent is drawn with a terminal.
const PROVIDER_ICONS: [(&str, Icon); 6] = [
    ("claude", phosphor::SPARKLE),
    ("copilot", phosphor::GITHUB_LOGO),
    ("openrouter", phosphor::GLOBE),
    ("openai", phosphor::LIGHTNING),
    ("azure", phosphor::LIGHTNING),
    ("gemini", phosphor::LIGHTBULB),
];

/// Emits the welcome page `start` describes over `workspace`, with the browser of the page beside it while it is open; the browser and the shells go to the session the page starts.
pub fn welcome_page(workspace: &Workspace, start: &Start, actions: &Actions) {
    let scheme = color_scheme();
    row_with(Modifier::new().fill_max_size(), || {
        stack_with(Modifier::new().weight(1.0).fill_max_height(), || {
            welcome_column(workspace, start, actions);
            if workspace.browser.is_none() {
                let open = actions.browser.clone();
                column_with(
                    Modifier::new().align(BoxAlignment::TopEnd).padding(8.0),
                    || {
                        IconButton::new(phosphor::GLOBE)
                            .size(Size::Sm)
                            .on_click(move || open(BrowserAction::Open))
                            .show();
                    },
                );
            }
        });
        if let Some(browser) = &workspace.browser {
            column_with(
                Modifier::new().weight(1.0).fill_max_height().border(
                    1.0,
                    scheme.outline_variant,
                    0.0,
                ),
                || browser_pane(browser, actions),
            );
        }
    });
}

/// Emits the column of the welcome page: the question naming the project, a chip for each agent and the composer whose first prompt starts the work.
fn welcome_column(workspace: &Workspace, start: &Start, actions: &Actions) {
    let scheme = color_scheme();
    column_arranged(
        Modifier::new()
            .fill_max_size()
            .background(scheme.background)
            .padding_xy(24.0, 40.0),
        Arrangement::Center,
        Alignment::Center,
        || {
            column_arranged(
                Modifier::new().width_in(0.0, COLUMN_WIDTH),
                Arrangement::SpacedBy(pm_components::spacing(28.0)),
                Alignment::Center,
                || {
                    Label::new(start.heading.clone())
                        .variant(LabelVariant::Heading)
                        .size(Size::Lg)
                        .show();
                    flow_row_with(
                        Modifier::new().fill_max_width(),
                        FlowOptions::new(8.0, 8.0).arrangement(Arrangement::Center),
                        || {
                            let shown: Vec<usize> = if start.shown.is_empty() {
                                (0..workspace.agents.len()).collect()
                            } else {
                                start.shown.clone()
                            };
                            for index in shown {
                                let Some(agent) = workspace.agents.get(index) else {
                                    continue;
                                };
                                key(agent.id.clone(), || {
                                    let choose = actions.choose_agent.clone();
                                    Chip::new(agent.name.clone())
                                        .icon(agent_icon(agent))
                                        .selected(index == start.agent)
                                        .on_click(move || choose(index))
                                        .show();
                                });
                            }
                        },
                    );
                    welcome_composer(workspace, start, actions);
                },
            );
        },
    );
}

/// Emits the composer whose first prompt starts the work `start` describes,
/// with the agent's settings, where the work runs and the branch it starts
/// from, and what is attached in `workspace`.
fn welcome_composer(workspace: &Workspace, start: &Start, actions: &Actions) {
    let send = actions.start.clone();
    let place = actions.composer.clone();
    let mut composer = Composer::new("Describe a task, ask a question, or type / for commands")
        .variant(ComposerVariant::Start)
        .menu(
            start
                .place_menu
                .then(|| ComposerMenu::Setting(PLACE.to_owned())),
        )
        .modifier(Modifier::new().fill_max_width())
        .on_pick(move |id, value| {
            if id == PLACE {
                let (worktree, container) = place_of(&value);
                place(ComposerAction::Place(worktree, container))
            } else {
                place(ComposerAction::Set(id, value))
            }
        })
        .on_send(move |text| send(text));
    for setting in &start.settings {
        composer = composer.setting(choice_of(setting));
    }
    for command in &start.commands {
        composer = composer.command(command.name.clone(), command.description.clone());
    }
    if start.at.is_some() {
        composer = composer.setting(place_choice(start));
        if start.worktree && !start.branches.is_empty() {
            composer = composer.setting(base_choice(start));
        }
    }
    composer = with_attachments(composer, &workspace.extras.composer, actions);
    let mention = actions.composer.clone();
    with_attaching(composer, actions)
        .mentions(workspace.tree.paths.clone())
        .on_mention(move || mention(ComposerAction::Mention))
        .show();
}

/// The choice of the branch the worktree of the work `start` describes is made from.
fn base_choice(start: &Start) -> ComposerChoice {
    let current = start
        .base
        .clone()
        .or_else(|| start.branches.first().cloned())
        .unwrap_or_default();
    ComposerChoice {
        icon: Some(phosphor::GIT_FORK),
        label: format!("From {current}"),
        id: BASE_SETTING.to_owned(),
        heading: "Branch from".to_owned(),
        current,
        picks: start
            .branches
            .iter()
            .map(|branch| ComposerPick {
                value: branch.clone(),
                name: branch.clone(),
                description: String::new(),
            })
            .collect(),
    }
}

/// The id of the welcome composer's choice of where the work runs.
const PLACE: &str = "place";

/// The places new work can run, as the value its pick hands back, its name and what it means.
const PLACES: [(&str, &str, &str); 3] = [
    ("local", "Local", "The main checkout"),
    ("worktree", "Worktree", "Detached head"),
    ("container", "Dev container", "In a worktree"),
];

/// The choice of where the work `start` describes runs, for the composer's toolbar.
fn place_choice(start: &Start) -> ComposerChoice {
    let current = match (start.worktree, start.container) {
        (true, true) => "container",
        (true, false) => "worktree",
        _ => "local",
    };
    let label = PLACES
        .iter()
        .find(|(value, _, _)| *value == current)
        .map_or("Local", |(_, name, _)| name);
    ComposerChoice {
        icon: Some(if start.worktree {
            phosphor::GIT_BRANCH
        } else {
            phosphor::FOLDER
        }),
        label: label.to_owned(),
        id: PLACE.to_owned(),
        heading: "Run in".to_owned(),
        current: current.to_owned(),
        picks: PLACES
            .iter()
            .filter(|(value, _, _)| match *value {
                "worktree" => !start.branches.is_empty(),
                "container" => start.container_offered && !start.branches.is_empty(),
                _ => true,
            })
            .map(|(value, name, description)| ComposerPick {
                value: (*value).to_owned(),
                name: (*name).to_owned(),
                description: (*description).to_owned(),
            })
            .collect(),
    }
}

/// Whether the place picked as `value` is a worktree, and whether its agent runs in the dev container.
fn place_of(value: &str) -> (bool, bool) {
    match value {
        "worktree" => (true, false),
        "container" => (true, true),
        _ => (false, false),
    }
}

/// The glyph `agent` is drawn with: a plug for a model server the reader
/// declared, the one of its provider, or a terminal when the provider has
/// none of its own.
fn agent_icon(agent: &AgentChoice) -> Icon {
    if agent.server {
        return phosphor::PLUG;
    }
    PROVIDER_ICONS
        .iter()
        .find(|(provider, _)| *provider == agent.provider)
        .map_or(phosphor::TERMINAL_WINDOW, |(_, glyph)| glyph.clone())
}
