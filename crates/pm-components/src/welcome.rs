//! The empty page of a new session or a new chat: pick an agent, then describe the work.

use crate::areas::{DiffEvent, SidebarEvent};
use crate::kit;
use crate::target::Target;
use crate::theme;
use rustle::{Alignment, Arrangement, FlowOptions, column_arranged, flow_row_with};
use rustle::{BoxAlignment, Modifier, phosphor};

use crate::Button;
use crate::button::Variant;
use crate::composer::composer;
use crate::icons::panel_right;
use crate::size::Size;
use crate::state::{Page, WorkspaceState};
use crate::theme::{Role, sp};

/// Emits a chip naming one agent, filled while it is the one chosen, with how much of its account's allowance is used as its tooltip once the provider has said.
#[track_caller]
fn chip(state: &WorkspaceState, index: usize) {
    let agent = state.agent_look(index);
    let chip = Button::new(agent.name, Target::Sidebar(SidebarEvent::Agent(index)))
        .variant(Variant::Outline)
        .round()
        .with_icon(agent.icon)
        .size(Size::Md)
        .selected(index == state.agent);
    match state
        .allowance_of(index, state.login_for_new_work(index))
        .and_then(crate::usage::tooltip)
    {
        Some(words) => chip.tooltip(words).build(),
        None => chip.build(),
    }
}

/// What the page for starting work asks: what to build in the project or the submodule work starts in, or, for a chat, which belongs to no project, what to talk about.
fn heading(state: &WorkspaceState) -> String {
    let project = match state.page {
        Page::Chat => None,
        _ => state.projects.get(state.current_project()),
    };
    match project {
        Some(project) => format!(
            "What should we build in {}?",
            state
                .new_in
                .as_deref()
                .and_then(|submodule| submodule.rsplit('/').next())
                .unwrap_or(&project.name)
        ),
        None => "What would you like to talk about?".to_owned(),
    }
}

/// Emits the page for starting work in a project, naming the project it starts in, or for starting a chat, which belongs to none.
#[track_caller]
pub fn welcome(state: &WorkspaceState, above: f32) {
    let reviewable = state.review_session().is_some();
    rustle::stack_with(Modifier::new().weight(1.0), || {
        column_arranged(
            Modifier::new()
                .fill_max_size()
                .padding_xy(theme::sp(6), theme::sp(10)),
            Arrangement::Center,
            Alignment::Center,
            || {
                column_arranged(
                    Modifier::new().width_in(0.0, 680.0).fill_max_width(),
                    Arrangement::SpacedBy(sp(7)),
                    Alignment::Center,
                    || {
                        kit::text(
                            Modifier::new(),
                            heading(state),
                            theme::text_style(28, 1.25)
                                .font_weight(rustle::FontWeight::SEMI_BOLD)
                                .color(theme::color(Role::Text)),
                        );
                        flow_row_with(
                            Modifier::new().fill_max_width(),
                            FlowOptions::new(sp(2), sp(2)).arrangement(Arrangement::Center),
                            || {
                                for index in state.startable_agents() {
                                    chip(state, index);
                                }
                            },
                        );
                        composer(
                            state,
                            "Describe a task, ask a question, or type / for commands",
                            None,
                            3,
                            above,
                        );
                    },
                );
            },
        );
        if !state.review && reviewable {
            Button::icon(phosphor::SIDEBAR_SIMPLE, Target::Diff(DiffEvent::Review))
                .mark(|| panel_right(false, 4.0, Role::Muted))
                .modifier(
                    Modifier::new()
                        .align(BoxAlignment::TopEnd)
                        .offset(-sp(2.0), sp(2.0)),
                )
                .build();
        }
    });
}
