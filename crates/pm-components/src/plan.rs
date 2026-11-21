//! The plan an agent proposes before it starts: the panel that reads it and the band above the prompt that answers it.

use crate::areas::ChatEvent;
use crate::button;
use crate::parts;
use crate::target::Target;
use crate::theme;
use crate::theme::Frame;
use crate::theme::sp;
use crate::theme::{Radius, Role};
use pm_harness::{Permission, Verdict};
use rustle::Modifier;
use rustle::phosphor;
use rustle::{Alignment, FlowOptions, flow_row_with};
use rustle::{Arrangement, TextOverflow};

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

/// Emits the panel `width` logical pixels wide that reads `source`, the plan the agent proposed, as markdown beside the conversation, scrolled by `scroll`. It takes nothing else, so it is composed again only when one of those changes.
#[track_caller]
pub fn panel(scroll: &rustle::ScrollState, source: &str, width: f32) {
    rustle::row_arranged(
        Modifier::new()
            .width(width)
            .fill_max_height()
            .background(theme::color(Role::Background)),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::vertical_divider(Modifier::new(), 1.0, theme::color(Role::Border));
            rustle::column_arranged(
                Modifier::new().weight(1.0).fill_max_height(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    parts::bar(12, 4, 2, || {
                        line("Plan", 13, 1.4, Role::Muted);
                        spacer();
                        Button::icon(phosphor::X, Target::Chat(ChatEvent::ClosePlan)).build();
                    });
                    rustle::column_arranged(
                        Modifier::new()
                            .fill_max_width()
                            .weight(1.0)
                            .clip()
                            .vertical_scroll(scroll)
                            .fill_max_width()
                            .padding_xy(theme::sp(4), theme::sp(4)),
                        Arrangement::Start,
                        Alignment::Start,
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
    button::action(
        name,
        Target::Chat(ChatEvent::Choice(index)),
        crate::steps::variant_of(verdict),
    );
}

/// Emits the card that stands in for the prompt box while the plan in `ask` waits for a decision; emits nothing when `ask` proposes none.
///
/// Returns whether it emitted the card.
#[track_caller]
pub fn proposal(ask: &Permission) -> bool {
    if ask.plan.is_none() {
        return false;
    }
    rustle::column_arranged(
        Modifier::new()
            .panel(Radius::Xl)
            .padding_xy(theme::sp(4), theme::sp(3)),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::Start,
                Alignment::Center,
                || {
                    rustle::basic_text(
                        Modifier::new().weight(1.0),
                        "The agent proposed a plan",
                        theme::line(14, Role::Text),
                        TextOverflow::Ellipsis,
                    );
                    button::action(
                        "Open plan",
                        Target::Chat(ChatEvent::OpenPlan),
                        Variant::Ghost,
                    );
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
                    let revise = Target::Chat(ChatEvent::Revise);
                    button::small_button("Revise…", revise, Variant::Ghost)
                        .on_click(crate::focus::pressing_into(
                            revise,
                            Target::Composer(crate::areas::ComposerEvent::Prompt),
                        ))
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
