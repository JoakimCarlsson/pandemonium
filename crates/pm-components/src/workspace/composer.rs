//! The prompt box where work is handed to an agent.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_col, gap, items_center, pb, px, rounded,
    rounded_full, shadow_md, w_full,
};
use pm_ui::{Node, node};

use super::sample::AgentChoice;
use super::state::{PROMPT, SEND, WorkspaceState};
use crate::button::{Size, Variant};
use crate::input::{self, Size as InputSize};
use crate::parts::{line, spacer};
use crate::{Button, Input};

/// Attaches files to the prompt.
const ATTACH: FocusId = FocusId(318);

/// The prompt box for `state`: a line to write in, the agent's model and mode, and a send button.
pub(super) fn composer(state: &WorkspaceState, placeholder: &str, agent: &AgentChoice) -> Node {
    let empty = state.prompt.text().trim().is_empty();
    node()
        .styles([
            flex(),
            flex_col(),
            w_full(),
            rounded(Radius::Xl),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
            shadow_md(),
        ])
        .child(
            Input::new(&state.prompt, PROMPT)
                .variant(input::Variant::Ghost)
                .size(InputSize::Lg)
                .placeholder(placeholder)
                .build(),
        )
        .child(
            node()
                .styles([flex(), items_center(), gap(1), px(2.5), pb(2.5)])
                .child(Button::icon(IconName::Plus, ATTACH).disabled(true).build())
                .child(line(
                    format!("{} · {}", agent.model, agent.mode),
                    12,
                    1.4,
                    Role::Muted,
                ))
                .child(spacer())
                .child(
                    Button::icon(IconName::ArrowUp, SEND)
                        .variant(Variant::Default)
                        .size(Size::Icon)
                        .disabled(empty)
                        .build()
                        .styles([rounded_full()]),
                ),
        )
}
