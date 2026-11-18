//! The chips above the prompt box that sum up what the agent is doing, each opening its details.

use crate::kit::sp;
use pm_harness::Progress;
use rustle::{CursorIcon, Modifier};
use rustle::{Flex, flex_column, flex_row, flex_wrap};

use crate::Button;
use crate::Tally;
use crate::button::Variant;
use crate::kit::{
    FocusId, Hovered, IconName, Interactive, Looks, Placement, Radius, Role, Side, Styled,
    anchored, spinner,
};
use crate::parts::{clipped, line};
use crate::sample::{Session, Step};
use crate::size::Size;
use crate::state::{
    APPROVALS_CHIP, Activity, OPEN_DIFF, PLAN_CHIP, SHELLS_CHIP, SUBAGENTS_CHIP, SUBAGENTS_SCROLL,
    WorkspaceState,
};
use crate::steps::{elapsed, plan, question, subagents};

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
    flex_column(
        Modifier::new()
            .fill_rounded(Role::Surface, Radius::Lg)
            .stroke(Role::Border, Radius::Lg)
            .pad_x(3.5)
            .pad_y(3),
        Flex::new().gap(sp(2)),
        || {
            line(
                format!("{} running", session.shells.len()),
                12,
                1.4,
                Role::Muted,
            );
            for shell in &session.shells {
                flex_row(
                    Modifier::new(),
                    Flex::new().items_center().gap(sp(2.5)),
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
        Some((SUBAGENTS_SCROLL, state.subagents_scroll)),
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
    flex_column(
        Modifier::new()
            .width(width)
            .shadow_md(Radius::Xl)
            .fill_rounded(Role::Background, Radius::Xl)
            .stroke(Role::Border, Radius::Xl)
            .pad(2),
        Flex::new(),
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
        Modifier::new().pad_side(Side::Bottom, 2),
        Placement::Above,
        2.0,
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
    flex_wrap(
        Modifier::new(),
        Flex::new().gap(sp(2)).items_center(),
        || {
            if awaiting > 0 {
                chip(format!("{awaiting} awaiting approval"), APPROVALS_CHIP)
                    .mark(|| {
                        rustle::spacer(
                            Modifier::new()
                                .square(1.5)
                                .fill_rounded(Role::Danger, Radius::Full),
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
                flex_row(
                    Modifier::new()
                        .h(7)
                        .looks_of(
                            Looks::new().hover(Role::SurfaceHover).radius(Radius::Full),
                            OPEN_DIFF,
                        )
                        .stroke(Role::Border, Radius::Full)
                        .cursor(CursorIcon::Pointer)
                        .target(OPEN_DIFF)
                        .pad_x(3),
                    Flex::new().items_center(),
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
