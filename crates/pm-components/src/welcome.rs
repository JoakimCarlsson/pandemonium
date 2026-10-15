//! The empty page of a new session: pick an agent, then describe the work.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, absolute, flex, flex_1, flex_col, font_semibold, gap, items_center, justify_center,
    leading_ratio, max_w_px, min_h, px, py, relative, right, text_color, text_px, top,
    tracking_tight, w_full,
};
use pm_text::Shaper;
use pm_ui::{Node, node, text};

use crate::composer::composer;
use crate::parts::line;
use crate::state::{ADD_PROJECT, AGENT_BASE, REVIEW, WorkspaceState};
use crate::{Button, Chip, chip};

/// A chip naming one agent, filled while it is the one chosen.
fn chip(state: &WorkspaceState, index: usize) -> Node {
    let agent = state.agent_look(index);
    Chip::new(agent.name, FocusId(AGENT_BASE + index as u64))
        .icon(agent.icon)
        .size(chip::Size::Md)
        .selected(index == state.agent)
        .build()
}

/// The page for starting work in a project, naming the project it starts in.
pub fn welcome(state: &WorkspaceState, above: f32, text_width: f32, shaper: &mut Shaper) -> Node {
    let project = &state.projects[state.current_project()];
    node()
        .styles([
            flex(),
            flex_col(),
            items_center(),
            justify_center(),
            flex_1(),
            min_h(0),
            px(6),
            py(10),
            relative(),
        ])
        .children((!state.review).then(|| {
            Button::icon(IconName::PanelRight, REVIEW).build().styles([
                absolute(),
                top(2),
                right(2),
            ])
        }))
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    items_center(),
                    gap(7),
                    w_full(),
                    max_w_px(680.0),
                ])
                .child(
                    text(format!("What should we build in {}?", project.name)).styles([
                        text_px(28),
                        leading_ratio(1.25),
                        font_semibold(),
                        tracking_tight(),
                        text_color(Role::Text),
                    ]),
                )
                .child(
                    node().styles([flex(), justify_center(), gap(2)]).children(
                        state
                            .offered_agents()
                            .into_iter()
                            .map(|index| chip(state, index)),
                    ),
                )
                .child(composer(
                    state,
                    "Describe a task, ask a question, or type / for commands",
                    None,
                    3,
                    above,
                    text_width,
                    shaper,
                )),
        )
}

/// The page shown while no project has been added.
pub fn no_project() -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            items_center(),
            justify_center(),
            gap(3),
            flex_1(),
            min_h(0),
        ])
        .child(text("Add a project to get started").styles([
            text_px(20),
            leading_ratio(1.25),
            font_semibold(),
            tracking_tight(),
            text_color(Role::Text),
        ]))
        .child(line(
            "Agents work inside a folder or a Git repository.",
            14,
            1.4,
            Role::Muted,
        ))
        .child(Button::new("Add project", ADD_PROJECT).build())
}
