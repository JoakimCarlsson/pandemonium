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

use std::path::Path;

use pm_acp::{About, Ask, Kind, Knob, Output, Setting, Status, Step, ToolCall, Voice, Weight};
use pm_gfx::{Image, Rgba};
use pm_ui::{
    Div, IconName, IconSize, Scroll, Styled, Theme, button, h_flex, icon, measured, picture, rule,
    scroll_area, space, text, v_flex,
};

use crate::agent::{Block, Standing, Talk, TalkId};
use crate::input::input_view;
use crate::message::Message;

/// How many rows are built at once, however long the conversation runs.
const DRAWN: usize = 300;

/// How many lines of one tool call's result are shown before the rest.
const RESULT_LINES: usize = 2;

/// How far the conversation sits from the top and foot of its area, in
/// steps of the spacing scale.
const INSET: f32 = 2.0;

/// How far the edge of a bubble holding what the reader said sits from its
/// text, in steps of the spacing scale.
const BUBBLE: f32 = 1.25;

/// How many lines of the prompt the pane has room for.
const PROMPT_LINES: f32 = 3.0;

/// How many of the commands a slash narrows to are offered at once.
const OFFERED: usize = 8;

/// Estimated average width of a conversation character as a share of its size.
const ADVANCE: f32 = 0.55;

/// Fewest characters a line is wrapped at, however narrow the pane is.
const NARROWEST: usize = 24;

/// Mark before the agent's reply.
const BULLET: &str = "";

/// What stands against what the reader said.
const CHEVRON: &str = "";

/// Indentation before a tool call's result.
const RESULT: &str = "    ";

/// What a line continuing the one above it is indented by.
const WRAPPED: &str = "  ";

/// What a turn that is still running is marked with.
const WORKING: &str = "  ◐ ";

/// The colour a piece of a row is drawn in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tone {
    /// What the reader said.
    Said,
    /// What the agent said.
    Spoken,
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
}

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
    solid: bool,
    width: f32,
) -> Div<Message> {
    let columns = columns(theme, width);
    let (drawn, offset) = drawn(theme, talk, columns);

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(header(theme, talk))
        .child(rule(theme))
        .child(measured(
            talk.view(),
            scroll_area(
                std::rc::Rc::new(std::cell::Cell::new(Scroll::at(offset))),
                v_flex().w_full().px(1.75).py(INSET).children(drawn),
            )
            .w_full()
            .flex_1(),
        ))
        .children(
            talk.asks()
                .iter()
                .map(|ask| permission(theme, talk.id(), ask)),
        )
        .when(!talk.offered().is_empty(), |pane| {
            pane.child(commands(theme, talk))
        })
        .child(composer(theme, talk, typing, solid))
}

/// Builds the list of commands the slash being typed narrows to.
///
/// The agent says what it takes — its slash commands and its skills — and
/// this is where that list is: a reader who types a slash is shown what this
/// agent answers to, rather than having to know.
fn commands(theme: &Theme, talk: &Talk) -> Div<Message> {
    let session = talk.id();
    let chosen = talk.chosen();
    let rows = talk
        .offered()
        .into_iter()
        .enumerate()
        .skip(chosen.saturating_sub(OFFERED - 1))
        .take(OFFERED)
        .map(|(place, command)| {
            h_flex()
                .w_full()
                .px(1)
                .py(0.25)
                .gap(1)
                .items_center()
                .hover_bg(theme.colors.surface_hover)
                .when(place == chosen, |row| row.bg(theme.colors.surface_selected))
                .on_click(Message::TakeAgentCommand(session, place))
                .child(
                    text(format!("{}{}", command.prefix, command.name))
                        .text_xs()
                        .font_mono()
                        .color(tone(theme, Tone::Tool)),
                )
                .child(
                    text(first_line(&command.description))
                        .text_xs()
                        .color(theme.colors.text_subtle),
                )
        })
        .collect::<Vec<_>>();

    v_flex().w_full().px(1.25).pt(0.5).child(
        v_flex()
            .w_full()
            .py(0.5)
            .rounded(theme.radius.lg)
            .border_1(theme.colors.border)
            .bg(theme.colors.surface)
            .overflow_hidden()
            .children(rows),
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
    let rows = rows(talk, columns(theme, width));
    heights(theme, &rows).iter().sum::<f32>() + space(INSET) * 2.0
}

/// The rows of the pane from where it is scrolled to, and how far the first
/// of them is scrolled up past the top of the area.
///
/// Only the rows from there are built, however long the conversation runs,
/// so the view is drawn from the row it is scrolled into and shifted up by
/// the part of it already gone by. A row inside a bubble is drawn from the
/// top of its bubble, so the bubble keeps its edge as it scrolls by.
fn drawn(theme: &Theme, talk: &Talk, columns: usize) -> (Vec<Div<Message>>, f32) {
    let rows = rows(talk, columns);
    let heights = heights(theme, &rows);
    talk.drawn_height()
        .set(heights.iter().sum::<f32>() + space(INSET) * 2.0);
    let mut first = 0;
    let mut top = 0.0;
    while first < rows.len() && top + heights[first] <= talk.scroll() - space(INSET) {
        top += heights[first];
        first += 1;
    }
    while first > 0 && rows.get(first).is_some_and(is_said) && is_said(&rows[first - 1]) {
        first -= 1;
        top -= heights[first];
    }
    let offset = talk.scroll() - top;

    let mut visible = rows.into_iter().skip(first).take(DRAWN).peekable();
    let mut drawn = Vec::new();
    while let Some(line) = visible.next() {
        if is_said(&line) {
            let mut message = vec![self::row(theme, line, talk.id())];
            while visible.peek().is_some_and(is_said) {
                if let Some(next) = visible.next() {
                    message.push(self::row(theme, next, talk.id()));
                }
            }
            drawn.push(
                h_flex().w_full().justify_end().child(
                    v_flex()
                        .p(BUBBLE)
                        .rounded(theme.radius.xl)
                        .bg(theme.colors.surface_hover)
                        .children(message),
                ),
            );
        } else {
            drawn.push(self::row(theme, line, talk.id()));
        }
    }
    (drawn, offset)
}

/// Whether `row` is part of something the reader said, drawn in a bubble.
fn is_said(row: &Row) -> bool {
    row.iter().any(|piece| piece.tone == Tone::Said)
}

/// How tall each of `rows` is drawn, the edges of a bubble counted into the
/// first row and the last one it holds.
fn heights(theme: &Theme, rows: &[Row]) -> Vec<f32> {
    let edge = space(BUBBLE);
    rows.iter()
        .enumerate()
        .map(|(at, row)| {
            let mut height = row_height(theme, row);
            if is_said(row) {
                if at == 0 || !is_said(&rows[at - 1]) {
                    height += edge;
                }
                if rows.get(at + 1).is_none_or(|next| !is_said(next)) {
                    height += edge;
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
        .collect()
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
                    Tone::Said | Tone::Spoken => theme.text.lg.line_height,
                    _ => theme.text.sm.line_height,
                }
            }
        })
        .fold(0.0, f32::max)
}

/// Every row the conversation comes to, wrapped at `columns` characters.
fn rows(talk: &Talk, columns: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    let blocks = talk.transcript().blocks();
    let mut at = 0;
    while at < blocks.len() {
        let adjacent_picture = matches!(&blocks[at], Block::Picture(_))
            && at > 0
            && matches!(
                &blocks[at - 1],
                Block::Said(Voice::Reader, _) | Block::Picture(_)
            );
        if !rows.is_empty() && !adjacent_picture {
            rows.push(Row::new());
        }
        match &blocks[at] {
            Block::Said(Voice::Reader, passage) => {
                rows.extend(reader_rows(
                    talk,
                    at,
                    passage,
                    (columns * 2 / 3).max(NARROWEST),
                ));
            }
            Block::Picture(image) => rows.push(vec![image_piece(image.clone())]),
            Block::Said(Voice::Agent, passage) => {
                rows.extend(passage_rows(passage, BULLET, Tone::Spoken, columns));
            }
            Block::Said(Voice::Thought, passage) => {
                rows.push(vec![piece(
                    format!(
                        "{} Thinking",
                        if talk.details_expanded(at) {
                            "⌄"
                        } else {
                            "›"
                        }
                    ),
                    Tone::DetailGroup(at),
                )]);
                if talk.details_expanded(at) {
                    rows.extend(passage_rows(passage, BULLET, Tone::Quiet, columns));
                }
            }
            Block::Ran(_) => {
                let end = at
                    + blocks[at..]
                        .iter()
                        .take_while(|block| matches!(block, Block::Ran(_)))
                        .count();
                rows.push(tool_group_row(
                    &blocks[at..end],
                    at,
                    talk.details_expanded(at),
                ));
                if talk.details_expanded(at) {
                    for block in &blocks[at..end] {
                        if let Block::Ran(call) = block {
                            rows.extend(tool_rows(call, talk.root(), columns));
                        }
                    }
                }
                at = end - 1;
            }
            Block::Planned(steps) => rows.extend(steps.iter().map(step_row)),
            Block::Note(note) => rows.extend(passage_rows(note, BULLET, Tone::Note, columns)),
        }
        at += 1;
    }
    if talk.is_busy() {
        if !rows.is_empty() {
            rows.push(Row::new());
        }
        rows.push(vec![piece(format!("{WORKING}Working…"), Tone::Quiet)]);
    }
    rows
}

/// Reader text and any images restored from an agent's saved transcript.
fn reader_rows(talk: &Talk, block: usize, passage: &str, columns: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut rest = passage;
    let mut place = 0;
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
        if let Some(image) = talk
            .transcript()
            .restored_picture(block, place, &source[..end])
        {
            rows.push(vec![image_piece(image)]);
        } else {
            rows.extend(passage_rows("[Pasted image]", CHEVRON, Tone::Said, columns));
        }
        place += 1;
        rest = &source[end + 1..];
    }
    if !rest.is_empty() {
        rows.extend(passage_rows(rest, CHEVRON, Tone::Said, columns));
    }
    rows
}

/// The collapsed or expanded heading for adjacent tool calls.
fn tool_group_row(blocks: &[Block], at: usize, expanded: bool) -> Row {
    let mark = if expanded { "⌄" } else { "›" };
    let count = blocks.len();
    let label = if count == 1 {
        match &blocks[0] {
            Block::Ran(call) => call.title.clone(),
            _ => String::new(),
        }
    } else {
        let reads = blocks.iter().any(|block| {
            matches!(block, Block::Ran(call) if matches!(call.kind, Kind::Read | Kind::Search))
        });
        let runs = blocks
            .iter()
            .any(|block| matches!(block, Block::Ran(call) if call.kind == Kind::Execute));
        let edits = blocks.iter().any(|block| {
            matches!(block, Block::Ran(call) if matches!(call.kind, Kind::Edit | Kind::Delete | Kind::Move))
        });
        let mut activities = Vec::new();
        if reads {
            activities.push("Read files");
        }
        if runs {
            activities.push("ran commands");
        }
        if edits {
            activities.push("edited files");
        }
        if activities.is_empty() {
            format!("{count} tool calls")
        } else {
            activities.join(", ")
        }
    };
    let failed = blocks
        .iter()
        .any(|block| matches!(block, Block::Ran(call) if call.status == Status::Failed));
    let mut row = vec![piece(format!("{label}  {mark}"), Tone::DetailGroup(at))];
    if failed {
        row.push(piece(" · failed".to_owned(), Tone::Failed));
    }
    row
}

/// One passage, as rows marked with `mark` and wrapped to the width.
fn passage_rows(passage: &str, mark: &str, tone: Tone, columns: usize) -> Vec<Row> {
    let passage = hide_image_data(passage);
    wrap(&passage, columns.saturating_sub(mark.chars().count()))
        .into_iter()
        .enumerate()
        .map(|(at, line)| match at {
            0 => vec![piece(mark.to_owned(), quieten(tone)), piece(line, tone)],
            _ => vec![
                piece(
                    if tone == Tone::Said { "" } else { WRAPPED }.to_owned(),
                    tone,
                ),
                piece(line, tone),
            ],
        })
        .collect()
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

/// One tool call, as the line naming it and the lines of what it came to.
fn tool_rows(call: &ToolCall, root: &Path, columns: usize) -> Vec<Row> {
    let tone = match call.status {
        Status::Failed => Tone::Failed,
        _ => Tone::Quiet,
    };
    let mut rows = vec![called(call, root)];
    rows.extend(
        result(call, columns.saturating_sub(RESULT.chars().count()))
            .into_iter()
            .enumerate()
            .map(|(at, line)| match at {
                0 => vec![piece(RESULT.to_owned(), Tone::Quiet), piece(line, tone)],
                _ => vec![
                    piece("    ".to_owned(), Tone::Quiet),
                    piece(line, Tone::Quiet),
                ],
            }),
    );
    rows
}

/// The line naming one tool call: what was called, and on what.
///
/// An agent that says which tool it called gets the shape its own CLI uses —
/// the tool, then the file in brackets — and one that does not is left with
/// the sentence it wrote instead.
fn called(call: &ToolCall, root: &Path) -> Row {
    let mut row = vec![piece("  · ".to_owned(), Tone::Quiet)];
    let argument = call
        .locations
        .first()
        .map(|location| relative(&location.path, root));

    match (call.name.as_deref(), argument) {
        (Some(name), Some(argument)) => {
            row.push(piece(name.to_owned(), Tone::Tool));
            row.push(piece("(".to_owned(), Tone::Quiet));
            row.push(piece(argument, Tone::Argument));
            row.push(piece(")".to_owned(), Tone::Quiet));
        }
        (Some(name), None) => row.push(piece(name.to_owned(), Tone::Tool)),
        (None, _) => row.push(piece(call.title.clone(), Tone::Tool)),
    }
    row
}

/// What a tool call came to, in as many lines as it is worth showing.
///
/// A call that has produced nothing yet says where it has got to instead:
/// the line beneath a call is never blank, because a call with nothing under
/// it reads as one that did nothing.
fn result(call: &ToolCall, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for output in &call.output {
        match output {
            Output::Said(said) => lines.extend(wrap(said, columns)),
            Output::Changed { path, after, .. } => lines.push(format!(
                "{} · {} lines",
                name_of(path),
                after.lines().count()
            )),
        }
    }
    if lines.is_empty() {
        return match call.status {
            Status::Pending => vec!["waiting".to_owned()],
            Status::Running => vec!["running".to_owned()],
            Status::Done => Vec::new(),
            Status::Failed => vec!["failed".to_owned()],
        };
    }

    lines.retain(|line| !line.trim().is_empty());
    let over = lines.len().saturating_sub(RESULT_LINES);
    lines.truncate(RESULT_LINES);
    if over > 0 {
        lines.push(format!("… {over} more lines"));
    }
    lines
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

/// Builds one row out of its pieces.
///
/// The row is held to the height [`row_height`] gives it, so that where the
/// pane scrolls to and where it draws the rows are the same measurement.
fn row(theme: &Theme, row: Row, session: TalkId) -> Div<Message> {
    let height = row_height(theme, &row);
    if row.is_empty() {
        return h_flex().h_px(height);
    }
    let action = row.first().and_then(|piece| match piece.tone {
        Tone::DetailGroup(block) => Some(block),
        _ => None,
    });
    let detail = action.is_some();
    h_flex()
        .h_px(if detail { height + space(0.75) } else { height })
        .items_center()
        .when_some(action, |line, block| {
            line.on_click(Message::ToggleAgentDetails(session, block))
                .hover_bg(theme.colors.surface_hover)
        })
        .children(row.into_iter().map(|piece| {
            if let Some(image) = piece.image {
                return h_flex().child(picture(image).w_px(160.0).h_px(112.0));
            }
            let styled = text(piece.text).color(tone(theme, piece.tone));
            let styled = match piece.tone {
                Tone::Said | Tone::Spoken => styled.text_lg(),
                Tone::Argument => styled.text_sm().font_mono(),
                _ => styled.text_sm(),
            };
            h_flex().child(styled)
        }))
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
        .child(
            text(name_of(talk.root()))
                .text_xs()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
        .child(h_flex().flex_1())
        .child(text(doing(talk)).text_xs().color(theme.colors.text_subtle))
}

/// Builds the card asking whether the agent may do what it is asking about.
fn permission(theme: &Theme, session: TalkId, ask: &Ask) -> Div<Message> {
    let choices = ask
        .choices
        .iter()
        .enumerate()
        .map(|(place, choice)| {
            let message = Message::AnswerAgent(session, ask.id, place);
            let button = button(choice.name.clone(), message).h_px(theme.size.control);
            match choice.kind {
                Weight::AllowOnce | Weight::AllowAlways => button.filled(),
                Weight::RejectOnce | Weight::RejectAlways => button.outlined(),
            }
        })
        .collect::<Vec<_>>();

    v_flex().w_full().px(1.25).pt(0.5).child(
        v_flex()
            .w_full()
            .p(0.75)
            .gap(0.75)
            .rounded(theme.radius.lg)
            .border_1(theme.colors.warning)
            .bg(theme.colors.surface)
            .child(
                text(format!("{BULLET}{}", ask.tool.title))
                    .text_xs()
                    .font_mono()
                    .color(theme.colors.warning),
            )
            .child(h_flex().gap(0.75).children(choices)),
    )
}

/// Builds the box the next prompt is written in, and what it takes.
///
/// Everything a reader sets about the turn they are about to send sits in one
/// card with the box they are typing it in: which model, how hard it thinks,
/// what mode it is in and whether it is sent or stopped. They are facts about
/// the next turn, so they are where the next turn is written and not in a bar
/// at the top of the pane.
fn composer(theme: &Theme, talk: &Talk, typing: bool, solid: bool) -> Div<Message> {
    let id = talk.id();

    v_flex().w_full().px(1.25).pt(0.5).pb(1).child(
        v_flex()
            .w_full()
            .gap(0.5)
            .p(0.75)
            .rounded(theme.radius.lg)
            .border_1(theme.colors.border)
            .bg(theme.colors.surface)
            .when(!talk.attachments().is_empty(), |card| {
                card.child(attachment_list(theme, talk))
            })
            .child(input_view(
                theme,
                talk.prompt(),
                typing,
                solid,
                PROMPT_LINES,
                move |phase, from, to| Message::WriteAgentPrompt(id, phase, from, to),
                Message::ShowInputMenu,
            ))
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
fn controls(theme: &Theme, talk: &Talk) -> Div<Message> {
    let session = talk.id();

    h_flex()
        .w_full()
        .gap(0.5)
        .items_center()
        .child(
            pill(theme, "+", theme.colors.text_muted)
                .on_click(Message::AttachAgentFiles(session))
                .tooltip("Attach files"),
        )
        .child(
            pill(theme, "/", theme.syntax.function).on_click(Message::StartAgentCommand(session)),
        )
        .when(talk.agent().id == "codex", |row| {
            row.child(
                pill(theme, "$", theme.syntax.function).on_click(Message::StartAgentSkill(session)),
            )
        })
        .when(talk.can_list(), |row| {
            row.child(
                pill(theme, "History", theme.colors.text_muted)
                    .on_click(Message::ShowAgentHistory(session)),
            )
        })
        .children(
            talk.knobs()
                .into_iter()
                .enumerate()
                .filter(|(_, knob)| knob.about != About::Mode)
                .map(|(place, knob)| {
                    pill(theme, set_to(&knob), theme.colors.text_muted)
                        .on_click(Message::PressKnob(session, place))
                }),
        )
        .child(h_flex().flex_1())
        .when_some(mode_of(talk), |row, mode| {
            row.child(
                pill(theme, mode, theme.colors.text_muted)
                    .on_click(Message::ShowAgentModes(session)),
            )
        })
        .child(send(theme, talk))
}

/// What the session's mode is called, whichever way the agent says it.
///
/// An agent says its mode as a mode or as a knob that is about the mode; the
/// pill reads the same either way, and it sits where the mode belongs rather
/// than among the model and the rest.
fn mode_of(talk: &Talk) -> Option<String> {
    match talk.mode_name() {
        Some(mode) => Some(mode),
        None => talk.knob_about(About::Mode).map(|knob| set_to(&knob)),
    }
}

/// Builds one of the composer's pills: a label that is also a control.
fn pill(theme: &Theme, label: impl Into<String>, color: Rgba) -> Div<Message> {
    h_flex()
        .h_px(theme.size.icon_control)
        .px(0.75)
        .items_center()
        .rounded(theme.radius.full)
        .bg(theme.colors.surface_hover)
        .hover_bg(theme.colors.surface_active)
        .child(text(label.into()).text_xs().color(color))
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

/// Builds the control that sends the turn, or stops the one that is running.
fn send(theme: &Theme, talk: &Talk) -> Div<Message> {
    let session = talk.id();
    let (name, message) = match talk.is_busy() {
        true => (IconName::Close, Message::StopAgentTurn(session)),
        false => (IconName::ArrowUp, Message::SendPrompt(session)),
    };

    v_flex()
        .size_px(theme.size.icon_control)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .bg(theme.colors.accent)
        .hover_bg(theme.colors.accent_hover)
        .active_bg(theme.colors.accent_active)
        .on_click(message)
        .child(
            icon(name)
                .size(IconSize::Small)
                .color(theme.colors.text_on_accent),
        )
}

/// How many characters of the conversation's type fit across `width`.
fn columns(theme: &Theme, width: f32) -> usize {
    let advance = (theme.text.base.size * ADVANCE).max(1.0);
    ((width / advance) as usize).max(NARROWEST)
}

/// `passage` broken into lines of at most `columns` characters.
///
/// Where a line breaks is where a word ends; a word longer than the pane is
/// wide is broken anyway, because the alternative is a line nobody can read
/// the end of.
fn wrap(passage: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in passage.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            for part in split(word, columns) {
                let width = line.chars().count();
                if width > 0 && width + 1 + part.chars().count() > columns {
                    lines.push(std::mem::take(&mut line));
                } else if width > 0 {
                    line.push(' ');
                }
                line.push_str(&part);
            }
        }
        lines.push(line);
    }
    lines
}

/// `word` in pieces of at most `columns` characters.
fn split(word: &str, columns: usize) -> Vec<String> {
    if word.chars().count() <= columns {
        return vec![word.to_owned()];
    }
    word.chars()
        .collect::<Vec<_>>()
        .chunks(columns)
        .map(|part| part.iter().collect())
        .collect()
}

/// One run of text in one colour.
fn piece(text: String, tone: Tone) -> Piece {
    Piece {
        text,
        tone,
        image: None,
    }
}

/// A reader image shown as a thumbnail inside the message bubble.
fn image_piece(image: Image) -> Piece {
    Piece {
        text: String::new(),
        tone: Tone::Said,
        image: Some(image),
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

/// What the header says the session is doing.
fn doing(talk: &Talk) -> &'static str {
    match (talk.is_running(), talk.is_ready(), talk.is_busy()) {
        (false, ..) => "stopped",
        (_, false, _) => "starting",
        (_, _, true) => "working",
        _ => "ready",
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
