//! The plan an agent proposes before it starts: the panel that reads it and the band above the prompt that answers it.

use crate::focus::FocusId;
use crate::theme;
use crate::theme::sp;
use crate::theme::{Radius, Role};
use pm_harness::{Permission, Verdict};
use rustle::Modifier;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, FlowOptions, column_arranged, flow_row_with, row_arranged};

use crate::button::Variant;
use crate::parts::{line, spacer};
use crate::size::Size;
use crate::state::{CHOICE_BASE, CLOSE_PLAN, OPEN_PLAN, REVISE, WorkspaceState};
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
    column_arranged(
        Modifier::new()
            .width(width)
            .fill_max_height()
            .background(theme::color(Role::Background))
            .draw_behind(theme::edge(
                theme::Side::Left,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(1.0, 0.0, 0.0, 0.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(theme::sp(12))
                    .draw_behind(theme::edge(
                        theme::Side::Bottom,
                        1.0,
                        theme::color(Role::Border),
                    ))
                    .padding_each(0.0, 0.0, 0.0, 1.0)
                    .padding_xy(theme::sp(4), 0.0),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    line("Plan", 13, 1.4, Role::Muted);
                    spacer();
                    Button::icon(phosphor::X, CLOSE_PLAN).build();
                },
            );
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .clip()
                    .vertical_scroll(&state.plan_scroll)
                    .fill_max_width()
                    .padding_xy(theme::sp(4), 0.0)
                    .padding_xy(0.0, theme::sp(4)),
                Arrangement::Start,
                Alignment::Start,
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
    column_arranged(
        Modifier::new()
            .background_rounded(theme::color(Role::Surface), Radius::Xl.px())
            .border(1.0, theme::color(Role::Border), Radius::Xl.px())
            .padding(1.0)
            .padding_xy(theme::sp(4), 0.0)
            .padding_xy(0.0, theme::sp(3)),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::Start,
                Alignment::Center,
                || {
                    rustle::basic_text(
                        Modifier::new().weight(1.0),
                        "The agent proposed a plan",
                        theme::text_style(14, 1.4)
                            .color(theme::color(Role::Text))
                            .max_lines(1),
                        rustle::TextOverflow::Ellipsis,
                    );
                    Button::new("Open plan", OPEN_PLAN)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build();
                },
            );
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
                },
            );
        },
    );
    true
}
