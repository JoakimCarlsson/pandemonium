//! Review comments as they are drawn: the block under a line, the box one is
//! written in, and the bar that sends them all.
//!
//! Every block is a whole number of rows tall, whatever unit a pane counts
//! its rows in, so a pane that lays out rows one after another can also say
//! where any row is without asking the layout: a gesture down the numbers is
//! read against those heights.

use pm_ui::{Div, Styled, Theme, h_flex, text, v_flex};

use crate::input::text_view;
use crate::message::Message;
use crate::review::comment::{Anchor, Comment, CommentId, Composing, Side, State};
use crate::review::pane::worded;

/// How many characters of a comment's body are drawn before it wraps.
const WRAP: usize = 88;

/// How many rows the box a comment is written in is tall.
const COMPOSER_TEXT_ROWS: usize = 3;

/// How wide the bar down the left edge of a block is drawn.
const EDGE: f32 = 3.0;

/// Whether a review can be sent to the agent in the session it is of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Delivery {
    /// The session has an agent, and it is waiting.
    Ready,
    /// The session has no agent open.
    NoAgent,
    /// The agent is in a turn.
    Busy,
}

impl Delivery {
    /// Why the review cannot be sent, when it cannot.
    pub const fn reason(self) -> Option<&'static str> {
        match self {
            Self::Ready => None,
            Self::NoAgent => Some("No agent is open in this session"),
            Self::Busy => Some("Agent is working"),
        }
    }
}

/// How many rows the box a comment is written in takes up, including its
/// line range and buttons.
pub const fn composer_rows() -> usize {
    COMPOSER_TEXT_ROWS + 2
}

/// The lines a comment's body is drawn as, wrapped where it is long.
fn body_lines(body: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for line in body.lines() {
        let mut current = String::new();
        for word in line.split_inclusive(' ') {
            if !current.is_empty() && current.chars().count() + word.chars().count() > WRAP {
                lines.push(std::mem::take(&mut current));
            }
            current.push_str(word);
        }
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// How many rows the block drawn for `comment` is tall.
///
/// A comment that was sent is collapsed to its heading; one whose lines are
/// gone also carries the lines it was written on.
pub fn block_rows(comment: &Comment) -> usize {
    match comment.state {
        State::Sent { .. } => 1,
        State::Outdated => 1 + comment.quote.lines.len() + body_lines(&comment.body).len(),
        State::Pending => 1 + body_lines(&comment.body).len(),
    }
}

/// Where a comment is, in words.
fn placed(anchor: &Anchor) -> String {
    let lines = match anchor.first == anchor.last {
        true => format!("line {}", anchor.first),
        false => format!("lines {}-{}", anchor.first, anchor.last),
    };
    match anchor.side {
        Side::New => lines,
        Side::Old => format!("{lines} (removed)"),
    }
}

/// One row of a block, `unit` tall.
fn line(theme: &Theme, unit: f32, said: String, mono: bool, dim: bool) -> Div<Message> {
    let color = match dim {
        true => theme.colors.text_subtle,
        false => theme.colors.text,
    };
    let said = text(said).text_xs().color(color);
    h_flex()
        .w_full()
        .h_px(unit)
        .items_center()
        .overflow_hidden()
        .child(match mono {
            true => said.font_mono(),
            false => said,
        })
}

/// Builds the block drawn for `comment`, `inset` from the left edge.
///
/// `unit` is the height of one row of the pane it is drawn in, and `moving`
/// the comment waiting for a line to be put on, whose button then says so.
pub fn comment_block(
    theme: &Theme,
    comment: &Comment,
    unit: f32,
    inset: f32,
    moving: Option<CommentId>,
) -> Div<Message> {
    let id = comment.id;
    let sent = matches!(comment.state, State::Sent { .. });
    let outdated = comment.state == State::Outdated;
    let (label, color) = match comment.state {
        State::Pending => ("Comment", theme.colors.accent),
        State::Sent { .. } => ("Sent", theme.colors.text_subtle),
        State::Outdated => ("Outdated", theme.colors.warning),
    };
    let first = body_lines(&comment.body)
        .into_iter()
        .next()
        .unwrap_or_default();

    let heading = h_flex()
        .w_full()
        .h_px(unit)
        .gap(1)
        .items_center()
        .overflow_hidden()
        .child(text(label).text_xs().font_semibold().color(color))
        .child(
            text(placed(&comment.anchor))
                .text_xs()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
        .when(sent, |row| {
            row.child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(text(first).text_xs().color(theme.colors.text_subtle)),
            )
        })
        .when(!sent, |row| row.child(h_flex().flex_1()))
        .when(outdated, |row| {
            row.child(match moving == Some(id) {
                true => worded(theme, "Cancel move", true, Message::MoveComment(id)),
                false => worded(theme, "Move…", true, Message::MoveComment(id)),
            })
        })
        .when(!outdated, |row| {
            row.child(worded(theme, "Edit", true, Message::EditComment(id)))
        })
        .child(worded(theme, "Delete", true, Message::DeleteComment(id)));

    let body = v_flex()
        .w_full()
        .flex_1()
        .h_full()
        .bg(theme.colors.surface)
        .border_side(pm_ui::Side::Left, EDGE, color)
        .px(1.5)
        .overflow_hidden()
        .child(heading)
        .when(outdated, |block| {
            block.children(
                comment
                    .quote
                    .lines
                    .iter()
                    .map(|quoted| line(theme, unit, quoted.clone(), true, true)),
            )
        })
        .when(!sent, |block| {
            block.children(
                body_lines(&comment.body)
                    .into_iter()
                    .map(|said| line(theme, unit, said, false, false)),
            )
        });

    h_flex()
        .w_full()
        .h_px(block_rows(comment) as f32 * unit)
        .overflow_hidden()
        .child(v_flex().w_px(inset).h_full())
        .child(body)
}

/// Builds the box a comment is written in, `inset` from the left edge, with
/// the buttons that keep it or throw it away.
pub fn composer_block(
    theme: &Theme,
    composing: &Composing,
    unit: f32,
    inset: f32,
    focused: bool,
    solid: bool,
) -> Div<Message> {
    let editing = composing.editing.is_some();
    let range = format!("Commenting on {}", placed(&composing.anchor));
    let lines = COMPOSER_TEXT_ROWS as f32 * unit / theme.size.control;
    let text_box = text_view(
        theme,
        composing.text.clone(),
        focused,
        solid,
        lines,
        Message::WriteComment,
        Message::ShowInputMenu,
    );

    let footer = h_flex()
        .w_full()
        .h_px(unit)
        .gap(1)
        .items_center()
        .overflow_hidden()
        .child(
            text("Enter saves, Shift+Enter breaks the line")
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
        .child(h_flex().flex_1())
        .child(worded(theme, "Cancel", true, Message::CancelComment))
        .child(worded(
            theme,
            match editing {
                true => "Save",
                false => "Comment",
            },
            true,
            Message::SaveComment,
        ));

    h_flex()
        .w_full()
        .h_px(composer_rows() as f32 * unit)
        .overflow_hidden()
        .child(v_flex().w_px(inset).h_full())
        .child(
            v_flex()
                .flex_1()
                .h_full()
                .bg(theme.colors.surface)
                .border_side(pm_ui::Side::Left, EDGE, theme.colors.accent)
                .px(1.5)
                .overflow_hidden()
                .child(
                    h_flex()
                        .w_full()
                        .h_px(unit)
                        .items_center()
                        .child(text(range).text_xs().color(theme.colors.accent)),
                )
                .child(text_box)
                .child(footer),
        )
}

/// Builds the bar that offers to send the `pending` comments left on the
/// worktree, or to throw them away.
///
/// A review that cannot be sent says why beside the button, which is then
/// left dull: a button that does nothing when pressed is worse than one that
/// says it cannot.
pub fn pending_bar(theme: &Theme, pending: usize, delivery: Delivery) -> Div<Message> {
    let counted = match pending {
        1 => "Review: 1 comment".to_owned(),
        pending => format!("Review: {pending} comments"),
    };
    h_flex()
        .w_full()
        .h_px(theme.size.bar)
        .px(1.5)
        .gap(0.75)
        .items_center()
        .overflow_hidden()
        .bg(theme.colors.accent.alpha(theme.emphasis.change))
        .border_side(pm_ui::Side::Bottom, 1.0, theme.colors.border_variant)
        .child(text(counted).text_xs().font_semibold())
        .when_some(delivery.reason(), |bar, reason| {
            bar.child(
                text(reason)
                    .text_xs()
                    .font_light()
                    .color(theme.colors.text_subtle),
            )
        })
        .child(h_flex().flex_1())
        .child(worded(
            theme,
            "Send to agent",
            delivery == Delivery::Ready,
            Message::SendReview,
        ))
        .child(worded(theme, "Discard", true, Message::DiscardReview))
}
