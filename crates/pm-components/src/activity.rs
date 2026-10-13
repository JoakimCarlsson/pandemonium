//! The chips above the prompt box that sum up what the agent is doing, each opening its details.

use pm_acp::Progress;
use pm_icon::IconName;
use pm_style::primitives::{Length, Sides};
use pm_style::{
    Radius, Role, absolute, bg, border, border_color, cursor_pointer, flex, flex_col, flex_wrap,
    gap, h, inset_length, items_center, left, mb, p, px, relative, rounded, rounded_full,
    shadow_md, size, w_full, z,
};
use pm_ui::{Node, node};

use crate::Chip;
use crate::Tally;
use crate::sample::{Session, Step};
use crate::state::{APPROVALS_CHIP, Activity, OPEN_DIFF, PLAN_CHIP, WorkspaceState};
use crate::steps::{plan, question};

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

/// The chips for `session`, or nothing when there is nothing to sum up.
pub fn activity(state: &WorkspaceState, session: &Session) -> Option<Node> {
    let plan_items = latest_plan(session);
    let awaiting = usize::from(session.ask.is_some());
    if plan_items.is_none() && awaiting == 0 && session.changes.is_empty() {
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
            Chip::new(format!("{awaiting} awaiting approval"), APPROVALS_CHIP)
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
            Chip::new(format!("{done}/{} tasks", items.len()), PLAN_CHIP)
                .icon(IconName::CheckCircle)
                .selected(state.activity == Some(Activity::Plan))
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
