//! The chips above the prompt box that sum up what the agent is doing, each opening its details.

use pm_acp::Progress;
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::primitives::{Length, Sides};
use pm_style::{
    Radius, Role, absolute, animate_spin, bg, border, border_color, cursor_pointer, flex, flex_col,
    flex_none, flex_wrap, gap, h, inset_length, items_center, left, mb, p, px, py, relative,
    rounded, rounded_full, shadow_md, size, w_full, z,
};
use pm_ui::{Node, node};

use crate::Button;
use crate::Tally;
use crate::button::Variant;
use crate::parts::{clipped, glyph, line};
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
fn latest_plan(session: &Session) -> Option<&[pm_acp::PlanStep]> {
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

/// The card of the commands the agent left running, with how long each has gone on.
fn shells(session: &Session) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            gap(2),
            px(3.5),
            py(3),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(line(
            format!("{} running", session.shells.len()),
            12,
            1.4,
            Role::Muted,
        ))
        .children(session.shells.iter().map(|shell| {
            node()
                .styles([flex(), items_center(), gap(2.5)])
                .child(glyph(IconName::Loading, 3.5, Role::Muted).styles([animate_spin()]))
                .child(clipped(&shell.label, 13, Role::Text))
                .child(
                    line(
                        elapsed(shell.started.elapsed().as_secs()),
                        12,
                        1.4,
                        Role::Subtle,
                    )
                    .styles([flex_none()]),
                )
        }))
}

/// The card of the subagents of the latest reply that started any, or nothing when none was started.
fn subagent_panel(state: &WorkspaceState, session: &Session) -> Option<Node> {
    let (reply, _, _) = *session
        .subagents()
        .iter()
        .rev()
        .find(|(_, _, tool)| tool.is_working_agent())?;
    let positions: Vec<usize> = session
        .subagents()
        .into_iter()
        .filter(|(held, _, _)| *held == reply)
        .map(|(_, position, _)| position)
        .collect();
    let page = state.open_index();
    let held = state.opened.clone();
    let root = state.projects[session.project].directory();
    Some(subagents(
        &session.messages[reply].steps,
        &positions,
        reply,
        &|position| page.is_some_and(|open| held.contains(&(open, reply, position))),
        &root.to_string_lossy(),
        Some((SUBAGENTS_SCROLL, state.subagents_scroll)),
    ))
}

/// The chips for `session`, or nothing when there is nothing to sum up.
pub fn activity(state: &WorkspaceState, session: &Session) -> Option<Node> {
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
        return None;
    }
    let mut bar = node().styles([
        relative(),
        flex(),
        flex_wrap(),
        items_center(),
        gap(2),
        mb(2),
    ]);
    if let Some(open) = state.activity {
        let panel = match open {
            Activity::Plan => plan_items.map(plan),
            Activity::Approvals => session.ask.as_ref().map(question),
            Activity::Subagents => subagent_panel(state, session),
            Activity::Shells => (!session.shells.is_empty()).then(|| shells(session)),
        };
        if let Some(panel) = panel {
            bar = bar.child(
                node()
                    .styles([
                        absolute(),
                        inset_length(Sides::Bottom, Length::Fraction(1.0)),
                        left(0),
                        z(40),
                        w_full(),
                        flex(),
                        flex_col(),
                        p(2),
                        rounded(Radius::Xl),
                        border(),
                        border_color(Role::Border),
                        bg(Role::Background),
                        shadow_md(),
                        mb(2),
                    ])
                    .child(panel),
            );
        }
    }
    if awaiting > 0 {
        bar = bar.child(
            chip(format!("{awaiting} awaiting approval"), APPROVALS_CHIP)
                .mark(node().styles([size(1.5), rounded_full(), bg(Role::Danger)]))
                .selected(state.activity == Some(Activity::Approvals))
                .build(),
        );
    }
    if let Some(items) = plan_items {
        let done = items
            .iter()
            .filter(|item| item.progress == Progress::Done)
            .count();
        bar = bar.child(
            chip(format!("{done}/{} tasks", items.len()), PLAN_CHIP)
                .with_icon(IconName::CheckCircle)
                .selected(state.activity == Some(Activity::Plan))
                .build(),
        );
    }
    if working_agents > 0 {
        bar = bar.child(
            chip(format!("{working_agents} running"), SUBAGENTS_CHIP)
                .mark(glyph(IconName::Loading, 3.5, Role::Muted).styles([animate_spin()]))
                .selected(state.activity == Some(Activity::Subagents))
                .build(),
        );
    }
    if !session.shells.is_empty() {
        let count = session.shells.len();
        bar = bar.child(
            chip(
                format!(
                    "{count} {} running",
                    if count == 1 { "command" } else { "commands" }
                ),
                SHELLS_CHIP,
            )
            .mark(glyph(IconName::Loading, 3.5, Role::Muted).styles([animate_spin()]))
            .selected(state.activity == Some(Activity::Shells))
            .build(),
        );
    }
    if !session.changes.is_empty() {
        bar = bar.child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    h(7),
                    px(3),
                    rounded_full(),
                    border(),
                    border_color(Role::Border),
                    cursor_pointer(),
                    bg(Role::SurfaceHover).hover(),
                ])
                .child(
                    Tally::new(session.added(), session.removed())
                        .size(12)
                        .build(),
                )
                .focusable(OPEN_DIFF),
        );
    }
    Some(bar)
}
