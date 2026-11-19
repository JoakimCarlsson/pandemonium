//! The plan an agent proposes before it starts: the panel that reads it and the band above the prompt that answers it.

use crate::areas::ChatEvent;
use crate::kit::{self, Frame};
use crate::target::Target;
use crate::theme;
use crate::theme::sp;
use crate::theme::{Radius, Role};
use pm_harness::{Permission, Verdict};
use rustle::Modifier;
use rustle::phosphor;
use rustle::{Alignment, FlowOptions, flow_row_with};

use crate::button::Variant;
use crate::parts::{line, spacer};
use crate::state::WorkspaceState;
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

/// Emits the panel `width` logical pixels wide that reads `source`, the plan the agent proposed, as markdown beside the conversation.
#[track_caller]
pub fn panel(state: &WorkspaceState, source: &str, width: f32) {
    kit::column(
        Modifier::new()
            .width(width)
            .fill_max_height()
            .background(theme::color(Role::Background))
            .ruled(theme::Side::Left),
        0,
        || {
            kit::row(
                Modifier::new()
                    .fill_max_width()
                    .height(theme::sp(12))
                    .ruled(theme::Side::Bottom)
                    .padding_xy(theme::sp(4), 0.0),
                2,
                || {
                    line("Plan", 13, 1.4, Role::Muted);
                    spacer();
                    Button::icon(phosphor::X, Target::Chat(ChatEvent::ClosePlan)).build();
                },
            );
            kit::column(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .clip()
                    .vertical_scroll(&state.plan_scroll)
                    .fill_max_width()
                    .padding_xy(theme::sp(4), 0.0)
                    .padding_xy(0.0, theme::sp(4)),
                0,
                || {
                    Markdown::new(source, width - 33.0).build();
                },
            );
        },
    );
}

/// Emits the button answering with the choice at `index`, bold for the one that goes ahead without asking again.
#[track_caller]
fn answer(index: usize, name: &str, verdict: &Verdict) {
    kit::small_button(
        name,
        Target::Chat(ChatEvent::Choice(index)),
        match verdict {
            Verdict::Reject => Variant::Ghost,
            Verdict::Allow => Variant::Outline,
            Verdict::Always => Variant::Default,
        },
    )
    .build();
}

/// Emits the card that stands in for the prompt box while the plan in `ask` waits for a decision; emits nothing when `ask` proposes none.
///
/// Returns whether it emitted the card.
#[track_caller]
pub fn proposal(ask: &Permission) -> bool {
    if ask.plan.is_none() {
        return false;
    }
    kit::column(
        Modifier::new()
            .panel(Radius::Xl)
            .padding_xy(theme::sp(4), 0.0)
            .padding_xy(0.0, theme::sp(3)),
        2,
        || {
            kit::row(Modifier::new().fill_max_width(), 0, || {
                kit::text(
                    Modifier::new().weight(1.0),
                    "The agent proposed a plan",
                    kit::line(14, Role::Text),
                );
                kit::small_button(
                    "Open plan",
                    Target::Chat(ChatEvent::OpenPlan),
                    Variant::Ghost,
                )
                .build();
            });
            flow_row_with(
                Modifier::new().fill_max_width(),
                FlowOptions::new(sp(2), sp(2)).item_alignment(Alignment::Center),
                || {
                    for (index, _) in ask
                        .choices
                        .iter()
                        .enumerate()
                        .filter(|(_, choice)| choice.verdict == Verdict::Reject)
                        .take(1)
                    {
                        answer(index, "Reject", &Verdict::Reject);
                    }
                    kit::small_button("Revise…", Target::Chat(ChatEvent::Revise), Variant::Ghost)
                        .build();
                    spacer();
                    for (index, choice) in ask
                        .choices
                        .iter()
                        .enumerate()
                        .filter(|(_, choice)| choice.verdict != Verdict::Reject)
                    {
                        answer(index, &choice.name, &choice.verdict);
                    }
                },
            );
        },
    );
    true
}
