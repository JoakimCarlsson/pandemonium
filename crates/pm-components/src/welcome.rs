//! The empty page of a new session or a new chat: pick an agent, then describe the work.

use rustle::{BoxAlignment, Modifier};

use crate::Button;
use crate::button::Variant;
use crate::composer::composer;
use crate::kit::{
    Flex, FocusId, IconName, Role, Styled, Tooltip, Type, column, label, layer, sp, wrapping_with,
};
use crate::size::Size;
use crate::state::{AGENT_BASE, Page, REVIEW, WorkspaceState};

/// Emits a chip naming one agent, filled while it is the one chosen, with how much of its account's allowance is used as its tooltip once the provider has said.
#[track_caller]
fn chip(state: &WorkspaceState, index: usize) {
    let agent = state.agent_look(index);
    let chip = Button::new(agent.name, FocusId(AGENT_BASE + index as u64))
        .variant(Variant::Outline)
        .round()
        .with_icon(agent.icon)
        .size(Size::Md)
        .selected(index == state.agent);
    match state
        .allowance_of(index, state.login_for_new_work(index))
        .and_then(crate::usage::tooltip)
    {
        Some(words) => chip.modifier(Modifier::new().tooltip(words)).build(),
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
pub fn welcome(state: &WorkspaceState, above: f32, text_width: f32) {
    let reviewable = state.review_session().is_some();
    layer(Modifier::new().grow(), || {
        column(
            Modifier::new().fill_max_size().pad_x(6).pad_y(10),
            Flex::new().center().middle(),
            || {
                column(
                    Modifier::new().max_w_px(680.0).fill_max_width(),
                    Flex::new().center().gap(7),
                    || {
                        label(
                            heading(state),
                            Type::new(28).ratio(1.25).semibold().tone(Role::Text),
                        );
                        wrapping_with(
                            Modifier::new().fill_max_width(),
                            Flex::new().gap(2).middle(),
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
                            text_width,
                        );
                    },
                );
            },
        );
        if !state.review && reviewable {
            Button::icon(IconName::PanelRight, REVIEW)
                .modifier(
                    Modifier::new()
                        .placed(BoxAlignment::TopEnd)
                        .at(-sp(2.0), sp(2.0)),
                )
                .build();
        }
    });
}
