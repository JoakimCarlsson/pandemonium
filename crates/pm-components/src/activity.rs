//! The chips above the prompt box that sum up what the agent is doing, each opening its details.

use crate::events::reduce;
use crate::theme;
use crate::theme::sp;
use pm_harness::Progress;
use rustle::{Alignment, Arrangement, FlowOptions, column_arranged, flow_row_with, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::Button;
use crate::Tally;
use crate::button::Variant;
use crate::events::Interactive;
use crate::focus::FocusId;
use crate::icons::{IconName, spinner};
use crate::parts::{clipped, line};
use crate::popups::{Placement, anchored};
use crate::sample::{Session, Step};
use crate::size::Size;
use crate::state::{
    APPROVALS_CHIP, Activity, OPEN_DIFF, PLAN_CHIP, SHELLS_CHIP, SUBAGENTS_CHIP, WorkspaceState,
};
use crate::steps::{elapsed, plan, question, subagents};
use crate::theme::{Radius, Role};

/// A pill in the strip of activity that says what there is, filled while it is the one open, answering to `id`.
fn chip(label: impl Into<String>, id: FocusId) -> Button {
    Button::new(label, id)
        .variant(Variant::Outline)
        .round()
        .size(Size::Xs)
}

/// The plan the agent keeps, the latest one of its replies, if it keeps one.
pub(crate) fn latest_plan(session: &Session) -> Option<&[pm_harness::PlanStep]> {
    session
        .messages
        .iter()
        .rev()
        .flat_map(|message| message.steps.iter().rev())
        .find_map(|step| match step {
            Step::Plan(items) => Some(items.as_slice()),
            _ => None,
        })
}

/// Emits the card of the commands the agent left running, with how long each has gone on.
fn shells(session: &Session) {
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .background_rounded(theme::color(Role::Surface), Radius::Lg.px())
            .border(1.0, theme::color(Role::Border), Radius::Lg.px())
            .padding(1.0)
            .padding_xy(theme::sp(3.5), 0.0)
            .padding_xy(0.0, theme::sp(3)),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Start,
        || {
            line(
                format!("{} running", session.shells.len()),
                12,
                1.4,
                Role::Muted,
            );
            for shell in &session.shells {
                row_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(sp(2.5)),
                    Alignment::Center,
                    || {
                        spinner(3.5, Role::Muted);
                        clipped(&shell.label, 13, Role::Text);
                        line(
                            elapsed(shell.started.elapsed().as_secs()),
                            12,
                            1.4,
                            Role::Subtle,
                        );
                    },
                );
            }
        },
    );
}

/// Emits the card of the subagents of the latest reply that started any, or nothing when none was started.
fn subagent_panel(state: &WorkspaceState, session: &Session) {
    let Some(&(reply, _, _)) = session
        .subagents()
        .iter()
        .rev()
        .find(|(_, _, tool)| tool.is_working_agent())
    else {
        return;
    };
    let positions: Vec<usize> = session
        .subagents()
        .into_iter()
        .filter(|(held, _, _)| *held == reply)
        .map(|(_, position, _)| position)
        .collect();
    let page = state.open_index();
    let held = state.opened.clone();
    let root = state.root_of(session);
    subagents(
        &session.messages[reply].steps,
        &positions,
        reply,
        &|position| page.is_some_and(|open| held.contains(&(open, reply, position))),
        &root.to_string_lossy(),
        Some((&state.subagents_scroll, state.chat_list.scroll())),
    );
}

/// Whether the panel `open` has anything to show for `session`.
fn has_panel(session: &Session, open: Activity) -> bool {
    match open {
        Activity::Plan => latest_plan(session).is_some(),
        Activity::Approvals => session.ask.is_some(),
        Activity::Subagents => session
            .subagents()
            .iter()
            .any(|(_, _, tool)| tool.is_working_agent()),
        Activity::Shells => !session.shells.is_empty(),
    }
}

/// Emits the open panel `open` for `session`, `width` logical pixels wide, hanging over the chips.
#[track_caller]
fn panel(state: &WorkspaceState, session: &Session, open: Activity, width: f32) {
    column_arranged(
        Modifier::new()
            .width(width)
            .draw_behind(theme::shadows(&theme::MEDIUM_SHADOW, Radius::Xl))
            .background_rounded(theme::color(Role::Background), Radius::Xl.px())
            .border(1.0, theme::color(Role::Border), Radius::Xl.px())
            .padding(1.0)
            .padding(theme::sp(2)),
        Arrangement::Start,
        Alignment::Start,
        || match open {
            Activity::Plan => {
                if let Some(items) = latest_plan(session) {
                    plan(items);
                }
            }
            Activity::Approvals => {
                if let Some(ask) = session.ask.as_ref() {
                    question(ask, width);
                }
            }
            Activity::Subagents => subagent_panel(state, session),
            Activity::Shells => shells(session),
        },
    );
}

/// Emits the chips for `session`, whose panels are `width` logical pixels wide, or nothing when there is nothing to sum up.
#[track_caller]
pub fn activity(state: &WorkspaceState, session: &Session, width: f32) {
    let plan_items = latest_plan(session);
    let awaiting = usize::from(session.ask.as_ref().is_some_and(|ask| ask.plan.is_none()));
    let working_agents = session
        .subagents()
        .iter()
        .filter(|(_, _, tool)| tool.is_working_agent())
        .count();
    if plan_items.is_none()
        && awaiting == 0
        && session.changes.is_empty()
        && working_agents == 0
        && session.shells.is_empty()
    {
        return;
    }
    let open = state.activity.filter(|&open| has_panel(session, open));
    anchored(
        Modifier::new().padding_each(0.0, 0.0, 0.0, theme::sp(2)),
        Placement::Above,
        2.0,
        false,
        || chips(state, session, plan_items, awaiting, working_agents),
        open.map(|open| move || panel(state, session, open, width)),
    );
}

/// Emits the strip of chips: approvals awaited, the plan, running subagents and commands, and the diff.
#[track_caller]
fn chips(
    state: &WorkspaceState,
    session: &Session,
    plan_items: Option<&[pm_harness::PlanStep]>,
    awaiting: usize,
    working_agents: usize,
) {
    flow_row_with(
        Modifier::new(),
        FlowOptions::new(sp(2), sp(2)).item_alignment(Alignment::Center),
        || {
            if awaiting > 0 {
                chip(format!("{awaiting} awaiting approval"), APPROVALS_CHIP)
                    .mark(|| {
                        rustle::spacer(
                            Modifier::new()
                                .size(theme::sp(1.5), theme::sp(1.5))
                                .background_rounded(theme::color(Role::Danger), Radius::Full.px()),
                        )
                    })
                    .selected(state.activity == Some(Activity::Approvals))
                    .build();
            }
            if let Some(items) = plan_items {
                let done = items
                    .iter()
                    .filter(|item| item.progress == Progress::Done)
                    .count();
                chip(format!("{done}/{} tasks", items.len()), PLAN_CHIP)
                    .with_icon(IconName::CheckCircle)
                    .selected(state.activity == Some(Activity::Plan))
                    .build();
            }
            if working_agents > 0 {
                chip(format!("{working_agents} running"), SUBAGENTS_CHIP)
                    .mark(|| spinner(3.5, Role::Muted))
                    .selected(state.activity == Some(Activity::Subagents))
                    .build();
            }
            if !session.shells.is_empty() {
                let count = session.shells.len();
                chip(
                    format!(
                        "{count} {} running",
                        if count == 1 { "command" } else { "commands" }
                    ),
                    SHELLS_CHIP,
                )
                .mark(|| spinner(3.5, Role::Muted))
                .selected(state.activity == Some(Activity::Shells))
                .build();
            }
            if !session.changes.is_empty() {
                row_arranged(
                    Modifier::new()
                        .height(theme::sp(7))
                        .interaction_focused(crate::focus::is_held(OPEN_DIFF))
                        .interaction_background(
                            (theme::InteractionTones::new()
                                .hover(Role::SurfaceHover)
                                .radius(Radius::Full))
                            .colors(),
                        )
                        .border(1.0, theme::color(Role::Border), Radius::Full.px())
                        .padding(1.0)
                        .cursor(CursorIcon::Pointer)
                        .control(OPEN_DIFF, reduce(OPEN_DIFF))
                        .padding_xy(theme::sp(3), 0.0),
                    Arrangement::Start,
                    Alignment::Center,
                    || {
                        Tally::new(session.added(), session.removed())
                            .size(12)
                            .build();
                    },
                );
            }
        },
    );
}
