//! The button under the commit message, as both screens that offer it draw it.
//!
//! What it does is the store's to decide; what it says and what a press on it
//! sends are written once here, so the sidebar and the review pane can never
//! disagree about whether it commits, syncs or publishes.

use std::f32::consts::TAU;
use std::time::Instant;

use pm_ui::{Div, IconName, IconSize, Styled, Theme, h_flex, icon, text};

use crate::message::Message;
use crate::review::store::Primary;

/// What a press on the button sends, or nothing while it cannot be pressed.
pub fn primary_message(primary: &Primary) -> Option<Message> {
    match primary {
        Primary::Commit { stopped: None, .. } => Some(Message::Commit),
        Primary::Commit {
            stopped: Some(_), ..
        } => None,
        Primary::Sync { .. } => Some(Message::SyncBranch),
        Primary::Publish => Some(Message::PushBranch),
        Primary::Busy { .. } => None,
    }
}

/// Builds what the button says: its icon, its words and, while syncing, how
/// many commits go each way.
///
/// `commit` is the icon a commit is drawn with, because each screen has its
/// own; what stops a commit is written where its title would be.
pub fn primary_face(theme: &Theme, primary: &Primary, commit: IconName) -> Div<Message> {
    let color = match primary_message(primary) {
        Some(_) => theme.colors.text,
        None => theme.colors.text_subtle,
    };
    if let Primary::Busy { doing, since } = primary {
        return busy(theme, doing, *since);
    }
    let (symbol, label) = match primary {
        Primary::Commit { title, stopped } => (commit, stopped.unwrap_or(title).to_owned()),
        Primary::Sync { .. } => (IconName::Refresh, "Sync Changes".to_owned()),
        Primary::Publish => (IconName::GitPush, "Publish Branch".to_owned()),
        Primary::Busy { doing, .. } => (IconName::LoadCircle, (*doing).to_owned()),
    };
    let (ahead, behind) = match primary {
        Primary::Sync { ahead, behind } => (*ahead, *behind),
        _ => (0, 0),
    };

    h_flex()
        .gap(0.75)
        .items_center()
        .child(icon(symbol).size(IconSize::XSmall).color(color))
        .child(text(label).text_sm().font_medium().color(color))
        .children(drift(behind, IconName::ArrowDown, color))
        .children(drift(ahead, IconName::ArrowUp, color))
}

/// Builds the spinner and the words for a remote being waited on.
///
/// The spinner turns once a second from when the wait began.
fn busy(theme: &Theme, doing: &str, since: Instant) -> Div<Message> {
    let turn = since.elapsed().as_secs_f32().fract() * TAU;
    h_flex()
        .gap(0.75)
        .items_center()
        .child(
            icon(IconName::LoadCircle)
                .size(IconSize::XSmall)
                .color(theme.colors.text)
                .rotate(turn),
        )
        .child(
            text(doing.to_owned())
                .text_sm()
                .font_medium()
                .color(theme.colors.text),
        )
}

/// Builds one count of commits waiting to go the way `arrow` points, or
/// nothing when none are.
fn drift(count: usize, arrow: IconName, color: pm_gfx::Rgba) -> Option<Div<Message>> {
    (count > 0).then(|| {
        h_flex()
            .gap(0.25)
            .items_center()
            .child(text(count.to_string()).text_sm().font_medium().color(color))
            .child(icon(arrow).size(IconSize::XSmall).color(color))
    })
}
