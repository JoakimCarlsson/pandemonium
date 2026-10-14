//! The prompts above the prompt box that wait for the agent's turn to end, each of which can be handed over at once.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_col, gap, items_center, mb, pl, pr, py,
    rounded, w_full,
};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::{Size, Variant};
use crate::parts::{clipped, glyph, line};
use crate::sample::{Queued, Session};
use crate::state::HURRY_BASE;

/// One waiting prompt, at `index`: its first line, how many files go with it and a button that hands it over in place of the turn.
fn waiting(index: usize, queued: &Queued) -> Node {
    let first = queued.text.lines().next().unwrap_or_default();
    let mut row = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            w_full(),
            pl(3),
            pr(1),
            py(1),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(glyph(IconName::Progress, 3.5, Role::Muted))
        .child(clipped(first, 13, Role::Text));
    if !queued.files.is_empty() {
        let count = queued.files.len();
        let files = if count == 1 { "file" } else { "files" };
        row = row.child(line(format!("{count} {files}"), 12, 1.4, Role::Muted));
    }
    row.child(
        Button::new("Send now", FocusId(HURRY_BASE + index as u64))
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .with_icon(IconName::Zap)
            .build(),
    )
}

/// The prompts waiting for `session`'s agent, oldest first, or nothing when none waits.
pub fn queued(session: &Session) -> Option<Node> {
    if session.queued.is_empty() {
        return None;
    }
    Some(
        node().styles([flex(), flex_col(), gap(1), mb(2)]).children(
            session
                .queued
                .iter()
                .enumerate()
                .map(|(index, queued)| waiting(index, queued)),
        ),
    )
}
