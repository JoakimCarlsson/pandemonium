//! The commit message, drawn as what it is: a buffer being edited.
//!
//! Zed writes a commit message in the same editor it writes code in, and so
//! does this: the same cursor, the same selection, the same undo, the same
//! keys. What is different is only what is drawn around it — a message has no
//! line numbers to give, nothing to fold and nothing to blame — so it is the
//! editor's plain view, in whatever box the screen asking for it has.

use pm_ui::Element;

use crate::editor::plain_view;
use crate::message::Message;
use crate::review::store::Review;

/// Builds the editor the commit message is written in.
///
/// Both screens that offer to commit draw this, and they draw the same
/// buffer: a message typed in the sidebar is the message the review pane is
/// about to commit, because there is one message and two places to write it.
pub fn commit_editor(review: &Review, focused: bool) -> impl Element<Message> + use<> {
    plain_view(review.message(), focused)
        .caret(focused)
        .on_select(Message::WriteCommit)
}
