//! The chips above the prompt box that sum up what the agent is doing, each opening its details.

use crate::areas::ChatEvent;
use crate::areas::DiffEvent;
use crate::kit::{self, Frame};
use crate::theme::Indicated;
use crate::theme::ShadowLayers;

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use crate::theme::sp;
use pm_harness::Progress;
use rustle::phosphor;
use rustle::{Alignment, FlowOptions, flow_row_with};
use rustle::{CursorIcon, Modifier};

use crate::Button;
use crate::Tally;
use crate::button::Variant;
use crate::events::Interactive;
use crate::icons::spinner;
use crate::parts::{clipped, line};
use crate::popups::{Placement, anchored};
use crate::sample::{Session, Step};
use crate::size::Size;
use crate::state::{Activity, WorkspaceState};
use crate::steps::{elapsed, plan, question, subagents};
use crate::theme::{Radius, Role};

/// A pill in the strip of activity that says what there is, filled while it is the one open, answering to `id`.
fn chip(label: impl Into<String>, id: Target) -> Button {
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
    kit::column(
        Modifier::new()
            .fill_max_width()
            .panel(Radius::Lg)
            .padding_xy(theme::sp(3.5), 0.0)
            .padding_xy(0.0, theme::sp(3)),
        2,
        || {
            line(
                format!("{} running", session.shells.len()),
                12,
                1.4,
                Role::Muted,
            );
            for shell in &session.shells {
                kit::row(Modifier::new(), 2.5, || {
                    spinner(3.5, Role::Muted);
                    clipped(&shell.label, 13, Role::Text);
                    line(
                        elapsed(shell.started.elapsed().as_secs()),
                        12,
                        1.4,
                        Role::Subtle,
                    );
                });
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
    kit::column(
        Modifier::new()
            .width(width)
            .shadow_layers(&theme::MEDIUM_SHADOW, Radius::Xl.px())
            .background_rounded(theme::color(Role::Background), Radius::Xl.px())
            .outlined(Radius::Xl)
            .padding(theme::sp(2)),
        0,
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
                chip(
                    format!("{awaiting} awaiting approval"),
                    Target::Chat(ChatEvent::ApprovalsChip),
                )
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
                chip(
                    format!("{done}/{} tasks", items.len()),
                    Target::Chat(ChatEvent::PlanChip),
                )
                .with_icon(phosphor::CHECK_CIRCLE)
                .selected(state.activity == Some(Activity::Plan))
                .build();
            }
            if working_agents > 0 {
                chip(
                    format!("{working_agents} running"),
                    Target::Chat(ChatEvent::SubagentsChip),
                )
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
                    Target::Chat(ChatEvent::ShellsChip),
                )
                .mark(|| spinner(3.5, Role::Muted))
                .selected(state.activity == Some(Activity::Shells))
                .build();
            }
            if !session.changes.is_empty() {
                kit::row(
                    Modifier::new()
                        .height(theme::sp(7))
                        .indicated(
                            theme::InteractionTones::new()
                                .hover(Role::SurfaceHover)
                                .radius(Radius::Full),
                            theme::Focused::Held(crate::focus::is_held(Target::Diff(
                                DiffEvent::OpenDiff,
                            ))),
                        )
                        .outlined(Radius::Full)
                        .cursor(CursorIcon::Pointer)
                        .control(
                            Target::Diff(DiffEvent::OpenDiff),
                            send(Event::Diff(DiffEvent::OpenDiff)),
                        )
                        .padding_xy(theme::sp(3), 0.0),
                    0,
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
