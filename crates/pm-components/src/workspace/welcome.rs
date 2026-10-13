//! The empty page of a new workspace: pick an agent, then describe the work.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, flex_1, flex_col, font_semibold,
    gap, h, items_center, justify_center, leading_ratio, max_w_px, min_h, px, py, rounded,
    rounded_full, text_color, text_px, tracking_tight, w_full,
};
use pm_ui::{Node, node, text};

use super::composer::composer;
use super::sample::{AGENTS, SUGGESTIONS};
use super::state::{ADD_PROJECT, AGENT_BASE, SUGGESTION_BASE, WorkspaceState};
use crate::Button;
use crate::parts::{glyph, line};

/// A chip naming one agent, filled while it is the one chosen.
fn chip(index: usize, chosen: bool) -> Node {
    let agent = &AGENTS[index];
    let tone = if chosen { Role::Text } else { Role::Muted };
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(9),
        px(3.5),
        rounded_full(),
        border(),
        cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ];
    if chosen {
        styles.extend([bg(Role::SurfaceSelected), border_color(Role::Muted)]);
    } else {
        styles.push(border_color(Role::Border));
    }
    node()
        .styles(styles)
        .child(glyph(agent.icon, 4.0, tone))
        .child(line(agent.name, 13, 1.4, tone))
        .focusable(FocusId(AGENT_BASE + index as u64))
}

/// A one-click way to start: a glyph and what it asks for.
fn suggestion(index: usize) -> Node {
    let (icon, words) = SUGGESTIONS[index];
    node()
        .styles([
            flex(),
            items_center(),
            gap(2.5),
            flex_1(),
            px(3.5),
            py(3),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::SurfaceHover).hover(),
            cursor_pointer(),
        ])
        .child(glyph(icon, 4.0, Role::Subtle))
        .child(line(words, 13, 1.4, Role::Muted))
        .focusable(FocusId(SUGGESTION_BASE + index as u64))
}

/// The page for starting work in a project.
pub(super) fn welcome(state: &WorkspaceState) -> Node {
    let agent = &AGENTS[state.agent.min(AGENTS.len() - 1)];
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
        ])
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
                .child(text("What should we build?").styles([
                    text_px(28),
                    leading_ratio(1.25),
                    font_semibold(),
                    tracking_tight(),
                    text_color(Role::Text),
                ]))
                .child(
                    node()
                        .styles([flex(), justify_center(), gap(2)])
                        .children((0..AGENTS.len()).map(|index| chip(index, index == state.agent))),
                )
                .child(composer(
                    state,
                    "Describe a task, ask a question, or type / for commands",
                    agent,
                ))
                .child(
                    node()
                        .styles([flex(), flex_col(), gap(2), w_full()])
                        .children((0..SUGGESTIONS.len()).step_by(2).map(|first| {
                            node().styles([flex(), gap(2), w_full()]).children(
                                (first..(first + 2).min(SUGGESTIONS.len())).map(suggestion),
                            )
                        })),
                ),
        )
}

/// The page shown while no project has been added.
pub(super) fn no_project() -> Node {
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
