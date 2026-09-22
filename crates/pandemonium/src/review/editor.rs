//! The commit message, drawn as the box it is written in.
//!
//! Zed writes a commit message in the same editor it writes code in, and so
//! does this: the same cursor, the same selection, the same undo, the same
//! keys, the same menu under the right button. It is the window's one kind of
//! text box, asked for at whatever size the screen has room for.

use pm_ui::{Div, Theme};

use crate::input::input_view;
use crate::message::Message;
use crate::review::store::Review;

/// How many lines of the message a box has room for.
const MESSAGE_LINES: f32 = 3.0;

/// Builds the box the commit message is written in.
///
/// Both screens that offer to commit draw this, and they draw the same
/// message: one typed in the sidebar is the message the review pane is about
/// to commit, because there is one message and two places to write it.
pub fn commit_editor(theme: &Theme, review: &Review, focused: bool) -> Div<Message> {
    input_view(
        theme,
        review.message(),
        focused,
        MESSAGE_LINES,
        Message::WriteCommit,
        Message::ShowInputMenu,
    )
}
