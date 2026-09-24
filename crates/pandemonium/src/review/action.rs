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

/// What one of a repository's own controls asks for, once that repository
/// has been made the active one.
///
/// A folder of several repositories draws the same controls once for each,
/// and a press on one of them means that repository: the window makes it
/// active first and then carries out the plain command, so every command
/// keeps exactly one implementation whichever section it was pressed in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryAction {
    /// Only make the repository the active one.
    Activate,
    /// Commit what its index holds.
    Commit,
    /// Pull and push its branch.
    SyncBranch,
    /// Push its branch, publishing it if it follows nothing.
    PushBranch,
    /// Open the menu under its commit button.
    ShowCommitMenu,
    /// Open its source control menu.
    ShowSourceControlMenu,
    /// Open the list of its branches.
    ShowBranches,
}

impl RepositoryAction {
    /// The plain command this carries out in the active repository.
    pub fn message(self) -> Option<Message> {
        match self {
            Self::Activate => None,
            Self::Commit => Some(Message::Commit),
            Self::SyncBranch => Some(Message::SyncBranch),
            Self::PushBranch => Some(Message::PushBranch),
            Self::ShowCommitMenu => Some(Message::ShowCommitMenu),
            Self::ShowSourceControlMenu => Some(Message::ShowSourceControlMenu),
            Self::ShowBranches => Some(Message::ShowStatusBranches),
        }
    }
}

/// What a press on the `repository`-th button sends, or nothing while it
/// cannot be pressed.
pub fn primary_message(repository: usize, primary: &Primary) -> Option<Message> {
    let action = match primary {
        Primary::Commit { stopped: None, .. } => RepositoryAction::Commit,
        Primary::Commit {
            stopped: Some(_), ..
        } => return None,
        Primary::Sync { .. } => RepositoryAction::SyncBranch,
        Primary::Publish => RepositoryAction::PushBranch,
        Primary::Busy { .. } => return None,
    };
    Some(Message::InRepository(repository, action))
}

/// Builds what the button says: its icon, its words and, while syncing, how
/// many commits go each way.
///
/// `commit` is the icon a commit is drawn with, because each screen has its
/// own; what stops a commit is written where its title would be.
pub fn primary_face(theme: &Theme, primary: &Primary, commit: IconName) -> Div<Message> {
    let color = match primary_message(0, primary) {
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
