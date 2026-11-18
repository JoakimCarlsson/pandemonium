//! The plan an agent proposes before it starts: the panel that reads it and the band above the prompt that answers it.

use crate::kit::{
    Flex, FocusId, IconName, Radius, Role, Scroll, Side, Styled, column, row, scroll_area,
    wrapping_with,
};
use pm_harness::{Permission, Verdict};
use rustle::Modifier;

use crate::button::Variant;
use crate::parts::{line, spacer};
use crate::size::Size;
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

/// Emits the panel `width` logical pixels wide that reads `source`, the plan the agent proposed, as markdown beside the conversation.
#[track_caller]
pub fn panel(state: &WorkspaceState, source: &str, width: f32) {
    column(
        Modifier::new()
            .width(width)
            .fill_max_height()
            .fill(Role::Background)
            .stroke_side(Side::Left, 1.0, Role::Border),
        Flex::new(),
        || {
            row(
                Modifier::new()
                    .h(12)
                    .stroke_side(Side::Bottom, 1.0, Role::Border)
                    .pad_x(4),
                Flex::new().center().gap(2),
                || {
                    line("Plan", 13, 1.4, Role::Muted);
                    spacer();
                    Button::icon(IconName::Close, CLOSE_PLAN).build();
                },
            );
            scroll_area(
                Modifier::new().grow().clip_to(Radius::None),
                Scroll::from_start(PLAN_SCROLL, state.plan_scroll),
                || {
                    column(
                        Modifier::new().fill_max_width().pad_x(4).pad_y(4),
                        Flex::new(),
                        || {
                            Markdown::new(source, width - 33.0).build();
                        },
                    );
                },
            );
        },
    );
}

/// Emits the button answering with the choice at `index`, bold for the one that goes ahead without asking again.
#[track_caller]
fn answer(index: usize, name: &str, verdict: &Verdict) {
    Button::new(name, FocusId(CHOICE_BASE + index as u64))
        .variant(match verdict {
            Verdict::Reject => Variant::Ghost,
            Verdict::Allow => Variant::Outline,
            Verdict::Always => Variant::Default,
        })
        .size(Size::Sm)
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
    column(
        Modifier::new()
            .fill_rounded(Role::Surface, Radius::Xl)
            .stroke(Role::Border, Radius::Xl)
            .pad_x(4)
            .pad_y(3),
        Flex::new().gap(2),
        || {
            row(Modifier::new(), Flex::new().center(), || {
                line("The agent proposed a plan", 14, 1.4, Role::Text);
                spacer();
                Button::new("Open plan", OPEN_PLAN)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .build();
            });
            wrapping_with(Modifier::new(), Flex::new().gap(2).center(), || {
                for (index, _) in ask
                    .choices
                    .iter()
                    .enumerate()
                    .filter(|(_, choice)| choice.verdict == Verdict::Reject)
                    .take(1)
                {
                    answer(index, "Reject", &Verdict::Reject);
                }
                Button::new("Revise…", REVISE)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
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
            });
        },
    );
    true
}
