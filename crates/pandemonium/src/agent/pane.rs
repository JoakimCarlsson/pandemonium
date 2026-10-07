//! One agent session, in a pane: what was said, what it is doing, what next.
//!
//! The pane is a quiet reading surface: replies lead, while tool activity and
//! thoughts remain available in the same transcript at a lower visual weight.
//!
//! Above it is which agent, which worktree and how it is doing; below it is
//! the box the next prompt is written in. A permission the agent is waiting on
//! sits between the two, because it is the one thing that stops everything
//! else until it is answered.
//!
//! A pane is as wide as the window made it and the text is wrapped to fit, so
//! the width the last frame came out at is what this one is built against.

use std::cell::Ref;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use similar::{ChangeTag, TextDiff};

use pm_acp::{
    About, Ask, Input, Kind, Knob, Limits, Output, Setting, Status, Step, ToolCall, Usage, Voice,
};
use pm_gfx::{Image, Rect, Rgba, Size};
use pm_text::{Highlight, Language};
use pm_ui::{
    Div, Element, Font, Grain, IconName, IconSize, IntoElement, LayoutContext, PaintContext,
    PointerCursor, SCROLLBAR_GUTTER, STEP, Scroll, Selection, SelectionContent, SelectionRow, Side,
    Style, Styled, TextSize, Theme, above, h_flex, icon, icon_button, measured, paragraph, picture,
    rule, scroll_area, scrollbar, space, switch, text, v_flex,
};

use crate::agent::{Block, Form, FormRow, Pending, Spot, Standing, Talk, TalkId};
use crate::editor::{code_highlights, tint};
use crate::image::Decoding;
use crate::input::{bare_input_view, hinted_input_view};
use crate::markdown::blocks::{self, Block as MarkdownBlock, Emphasis, Run};
use crate::message::Message;

/// How many rows are built at once, however long the conversation runs.
const DRAWN: usize = 300;

/// How many lines of one tool call's result are shown before the rest.
const RESULT_LINES: usize = 3;

/// How far the conversation sits from the top and foot of its area, in
/// steps of the spacing scale.
const INSET: f32 = 2.0;

/// How far the conversation sits in from the left of its area, in steps of
/// the spacing scale.
const SIDE: f32 = 1.75;

/// How far the edge of a bubble holding what the reader said sits from its
/// text, in steps of the spacing scale.
const BUBBLE: f32 = 1.25;

/// Widest the prompt card is drawn, however wide the pane.
const COMPOSER_WIDTH: f32 = 760.0;

/// Fewest rows the prompt box is drawn at, however little it holds.
const PROMPT_LEAST: usize = 3;

/// Most rows the prompt box grows to before what it holds scrolls.
const PROMPT_ROWS: usize = 12;

/// How many of the commands a slash narrows to are offered at once.
const OFFERED: usize = 8;

/// Padding above and below each offered command, in layout steps.
const COMMAND_PADDING: f32 = 1.5;

/// Estimated average width of a conversation character as a share of its size.
const ADVANCE: f32 = 0.55;

/// Fewest characters a line is wrapped at, however narrow the pane is.
const NARROWEST: usize = 24;

/// Side of the mark before each row of a question.
const MARK: f32 = 16.0;

/// Most lines of an alternative's preview shown.
const PREVIEW_LINES: usize = 16;

/// Mark before the agent's reply.
const BULLET: &str = "";

/// What stands against what the reader said.
const CHEVRON: &str = "";

/// Indentation before a tool call's result.
const RESULT: &str = "    ";

/// What a line continuing the one above it is indented by.
const WRAPPED: &str = "  ";

/// What stands down the side of a quotation in the agent's reply.
const QUOTE: &str = "│ ";

/// What stands between two cells of a row of a table.
const DIVIDER: &str = " │ ";

/// What stands between two cells of the rule under a table's heading row.
const CROSSING: &str = "─┼─";

/// Frames of the activity mark shown during a turn.
const WORKING: [&str; 4] = ["◐", "◓", "◑", "◒"];

/// How many characters of a failed call's error its header shows.
const REASON: usize = 72;

/// How many characters of what a call is doing a heading shows.
const ACTIVITY: usize = 60;

/// How long a call inside another runs before the heading over it names it.
///
/// A subagent runs many calls of a few milliseconds each; naming every one
/// as it starts would rewrite the heading faster than it can be read.
const SETTLE: Duration = Duration::from_millis(750);

/// The colour a piece of a row is drawn in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tone {
    /// What the reader said.
    Said,
    /// What the agent said.
    Spoken,
    /// A heading in what the agent said, at its depth from one to six.
    Heading(usize),
    /// A line of a block of code, each run in what its grammar says it is.
    Code(Option<Highlight>),
    /// Syntax-coloured diff text, on an added or removed row.
    Diff(Option<Highlight>, ChangeTag),
    /// A cell of a table, set in fixed pitch so its columns line up.
    Table,
    /// What divides the cells of a table from one another.
    TableRule,
    /// What it is quieter about: results, thoughts, what it is doing now.
    Quiet,
    /// The name of a tool it called.
    Tool,
    /// What it called that tool on.
    Argument,
    /// A group of tool calls or thoughts the reader can open.
    DetailGroup(usize),
    /// One that did not.
    Failed,
    /// Something the editor has to say about the conversation itself.
    Note,
}

/// One piece of a row: a run of text and the colour it is drawn in.
struct Piece {
    /// The text itself.
    text: String,
    /// What colour it is drawn in.
    tone: Tone,
    /// A picture in place of text, when this piece is an attachment.
    image: Option<Image>,
    /// Where it leads when it is pressed, when it is part of a link.
    link: Option<String>,
    /// How it is set within its tone: heavier, slanted, as code, struck.
    emphasis: Emphasis,
    /// Whether it only indents a line carried on from the row above, which
    /// is a space between words rather than a line of its own once copied.
    wrapped: bool,
    /// A tool card whose header toggles its complete output.
    card: Option<String>,
    /// The outer tool card containing this row, for its shared background.
    owner: Option<String>,
    /// A clock-driven fragment drawn without rewrapping its part.
    live: Option<Live>,
}

/// A fragment whose clock changes independently of transcript wrapping.
#[derive(Clone, Copy)]
enum Live {
    /// The activity spinner of an unfinished call.
    Spinner(Instant),
    /// Elapsed seconds, frozen at completion when known.
    Timer(Option<Instant>, Option<Instant>),
    /// A streaming or completed thought's duration label.
    Thought(Instant, Option<Instant>),
}

impl Live {
    /// The fragment at the current spinner tick.
    fn text(self) -> String {
        match self {
            Self::Spinner(started) => WORKING
                [(started.elapsed().as_millis() / 250 % WORKING.len() as u128) as usize]
                .to_owned(),
            Self::Timer(started, finished) => format!(
                " · {}s",
                started.map_or(0, |start| finished
                    .unwrap_or_else(Instant::now)
                    .saturating_duration_since(start)
                    .as_secs())
            ),
            Self::Thought(started, finished) => {
                let seconds = finished
                    .unwrap_or_else(Instant::now)
                    .saturating_duration_since(started)
                    .as_secs();
                if finished.is_some() {
                    format!("Thought for {seconds}s")
                } else {
                    "Thinking".to_owned()
                }
            }
        }
    }
}

/// A run of a passage, and how it is set.
type Span = (String, Look);

/// How a run of a passage is set: where it leads when it is part of a link,
/// and how it is emphasised.
#[derive(Clone, Debug, Default, PartialEq)]
struct Look {
    /// Where it leads when it is pressed.
    link: Option<String>,
    /// How it is set within its tone.
    emphasis: Emphasis,
}

/// The schemes an address written out in a passage is known by.
const SCHEMES: [&str; 2] = ["https://", "http://"];

/// What trails an address in prose without being part of it.
const TRAILING: &[char] = &['.', ',', ';', ':', '!', '?', ')', ']', '\'', '"', '>'];

/// One line of the conversation, in the pieces it is coloured by.
type Row = Vec<Piece>;

/// Builds the pane showing `talk`, wrapped to `width` logical pixels.
///
/// `typing` says the prompt box has the keyboard, so that the caret is drawn
/// where the reader is actually writing; `solid` is its blink phase.
pub fn agent_pane(
    theme: &Theme,
    talk: &Talk,
    typing: bool,
    answering: Option<(u64, usize)>,
    solid: bool,
    width: f32,
) -> Div<Message> {
    talk.drawn_width().set(width);
    let columns = columns(theme, width);
    let (drawn, offset) = drawn(theme, talk, columns);
    let session = talk.id();

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(header(theme, talk))
        .when(!talk.background().is_empty(), |pane| {
            pane.child(background(theme, talk))
        })
        .child(rule(theme))
        .child(measured(
            talk.view(),
            scrollbar(
                scroll_area(
                    std::rc::Rc::new(std::cell::Cell::new(Scroll::at(offset))),
                    transcript_column(session, None)
                        .pl(SIDE)
                        .pr(SCROLLBAR_GUTTER / STEP)
                        .py(INSET)
                        .children(drawn),
                )
                .w_full()
                .flex_1(),
                talk.drawn_height().get(),
                talk.scroll(),
                move |event, step| Message::ScrollAgent(session, event, step),
            ),
        ))
        .when(!talk.logins().is_empty(), |pane| {
            pane.child(login(theme, talk))
        })
        .children(
            talk.asks()
                .iter()
                .map(|ask| permission(theme, talk, ask, columns)),
        )
        .children(
            talk.forms()
                .iter()
                .map(|form| question(theme, talk, form, answering, solid)),
        )
        .child(match talk.offered().is_empty() {
            true => composer(theme, talk, typing, solid).into_element(),
            false => {
                above(composer(theme, talk, typing, solid), commands(theme, talk)).into_element()
            }
        })
}

/// Lists work the agent left running after its latest turn.
fn background(theme: &Theme, talk: &Talk) -> Div<Message> {
    v_flex()
        .w_full()
        .px(1.5)
        .py(0.5)
        .bg(theme.colors.surface)
        .children(talk.background().values().map(|label| {
            text(format!("◐ {label} · running"))
                .text_xs()
                .color(theme.colors.text_muted)
        }))
}

/// A transcript column with selection gestures and the menu for its reply, if any.
fn transcript_column(session: TalkId, reply: Option<usize>) -> Div<Message> {
    v_flex()
        .w_full()
        .drag_cursor(PointerCursor::Text)
        .on_secondary_click(Message::ShowAgentTextMenu(session, reply))
        .on_drag(move |event| {
            Message::SelectAgentText(session, event.phase, event.start, event.current)
        })
}

/// Builds the list of commands the slash being typed narrows to.
///
/// The agent says what it takes — its slash commands and its skills — and
/// this is where that list is: a reader who types a slash is shown what this
/// agent answers to, rather than having to know.
fn commands(theme: &Theme, talk: &Talk) -> Div<Message> {
    let session = talk.id();
    let chosen = talk.chosen();
    let offered = talk.offered();
    let row = theme.text.sm.line_height + COMMAND_PADDING * 2.0 * STEP;
    let shown = offered.len().min(OFFERED) as f32 * row;
    talk.reveal_chosen(offered.len(), row, shown);
    let rows = offered
        .into_iter()
        .enumerate()
        .map(|(place, command)| {
            h_flex()
                .w_full()
                .h_px(row)
                .px(1)
                .items_center()
                .rounded(theme.radius.md)
                .hover_bg(theme.colors.surface_hover)
                .when(place == chosen, |row| row.bg(theme.colors.surface_selected))
                .when(!command.description.is_empty(), |row| {
                    row.tooltip(command.description.clone())
                })
                .on_click(Message::TakeAgentCommand(session, place))
                .child(
                    text(format!("{}{}", command.prefix, command.name))
                        .text_sm()
                        .color(theme.colors.text),
                )
        })
        .collect::<Vec<_>>();

    v_flex().w_full().items_center().px(1.25).pt(0.5).child(
        v_flex()
            .w_full()
            .max_w_px(COMPOSER_WIDTH)
            .p(0.5)
            .rounded(theme.radius.lg)
            .border_1(theme.colors.border)
            .bg(theme.colors.surface)
            .overflow_hidden()
            .child(
                h_flex().w_full().px(1).pt(0.25).pb(0.5).child(
                    text("Slash Commands")
                        .text_xs()
                        .color(theme.colors.text_subtle),
                ),
            )
            .child(measured(
                talk.command_view(),
                scroll_area(
                    talk.command_scroll(),
                    v_flex().w_full().pr(SCROLLBAR_GUTTER / STEP).children(rows),
                )
                .w_full()
                .h_px(shown)
                .with_scrollbar(move |event, step| {
                    Message::ScrollAgentCommands(session, event, step)
                }),
            )),
    )
}

/// The first line of `said`, which is as much of it as a row has room for.
fn first_line(said: &str) -> String {
    said.lines().next().unwrap_or_default().to_owned()
}

/// How tall the conversation comes to at `width` logical pixels.
///
/// The window asks this to know how far the pane can be scrolled, which only
/// the rows can say.
pub fn content_height(theme: &Theme, talk: &Talk, width: f32) -> f32 {
    wrapped(theme, talk, columns(theme, width)).height() + space(INSET) * 2.0
}

/// The rows of the pane from where it is scrolled to, and how far the first
/// of them is scrolled up past the top of the area.
///
/// Only the rows from there are built, however long the conversation runs,
/// so the view is drawn from the row it is scrolled into and shifted up by
/// the part of it already gone by. A bubble is built from its first row on
/// screen rather than from its top, so one taller than the pane scrolls
/// through like any other rows.
fn drawn(theme: &Theme, talk: &Talk, columns: usize) -> (Vec<Div<Message>>, f32) {
    let wrapped = wrapped(theme, talk, columns);
    talk.drawn_height()
        .set(wrapped.height() + space(INSET) * 2.0);
    let reached = talk.scroll() - space(INSET);
    let first = wrapped.tops[1..].partition_point(|end| *end <= reached);
    let offset = talk.scroll() - wrapped.tops[first];
    let count = covering(
        &wrapped.heights[first..],
        offset + talk.view().get().size.height,
    );
    talk.drawn_links().borrow_mut().clear();
    talk.drawn_cards().borrow_mut().clear();
    talk.drawn_text().borrow_mut().clear();
    talk.drawn_spots().borrow_mut().clear();

    let mut visible = (first..wrapped.len()).take(count).peekable();
    let mut drawn = Vec::new();
    while let Some(at) = visible.next() {
        if wrapped.said[at] {
            let mut message = vec![self::row(theme, wrapped.row(at), at, talk, None)];
            let mut last = at;
            while let Some(next) = visible.next_if(|next| wrapped.said[*next]) {
                message.push(self::row(theme, wrapped.row(next), next, talk, None));
                last = next;
            }
            let opens = at == 0 || !wrapped.said[at - 1];
            let closes = wrapped.said.get(last + 1) != Some(&true);
            let content = bubble(theme, message, opens, closes);
            if let Some(block) = wrapped.reader_message(talk, last) {
                let session = talk.id();
                let actions = h_flex()
                    .w_full()
                    .justify_end()
                    .gap(0.5)
                    .when_some(wrapped.reader_turn(talk, last), |bar, turn| {
                        bar.child(
                            icon_button(
                                theme,
                                IconName::GitCompare,
                                Message::DiffAgentTurn(session, turn),
                            )
                            .tooltip("Diff this turn"),
                        )
                        .child(
                            icon_button(
                                theme,
                                IconName::History,
                                Message::RewindAgentTurn(session, turn),
                            )
                            .tooltip("Restore files to before this turn"),
                        )
                    })
                    .when(talk.can_rewind(), |bar| {
                        bar.child(
                            icon_button(
                                theme,
                                IconName::Undo,
                                Message::RewindAgentContext(session, block),
                            )
                            .tooltip(
                                "Rewind conversation to before this message · edit and resend",
                            ),
                        )
                    });
                drawn.push(v_flex().w_full().child(HoverMessage {
                    content,
                    actions,
                    closes,
                    actions_height: message_actions_height(theme),
                }));
            } else {
                drawn.push(content);
            }
        } else if matches!(wrapped.entries[at], Entry::Working) {
            drawn.push(
                v_flex()
                    .w_full()
                    .pb(0.75)
                    .border_side(Side::Bottom, 1.0, theme.colors.border_variant)
                    .child(self::row(theme, wrapped.row(at), at, talk, None)),
            );
        } else if let Some(block) = wrapped.reply(talk, at) {
            let mut rows = vec![self::row(theme, wrapped.row(at), at, talk, Some(block))];
            let mut last = at;
            while let Some(next) = visible.next_if(|next| wrapped.reply(talk, *next) == Some(block))
            {
                rows.push(self::row(theme, wrapped.row(next), next, talk, Some(block)));
                last = next;
            }
            let session = talk.id();
            let content = transcript_column(session, Some(block)).children(rows);
            let name = if talk.reply_copied(block) {
                IconName::Check
            } else {
                IconName::Copy
            };
            let button = icon_button(theme, name, Message::CopyAgentReply(session, block, None))
                .bg(theme.colors.background)
                .tooltip("Copy · Shift: copy formatted")
                .on_secondary_click(Message::ShowAgentTextMenu(session, Some(block)));
            let closes = wrapped.reply_ends(last);
            drawn.push(v_flex().w_full().child(HoverMessage {
                content,
                actions: button,
                closes,
                actions_height: message_actions_height(theme),
            }));
        } else if let Some(owner) = wrapped
            .row(at)
            .first()
            .and_then(|piece| piece.owner.as_deref())
        {
            let mut rows = vec![self::row(theme, wrapped.row(at), at, talk, None)];
            while let Some(next) = visible.next_if(|next| {
                wrapped
                    .row(*next)
                    .first()
                    .and_then(|piece| piece.owner.as_deref())
                    == Some(owner)
            }) {
                rows.push(self::row(theme, wrapped.row(next), next, talk, None));
            }
            drawn.push(
                v_flex()
                    .w_full()
                    .rounded(theme.radius.md)
                    .bg(theme.colors.surface)
                    .overflow_hidden()
                    .children(rows),
            );
        } else {
            drawn.push(self::row(theme, wrapped.row(at), at, talk, None));
        }
    }
    (drawn, offset)
}

/// A message with actions beneath its text while hovered.
struct HoverMessage {
    /// The visible rows and their transcript gestures.
    content: Div<Message>,
    /// The message actions, including any confirmation icons.
    actions: Div<Message>,
    /// Whether the message's last row is among the rows built.
    closes: bool,
    /// The reserved height beneath the text for message actions.
    actions_height: f32,
}

impl Element<Message> for HoverMessage {
    /// Uses the message's own layout, including its reserved action row.
    fn layout_style(&self) -> Style {
        self.content.layout_style()
    }

    /// Measures the visible text and reserves space for actions at the message's end.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let mut size = self.content.measure(available, cx);
        if self.closes {
            size.height += self.actions_height;
        }
        size
    }

    /// Paints the message and its actions when the pointer is over it.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, Message>) {
        let actions_height = if self.closes {
            self.actions_height
        } else {
            0.0
        };
        self.content.paint(
            Rect::from_xywh(
                bounds.left(),
                bounds.top(),
                bounds.size.width,
                bounds.size.height - actions_height,
            ),
            cx,
        );
        if self.closes && cx.input().is_over(bounds) {
            let size = self.actions.measure(bounds.size, &mut cx.layout);
            self.actions.paint(
                Rect::from_xywh(
                    bounds.left(),
                    bounds.bottom() - size.height,
                    size.width,
                    size.height,
                ),
                cx,
            );
        }
    }
}

/// The height reserved beneath a message for its hover actions.
fn message_actions_height(theme: &Theme) -> f32 {
    theme.size.icon_control + space(0.5)
}

/// The part of a bubble holding `rows`, set against the right of the pane.
///
/// `opens` and `closes` say whether the bubble's own top and foot are among
/// the rows built; only there does it get its edge and its rounded corners,
/// so a slice of a tall bubble runs flat off the pane where it goes on.
fn bubble(theme: &Theme, rows: Vec<Div<Message>>, opens: bool, closes: bool) -> Div<Message> {
    let top = if opens { theme.radius.xl } else { 0.0 };
    let foot = if closes { theme.radius.xl } else { 0.0 };
    h_flex().w_full().justify_end().child(
        v_flex()
            .px(BUBBLE)
            .when(opens, |bubble| bubble.pt(BUBBLE))
            .when(closes, |bubble| bubble.pb(BUBBLE))
            .rounded_corners([top, top, foot, foot])
            .bg(theme.colors.surface_hover)
            .children(rows),
    )
}

/// How many of the rows `heights` measures are built to fill `reach` logical
/// pixels, and never fewer than [`DRAWN`].
///
/// A row can be far taller than a line, a picture most of all; counting by
/// height keeps the rows built reaching the foot of the pane whatever they
/// hold.
fn covering(heights: &[f32], reach: f32) -> usize {
    let mut filled = 0.0;
    let needed = heights
        .iter()
        .take_while(|height| {
            let short = filled < reach;
            filled += *height;
            short
        })
        .count();
    needed.max(DRAWN)
}

/// Whether `row` is part of something the reader said, drawn in a bubble.
fn is_said(row: &Row) -> bool {
    row.iter().any(|piece| piece.tone == Tone::Said)
}

/// How tall one row is drawn: an empty one a line of code, and one with
/// text in it as tall as its tallest piece.
fn row_height(theme: &Theme, row: &Row) -> f32 {
    if row.is_empty() {
        return theme.text.code.line_height;
    }
    row.iter()
        .map(|piece| {
            if piece.image.is_some() {
                112.0
            } else {
                match piece.tone {
                    Tone::Heading(1) => theme.text.xl.line_height,
                    Tone::Said | Tone::Spoken | Tone::Heading(_) => theme.text.lg.line_height,
                    _ => theme.text.sm.line_height,
                }
            }
        })
        .fold(0.0, f32::max)
}

/// The conversation as `talk`'s pane last wrapped it, brought up to date
/// for `columns` characters and `theme`'s measures.
fn wrapped<'a>(theme: &Theme, talk: &'a Talk, columns: usize) -> Ref<'a, Wrapped> {
    talk.wrapped().borrow_mut().refresh(theme, talk, columns);
    talk.wrapped().borrow()
}

/// The conversation as the pane last wrapped it, kept between frames so
/// that only the parts of it that have changed since are parsed and wrapped
/// again: while an agent streams, that is the last passage alone.
#[derive(Default)]
pub struct Wrapped {
    /// What it was last wrapped against, while every part of it is settled;
    /// as long as that holds, it stands as it is.
    wrapping: Option<Wrapping>,
    /// The rows of each part of the conversation, oldest first.
    parts: Vec<Part>,
    /// Every row, in order, by where it is kept.
    entries: Vec<Entry>,
    /// Rows with clock fragments, refreshed independently of their wraps.
    clocks: Vec<usize>,
    /// Whether each row is part of something the reader said.
    said: Vec<bool>,
    /// How tall each row is drawn, the edges of a bubble counted into the
    /// first row and the last one it holds.
    heights: Vec<f32>,
    /// Where each row begins below the first, and after the last of them
    /// how tall they all come to.
    tops: Vec<f32>,
    /// The measures of the theme the heights were taken in.
    measures: Option<Measures>,
    /// The elapsed-time heading above the current response.
    working: Row,
    /// The row between two parts, which holds nothing.
    gap: Row,
    /// The logical content position at each painted row’s start.
    selection_starts: Vec<Spot>,
    /// Unwrapped text rows used by the shared selection model.
    selection_rows: SelectionContent,
}

/// What the conversation was wrapped against as a whole.
#[derive(Clone, Copy, PartialEq)]
struct Wrapping {
    /// The transcript's revision.
    revision: u64,
    /// The talk's count of changes the transcript does not hold.
    shown: u64,
    /// How many characters a line was wrapped at.
    columns: usize,
    /// Whether a turn was running, which adds a heading above its response.
    busy: bool,
}

/// The measures of a theme the height of a row is taken in: the lines of a
/// top heading, of the conversation's type, of its smaller type and of code,
/// the edge of a bubble, the room around details and the message action control.
type Measures = [f32; 7];

/// One part of the conversation: one block, or a run of tool calls drawn
/// under one heading, and the rows it comes to.
struct Part {
    /// What the rows were wrapped from.
    key: PartKey,
    /// Whether an empty row sets it apart from the part before it.
    leads: bool,
    /// Its rows.
    rows: Vec<Row>,
    /// Whether its rows are final: a restored picture still being decoded
    /// leaves them to be wrapped again once it is.
    settled: bool,
}

/// What one part's rows were wrapped from.
#[derive(Clone, Copy, PartialEq)]
struct PartKey {
    /// The first block of it.
    start: usize,
    /// The block after its last.
    end: usize,
    /// The latest revision any of its blocks was changed at.
    stamp: u64,
    /// Whether its details are open.
    expanded: bool,
    /// The talk's count of changes the transcript does not hold, where the
    /// part shows a terminal whose latest lines are counted there.
    shown: u64,
    /// How many characters a line was wrapped at.
    columns: usize,
}

/// Where one row of the conversation is kept.
#[derive(Clone, Copy)]
enum Entry {
    /// The empty row between two parts.
    Gap,
    /// A row of a part, by the part's place and the row's place in it.
    Part(usize, usize),
    /// The elapsed-time heading above the current response.
    Working,
}

impl Wrapped {
    /// How many rows the conversation comes to.
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// The row at `at`.
    fn row(&self, at: usize) -> &Row {
        match self.entries[at] {
            Entry::Gap => &self.gap,
            Entry::Part(part, row) => &self.parts[part].rows[row],
            Entry::Working => &self.working,
        }
    }

    /// The agent reply containing `at`, when this row belongs to one.
    fn reply(&self, talk: &Talk, at: usize) -> Option<usize> {
        let Entry::Part(part, _) = self.entries[at] else {
            return None;
        };
        let block = self.parts[part].key.start;
        matches!(
            talk.transcript().blocks()[block],
            Block::Said(Voice::Agent, _)
        )
        .then_some(block)
    }

    /// The reader message containing this row, including messages from loaded history.
    fn reader_message(&self, talk: &Talk, at: usize) -> Option<usize> {
        let Entry::Part(part, _) = self.entries[at] else {
            return None;
        };
        let block = self.parts[part].key.start;
        let blocks = talk.transcript().blocks();
        let block = if matches!(blocks[block], Block::Picture(_)) {
            blocks[..block]
                .iter()
                .rposition(|block| !matches!(block, Block::Picture(_)))?
        } else {
            block
        };
        matches!(blocks[block], Block::Said(Voice::Reader, _)).then_some(block)
    }

    /// The filesystem turn attached to the reader message containing `at`.
    fn reader_turn(&self, talk: &Talk, at: usize) -> Option<u64> {
        talk.checkpoint_turns
            .get(&self.reader_message(talk, at)?)
            .copied()
    }

    /// Whether `at` is the last row of its transcript block.
    fn reply_ends(&self, at: usize) -> bool {
        matches!(self.entries[at], Entry::Part(part, row) if row + 1 == self.parts[part].rows.len())
    }

    /// How tall the rows come to together.
    fn height(&self) -> f32 {
        self.tops.last().copied().unwrap_or_default()
    }

    /// Brings the rows up to date with `talk` at `columns` characters, and
    /// their heights with `theme`.
    fn refresh(&mut self, theme: &Theme, talk: &Talk, columns: usize) {
        let wrapping = Wrapping {
            revision: talk.transcript().revision(),
            shown: talk.shown_revision(),
            columns,
            busy: talk.is_busy(),
        };
        if wrapping.busy {
            self.working = vec![piece(working(talk), Tone::Quiet)];
        }
        if self.wrapping != Some(wrapping) {
            self.rewrap(talk, columns);
            self.wrapping = self
                .parts
                .iter()
                .all(|part| part.settled)
                .then_some(wrapping);
            self.refresh_selection_rows();
            self.measures = None;
        }
        self.refresh_clocks();
        if wrapping.busy
            && let Some(at) = self
                .entries
                .iter()
                .position(|entry| matches!(entry, Entry::Working))
        {
            self.selection_rows.replace(
                self.selection_starts[at].row,
                SelectionRow {
                    text: working(talk),
                    lead: 0,
                    separator: "\n",
                },
            );
        }
        let measures = measures(theme);
        if self.measures != Some(measures) {
            self.measure(theme, talk);
            self.measures = Some(measures);
        }
    }

    /// Refreshes only clock fragments and their selectable text, keeping wraps.
    fn refresh_clocks(&mut self) {
        for &at in &self.clocks {
            let Entry::Part(part, row) = self.entries[at] else {
                continue;
            };
            let row = &mut self.parts[part].rows[row];
            let mut changed = false;
            for piece in row.iter_mut() {
                if let Some(live) = piece.live {
                    let text = live.text();
                    if piece.text != text {
                        piece.text = text;
                        changed = true;
                    }
                }
            }
            if changed {
                self.selection_rows.replace(
                    self.selection_starts[at].row,
                    SelectionRow {
                        text: row.iter().map(|piece| piece.text.as_str()).collect(),
                        lead: 0,
                        separator: "\n",
                    },
                );
            }
        }
    }

    /// Wraps again every part of `talk`'s conversation that has changed
    /// since it was last wrapped, keeping the rows of every other.
    fn rewrap(&mut self, talk: &Talk, columns: usize) {
        let blocks = talk.transcript().blocks();
        let mut kept = std::mem::take(&mut self.parts).into_iter().peekable();
        let mut at = 0;
        while at < blocks.len() {
            let key = part_key(talk, at, columns);
            while kept.next_if(|part| part.key.start < at).is_some() {}
            let part = kept
                .next_if(|part| part.key == key && part.settled)
                .unwrap_or_else(|| wrap_part(talk, key));
            self.parts.push(part);
            at = key.end;
        }
        let turn = blocks
            .iter()
            .rposition(|block| matches!(block, Block::Said(Voice::Reader, _)))
            .map_or(0, |prompt| prompt + 1);
        let response = turn
            + blocks[turn..]
                .iter()
                .take_while(|block| matches!(block, Block::Picture(_)))
                .count();
        self.entries.clear();
        for (place, part) in self.parts.iter().enumerate() {
            if talk.is_busy() && part.key.start == response {
                if !self.entries.is_empty() {
                    self.entries.push(Entry::Gap);
                }
                self.entries.push(Entry::Working);
            }
            if !self.entries.is_empty() && part.leads {
                self.entries.push(Entry::Gap);
            }
            self.entries
                .extend((0..part.rows.len()).map(|row| Entry::Part(place, row)));
        }
        if talk.is_busy() && response == blocks.len() {
            if !self.entries.is_empty() {
                self.entries.push(Entry::Gap);
            }
            self.entries.push(Entry::Working);
        }
        self.clocks = (0..self.entries.len())
            .filter(|at| self.row(*at).iter().any(|piece| piece.live.is_some()))
            .collect();
    }

    /// Rejoins visual continuations and records stable logical content positions.
    fn refresh_selection_rows(&mut self) {
        self.selection_starts.clear();
        self.selection_rows.clear();
        for at in 0..self.len() {
            let row = self.row(at);
            let carried = row.first().is_some_and(|piece| piece.wrapped);
            let lead = if carried {
                row[0].text.chars().count()
            } else {
                0
            };
            let content = row
                .iter()
                .filter(|piece| piece.image.is_none())
                .map(|piece| piece.text.as_str())
                .collect::<String>();
            let start = self.selection_rows.push(SelectionRow {
                text: content,
                lead,
                separator: if carried { " " } else { "\n" },
            });
            self.selection_starts.push(start);
        }
    }

    /// Takes the height of every row, and where each begins, in `theme`.
    fn measure(&mut self, theme: &Theme, talk: &Talk) {
        let edge = space(BUBBLE);
        self.said = (0..self.len()).map(|at| is_said(self.row(at))).collect();
        self.heights = (0..self.len())
            .map(|at| {
                let row = self.row(at);
                let mut height = row_height(theme, row);
                if matches!(self.entries[at], Entry::Working) {
                    height += space(0.75) + 1.0;
                }
                if self.reply(talk, at).is_some() && self.reply_ends(at) {
                    height += message_actions_height(theme);
                }
                if self.said[at] {
                    if at == 0 || !self.said[at - 1] {
                        height += edge;
                    }
                    if self.said.get(at + 1) != Some(&true) {
                        height += edge;
                        if self.reader_message(talk, at).is_some() {
                            height += message_actions_height(theme);
                        }
                    }
                }
                if row
                    .first()
                    .is_some_and(|piece| matches!(piece.tone, Tone::DetailGroup(_)))
                {
                    height += space(0.75);
                }
                height
            })
            .collect();
        let mut top = 0.0;
        self.tops = std::iter::once(0.0)
            .chain(self.heights.iter().map(|height| {
                top += height;
                top
            }))
            .collect();
    }
}

/// The measures of `theme` a row's height is taken in.
fn measures(theme: &Theme) -> Measures {
    [
        theme.text.xl.line_height,
        theme.text.lg.line_height,
        theme.text.sm.line_height,
        theme.text.code.line_height,
        space(BUBBLE),
        space(0.75),
        theme.size.icon_control,
    ]
}

/// What the part of `talk`'s conversation starting at block `at` is wrapped
/// from at `columns` characters.
///
/// Tool calls side by side are one part, under one heading.
fn part_key(talk: &Talk, at: usize, columns: usize) -> PartKey {
    let transcript = talk.transcript();
    let blocks = transcript.blocks();
    let end = match &blocks[at] {
        Block::Ran(_) => {
            at + blocks[at..]
                .iter()
                .take_while(|block| matches!(block, Block::Ran(_)))
                .count()
        }
        _ => at + 1,
    };
    let expanded = talk.details_expanded(at);
    let terminal = blocks[at..end].iter().any(|block| {
        matches!(block, Block::Ran(call) if call.output.iter().any(|output| matches!(output, Output::Terminal(_))))
    });
    PartKey {
        start: at,
        end,
        stamp: transcript.stamps()[at..end]
            .iter()
            .copied()
            .max()
            .unwrap_or_default(),
        expanded,
        shown: if matches!(blocks[at], Block::Said(Voice::Reader, _))
            || expanded
            || terminal
            || blocks[at..end]
                .iter()
                .any(|block| matches!(block, Block::Ran(call) if call.subagent))
        {
            talk.shown_revision()
        } else {
            0
        },
        columns,
    }
}

/// The part of `talk`'s conversation `key` names, wrapped.
fn wrap_part(talk: &Talk, key: PartKey) -> Part {
    let blocks = talk.transcript().blocks();
    let PartKey {
        start: at,
        end,
        expanded,
        columns,
        ..
    } = key;
    let leads = !(matches!(&blocks[at], Block::Picture(_))
        && at > 0
        && matches!(
            &blocks[at - 1],
            Block::Said(Voice::Reader, _) | Block::Picture(_)
        ));
    let mut settled = true;
    let rows = match &blocks[at] {
        Block::Said(Voice::Reader, passage) => {
            let (rows, decoded) = reader_rows(talk, at, passage, (columns * 2 / 3).max(NARROWEST));
            settled = decoded;
            rows
        }
        Block::Picture(image) => vec![vec![image_piece(image.clone())]],
        Block::Said(Voice::Agent, passage) => markdown_rows(passage, BULLET, Tone::Spoken, columns),
        Block::Said(Voice::Thought, passage) => {
            let mut header = piece(
                format!("{} ", if expanded { "⌄" } else { "›" }),
                Tone::DetailGroup(at),
            );
            if let Some((started, finished)) = talk.transcript().thought(at) {
                header.live = None;
                let mut label = piece(String::new(), Tone::Quiet);
                let live = Live::Thought(started, finished);
                label.text = live.text();
                label.live = finished.is_none().then_some(live);
                let mut rows = vec![vec![header, label]];
                if expanded {
                    rows.extend(passage_rows(passage, BULLET, Tone::Quiet, columns));
                }
                rows
            } else {
                vec![vec![header, piece("Thinking".to_owned(), Tone::Quiet)]]
            }
        }
        Block::Ran(_) => {
            let mut rows = vec![tool_group_row(talk, &blocks[at..end], at, expanded)];
            if expanded {
                let calls = blocks[at..end]
                    .iter()
                    .filter_map(|block| match block {
                        Block::Ran(call) => Some(call.as_ref()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                rows.extend(grouped_tool_rows(talk, &calls, columns));
            } else {
                for block in &blocks[at..end] {
                    if let Block::Ran(call) = block
                        && call.subagent
                    {
                        rows.extend(tool_rows(talk, call, columns));
                    }
                }
            }
            rows
        }
        Block::Planned(steps) => steps.iter().map(step_row).collect(),
        Block::Note(note) => passage_rows(note, BULLET, Tone::Note, columns),
        Block::Failure(message, compact) => {
            let mut rows = passage_rows(message, BULLET, Tone::Note, columns);
            if *compact {
                rows.extend(markdown_rows(
                    "[Run /compact](pandemonium:agent/compact)",
                    BULLET,
                    Tone::Note,
                    columns,
                ));
            }
            rows
        }
    };
    Part {
        key,
        leads,
        rows,
        settled,
    }
}

/// Renders a message's Markdown blocks as transcript rows.
fn markdown_rows(source: &str, mark: &str, tone: Tone, columns: usize) -> Vec<Row> {
    let hang = " ".repeat(mark.chars().count());
    let mut rows = Vec::new();
    for block in blocks::blocks(source) {
        if !rows.is_empty() {
            rows.push(Row::new());
        }
        markdown_block_rows(&block, mark, &hang, tone, columns, &mut rows);
    }
    rows
}

/// Appends one Markdown block, including nested list and quote blocks.
///
/// Its first row is led by `mark` and every row after it by `hang`, so a
/// block inside a list item or a quotation stands in from the text around
/// it by as much as the item's marker or the quotation's bar.
fn markdown_block_rows(
    block: &MarkdownBlock,
    mark: &str,
    hang: &str,
    tone: Tone,
    columns: usize,
    rows: &mut Vec<Row>,
) {
    match block {
        MarkdownBlock::Heading(depth, runs) => {
            let columns = match depth {
                1 => columns * 4 / 5,
                _ => columns,
            };
            rows.extend(hung_rows(
                spans(runs),
                mark,
                hang,
                Tone::Heading(*depth),
                columns,
            ));
        }
        MarkdownBlock::Paragraph(runs) => {
            rows.extend(hung_rows(spans(runs), mark, hang, tone, columns));
        }
        MarkdownBlock::Code(language, code) => {
            code_rows(language.as_deref(), code, (mark, hang), columns, rows);
        }
        MarkdownBlock::Diagram(source) => {
            code_rows(Some("mermaid"), source, (mark, hang), columns, rows);
        }
        MarkdownBlock::Quote(blocks) => {
            let (mark, hang) = (format!("{mark}{QUOTE}"), format!("{hang}{QUOTE}"));
            for (position, block) in blocks.iter().enumerate() {
                let lead = if position == 0 { &mark } else { &hang };
                markdown_block_rows(block, lead, &hang, Tone::Quiet, columns, rows);
            }
        }
        MarkdownBlock::List(first, items) => {
            for (index, item) in items.iter().enumerate() {
                let marker = match (item.task, first) {
                    (Some(true), _) => "☑ ".to_owned(),
                    (Some(false), _) => "☐ ".to_owned(),
                    (None, Some(first)) => format!("{}. ", first + index as u64),
                    (None, None) => "• ".to_owned(),
                };
                let lead = format!("{}{marker}", if index == 0 { mark } else { hang });
                let nested = format!("{hang}{}", " ".repeat(marker.chars().count()));
                for (position, block) in item.blocks.iter().enumerate() {
                    let lead = if position == 0 { &lead } else { &nested };
                    markdown_block_rows(block, lead, &nested, tone, columns, rows);
                }
            }
        }
        MarkdownBlock::Table(table) => table_rows(table, (mark, hang), columns, rows),
        MarkdownBlock::Rule => {
            let rule = "─".repeat(columns.saturating_sub(hang.chars().count()).max(1));
            rows.push(vec![
                piece(mark.to_owned(), Tone::Quiet),
                piece(rule, Tone::TableRule),
            ]);
        }
        MarkdownBlock::Picture(target, description) => {
            let said = match description.is_empty() {
                true => target.clone(),
                false => description.clone(),
            };
            let look = Look {
                link: None,
                emphasis: Emphasis {
                    italic: true,
                    ..Emphasis::default()
                },
            };
            rows.extend(hung_rows(
                vec![(said, look)],
                mark,
                hang,
                Tone::Quiet,
                columns,
            ));
        }
    }
}

/// Appends a block of code: the language its fence named, then its lines
/// coloured the way that language is in an editor pane and broken where
/// they run past the width, all on a ground of their own.
///
/// `lead` is the mark before its first row and what stands before the rest.
fn code_rows(
    language: Option<&str>,
    code: &str,
    lead: (&str, &str),
    columns: usize,
    rows: &mut Vec<Row>,
) {
    let (mark, hang) = lead;
    let lines = code.trim_end_matches('\n').lines().collect::<Vec<_>>();
    let width = columns.saturating_sub(hang.chars().count() + 2).max(1);
    rows.push(vec![
        piece(mark.to_owned(), Tone::Quiet),
        piece(
            language.unwrap_or("code").to_owned(),
            Tone::Code(Some(Highlight::Comment)),
        ),
    ]);
    for line in code_highlights(language.and_then(Language::fenced), &lines) {
        for chunk in chunked(line, width) {
            let mut row = vec![
                piece(hang.to_owned(), Tone::Quiet),
                piece(String::new(), Tone::Code(None)),
            ];
            row.extend(
                chunk
                    .into_iter()
                    .map(|(text, highlight)| piece(text, Tone::Code(highlight))),
            );
            rows.push(row);
        }
    }
}

/// Appends a table, its columns lined up in fixed pitch and its heading row
/// set heavier, with a rule under it.
///
/// A table wider than the pane gives each column its fair share of the
/// width, the narrow ones first, and wraps the cells of the rest inside it.
/// `lead` is the mark before its first row and what stands before the rest.
fn table_rows(table: &[Vec<Vec<Run>>], lead: (&str, &str), columns: usize, rows: &mut Vec<Row>) {
    let (mark, hang) = lead;
    let cells = table
        .iter()
        .map(|row| {
            row.iter()
                .map(|runs| runs.iter().map(|run| run.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let count = cells.iter().map(Vec::len).max().unwrap_or_default();
    if count == 0 {
        return;
    }
    let dividers = (count - 1) * DIVIDER.chars().count();
    let room = columns.saturating_sub(hang.chars().count() + dividers);
    let widths = column_widths(&cells, room.max(count));
    for (index, row) in cells.iter().enumerate() {
        let lines = widths
            .iter()
            .enumerate()
            .map(|(column, width)| {
                let mut lines = Vec::new();
                let cell = row.get(column).map_or("", String::as_str);
                wrap(cell, *width, |line| lines.push(line.to_owned()));
                lines
            })
            .collect::<Vec<_>>();
        let height = lines.iter().map(Vec::len).max().unwrap_or(1);
        for line in 0..height {
            let lead = if index == 0 && line == 0 { mark } else { hang };
            let mut built = vec![piece(lead.to_owned(), Tone::Quiet)];
            for (column, width) in widths.iter().enumerate() {
                if column > 0 {
                    built.push(piece(DIVIDER.to_owned(), Tone::TableRule));
                }
                let said = lines[column].get(line).map_or("", String::as_str);
                let text = match column + 1 == count {
                    true => said.to_owned(),
                    false => format!("{said:<width$}"),
                };
                built.push(Piece {
                    emphasis: Emphasis {
                        strong: index == 0,
                        ..Emphasis::default()
                    },
                    ..piece(text, Tone::Table)
                });
            }
            rows.push(built);
        }
        if index == 0 && cells.len() > 1 {
            let rule = widths
                .iter()
                .map(|width| "─".repeat(*width))
                .collect::<Vec<_>>()
                .join(CROSSING);
            rows.push(vec![
                piece(hang.to_owned(), Tone::Quiet),
                piece(rule, Tone::TableRule),
            ]);
        }
    }
}

/// How many characters wide each column of `cells` is drawn, the widths and
/// the dividers between them coming to no more than `room`.
///
/// Every column is as wide as its widest cell while they all fit; when they
/// do not, the narrowest columns keep their width and the rest share what is
/// left evenly.
fn column_widths(cells: &[Vec<String>], room: usize) -> Vec<usize> {
    let count = cells.iter().map(Vec::len).max().unwrap_or_default();
    let natural = (0..count)
        .map(|column| {
            cells
                .iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or_default()
                .max(1)
        })
        .collect::<Vec<_>>();
    let mut order = (0..count).collect::<Vec<_>>();
    order.sort_by_key(|column| natural[*column]);
    let mut widths = vec![0; count];
    let mut left = room;
    for (place, column) in order.into_iter().enumerate() {
        let share = (left / (count - place)).max(1);
        widths[column] = natural[column].min(share);
        left = left.saturating_sub(widths[column]);
    }
    widths
}

/// `runs` in lines of at most `width` characters, each run keeping what it
/// was tagged with across the break.
fn chunked<T: Copy + PartialEq>(runs: Vec<(String, T)>, width: usize) -> Vec<Vec<(String, T)>> {
    let mut lines: Vec<Vec<(String, T)>> = vec![Vec::new()];
    let mut filled = 0;
    for (text, tag) in runs {
        for character in text.chars() {
            if filled == width.max(1) {
                lines.push(Vec::new());
                filled = 0;
            }
            let Some(line) = lines.last_mut() else {
                continue;
            };
            match line.last_mut() {
                Some((run, last)) if *last == tag => run.push(character),
                _ => line.push((character.to_string(), tag)),
            }
            filled += 1;
        }
    }
    lines
}

/// The runs of an inline Markdown passage as spans, each with its link and
/// its emphasis.
fn spans(runs: &[Run]) -> Vec<Span> {
    runs.iter()
        .map(|run| {
            (
                run.text.clone(),
                Look {
                    link: run.target.clone(),
                    emphasis: run.emphasis,
                },
            )
        })
        .collect()
}

/// Reader text and any images restored from an agent's saved transcript,
/// and whether every one of those images has finished decoding.
fn reader_rows(talk: &Talk, block: usize, passage: &str, columns: usize) -> (Vec<Row>, bool) {
    let mut rows = Vec::new();
    let mut rest = passage;
    let mut place = 0;
    let mut decoded = true;
    while let Some(start) = rest.find("[@image](") {
        let before = &rest[..start];
        if !before.is_empty() {
            rows.extend(passage_rows(before, CHEVRON, Tone::Said, columns));
        }
        let source = &rest[start + "[@image](".len()..];
        let Some(end) = source.find(')') else {
            rest = &rest[start..];
            break;
        };
        match talk
            .transcript()
            .restored_picture(block, place, &source[..end])
        {
            Decoding::Ready(image) => rows.push(vec![image_piece(image)]),
            decoding => {
                decoded &= !matches!(decoding, Decoding::Pending);
                rows.extend(passage_rows("[Pasted image]", CHEVRON, Tone::Said, columns));
            }
        }
        place += 1;
        rest = &source[end + 1..];
    }
    if !rest.is_empty() {
        rows.extend(passage_rows(rest, CHEVRON, Tone::Said, columns));
    }
    (rows, decoded)
}

/// The collapsed or expanded heading for adjacent tool calls.
fn tool_group_row(talk: &Talk, blocks: &[Block], at: usize, expanded: bool) -> Row {
    let mark = if expanded { "⌄" } else { "›" };
    let calls = blocks
        .iter()
        .filter_map(|block| match block {
            Block::Ran(call) => Some(call.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut all = Vec::new();
    for call in &calls {
        flatten_calls(talk, call, &mut all);
    }
    let agents = all.iter().any(|call| call.subagent && call.is_running());
    if let Some((parent, call)) = calls
        .iter()
        .rev()
        .find_map(|parent| active_call(talk, parent).map(|call| (*parent, call)))
    {
        let mut row = if expanded {
            called(talk, call)
        } else {
            vec![
                piece(String::new(), Tone::DetailGroup(at)),
                piece(String::new(), Tone::Quiet),
                piece(format!(" {}", activity(call, talk.root())), Tone::Quiet),
            ]
        };
        if call.id != parent.id {
            row[2].text = format!(
                " {} · {}",
                subject(parent, talk.root()),
                activity(call, talk.root())
            );
        }
        row[0].text = format!("{mark} ");
        row[0].tone = Tone::DetailGroup(at);
        row[0].card = None;
        if agents {
            row[2].text = format!(" {} ·{}", tally(&all), row[2].text);
        }
        return row;
    }
    let mut summary = Vec::new();
    for (kind, verb, noun) in [
        (Kind::Read, "Read", "files"),
        (Kind::Search, "searched", "patterns"),
        (Kind::Execute, "ran", "commands"),
        (Kind::Edit, "edited", "files"),
        (Kind::Delete, "deleted", "files"),
        (Kind::Move, "moved", "files"),
        (Kind::Fetch, "fetched", "URLs"),
    ] {
        let count = all
            .iter()
            .filter(|call| call.kind == kind && !call.subagent)
            .count();
        if count > 0 {
            summary.push(format!("{verb} {count} {noun}"));
        }
    }
    let agents = all.iter().filter(|call| call.subagent).count();
    if agents > 0 {
        summary.push(format!("{agents} subagents"));
    }
    if summary.is_empty() {
        summary.push(format!("{} tool calls", all.len()));
    }
    let started = all.iter().filter_map(|call| call.started).min();
    let finished = all.iter().filter_map(|call| call.finished).max();
    let mut row = vec![piece(
        format!("{mark} {}", summary.join(", ")),
        Tone::DetailGroup(at),
    )];
    for (status, state, tone) in [
        (Status::Failed, "failed", Tone::Failed),
        (Status::Cancelled, "cancelled", Tone::Quiet),
        (Status::Disconnected, "disconnected", Tone::Quiet),
    ] {
        let count = all.iter().filter(|call| call.status == status).count();
        if count > 0 {
            row.push(piece(format!(" · {count} {state}"), tone));
        }
    }
    if expanded {
        row.push(timer(started, finished));
    }
    row
}

/// Collects a card and all descendants for group counts and elapsed time.
fn flatten_calls<'a>(talk: &'a Talk, call: &'a ToolCall, calls: &mut Vec<&'a ToolCall>) {
    calls.push(call);
    for child in talk.transcript().children(&call.id) {
        flatten_calls(talk, child, calls);
    }
}

/// The deepest active descendant that has run for [`SETTLE`], or this call
/// if it is unfinished.
fn active_call<'a>(talk: &'a Talk, call: &'a ToolCall) -> Option<&'a ToolCall> {
    talk.transcript()
        .children(&call.id)
        .iter()
        .rev()
        .filter(|child| settled(child))
        .find_map(|child| active_call(talk, child))
        .or_else(|| call.is_running().then_some(call))
}

/// Whether `call` has been running long enough to be named in a heading.
fn settled(call: &ToolCall) -> bool {
    call.started
        .is_some_and(|started| started.elapsed() >= SETTLE)
}

/// One passage, as rows marked with `mark` and wrapped to the width.
fn passage_rows(passage: &str, mark: &str, tone: Tone, columns: usize) -> Vec<Row> {
    span_rows(
        vec![(passage.to_owned(), Look::default())],
        mark,
        tone,
        columns,
    )
}

/// One passage made of `spans`, as rows marked with `mark` and wrapped to
/// the width, with its links, and the addresses written out in it, pressable.
fn span_rows(spans: Vec<Span>, mark: &str, tone: Tone, columns: usize) -> Vec<Row> {
    let hang = if tone == Tone::Said { "" } else { WRAPPED };
    hung_rows(spans, mark, hang, tone, columns)
}

/// One passage made of `spans`, as rows wrapped to the width: the first led
/// by `mark` and each line carried on from it by `hang`.
fn hung_rows(spans: Vec<Span>, mark: &str, hang: &str, tone: Tone, columns: usize) -> Vec<Row> {
    let spans = spans
        .into_iter()
        .map(|(text, look)| (hide_image_data(&text), look))
        .collect::<Vec<_>>();
    let lead = mark.chars().count().max(hang.chars().count());
    linked_wrap(&spans, columns.saturating_sub(lead))
        .into_iter()
        .enumerate()
        .map(|(at, line)| {
            let lead = match at {
                0 => piece(mark.to_owned(), quieten(tone)),
                _ => Piece {
                    wrapped: true,
                    ..piece(hang.to_owned(), tone)
                },
            };
            let mut row = vec![lead];
            if line.is_empty() {
                row.push(piece(String::new(), tone));
            }
            row.extend(line.into_iter().map(|(text, look)| Piece {
                link: look.link,
                emphasis: look.emphasis,
                ..piece(text, tone)
            }));
            row
        })
        .collect()
}

/// `spans` broken into lines of at most `columns` characters, each line the
/// spans it is made of.
///
/// Lines break where [`wrap`] breaks them. A space between two words set the
/// same way is set that way too, so the whole of a link is one thing to
/// press and the whole of a stretch of code sits on one ground.
fn linked_wrap(spans: &[Span], columns: usize) -> Vec<Vec<Span>> {
    let mut lines = Vec::new();
    for paragraph in paragraphs(spans) {
        let mut line: Vec<Span> = Vec::new();
        let mut width = 0;
        for word in paragraph
            .into_iter()
            .flat_map(|word| split_linked(addressed(word), columns))
        {
            let length = word
                .iter()
                .map(|(text, _)| text.chars().count())
                .sum::<usize>();
            if width > 0 && width + 1 + length > columns {
                lines.push(std::mem::take(&mut line));
                width = 0;
            } else if width > 0 {
                let before = line.last().map(|(_, look)| look);
                let after = word.first().map(|(_, look)| look);
                let joined = before.filter(|_| before == after).cloned();
                join(&mut line, " ", &joined.unwrap_or_default());
                width += 1;
            }
            for (text, look) in &word {
                join(&mut line, text, look);
            }
            width += length;
        }
        lines.push(line);
    }
    lines
}

/// The paragraphs of `spans`, each the words it is made of, each word the
/// spans it is made of.
///
/// A word is what lies between two spaces, so two spaces side by side make
/// an empty word between them, as [`wrap`] reads them too.
fn paragraphs(spans: &[Span]) -> Vec<Vec<Vec<Span>>> {
    let mut paragraphs = vec![Vec::new()];
    let mut word = Vec::new();
    for (text, look) in spans {
        for character in text.chars() {
            match character {
                ' ' | '\n' => {
                    if let Some(paragraph) = paragraphs.last_mut() {
                        paragraph.push(std::mem::take(&mut word));
                    }
                    if character == '\n' {
                        paragraphs.push(Vec::new());
                    }
                }
                character => join(&mut word, character.encode_utf8(&mut [0; 4]), look),
            }
        }
    }
    if let Some(paragraph) = paragraphs.last_mut() {
        paragraph.push(word);
    }
    paragraphs
}

/// `word` with the address written out in it made a link to itself.
///
/// Only a word that is not already part of a link is looked in, and what
/// trails the address in prose — the full stop after it, the bracket around
/// it — is left out of what it leads to.
fn addressed(word: Vec<Span>) -> Vec<Span> {
    if word.iter().any(|(_, look)| look.link.is_some()) {
        return word;
    }
    let emphasis = word
        .first()
        .map(|(_, look)| look.emphasis)
        .unwrap_or_default();
    let text = word
        .iter()
        .map(|(text, _)| text.as_str())
        .collect::<String>();
    let Some(start) = SCHEMES.iter().filter_map(|scheme| text.find(scheme)).min() else {
        return word;
    };
    let address = text[start..].trim_end_matches(TRAILING);
    if SCHEMES.contains(&address) {
        return word;
    }
    let end = start + address.len();
    let look = |link: Option<&str>| Look {
        link: link.map(str::to_owned),
        emphasis,
    };
    [
        (text[..start].to_owned(), look(None)),
        (address.to_owned(), look(Some(address))),
        (text[end..].to_owned(), look(None)),
    ]
    .into_iter()
    .filter(|(text, _)| !text.is_empty())
    .collect()
}

/// `word` in pieces of at most `columns` characters, each keeping the looks
/// of the characters it holds.
fn split_linked(word: Vec<Span>, columns: usize) -> Vec<Vec<Span>> {
    let length = word
        .iter()
        .map(|(text, _)| text.chars().count())
        .sum::<usize>();
    if length <= columns.max(1) {
        return vec![word];
    }
    let mut pieces = vec![Vec::new()];
    let mut filled = 0;
    for (text, look) in &word {
        for character in text.chars() {
            if filled == columns.max(1) {
                pieces.push(Vec::new());
                filled = 0;
            }
            if let Some(piece) = pieces.last_mut() {
                join(piece, character.encode_utf8(&mut [0; 4]), look);
            }
            filled += 1;
        }
    }
    pieces
}

/// Adds `text` to the end of `spans`, into the last span where it is set
/// the same way.
fn join(spans: &mut Vec<Span>, text: &str, look: &Look) {
    match spans.last_mut() {
        Some((last, had)) if had == look => last.push_str(text),
        _ => spans.push((text.to_owned(), look.clone())),
    }
}

/// Replaces image payloads echoed in a saved transcript with a short label.
fn hide_image_data(passage: &str) -> String {
    let bytes = passage.as_bytes();
    let mut shown = String::with_capacity(passage.len().min(4096));
    let mut copied = 0;
    let mut at = 0;
    while at < bytes.len() {
        if !is_image_data_byte(bytes[at]) {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && is_image_data_byte(bytes[at]) {
            at += 1;
        }
        if at - start < 512 {
            continue;
        }
        let prefix = passage[copied..start]
            .rfind("data:image/")
            .filter(|prefix| start - (copied + prefix) < 80)
            .map_or(start, |prefix| copied + prefix);
        shown.push_str(&passage[copied..prefix]);
        shown.push_str("[Pasted image]");
        copied = at;
    }
    shown.push_str(&passage[copied..]);
    shown
}

/// Whether a byte can occur in a base64 image payload.
fn is_image_data_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'-' | b'_')
}

/// Consecutive reads share one heading and retain their individual paths.
fn grouped_tool_rows(talk: &Talk, calls: &[&ToolCall], columns: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut at = 0;
    while at < calls.len() {
        if at > 0 {
            rows.push(Vec::new());
        }
        let call = calls[at];
        let reads = calls[at..]
            .iter()
            .take_while(|call| {
                call.kind == Kind::Read && talk.transcript().children(&call.id).is_empty()
            })
            .count();
        if reads > 1 {
            let group = &calls[at..at + reads];
            let active = group
                .iter()
                .rev()
                .find(|call| call.is_running())
                .copied()
                .or_else(|| {
                    group
                        .iter()
                        .find(|call| call.status == Status::Failed)
                        .copied()
                })
                .unwrap_or(call);
            let mut header = called(talk, active);
            header[2].text = format!(" Read ({reads})");
            header[3] = timer(
                group.iter().filter_map(|call| call.started).min(),
                if group.iter().any(|call| call.is_running()) {
                    None
                } else {
                    group.iter().filter_map(|call| call.finished).max()
                },
            );
            let start = rows.len();
            rows.push(header);
            for call in group {
                let mut path = called(talk, call);
                path[2].text = format!(" {}", subject(call, talk.root()));
                rows.push(path);
                if talk.card_expanded(&call.id) {
                    rows.extend(result(talk, call, columns, true));
                }
            }
            for row in &mut rows[start..] {
                if let Some(first) = row.first_mut() {
                    first.owner = Some(call.id.clone());
                }
            }
            at += reads;
        } else {
            rows.extend(tool_rows(talk, call, columns));
            at += 1;
        }
    }
    rows
}

/// One live tool card, with its own renderer and nested subagent cards.
fn tool_rows(talk: &Talk, call: &ToolCall, columns: usize) -> Vec<Row> {
    let expanded = talk.card_expanded(&call.id);
    let mut rows = vec![called(talk, call)];
    rows.extend(tool_body(talk, call, columns, expanded));
    let children = talk.transcript().children(&call.id);
    if expanded && !children.is_empty() {
        let calls = children.iter().collect::<Vec<_>>();
        let mut nested = grouped_tool_rows(talk, &calls, columns.saturating_sub(4).max(1));
        for row in &mut nested {
            if let Some(first) = row.first_mut() {
                first.text.insert_str(0, "    ");
            }
        }
        rows.extend(nested);
    }
    for row in &mut rows {
        if let Some(first) = row.first_mut() {
            first.owner = Some(call.id.clone());
        }
    }
    rows
}

/// Dispatches transcript and permission content through the same renderer.
fn tool_body(talk: &Talk, call: &ToolCall, columns: usize, expanded: bool) -> Vec<Row> {
    match call.kind {
        Kind::Edit | Kind::Delete | Kind::Move => edit_rows(talk, call, columns, expanded),
        Kind::Execute => execute_rows(talk, call, columns, expanded),
        Kind::Read => read_rows(talk, call, columns, expanded),
        Kind::Search => search_rows(talk, call, columns, expanded),
        Kind::Fetch => fetch_rows(talk, call, columns, expanded),
        _ if call
            .output
            .iter()
            .any(|output| matches!(output, Output::Changed { .. })) =>
        {
            edit_rows(talk, call, columns, expanded)
        }
        _ => result(talk, call, columns, expanded),
    }
}

/// The human label for a protocol kind, falling back to the tool's name.
fn tool_label(call: &ToolCall) -> &str {
    if call.subagent {
        return "Task";
    }
    match call.kind {
        Kind::Read => "Read",
        Kind::Edit => "Edit",
        Kind::Delete => "Delete",
        Kind::Move => "Move",
        Kind::Search => "Search",
        Kind::Execute => "Run",
        Kind::Fetch => "Fetch",
        Kind::Think => "Think",
        _ => call
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or_else(|| (!call.title.is_empty()).then_some(call.title.as_str()))
            .unwrap_or("Tool"),
    }
}

/// The worktree-relative subject of a call, including a reported line.
fn subject(call: &ToolCall, root: &Path) -> String {
    call.locations
        .first()
        .map(|location| {
            let path = relative(&location.path, root);
            location
                .line
                .map_or_else(|| path.clone(), |line| format!("{path}:{line}"))
        })
        .or_else(|| call.argument.as_deref().map(first_line))
        .unwrap_or_else(|| call.title.clone())
}

/// The activity named by a call without its status and timer, on one line
/// cut to fit a heading.
fn activity(call: &ToolCall, root: &Path) -> String {
    shortened(
        &format!(
            "{} {}",
            match call.kind {
                Kind::Read => "Reading",
                Kind::Search => "Searching",
                Kind::Execute => "Running",
                Kind::Edit => "Editing",
                Kind::Delete => "Deleting",
                Kind::Move => "Moving",
                Kind::Fetch => "Fetching",
                Kind::Think => "Thinking",
                _ => tool_label(call),
            },
            first_line(&subject(call, root))
        ),
        ACTIVITY,
    )
}

/// A timed card header including a subagent's count and current activity.
fn called(talk: &Talk, call: &ToolCall) -> Row {
    let mut toggle = piece(
        format!(
            "  {} ",
            if talk.card_expanded(&call.id) {
                "⌄"
            } else {
                "›"
            }
        ),
        Tone::Quiet,
    );
    toggle.card = Some(call.id.clone());
    let mut status = piece(
        match call.status {
            Status::Done => "✓",
            Status::Failed => "✗",
            Status::Cancelled => "⊘",
            Status::Disconnected => "◌",
            Status::Pending | Status::Running => "◐",
        }
        .to_owned(),
        match call.status {
            Status::Failed => Tone::Failed,
            Status::Cancelled | Status::Disconnected => Tone::Quiet,
            _ => Tone::Tool,
        },
    );
    if call.is_running() {
        status.live = call.started.map(Live::Spinner);
    }
    let children = talk.transcript().children(&call.id);
    let mut label = format!(" {} {}", tool_label(call), subject(call, talk.root()));
    if !children.is_empty() {
        let mut descendants = Vec::new();
        for child in children {
            flatten_calls(talk, child, &mut descendants);
        }
        label.push_str(&format!(" · {} calls", descendants.len()));
        if let Some(current) = children
            .iter()
            .rev()
            .find_map(|child| active_call(talk, child))
        {
            label.push_str(&format!(" · {}", activity(current, talk.root())));
        }
    }
    let mut row = vec![
        toggle,
        status,
        piece(label, Tone::Tool),
        timer(call.started, call.finished),
    ];
    match call.status {
        Status::Failed => {
            row.extend(reason(call).map(|reason| piece(format!(" · {reason}"), Tone::Failed)))
        }
        Status::Cancelled => row.push(piece(" · cancelled".to_owned(), Tone::Quiet)),
        Status::Disconnected => row.push(piece(" · disconnected".to_owned(), Tone::Quiet)),
        Status::Pending | Status::Running | Status::Done => {}
    }
    row
}

/// The first telling line of the error a failed call reported, cut to fit a
/// header, without the fence or the tag the agent wrapped it in.
fn reason(call: &ToolCall) -> Option<String> {
    let line = call.error.as_deref()?.lines().find_map(|line| {
        let line = line.trim();
        let line = line
            .strip_prefix('<')
            .and_then(|rest| rest.split_once('>'))
            .filter(|(tag, _)| is_tag(tag))
            .map_or(line, |(_, rest)| rest);
        let line = line
            .rsplit_once("</")
            .filter(|(_, tag)| tag.strip_suffix('>').is_some_and(is_tag))
            .map_or(line, |(text, _)| text)
            .trim();
        (!line.is_empty() && !line.starts_with("```")).then_some(line)
    })?;
    Some(shortened(line, REASON))
}

/// `line` cut to `most` characters, ending in an ellipsis where it was cut.
fn shortened(line: &str, most: usize) -> String {
    match line.chars().count() > most {
        true => format!("{}…", line.chars().take(most - 1).collect::<String>()),
        false => line.to_owned(),
    }
}

/// Whether `name` reads as the name of a markup tag.
fn is_tag(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
}

/// How the subagents among `calls` stand, as "3 running · 1 failed",
/// leaving out the ones that finished well.
fn tally(calls: &[&ToolCall]) -> String {
    let agents = calls.iter().filter(|call| call.subagent);
    let count =
        |status: &dyn Fn(&ToolCall) -> bool| agents.clone().filter(|call| status(call)).count();
    [
        (count(&|call| call.is_running()), "running"),
        (count(&|call| call.status == Status::Failed), "failed"),
        (count(&|call| call.status == Status::Cancelled), "cancelled"),
        (
            count(&|call| call.status == Status::Disconnected),
            "disconnected",
        ),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, state)| format!("{count} {state}"))
    .collect::<Vec<_>>()
    .join(" · ")
}

/// An elapsed-time fragment refreshed by the existing spinner tick.
fn timer(started: Option<Instant>, finished: Option<Instant>) -> Piece {
    let live = Live::Timer(started, finished);
    let mut piece = piece(live.text(), Tone::Quiet);
    piece.live = started.filter(|_| finished.is_none()).map(|_| live);
    piece
}

/// Clips wrapped output at its preview cap and names the omitted lines.
fn clipped(mut rows: Vec<Row>, expanded: bool, cap: usize) -> Vec<Row> {
    if !expanded && rows.len() > cap {
        let hidden = rows.len() - cap;
        rows.truncate(cap);
        rows.push(vec![piece(
            format!("{RESULT}… {hidden} more lines"),
            Tone::Quiet,
        )]);
    }
    rows
}

/// The fallback text output, followed by the raw result when needed.
fn result(talk: &Talk, call: &ToolCall, columns: usize, expanded: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    for output in &call.output {
        match output {
            Output::Said(said) => rows.extend(passage_rows(
                said,
                RESULT,
                if call.status == Status::Failed && call.error.is_some() {
                    Tone::Failed
                } else {
                    Tone::Quiet
                },
                columns,
            )),
            Output::Terminal(id) => {
                if let Some(output) = talk.terminal_tail(id) {
                    rows.extend(passage_rows(output, RESULT, Tone::Quiet, columns));
                }
            }
            Output::Changed {
                path,
                before,
                after,
            } => rows.extend(diff_rows(
                path,
                talk.root(),
                before.as_deref().unwrap_or_default(),
                after,
                columns,
                expanded,
            )),
        }
    }
    if rows.is_empty()
        && let Some(returned) = &call.returned
    {
        rows.extend(passage_rows(
            returned,
            RESULT,
            if call.status == Status::Failed {
                Tone::Failed
            } else {
                Tone::Quiet
            },
            columns,
        ));
    }
    clipped(rows, expanded, RESULT_LINES)
}

/// File changes, each with a path heading and a unified diff.
fn edit_rows(talk: &Talk, call: &ToolCall, columns: usize, expanded: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    for output in &call.output {
        if let Output::Changed {
            path,
            before,
            after,
        } = output
        {
            rows.extend(diff_rows(
                path,
                talk.root(),
                before.as_deref().unwrap_or_default(),
                after,
                columns,
                expanded,
            ));
        }
    }
    if rows.is_empty() {
        result(talk, call, columns, expanded)
    } else {
        rows
    }
}

/// A unified diff with three context lines, line numbers and editor syntax.
fn diff_rows(
    path: &Path,
    root: &Path,
    before: &str,
    after: &str,
    columns: usize,
    expanded: bool,
) -> Vec<Row> {
    let diff = TextDiff::from_lines(before, after);
    let added = diff
        .iter_all_changes()
        .filter(|change| change.tag() == ChangeTag::Insert)
        .count();
    let removed = diff
        .iter_all_changes()
        .filter(|change| change.tag() == ChangeTag::Delete)
        .count();
    let mut rows = vec![vec![piece(
        format!("{RESULT}{} [+{added} −{removed}]", relative(path, root)),
        Tone::Tool,
    )]];
    let old_lines = before.lines().collect::<Vec<_>>();
    let new_lines = after.lines().collect::<Vec<_>>();
    let old = code_highlights(Language::of(path), &old_lines);
    let new = code_highlights(Language::of(path), &new_lines);
    let mut body = Vec::new();
    let mut hidden = 0;
    for (hunk, ops) in diff.grouped_ops(3).iter().enumerate() {
        let first = ops.first().unwrap();
        let last = ops.last().unwrap();
        let header = format!(
            "{RESULT}@@ -{},{} +{},{} @@",
            first.old_range().start + 1,
            last.old_range().end - first.old_range().start,
            first.new_range().start + 1,
            last.new_range().end - first.new_range().start
        );
        if expanded || hunk < 8 && body.len() < 40 {
            body.push(vec![piece(header, Tone::Quiet)]);
        } else {
            hidden += 1;
        }
        for op in ops {
            for change in diff.iter_changes(op) {
                let tag = change.tag();
                let sign = match tag {
                    ChangeTag::Insert => "+",
                    ChangeTag::Delete => "-",
                    ChangeTag::Equal => " ",
                };
                let old_number = change
                    .old_index()
                    .map_or(String::new(), |index| (index + 1).to_string());
                let new_number = change
                    .new_index()
                    .map_or(String::new(), |index| (index + 1).to_string());
                let prefix = format!("{RESULT}{old_number:>4} {new_number:>4} {sign} ");
                let highlights = match tag {
                    ChangeTag::Delete => change.old_index().and_then(|index| old.get(index)),
                    _ => change.new_index().and_then(|index| new.get(index)),
                };
                let fallback = vec![(
                    change.value().trim_end_matches(['\n', '\r']).to_owned(),
                    None,
                )];
                for chunk in chunked(
                    highlights.cloned().unwrap_or(fallback),
                    columns.saturating_sub(prefix.chars().count()).max(1),
                ) {
                    if !expanded && (hunk >= 8 || body.len() >= 40) {
                        hidden += 1;
                        continue;
                    }
                    let mut row = vec![piece(prefix.clone(), Tone::Diff(None, tag))];
                    row.extend(
                        chunk
                            .into_iter()
                            .map(|(text, highlight)| piece(text, Tone::Diff(highlight, tag))),
                    );
                    body.push(row);
                }
            }
        }
    }
    rows.extend(body);
    if hidden > 0 {
        rows.push(vec![piece(
            format!("{RESULT}… {hidden} more lines"),
            Tone::Quiet,
        )]);
    }
    rows
}

/// A command, its live terminal tail and its reported exit status or error.
fn execute_rows(talk: &Talk, call: &ToolCall, columns: usize, expanded: bool) -> Vec<Row> {
    let mut rows = passage_rows(
        &format!("$ {}", call.argument.as_deref().unwrap_or(&call.title)),
        RESULT,
        Tone::Argument,
        columns,
    );
    let mut output = Vec::new();
    for part in &call.output {
        match part {
            Output::Terminal(id) => {
                if let Some(text) = talk.terminal_tail(id) {
                    output.extend(passage_rows(text, RESULT, Tone::Quiet, columns));
                }
            }
            Output::Said(text) => output.extend(passage_rows(text, RESULT, Tone::Quiet, columns)),
            _ => {}
        }
    }
    let returned = call
        .returned
        .as_deref()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
    if output.is_empty()
        && let Some(text) = call.returned.as_deref()
    {
        let content = returned
            .as_ref()
            .and_then(|value| {
                ["output", "stdout", "result"]
                    .iter()
                    .find_map(|key| value[*key].as_str())
            })
            .unwrap_or(text);
        output.extend(passage_rows(content, RESULT, Tone::Quiet, columns));
    }
    if !expanded && output.len() > RESULT_LINES {
        let earlier = output.len() - RESULT_LINES;
        rows.push(vec![piece(
            format!("{RESULT}… {earlier} earlier lines"),
            Tone::Quiet,
        )]);
        rows.extend(output.into_iter().skip(earlier));
    } else {
        rows.extend(output);
    }
    if !call.is_running() {
        let exit = returned.as_ref().and_then(|value| {
            ["exitCode", "exit_code", "exitStatus", "exit_status"]
                .iter()
                .find_map(|key| value.get(*key).filter(|value| !value.is_null()))
        });
        let error = returned
            .as_ref()
            .and_then(|value| value.get("error"))
            .filter(|value| !value.is_null());
        let footer = if call.status == Status::Failed {
            format!(
                "Failed: {}",
                error.map_or_else(
                    || call.returned.clone().unwrap_or_else(|| call.title.clone()),
                    |error| error
                        .as_str()
                        .map_or_else(|| error.to_string(), str::to_owned)
                )
            )
        } else if let Some(exit) = exit {
            format!("Exit status: {exit}")
        } else {
            "Completed".to_owned()
        };
        rows.extend(passage_rows(
            &footer,
            RESULT,
            if call.status == Status::Failed {
                Tone::Failed
            } else {
                Tone::Quiet
            },
            columns,
        ));
    }
    rows
}

/// A read's path and, when opened, its complete content.
fn read_rows(talk: &Talk, call: &ToolCall, columns: usize, expanded: bool) -> Vec<Row> {
    let mut rows = passage_rows(&subject(call, talk.root()), RESULT, Tone::Argument, columns);
    if expanded {
        rows.extend(result(talk, call, columns, true));
    }
    rows
}

/// A search pattern followed by reported match counts and matched paths.
fn search_rows(talk: &Talk, call: &ToolCall, columns: usize, expanded: bool) -> Vec<Row> {
    let mut rows = passage_rows(
        call.argument.as_deref().unwrap_or(&call.title),
        RESULT,
        Tone::Argument,
        columns,
    );
    if let Some(value) = call
        .returned
        .as_deref()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
    {
        let matches = value.get("matches");
        let count = ["numMatches", "matchCount", "match_count", "count"]
            .iter()
            .find_map(|key| value[*key].as_u64())
            .or_else(|| {
                matches.and_then(|matches| {
                    matches
                        .as_array()
                        .map(|matches| matches.len() as u64)
                        .or_else(|| matches.as_u64())
                })
            });
        if let Some(count) = count {
            rows.push(vec![piece(format!("{RESULT}{count} matches"), Tone::Quiet)]);
        }
        let entries = value["files"]
            .as_array()
            .or_else(|| matches.and_then(serde_json::Value::as_array));
        let mut paths = std::collections::BTreeSet::new();
        for entry in entries.into_iter().flatten() {
            if let Some(path) = entry
                .as_str()
                .or_else(|| entry["path"].as_str())
                .or_else(|| entry["file"].as_str())
            {
                paths.insert(relative(Path::new(path), talk.root()));
            }
        }
        if count.is_some() || !paths.is_empty() {
            let files = paths
                .into_iter()
                .flat_map(|path| passage_rows(&path, RESULT, Tone::Argument, columns))
                .collect();
            rows.extend(clipped(files, expanded, RESULT_LINES));
            return rows;
        }
    }
    rows.extend(result(talk, call, columns, expanded));
    rows
}

/// A fetched URL followed by the first result lines or the complete result.
fn fetch_rows(talk: &Talk, call: &ToolCall, columns: usize, expanded: bool) -> Vec<Row> {
    let mut rows = passage_rows(
        call.argument.as_deref().unwrap_or(&call.title),
        RESULT,
        Tone::Argument,
        columns,
    );
    rows.extend(result(talk, call, columns, expanded));
    rows
}

/// One step of the plan, marked with how far along it is.
fn step_row(step: &Step) -> Row {
    let mark = match step.status {
        Status::Done => "  [x] ",
        Status::Running => "  [>] ",
        _ => "  [ ] ",
    };
    vec![
        piece(mark.to_owned(), Tone::Quiet),
        piece(
            step.text.clone(),
            match step.status {
                Status::Done => Tone::Quiet,
                _ => Tone::Spoken,
            },
        ),
    ]
}

/// The added or removed background shared by transcript and permission diffs.
fn diff_background(theme: &Theme, row: &Row) -> Option<Rgba> {
    row.iter().find_map(|piece| match piece.tone {
        Tone::Diff(_, ChangeTag::Insert) => Some(theme.colors.success.alpha(0.08)),
        Tone::Diff(_, ChangeTag::Delete) => Some(theme.colors.danger.alpha(0.08)),
        _ => None,
    })
}

/// Builds one row out of its pieces, the row `at` of the conversation.
///
/// The row is held to the height [`row_height`] gives it, so that where the
/// pane scrolls to and where it draws the rows are the same measurement. A
/// piece that is part of a link is drawn in the link colour and follows the
/// link when pressed; where it leads is written down in `talk` as it is
/// drawn, and the press names it by its place there. Every piece of text
/// writes down where it begins in the conversation and where its characters
/// land, so a drag over it can be read back as the text it passed over.
fn row(theme: &Theme, row: &Row, at: usize, talk: &Talk, reply: Option<usize>) -> Div<Message> {
    let session = talk.id();
    let height = row_height(theme, row);
    let mut selection = Selection::default();
    if let Some((anchor, head)) = talk.selection() {
        selection.select(anchor, head);
    }
    let start = talk.wrapped().borrow().selection_starts[at];
    if row.is_empty() {
        return h_flex().h_px(height);
    }
    let action = row.first().and_then(|piece| match piece.tone {
        Tone::DetailGroup(block) => Some(block),
        _ => None,
    });
    let detail = action.is_some();
    let code = row
        .iter()
        .any(|piece| matches!(piece.tone, Tone::Code(_) | Tone::Diff(_, _)));
    let card = row.first().and_then(|piece| piece.card.clone()).map(|id| {
        let mut cards = talk.drawn_cards().borrow_mut();
        let place = cards.len();
        cards.push(id);
        place
    });
    let mut column = 0;
    h_flex()
        .h_px(if detail { height + space(0.75) } else { height })
        .items_center()
        .when(code, |line| line.w_full().px(1).bg(theme.colors.surface))
        .when_some(diff_background(theme, row), |line, color| line.bg(color))
        .when_some(card, |line, id| {
            line.on_click(Message::ToggleAgentCard(session, id))
                .hover_bg(theme.colors.surface_hover)
        })
        .when_some(action, |line, block| {
            line.on_click(Message::ToggleAgentDetails(session, block))
                .hover_bg(theme.colors.surface_hover)
        })
        .children(row.iter().map(|piece| {
            if let Some(image) = &piece.image {
                return h_flex().child(picture(image.clone()).w_px(160.0).h_px(112.0));
            }
            let color = match (&piece.link, piece.emphasis.struck) {
                (Some(_), _) => theme.colors.link,
                (None, true) => theme.colors.text_subtle,
                (None, false) => tone(theme, piece.tone),
            };
            let start = Spot {
                row: start.row,
                column: start.column + column,
            };
            let length = piece.live.map_or_else(
                || piece.text.chars().count(),
                |live| live.text().chars().count(),
            );
            column += length;
            let spots = talk.drawn_spots();
            let key = spots.borrow().len();
            spots.borrow_mut().push(start);
            let styled = text(piece.live.map_or_else(|| piece.text.clone(), Live::text))
                .color(color)
                .placed(talk.drawn_text(), key);
            let styled = match selection.picked(start, length) {
                Some(characters) => styled.selected(characters),
                None => styled,
            };
            let styled = match piece.tone {
                Tone::Said | Tone::Spoken => styled.text_lg(),
                Tone::Heading(1) => styled.text_xl().font_semibold(),
                Tone::Heading(_) => styled.text_lg().font_semibold(),
                Tone::Argument
                | Tone::Code(_)
                | Tone::Diff(_, _)
                | Tone::Table
                | Tone::TableRule => styled.text_sm().font_mono(),
                _ => styled.text_sm(),
            };
            let styled = emphasised(styled, piece.emphasis);
            let Some(link) = piece.link.clone() else {
                return h_flex()
                    .when(piece.emphasis.code, |run| {
                        run.rounded(theme.radius.sm).bg(theme.colors.surface)
                    })
                    .child(styled);
            };
            let links = talk.drawn_links();
            let place = links.borrow().len();
            links.borrow_mut().push(link.clone());
            h_flex()
                .rounded(theme.radius.sm)
                .hover_bg(theme.colors.surface_hover)
                .on_click(Message::FollowAgentLink(session, place))
                .on_secondary_click(Message::ShowAgentTextMenu(session, reply))
                .tooltip(link)
                .child(styled)
        }))
}

/// `styled` set the way `emphasis` asks: heavier, slanted, or in the fixed
/// pitch code is written in, a step smaller than the prose around it.
fn emphasised(styled: pm_ui::Text, emphasis: Emphasis) -> pm_ui::Text {
    let styled = match emphasis.strong {
        true => styled.font_bold(),
        false => styled,
    };
    let styled = match emphasis.italic {
        true => styled.italic(),
        false => styled,
    };
    match emphasis.code {
        true => styled.text_base().font_mono(),
        false => styled,
    }
}

/// Reads the transcript's selected logical content through the shared model.
pub fn selected_text(theme: &Theme, talk: &Talk) -> Option<String> {
    let (anchor, head) = talk.selection()?;
    let mut selection = Selection::default();
    selection.select(anchor, head);
    let wrapped = wrapped(theme, talk, columns(theme, talk.drawn_width().get()));
    selection.text(wrapped.selection_rows.len(), |at| {
        wrapped.selection_rows.row(at)
    })
}

/// Expands the transcript's boundaries to whole words using the shared model.
pub fn words_between(theme: &Theme, talk: &Talk, anchor: Spot, head: Spot) -> (Spot, Spot) {
    selection_between(theme, talk, anchor, head, Grain::Word)
}

/// Expands the transcript's boundaries to logical paragraphs using the shared model.
pub fn lines_between(theme: &Theme, talk: &Talk, anchor: Spot, head: Spot) -> (Spot, Spot) {
    selection_between(theme, talk, anchor, head, Grain::Paragraph)
}

/// Applies a selection grain to the transcript's logical rows.
fn selection_between(
    theme: &Theme,
    talk: &Talk,
    anchor: Spot,
    head: Spot,
    grain: Grain,
) -> (Spot, Spot) {
    let wrapped = wrapped(theme, talk, columns(theme, talk.drawn_width().get()));
    Selection::extend(anchor, head, grain, wrapped.selection_rows.len(), |at| {
        wrapped.selection_rows.row(at)
    })
}

/// Returns the boundaries covering the transcript's complete logical content.
pub fn everything(theme: &Theme, talk: &Talk) -> Option<(Spot, Spot)> {
    let wrapped = wrapped(theme, talk, columns(theme, talk.drawn_width().get()));
    Selection::everything(wrapped.selection_rows.len(), |at| {
        wrapped.selection_rows.row(at)
    })
}

/// Builds the bar above the conversation: which agent, where, and how it is.
fn header(theme: &Theme, talk: &Talk) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.control)
        .px(1.5)
        .gap(1)
        .items_center()
        .bg(theme.colors.surface)
        .child(
            text("●")
                .text_xs()
                .color(standing_color(theme, talk.standing())),
        )
        .child(
            text(talk.agent().name.to_owned())
                .text_xs()
                .color(theme.colors.text_muted),
        )
        .when_some(talk.organisation(), |bar, id| {
            bar.child(
                h_flex().tooltip(id.to_owned()).child(
                    text("Organisation")
                        .text_xs()
                        .color(theme.colors.text_subtle),
                ),
            )
        })
        .child(
            text(name_of(talk.root()))
                .text_xs()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
        .when_some(talk.title(), |bar, title| {
            bar.child(
                text(title.to_owned())
                    .text_xs()
                    .color(theme.colors.text_muted),
            )
        })
        .child(h_flex().flex_1())
        .when_some(talk.limits(), |bar, limits| {
            bar.child(limit_usage(theme, limits))
        })
        .child(context_usage(theme, talk.usage()))
        .when_some(doing(talk), |bar, status| {
            bar.child(text(status).text_xs().color(theme.colors.text_subtle))
        })
        .child(
            icon_button(
                theme,
                IconName::CircleUser,
                Message::ShowAgentAccounts(talk.id()),
            )
            .tooltip(format!(
                "Switch account or organisation · {}",
                talk.profile_name().unwrap_or("Default account")
            )),
        )
        .when_some(talk.fork_metadata(), |bar, fork| {
            bar.child(
                h_flex()
                    .tooltip(format!(
                        "Whole conversation forked from {} · files at {}",
                        fork.source,
                        fork.shared_root.display()
                    ))
                    .child(
                        text("Native fork · shared files")
                            .text_xs()
                            .color(theme.colors.text_subtle),
                    ),
            )
        })
        .when(talk.can_fork(), |bar| {
            bar.child(
                icon_button(theme, IconName::GitFork, Message::ForkAgent(talk.id()))
                    .tooltip("Fork whole conversation · native context · shared current files"),
            )
        })
        .when(talk.can_list(), |bar| {
            bar.child(
                icon_button(
                    theme,
                    IconName::History,
                    Message::ShowAgentHistory(talk.id()),
                )
                .tooltip("History"),
            )
        })
        .child(
            icon_button(
                theme,
                IconName::Plus,
                Message::ShowTool(crate::panes::Tool::Chat),
            )
            .tooltip("Open Chat"),
        )
}

/// Shows the fullest account limit with its plan and reset times on hover.
fn limit_usage(theme: &Theme, limits: &Limits) -> Div<Message> {
    let percentage = limits
        .windows
        .iter()
        .map(|window| window.used)
        .reduce(f64::max);
    let label = percentage.map_or_else(
        || "Limits: pending".to_owned(),
        |used| format!("Limits: {used:.0}%"),
    );
    usage_indicator(theme, label, percentage, limit_details(limits))
}

/// Shows compact context token counts with exact counts and cost on hover.
fn context_usage(theme: &Theme, usage: Option<&Usage>) -> Div<Message> {
    let percentage = usage
        .filter(|usage| usage.size > 0)
        .map(|usage| usage.used as f64 / usage.size as f64 * 100.0);
    let mut detail = usage.map_or_else(
        || "Waiting for the agent to report context usage after a prompt.".to_owned(),
        |usage| format!("Context: {} / {} tokens", usage.used, usage.size),
    );
    if let Some(cost) = usage.and_then(|usage| usage.cost.as_ref()) {
        detail.push_str(&match cost.currency.as_str() {
            "USD" => format!("\nCost: ${:.2}", cost.amount),
            _ => format!("\nCost: {:.2} {}", cost.amount, cost.currency),
        });
    }
    let label = usage.map_or_else(
        || "Context: pending".to_owned(),
        |usage| {
            format!(
                "Context: {} / {}",
                thousands(usage.used),
                thousands(usage.size)
            )
        },
    );
    usage_indicator(theme, label, percentage, detail)
}

/// Builds a compact usage label coloured by fullness with a detailed hover tooltip.
fn usage_indicator(
    theme: &Theme,
    label: String,
    percentage: Option<f64>,
    detail: String,
) -> Div<Message> {
    h_flex().tooltip(detail).child(
        text(label)
            .text_xs()
            .color(percentage.map_or(theme.colors.text_subtle, |used| heat(theme, used))),
    )
}

/// Formats token counts in whole thousands once they reach a thousand.
fn thousands(count: u64) -> String {
    match count {
        0..1000 => count.to_string(),
        _ => format!("{}k", count / 1000),
    }
}

/// The share used below which a window reads as plenty left.
const COMFORTABLE: f64 = 50.0;

/// The share used past which a window turns from warning towards danger.
const NEARING: f64 = 80.0;

/// Formats the plan and each rate limit's usage and remaining reset time.
fn limit_details(limits: &Limits) -> String {
    let plan = limits.plan.iter().map(|plan| format!("Plan: {plan}"));
    let windows = limits.windows.iter().map(|window| {
        let used = format!("{}: {:.0}% used", window.label, window.used);
        match window.resets.and_then(until) {
            Some(left) => format!("{used} · resets in {left}"),
            None => used,
        }
    });
    plan.chain(windows).collect::<Vec<_>>().join("\n")
}

/// The colour a window `used` percent through is drawn in: success while
/// there is plenty left, fading through warning to danger as it fills.
fn heat(theme: &Theme, used: f64) -> Rgba {
    let colors = &theme.colors;
    match used {
        used if used < COMFORTABLE => colors.success,
        used if used < NEARING => colors.success.mix(
            colors.warning,
            ((used - COMFORTABLE) / (NEARING - COMFORTABLE)) as f32,
        ),
        used => colors.warning.mix(
            colors.danger,
            (((used - NEARING) / (100.0 - NEARING)) as f32).min(1.0),
        ),
    }
}

/// How long until `moment`, to the largest two units that say it, where it
/// is still to come.
fn until(moment: SystemTime) -> Option<String> {
    let minutes = moment.duration_since(SystemTime::now()).ok()?.as_secs() / 60;
    Some(
        match (minutes / (24 * 60), minutes / 60 % 24, minutes % 60) {
            (0, 0, minutes) => format!("{minutes}m"),
            (0, hours, minutes) => format!("{hours}h {minutes}m"),
            (days, hours, _) => format!("{days}d {hours}h"),
        },
    )
}

/// What each row of the card in front sends, in the order the rows are drawn.
///
/// A row is pressed with the pointer or reached with the keyboard, and both
/// send what this says, so the card and the keys never disagree.
pub fn pending_messages(talk: &Talk) -> Vec<Message> {
    let session = talk.id();
    match talk.pending() {
        Some(Pending::Ask(id)) => talk
            .asks()
            .iter()
            .find(|ask| ask.id == id)
            .map_or_else(Vec::new, |ask| ask_messages(session, ask)),
        Some(Pending::Form(ticket)) => talk
            .forms()
            .iter()
            .find(|form| form.id() == ticket)
            .map_or_else(Vec::new, |form| {
                form.rows()
                    .into_iter()
                    .map(|row| form_message(session, form, row))
                    .collect()
            }),
        Some(Pending::Login) => login_messages(talk),
        None => Vec::new(),
    }
}

/// What each choice of the permission request `ask` sends.
fn ask_messages(session: TalkId, ask: &Ask) -> Vec<Message> {
    (0..ask.choices.len())
        .map(|place| Message::AnswerAgent(session, ask.id, place))
        .collect()
}

/// What the row `row` of `form` sends.
fn form_message(session: TalkId, form: &Form, row: FormRow) -> Message {
    let ticket = form.id();
    match row {
        FormRow::Choice(place, option) => Message::ChooseAnswer(session, ticket, place, option),
        FormRow::Own(place) => Message::TypeAnswer(session, ticket, place),
        FormRow::Submit => Message::SendAnswer(session, ticket),
        FormRow::Link => Message::OpenAnswerLink(session, ticket),
    }
}

/// What each way of logging in the agent offers sends.
fn login_messages(talk: &Talk) -> Vec<Message> {
    (0..talk.logins().len())
        .map(|place| Message::LogInAgent(talk.id(), place))
        .collect()
}

/// The row of `card` the keyboard is on, when it is the card in front.
fn lit_row(talk: &Talk, card: Pending) -> Option<usize> {
    (talk.pending() == Some(card)).then(|| talk.pending_cursor())
}

/// Builds the card something waiting on the reader is drawn in: `header`
/// across its top, `body` under it, and `hint` at its foot.
///
/// A login, a permission and a question are one card three times, so they
/// sit between the conversation and the prompt looking like one thing.
fn pending_card(
    theme: &Theme,
    header: Div<Message>,
    body: Vec<Div<Message>>,
    hint: Option<&str>,
) -> Div<Message> {
    v_flex().w_full().items_center().px(1.25).pt(0.5).child(
        v_flex()
            .w_full()
            .max_w_px(COMPOSER_WIDTH)
            .px(1)
            .py(0.75)
            .gap(1)
            .rounded(theme.radius.lg)
            .border_1(theme.colors.border)
            .bg(theme.colors.surface)
            .child(header)
            .children(body)
            .when_some(hint, |card, hint| {
                card.child(text(hint).text_xs().color(theme.colors.text_subtle))
            }),
    )
}

/// The header of a card waiting on the reader: its `tabs`, then `controls`
/// at the far end.
fn card_header(tabs: Vec<Div<Message>>, controls: Vec<Div<Message>>) -> Div<Message> {
    h_flex()
        .w_full()
        .items_center()
        .gap(0.5)
        .child(h_flex().flex_1().gap(1).children(tabs))
        .children(controls)
}

/// What a card waiting on the reader says it is asking, wrapped to the card.
fn asked(theme: &Theme, asked: &str) -> Div<Message> {
    v_flex().w_full().child(
        paragraph()
            .break_long_words()
            .span(asked, Font::new(TextSize::Base), theme.colors.text)
            .w_full(),
    )
}

/// Builds the card offering the ways the agent can be logged in.
///
/// The agent opens no conversation until it is logged in, so this sits where
/// a question from it would: under the conversation, above the prompt.
fn login(theme: &Theme, talk: &Talk) -> Div<Message> {
    let lit = lit_row(talk, Pending::Login);
    let rows = talk
        .logins()
        .iter()
        .zip(login_messages(talk))
        .enumerate()
        .map(|(place, (method, message))| {
            card_row(
                theme,
                Line {
                    mark: None,
                    title: method.name.clone(),
                    description: method.description.clone().unwrap_or_default(),
                    lit: lit == Some(place),
                    key: place + 1,
                    message,
                },
            )
        })
        .collect::<Vec<_>>();

    pending_card(
        theme,
        card_header(
            vec![card_tab(theme, "Log in".to_owned(), true, None)],
            Vec::new(),
        ),
        vec![
            asked(theme, &format!("{} needs logging in.", talk.agent().name)),
            v_flex().w_full().gap(0.25).children(rows),
        ],
        None,
    )
}

/// Builds the card asking whether the agent may do what it is asking about.
///
/// The call is shown as the conversation shows it, and each answer the agent
/// takes is a row under it; the cross and Escape refuse it.
fn permission(theme: &Theme, talk: &Talk, ask: &Ask, columns: usize) -> Div<Message> {
    let session = talk.id();
    let mut header = called(talk, &ask.tool);
    header.remove(0);
    let content = std::iter::once(header)
        .chain(tool_body(talk, &ask.tool, columns, false))
        .map(|row| {
            h_flex()
                .when_some(diff_background(theme, &row), |row, color| {
                    row.w_full().bg(color)
                })
                .children(row.into_iter().map(|piece| {
                    text(piece.text)
                        .text_sm()
                        .font_mono()
                        .color(tone(theme, piece.tone))
                }))
        })
        .collect::<Vec<_>>();
    let lit = lit_row(talk, Pending::Ask(ask.id));
    let rows = ask
        .choices
        .iter()
        .zip(ask_messages(session, ask))
        .enumerate()
        .map(|(place, (choice, message))| {
            card_row(
                theme,
                Line {
                    mark: None,
                    title: choice.name.clone(),
                    description: String::new(),
                    lit: lit == Some(place),
                    key: place + 1,
                    message,
                },
            )
        })
        .collect::<Vec<_>>();

    pending_card(
        theme,
        card_header(
            vec![card_tab(theme, "Permission".to_owned(), true, None)],
            vec![icon_button(
                theme,
                IconName::Close,
                Message::DenyAgent(session, ask.id),
            )],
        ),
        vec![
            v_flex()
                .w_full()
                .p(0.75)
                .rounded(theme.radius.md)
                .bg(theme.colors.background)
                .overflow_hidden()
                .children(content),
            v_flex().w_full().gap(0.25).children(rows),
        ],
        Some("Esc to deny"),
    )
}

/// Builds the card holding something the agent needs from the reader: a form
/// to fill in, or a page to visit.
///
/// The form is shown a question at a time, a tab each, with what can be
/// chosen laid out as rows to press and a row for the reader's own words
/// where the agent takes them. The card folds down to its tabs, and Escape
/// walks away from it as the cross does.
///
/// `answering` is the ticket and the field of the box that has the keyboard,
/// if one has, and `solid` whether its caret is in its visible blink phase.
fn question(
    theme: &Theme,
    talk: &Talk,
    form: &Form,
    answering: Option<(u64, usize)>,
    solid: bool,
) -> Div<Message> {
    let session = talk.id();
    let ticket = form.id();
    let pages = form.pages();
    let tabs = match form.link() {
        Some(_) => vec![card_tab(theme, "Sign in".to_owned(), true, None)],
        None => pages
            .iter()
            .enumerate()
            .map(|(place, page)| {
                card_tab(
                    theme,
                    form.fields()[page.field].title.clone(),
                    place == form.page(),
                    Some(Message::ShowAnswerPage(session, ticket, place)),
                )
            })
            .collect(),
    };
    let fold = match form.folded() {
        true => IconName::ChevronUp,
        false => IconName::ChevronDown,
    };
    let header = card_header(
        tabs,
        vec![
            icon_button(theme, fold, Message::FoldAnswer(session, ticket)),
            icon_button(
                theme,
                IconName::Close,
                Message::CancelAnswer(session, ticket),
            ),
        ],
    );
    if form.folded() {
        return pending_card(theme, header, Vec::new(), None);
    }
    let question = pages
        .get(form.page())
        .map(|page| form.fields()[page.field].description.as_str())
        .filter(|description| !description.is_empty())
        .unwrap_or(form.message());
    let typing = answering
        .filter(|(asked, _)| *asked == ticket)
        .map(|(_, place)| place);
    let lit = lit_row(talk, Pending::Form(ticket));
    let rows = form
        .rows()
        .into_iter()
        .enumerate()
        .map(|(place, row)| {
            let line = Line {
                mark: None,
                title: String::new(),
                description: String::new(),
                lit: lit == Some(place),
                key: place + 1,
                message: form_message(session, form, row),
            };
            form_row(theme, session, form, row, line, typing, solid)
        })
        .collect::<Vec<_>>();

    pending_card(
        theme,
        header,
        vec![
            asked(theme, question),
            v_flex().w_full().gap(0.25).children(rows),
        ]
        .into_iter()
        .chain(form.preview().map(|preview| preview_block(theme, preview)))
        .collect(),
        Some("Esc to cancel"),
    )
}

/// Builds the row `row` of `form`, filling `line` in with what it shows.
/// `typing` is the field whose box has the keyboard, if one of this form's has.
fn form_row(
    theme: &Theme,
    session: TalkId,
    form: &Form,
    row: FormRow,
    line: Line,
    typing: Option<usize>,
    solid: bool,
) -> Div<Message> {
    let round = |place: usize| {
        !matches!(
            form.fields().get(place).map(|field| &field.input),
            Some(Input::Many { .. })
        )
    };
    match row {
        FormRow::Choice(place, option) => {
            let chosen = form.chosen(place, option);
            let (title, description) = match &form.fields()[place].input {
                Input::One { options, .. } | Input::Many { options, .. } => options
                    .get(option)
                    .map(|alternative| (alternative.title.clone(), alternative.description.clone()))
                    .unwrap_or_default(),
                _ => (
                    match option {
                        0 => "Yes",
                        _ => "No",
                    }
                    .to_owned(),
                    String::new(),
                ),
            };
            card_row(
                theme,
                Line {
                    mark: Some(Mark {
                        round: round(place),
                        chosen,
                    }),
                    title,
                    description,
                    ..line
                },
            )
        }
        FormRow::Own(place) => {
            let written = answer_box(theme, session, form, place, typing == Some(place), solid);
            let owner = form
                .pages()
                .into_iter()
                .find(|page| page.other == Some(place));
            match (owner, written) {
                (Some(owner), written) => {
                    let picked = form.other_picked(place);
                    v_flex()
                        .w_full()
                        .child(card_row(
                            theme,
                            Line {
                                mark: Some(Mark {
                                    round: round(owner.field),
                                    chosen: picked,
                                }),
                                title: form.fields()[place].title.clone(),
                                ..line
                            },
                        ))
                        .when_some(written.filter(|_| picked), |column, written| {
                            column.child(h_flex().w_full().pl(MARK / STEP + 1.5).child(written))
                        })
                }
                (None, written) => written.unwrap_or_else(v_flex),
            }
        }
        FormRow::Submit => submit_row(theme, "Submit answers", form.answered(), line),
        FormRow::Link => {
            let url = form
                .link()
                .map_or_else(String::new, |link| link.url.clone());
            let site = url
                .split("://")
                .nth(1)
                .and_then(|rest| rest.split('/').next())
                .unwrap_or(&url)
                .to_owned();
            card_row(
                theme,
                Line {
                    title: format!("Open {site} in the browser"),
                    description: url,
                    ..line
                },
            )
        }
    }
}

/// Builds what an alternative would look like, as the agent drew it.
fn preview_block(theme: &Theme, preview: &str) -> Div<Message> {
    v_flex()
        .w_full()
        .p(0.75)
        .rounded(theme.radius.md)
        .border_1(theme.colors.border)
        .bg(theme.colors.background)
        .overflow_hidden()
        .children(preview.lines().take(PREVIEW_LINES).map(|line| {
            text(line.to_owned())
                .text_sm()
                .font_mono()
                .color(theme.colors.text_muted)
        }))
}

/// One tab of a card's header, underlined while it is the one shown.
fn card_tab(theme: &Theme, title: String, shown: bool, message: Option<Message>) -> Div<Message> {
    let (color, edge) = match shown {
        true => (theme.colors.text, theme.colors.accent),
        false => (theme.colors.text_muted, theme.colors.surface),
    };
    h_flex()
        .px(0.25)
        .pb(0.25)
        .border_side(Side::Bottom, 2.0, edge)
        .when_some(message, Div::on_click)
        .child(text(title).text_sm().color(color))
}

/// The box the field at `place` is written in, lit while it has the keyboard.
fn answer_box(
    theme: &Theme,
    session: TalkId,
    form: &Form,
    place: usize,
    typing: bool,
    solid: bool,
) -> Option<Div<Message>> {
    let ticket = form.id();
    Some(hinted_input_view(
        theme,
        form.text_box(place)?,
        typing,
        solid,
        "Type your answer…",
        move |phase, from, to| Message::WriteAnswer(session, ticket, place, phase, from, to),
        Message::ShowInputMenu,
    ))
}

/// The row a form is sent from, lit once there is an answer to send.
fn submit_row(theme: &Theme, label: &str, ready: bool, line: Line) -> Div<Message> {
    let (fill, hover, edge, ink) = match ready {
        true => (
            theme.colors.accent,
            theme.colors.accent_hover,
            theme.colors.accent,
            theme.colors.text_on_accent,
        ),
        false => (
            match line.lit {
                true => theme.colors.surface_hover,
                false => theme.colors.surface,
            },
            theme.colors.surface_hover,
            theme.colors.border,
            theme.colors.text_muted,
        ),
    };
    h_flex()
        .w_full()
        .h_px(theme.size.control)
        .px(1)
        .gap(1)
        .items_center()
        .rounded(theme.radius.md)
        .border_1(match line.lit {
            true => theme.colors.border_focused,
            false => edge,
        })
        .bg(fill)
        .hover_bg(hover)
        .on_click(line.message)
        .child(key_hint(line.key).text_sm().color(ink))
        .child(text(label).text_sm().font_semibold().color(ink))
}

/// The digit that presses the row counted `key` from one, while there is one.
fn key_hint(key: usize) -> pm_ui::Text {
    text(match key {
        1..=9 => key.to_string(),
        _ => String::new(),
    })
}

/// The mark before a row that can be chosen.
#[derive(Clone, Copy)]
struct Mark {
    /// Whether it is round, one of several, rather than square, any of several.
    round: bool,
    /// Whether it is chosen.
    chosen: bool,
}

/// One row of a card waiting on the reader.
struct Line {
    /// The mark before it, where it is a choice.
    mark: Option<Mark>,
    /// What it says.
    title: String,
    /// What else there is to say about it, under the title.
    description: String,
    /// Whether the keyboard is on it.
    lit: bool,
    /// Its place in the card, counted from one, which is the digit that presses it.
    key: usize,
    /// What pressing it sends.
    message: Message,
}

/// Builds `line`: its mark, its title with its description under it, and
/// the digit that presses it at the far end.
fn card_row(theme: &Theme, line: Line) -> Div<Message> {
    let chosen = line.mark.is_some_and(|mark| mark.chosen);
    h_flex()
        .w_full()
        .px(0.5)
        .py(0.5)
        .gap(1)
        .items_center()
        .rounded(theme.radius.md)
        .when(line.lit, |row| row.bg(theme.colors.surface_hover))
        .when(chosen && !line.lit, |row| {
            row.bg(theme.colors.surface_selected)
        })
        .hover_bg(theme.colors.surface_hover)
        .on_click(line.message)
        .when_some(line.mark, |row, mark| row.child(choice_mark(theme, mark)))
        .child(
            v_flex()
                .flex_1()
                .gap(0.25)
                .child(text(line.title).text_sm().color(theme.colors.text))
                .when(!line.description.is_empty(), |column| {
                    column.child(
                        paragraph()
                            .break_long_words()
                            .span(
                                line.description,
                                Font::new(TextSize::Xs),
                                theme.colors.text_muted,
                            )
                            .w_full(),
                    )
                }),
        )
        .child(key_hint(line.key).text_xs().color(theme.colors.text_subtle))
}

/// Builds `mark`: a ring or a square, filled while it is chosen.
fn choice_mark(theme: &Theme, mark: Mark) -> Div<Message> {
    let corner = match mark.round {
        true => theme.radius.full,
        false => theme.radius.sm,
    };
    let edge = match mark.chosen {
        true => theme.colors.accent,
        false => theme.colors.border_selected,
    };
    v_flex()
        .size_px(MARK)
        .items_center()
        .justify_center()
        .rounded(corner)
        .border_1(edge)
        .when(mark.chosen, |ring| {
            ring.child(
                v_flex()
                    .size_px(MARK / 2.0)
                    .rounded(corner)
                    .bg(theme.colors.accent),
            )
        })
}

/// Builds the box the next prompt is written in, and what it takes.
///
/// Everything a reader sets about the turn they are about to send sits in one
/// card with the box they are typing it in: which model, how hard it thinks,
/// what mode it is in and whether it is sent or stopped. They are facts about
/// the next turn, so they are where the next turn is written and not in a bar
/// at the top of the pane. The card is drawn in the pane's own colour, at a
/// readable width, and the box grows with what is written in it until it
/// has grown to [`PROMPT_ROWS`] and scrolls instead.
fn composer(theme: &Theme, talk: &Talk, typing: bool, solid: bool) -> Div<Message> {
    let id = talk.id();
    let rows = talk.prompt().rows().clamp(PROMPT_LEAST, PROMPT_ROWS);
    let height = rows as f32 * theme.text.code.line_height;
    let edge = match typing {
        true => theme.colors.accent,
        false => theme.colors.border,
    };

    v_flex()
        .w_full()
        .items_center()
        .px(1.25)
        .pt(0.5)
        .pb(1)
        .child(
            v_flex()
                .w_full()
                .max_w_px(COMPOSER_WIDTH)
                .rounded(theme.radius.lg)
                .border_1(edge)
                .bg(theme.colors.background)
                .overflow_hidden()
                .when(!talk.attachments().is_empty(), |card| {
                    card.child(attachment_list(theme, talk).px(1.5).pt(1))
                })
                .child(
                    h_flex().w_full().px(1.5).pt(1).pb(0.75).child(
                        bare_input_view(
                            talk.prompt(),
                            typing,
                            solid,
                            &format!("Message {}", talk.agent().name),
                            move |phase, from, to| Message::WriteAgentPrompt(id, phase, from, to),
                            move |event, step| Message::DragAgentPrompt(id, event, step),
                            Message::ShowInputMenu,
                        )
                        .h_px(height),
                    ),
                )
                .child(rule(theme))
                .child(controls(theme, talk)),
        )
}

/// The files and images waiting to go with the next prompt.
fn attachment_list(theme: &Theme, talk: &Talk) -> Div<Message> {
    v_flex().gap(0.5).children(
        talk.attachments()
            .iter()
            .enumerate()
            .map(|(place, attachment)| {
                h_flex()
                    .gap(0.5)
                    .items_center()
                    .when_some(talk.attachment_preview(place), |row, preview| {
                        row.child(
                            h_flex()
                                .size_px(48.0)
                                .items_center()
                                .justify_center()
                                .rounded(theme.radius.sm)
                                .bg(theme.colors.surface_hover)
                                .overflow_hidden()
                                .child(picture(preview).size_px(44.0)),
                        )
                    })
                    .child(
                        text(attachment.label())
                            .text_xs()
                            .color(theme.colors.text_muted),
                    )
                    .child(
                        h_flex()
                            .px(0.25)
                            .rounded(theme.radius.sm)
                            .hover_bg(theme.colors.surface_hover)
                            .on_click(Message::RemoveAgentAttachment(talk.id(), place))
                            .child(text("×").text_xs().color(theme.colors.text_subtle)),
                    )
            }),
    )
}

/// Builds the row of controls under the prompt.
///
/// What the turn is sent with leads on the left — attachments, commands and
/// the model — and how it is sent closes on the right: the mode, then the
/// button itself. A model's effort reads beside its name, so one pill says
/// both; the knobs that have no pill of their own are set from the model's
/// choices instead.
fn controls(theme: &Theme, talk: &Talk) -> Div<Message> {
    let session = talk.id();
    let knobs = talk.knobs();
    let model = knobs.iter().position(|knob| knob.about == About::Model);
    let effort = knobs
        .iter()
        .find(|knob| knob.about == About::Thinking)
        .map(set_to);

    h_flex()
        .w_full()
        .px(1)
        .py(0.75)
        .gap(0.5)
        .items_center()
        .child(
            icon_button(theme, IconName::Plus, Message::AttachAgentFiles(session))
                .tooltip("Attach files"),
        )
        .child(
            icon_button(
                theme,
                IconName::SquareSlash,
                Message::StartAgentCommand(session),
            )
            .tooltip("Commands"),
        )
        .when(talk.agent().id == "codex", |row| {
            row.child(pill(theme, "$", None).on_click(Message::StartAgentSkill(session)))
        })
        .when_some(model, |row, place| {
            row.child(
                pill(theme, set_to(&knobs[place]), effort.clone())
                    .on_click(Message::PressKnob(session, place)),
            )
        })
        .children(
            knobs
                .iter()
                .enumerate()
                .filter(|(_, knob)| model.is_none() && knob.about != About::Mode)
                .map(|(place, knob)| {
                    pill(theme, set_to(knob), None).on_click(Message::PressKnob(session, place))
                }),
        )
        .child(h_flex().flex_1())
        .child(
            icon_button(theme, IconName::Plug, Message::ShowAgentMcp(session)).tooltip(format!(
                "MCP servers: {} given",
                talk.mcp_servers()
                    .iter()
                    .filter(|server| server.given)
                    .count()
            )),
        )
        .when_some(mode_of(talk), |row, (id, name)| {
            row.child(
                h_flex()
                    .h_px(theme.size.icon_control)
                    .px(0.75)
                    .gap(0.5)
                    .items_center()
                    .rounded(theme.radius.md)
                    .hover_bg(theme.colors.surface_hover)
                    .active_bg(theme.colors.surface_active)
                    .on_click(Message::ShowAgentModes(session))
                    .when_some(mode_icon(&id), |button, name| {
                        button.child(
                            icon(name)
                                .size(IconSize::Small)
                                .color(theme.colors.text_muted),
                        )
                    })
                    .child(text(name).text_xs().color(theme.colors.text_muted)),
            )
        })
        .child(send(theme, talk))
}

/// What the session's mode is, as the id it is known by and the name it reads
/// as, whichever way the agent says it.
///
/// An agent says its mode as a mode or as a knob that is about the mode; the
/// control reads the same either way, and it sits where the mode belongs
/// rather than among the model and the rest.
fn mode_of(talk: &Talk) -> Option<(String, String)> {
    if let (Some(id), Some(name)) = (talk.mode(), talk.mode_name()) {
        return Some((id.to_owned(), name));
    }
    let knob = talk.knob_about(About::Mode)?;
    let id = match &knob.setting {
        Setting::Picked { value, .. } => value.clone(),
        Setting::Switched(_) => knob.id.clone(),
    };
    Some((id, set_to(&knob)))
}

/// The icon that stands beside a mode, by what its id says it does.
///
/// Agents name their modes themselves, so this reads the id for the handful
/// of kinds of mode there are, and a mode it does not recognise goes without.
pub fn mode_icon(id: &str) -> Option<IconName> {
    let id = id.to_ascii_lowercase();
    let says = |words: &[&str]| words.iter().any(|word| id.contains(word));
    if says(&["plan"]) {
        Some(IconName::ScrollText)
    } else if says(&["bypass", "auto", "full", "yolo", "dangerous"]) {
        Some(IconName::Zap)
    } else if says(&["accept", "edit", "write"]) {
        Some(IconName::Code)
    } else if says(&["default", "manual", "ask", "read"]) {
        Some(IconName::Hand)
    } else {
        None
    }
}

/// Builds one of the composer's pills: a label that is also a control, with
/// `quiet` after it in the subtler colour.
fn pill(theme: &Theme, label: impl Into<String>, quiet: Option<String>) -> Div<Message> {
    h_flex()
        .h_px(theme.size.icon_control)
        .px(0.75)
        .gap(0.5)
        .items_center()
        .rounded(theme.radius.full)
        .bg(theme.colors.surface_hover)
        .hover_bg(theme.colors.surface_active)
        .child(text(label.into()).text_xs().color(theme.colors.text_muted))
        .when_some(quiet, |pill, quiet| {
            pill.child(text(quiet).text_xs().color(theme.colors.text_subtle))
        })
}

/// What a knob's pill says: what it is set to, or what it is and whether.
fn set_to(knob: &Knob) -> String {
    match &knob.setting {
        Setting::Picked { value, picks } => picks
            .iter()
            .find(|pick| &pick.id == value)
            .map_or_else(|| value.clone(), |pick| pick.name.clone()),
        Setting::Switched(true) => format!("{} on", knob.name),
        Setting::Switched(false) => format!("{} off", knob.name),
    }
}

/// Builds the rows for the knobs set beside a list of the session's choices.
///
/// A model's or a mode's choices are a list, but how hard it thinks and the
/// switches it offers are set in place underneath, without leaving it: a
/// scale of dots for a knob of several values, a switch for one of two. The
/// knob named `shown` is the list itself and is left out, as are the model
/// and the mode, which have lists of their own.
pub fn knob_rows(theme: &Theme, talk: &Talk, shown: Option<&str>) -> Vec<Div<Message>> {
    let session = talk.id();
    talk.knobs()
        .into_iter()
        .enumerate()
        .filter(|(_, knob)| !matches!(knob.about, About::Model | About::Mode))
        .filter(|(_, knob)| Some(knob.id.as_str()) != shown)
        .map(|(place, knob)| {
            let row = h_flex()
                .w_full()
                .gap(0.5)
                .items_center()
                .child(text(knob.name.clone()).text_sm().color(theme.colors.text));
            match &knob.setting {
                Setting::Picked { value, picks } => {
                    let at = picks.iter().position(|pick| &pick.id == value);
                    row.child(
                        text(format!("({})", set_to(&knob)))
                            .text_sm()
                            .color(theme.colors.text_subtle),
                    )
                    .child(h_flex().flex_1())
                    .child(scale(theme, session, place, picks.len(), at))
                }
                Setting::Switched(on) => row
                    .child(h_flex().flex_1())
                    .child(switch(*on, Message::FlipAgentKnob(session, place))),
            }
        })
        .collect()
}

/// Builds a scale of `count` dots for the knob in `place`, the one at `at`
/// lit and larger, each setting the knob to its value when pressed.
fn scale(
    theme: &Theme,
    session: TalkId,
    place: usize,
    count: usize,
    at: Option<usize>,
) -> Div<Message> {
    h_flex()
        .px(0.5)
        .items_center()
        .rounded(theme.radius.full)
        .bg(theme.colors.surface_hover)
        .children((0..count).map(|pick| {
            let lit = Some(pick) == at;
            let (size, color) = match lit {
                true => (12.0, theme.colors.text),
                false => (4.0, theme.colors.text_subtle),
            };
            v_flex()
                .size_px(18.0)
                .items_center()
                .justify_center()
                .on_click(Message::SetAgentKnob(session, place, pick))
                .child(v_flex().size_px(size).rounded(theme.radius.full).bg(color))
        }))
}

/// Builds the control that sends the turn, or stops the one that is running.
///
/// With nothing written and nothing running there is nothing to send, so it
/// is drawn faded until there is.
fn send(theme: &Theme, talk: &Talk) -> Div<Message> {
    let session = talk.id();
    let busy = talk.is_busy();
    let (name, message) = match busy {
        true => (IconName::Stop, Message::StopAgentTurn(session)),
        false => (IconName::ArrowUp, Message::SendPrompt(session)),
    };
    let idle = !busy && talk.prompt().is_empty() && talk.attachments().is_empty();
    let fade = |color: Rgba| match idle {
        true => color.alpha(0.45),
        false => color,
    };

    v_flex()
        .size_px(theme.size.icon_control)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .bg(fade(theme.colors.accent))
        .hover_bg(fade(theme.colors.accent_hover))
        .active_bg(fade(theme.colors.accent_active))
        .on_click(message)
        .child(
            icon(name)
                .size(IconSize::Small)
                .color(fade(theme.colors.text_on_accent)),
        )
}

/// How many characters of the conversation's type fit across a pane
/// `width` logical pixels wide, once the room either side of the text and
/// the scrollbar's gutter are taken off it.
fn columns(theme: &Theme, width: f32) -> usize {
    let advance = (theme.text.lg.size * ADVANCE).max(1.0);
    let room = width - space(SIDE) - SCROLLBAR_GUTTER;
    ((room / advance) as usize).max(NARROWEST)
}

/// `passage` broken into lines of at most `columns` characters, each handed
/// to `line` in turn.
///
/// Where a line breaks is where a word ends; a word longer than the pane is
/// wide is broken anyway, because the alternative is a line nobody can read
/// the end of. One line is written at a time and lent out, so a long result
/// costs no more than the lines of it that are kept.
fn wrap(passage: &str, columns: usize, mut line: impl FnMut(&str)) {
    let mut written = String::new();
    for paragraph in passage.split('\n') {
        written.clear();
        let mut width = 0;
        for word in paragraph.split(' ') {
            for part in split(word, columns) {
                let length = part.chars().count();
                if width > 0 && width + 1 + length > columns {
                    line(&written);
                    written.clear();
                    width = 0;
                } else if width > 0 {
                    written.push(' ');
                    width += 1;
                }
                written.push_str(part);
                width += length;
            }
        }
        line(&written);
    }
}

/// `word` in pieces of at most `columns` characters.
fn split(word: &str, columns: usize) -> impl Iterator<Item = &str> {
    let mut rest = Some(word);
    std::iter::from_fn(move || {
        let word = rest?;
        match word.char_indices().nth(columns) {
            Some((at, _)) => {
                rest = Some(&word[at..]);
                Some(&word[..at])
            }
            None => {
                rest = None;
                Some(word)
            }
        }
    })
}

/// One run of text in one colour.
fn piece(text: String, tone: Tone) -> Piece {
    Piece {
        text,
        tone,
        image: None,
        link: None,
        emphasis: Emphasis::default(),
        wrapped: false,
        card: None,
        owner: None,
        live: None,
    }
}

/// A reader image shown as a thumbnail inside the message bubble.
fn image_piece(image: Image) -> Piece {
    Piece {
        image: Some(image),
        ..piece(String::new(), Tone::Said)
    }
}

/// The colour a mark against `tone` is drawn in.
fn quieten(tone: Tone) -> Tone {
    match tone {
        Tone::Note => Tone::Note,
        _ => Tone::Quiet,
    }
}

/// The colour `tone` comes out in.
fn tone(theme: &Theme, tone: Tone) -> Rgba {
    match tone {
        Tone::Said => theme.colors.text,
        Tone::Spoken => theme.colors.text,
        Tone::Heading(_) => theme.colors.text,
        Tone::Code(Some(highlight)) => tint(highlight, theme),
        Tone::Code(None) => theme.colors.text,
        Tone::Diff(Some(highlight), _) => tint(highlight, theme),
        Tone::Diff(None, ChangeTag::Insert) => theme.colors.success,
        Tone::Diff(None, ChangeTag::Delete) => theme.colors.danger,
        Tone::Diff(None, ChangeTag::Equal) => theme.colors.text,
        Tone::Table => theme.colors.text,
        Tone::TableRule => theme.colors.text_subtle,
        Tone::Quiet => theme.colors.text_subtle,
        Tone::Tool => theme.colors.text_muted,
        Tone::Argument => theme.colors.text_subtle,
        Tone::DetailGroup(_) => theme.colors.text_subtle,
        Tone::Failed => theme.colors.danger,
        Tone::Note => theme.colors.warning,
    }
}

/// The colour a session's mark is drawn in, for how it is doing.
///
/// The header, the sidebar's row and the status bar's tally all mark a
/// session this way, so one reading of a colour holds everywhere.
pub fn standing_color(theme: &Theme, standing: Standing) -> Rgba {
    match standing {
        Standing::Stopped => theme.colors.danger,
        Standing::Waiting => theme.colors.warning,
        Standing::Working => theme.colors.success,
        Standing::Done => theme.colors.link,
        Standing::Idle => theme.colors.text_subtle,
    }
}

/// The heading above the current turn’s response: the spinner and a
/// waiting label while the prompt is unsent, the elapsed time once it is.
fn working(talk: &Talk) -> String {
    if talk.is_sending() {
        return format!("{} Sending prompt…", sending_frame());
    }
    format!(
        "Working for {}s",
        talk.working_for().unwrap_or_default().as_secs()
    )
}

/// The spinner glyph for the wall clock, so it turns without being told when
/// the wait began.
fn sending_frame() -> &'static str {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    WORKING[(millis / 250 % WORKING.len() as u128) as usize]
}

/// Shows startup and stopped states in the header, leaving ready sessions quiet.
fn doing(talk: &Talk) -> Option<String> {
    match (talk.is_running(), talk.is_ready(), talk.is_busy()) {
        (_, _, true) => None,
        (false, ..) => Some("stopped".to_owned()),
        (_, false, _) => Some("starting".to_owned()),
        _ => None,
    }
}

/// What `path` is called, without the directories above it.
fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// `path` as it reads from `root`, or in full when it is not under it.
fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Finds a tool's wrapped row after expanding its enclosing tool group.
pub(crate) fn tool_offset(theme: &Theme, talk: &Talk, width: f32, id: &str) -> Option<f32> {
    let block = talk
        .transcript()
        .blocks()
        .iter()
        .position(|block| matches!(block, Block::Ran(call) if call.id == id))?;
    let columns = columns(theme, width);
    let wrapped = wrapped(theme, talk, columns);
    let row = wrapped.entries.iter().position(|entry| match entry {
        Entry::Part(part, row) => {
            let part = &wrapped.parts[*part];
            part.key.start <= block
                && part.key.end > block
                && part.rows[*row]
                    .first()
                    .is_some_and(|piece| piece.card.as_deref() == Some(id))
        }
        _ => false,
    })?;
    Some(wrapped.tops[row] + space(INSET))
}
