//! The plan an agent proposes before it starts: the panel that reads it and the band above the prompt that answers it.

use pm_acp::{Permission, Verdict};
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_b, border_color, border_l, flex, flex_col, flex_none,
    flex_wrap, gap, h, h_full, items_center, overflow_hidden, px, py, rounded, w_px,
};
use pm_text::Shaper;
use pm_ui::{Node, node};

use crate::button::{Size, Variant};
use crate::parts::{line, spacer};
use crate::state::{CHOICE_BASE, CLOSE_PLAN, OPEN_PLAN, PLAN_SCROLL, REVISE, WorkspaceState};
use crate::{Button, Markdown};

/// How narrow and how wide the plan panel can be dragged, in logical pixels.
pub const WIDTHS: (f32, f32) = (300.0, 960.0);

/// How wide the plan panel is when `room` logical pixels are left beside the sidebar and the review panel: as wide as it was dragged, or else nearly half of the room, kept within what a plan reads well in.
pub fn width_in(state: &WorkspaceState, room: f32) -> f32 {
    if state.plan_width > 0.0 {
        state.plan_width.clamp(WIDTHS.0, WIDTHS.1)
    } else {
        (room * 0.45).clamp(300.0, 520.0)
    }
}

/// The panel `width` logical pixels wide that reads `source`, the plan the agent proposed, as markdown beside the conversation.
pub fn panel(state: &WorkspaceState, source: &str, width: f32, shaper: &mut Shaper) -> Node {
    let header = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(12),
            px(4),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(line("Plan", 13, 1.4, Role::Muted))
        .child(spacer())
        .child(Button::icon(IconName::Close, CLOSE_PLAN).build());
    let body = node()
        .styles([flex(), flex_col(), px(4), py(4)])
        .child(Markdown::new(source, width - 33.0).build(shaper));
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            w_px(width),
            h_full(),
            bg(Role::Background),
            border_l(),
            border_color(Role::Border),
        ])
        .child(header)
        .child(
            node()
                .styles([flex(), flex_col(), pm_style::flex_1(), pm_style::min_h(0)])
                .styles([overflow_hidden()])
                .scroll_from_start(PLAN_SCROLL, state.plan_scroll)
                .child(body),
        )
}

/// The button answering with the choice at `index`, bold for the one that goes ahead without asking again.
fn answer(index: usize, name: &str, verdict: &Verdict) -> Node {
    Button::new(name, FocusId(CHOICE_BASE + index as u64))
        .variant(match verdict {
            Verdict::Reject => Variant::Ghost,
            Verdict::Allow => Variant::Outline,
            Verdict::Always => Variant::Default,
        })
        .size(Size::Sm)
        .build()
}

/// The card that stands in for the prompt box while the plan in `ask` waits for a decision, or nothing when `ask` proposes none.
pub fn proposal(ask: &Permission) -> Option<Node> {
    ask.plan.as_ref()?;
    let refusals = ask
        .choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| choice.verdict == Verdict::Reject)
        .take(1)
        .map(|(index, _)| answer(index, "Reject", &Verdict::Reject));
    let accepts = ask
        .choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| choice.verdict != Verdict::Reject)
        .map(|(index, choice)| answer(index, &choice.name, &choice.verdict));
    let title = node()
        .styles([flex(), items_center()])
        .child(line("The agent proposed a plan", 14, 1.4, Role::Text))
        .child(spacer())
        .child(
            Button::new("Open plan", OPEN_PLAN)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build(),
        );
    let answers = node()
        .styles([flex(), flex_wrap(), items_center(), gap(2)])
        .children(refusals)
        .child(
            Button::new("Revise…", REVISE)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build(),
        )
        .child(spacer())
        .children(accepts);
    Some(
        node()
            .styles([
                flex(),
                flex_col(),
                gap(2),
                px(4),
                py(3),
                rounded(Radius::Xl),
                border(),
                border_color(Role::Border),
                bg(Role::Surface),
            ])
            .child(title)
            .child(answers),
    )
}
