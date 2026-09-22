//! One agent session, in a pane: what was said, what it is doing, what next.
//!
//! The pane is the agent's own transcript, drawn the way the agent's own CLI
//! draws it: one monospaced column, a bullet against everything the agent
//! says, a tool call named with its argument beside it and its result on the
//! line beneath. Nothing is folded away behind a control, because a reader
//! conducting four of these wants to see what the fourth one did without
//! opening anything.
//!
//! Above it is which agent, which worktree and how it is doing; below it is
//! the box the next prompt is written in. A permission the agent is waiting on
//! sits between the two, because it is the one thing that stops everything
//! else until it is answered.
//!
//! A pane is as wide as the window made it and the text is wrapped to fit, so
//! the width the last frame came out at is what this one is built against.

use std::path::Path;

use pm_acp::{Ask, Output, Status, Step, ToolCall, Voice, Weight};
use pm_gfx::Rgba;
use pm_ui::{Div, Styled, Theme, button, h_flex, rule, text, v_flex};

use crate::agent::{Block, SessionId, Talk};
use crate::editor::plain_view;
use crate::message::Message;

/// How many rows are built at once, however long the conversation runs.
const DRAWN: usize = 300;

/// How many lines of one tool call's result are shown before the rest.
const RESULT_LINES: usize = 8;

/// How many lines of the prompt the pane has room for.
const PROMPT_LINES: f32 = 3.0;

/// How many of the commands a slash narrows to are offered at once.
const OFFERED: usize = 8;

/// How wide one character of the conversation's type is, as a share of its
/// size.
///
/// The conversation is set in the monospaced face, whose characters are all
/// this wide; the pane wraps to what it was drawn at last frame rather than
/// asking the shaper, because a paragraph is wrapped before it is measured.
const ADVANCE: f32 = 0.6;

/// Fewest characters a line is wrapped at, however narrow the pane is.
const NARROWEST: usize = 24;

/// What stands against everything the agent says.
const BULLET: &str = "● ";

/// What stands against what the reader said.
const CHEVRON: &str = "> ";

/// What a tool call's result hangs from.
const RESULT: &str = "  └ ";

/// What a line continuing the one above it is indented by.
const WRAPPED: &str = "  ";

/// What a turn that is still running is marked with.
const WORKING: &str = "◐ ";

/// What the rule above a conversation begins with.
const RULED: &str = "───";

/// The colour a piece of a row is drawn in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tone {
    /// The rule above the conversation.
    Rule,
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
    /// A tool call that came to something.
    Done,
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
}

/// One line of the conversation, in the pieces it is coloured by.
type Row = Vec<Piece>;

/// Builds the pane showing `talk`, wrapped to `width` logical pixels.
///
/// `typing` says the prompt box has the keyboard, so that the caret is drawn
/// where the reader is actually writing.
pub fn agent_pane(theme: &Theme, talk: &Talk, typing: bool, width: f32) -> Div<Message> {
    let columns = columns(theme, width);

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(header(theme, talk))
        .child(rule(theme))
        .child(
            v_flex()
                .w_full()
                .flex_1()
                .overflow_hidden()
                .px(1.75)
                .py(1)
                .children(drawn(theme, talk, columns)),
        )
        .children(
            talk.asks()
                .iter()
                .flat_map(|ask| [rule(theme), permission(theme, talk.id(), ask)]),
        )
        .child(rule(theme))
        .when(!talk.offered().is_empty(), |pane| {
            pane.child(commands(theme, talk))
        })
        .child(prompt_bar(theme, talk, typing))
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
                .px(1.75)
                .py(0.25)
                .gap(1)
                .items_center()
                .hover_bg(theme.colors.surface_hover)
                .when(place == chosen, |row| row.bg(theme.colors.surface_selected))
                .on_click(Message::TakeAgentCommand(session, place))
                .child(
                    text(format!("/{}", command.name))
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

    v_flex()
        .w_full()
        .py(0.5)
        .bg(theme.colors.surface)
        .children(rows)
}

/// The first line of `said`, which is as much of it as a row has room for.
fn first_line(said: &str) -> String {
    said.lines().next().unwrap_or_default().to_owned()
}

/// How many rows the conversation comes to at `width` logical pixels.
///
/// The window asks this to know how far the pane can be scrolled, which only
/// the rows can say.
pub fn row_count(theme: &Theme, talk: &Talk, width: f32) -> usize {
    rows(talk, columns(theme, width)).len()
}

/// The rows of the pane, from where it is scrolled to.
fn drawn(theme: &Theme, talk: &Talk, columns: usize) -> Vec<Div<Message>> {
    rows(talk, columns)
        .into_iter()
        .skip(talk.scroll())
        .take(DRAWN)
        .map(|row| self::row(theme, row))
        .collect()
}

/// Every row the conversation comes to, wrapped at `columns` characters.
fn rows(talk: &Talk, columns: usize) -> Vec<Row> {
    let mut rows = vec![opening(talk, columns)];
    for block in talk.transcript().blocks() {
        rows.push(Row::new());
        match block {
            Block::Said(Voice::Reader, passage) => {
                rows.extend(passage_rows(passage, CHEVRON, Tone::Said, columns));
            }
            Block::Said(Voice::Agent, passage) => {
                rows.extend(passage_rows(passage, BULLET, Tone::Spoken, columns));
            }
            Block::Said(Voice::Thought, passage) => {
                rows.extend(passage_rows(passage, BULLET, Tone::Quiet, columns));
            }
            Block::Ran(call) => rows.extend(tool_rows(call, talk.root(), columns)),
            Block::Planned(steps) => rows.extend(steps.iter().map(step_row)),
            Block::Note(note) => rows.extend(passage_rows(note, BULLET, Tone::Note, columns)),
        }
    }
    if talk.is_busy() {
        rows.push(Row::new());
        rows.push(vec![piece(
            format!("{WORKING}Working… (esc to interrupt)"),
            Tone::Quiet,
        )]);
    }
    rows
}

/// The rule the conversation opens with: which agent, over which worktree.
fn opening(talk: &Talk, columns: usize) -> Row {
    let worktree = name_of(talk.root());
    let said = match talk.mode() {
        Some(mode) => format!("{RULED} {} ── {worktree} ── {mode} ", talk.agent().name),
        None => format!("{RULED} {} ── {worktree} ", talk.agent().name),
    };
    let over = columns.saturating_sub(said.chars().count());
    vec![piece(said + &"─".repeat(over), Tone::Rule)]
}

/// One passage, as rows marked with `mark` and wrapped to the width.
fn passage_rows(passage: &str, mark: &str, tone: Tone, columns: usize) -> Vec<Row> {
    wrap(passage, columns.saturating_sub(mark.chars().count()))
        .into_iter()
        .enumerate()
        .map(|(at, line)| match at {
            0 => vec![piece(mark.to_owned(), quieten(tone)), piece(line, tone)],
            _ => vec![piece(WRAPPED.to_owned(), tone), piece(line, tone)],
        })
        .collect()
}

/// One tool call, as the line naming it and the lines of what it came to.
fn tool_rows(call: &ToolCall, root: &Path, columns: usize) -> Vec<Row> {
    let tone = match call.status {
        Status::Failed => Tone::Failed,
        Status::Done => Tone::Done,
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
    let mut row = vec![piece(BULLET.to_owned(), Tone::Quiet)];
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
            Status::Done => vec![call.title.clone()],
            Status::Failed => vec!["failed".to_owned()],
        };
    }

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
fn row(theme: &Theme, row: Row) -> Div<Message> {
    h_flex().children(row.into_iter().map(|piece| {
        text(piece.text)
            .text_xs()
            .font_mono()
            .color(tone(theme, piece.tone))
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
        .child(text("●").text_xs().color(standing(theme, talk)))
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
        .when_some(talk.mode().map(str::to_owned), |bar, mode| {
            bar.child(chip(theme, mode))
        })
        .child(chip(theme, doing(talk)))
}

/// Builds one of the header's chips.
fn chip(theme: &Theme, label: impl Into<String>) -> Div<Message> {
    h_flex()
        .px(0.5)
        .items_center()
        .rounded(theme.radius.sm)
        .border_1(theme.colors.border)
        .child(
            text(label.into())
                .text_xs()
                .font_mono()
                .color(theme.colors.text_muted),
        )
}

/// Builds the card asking whether the agent may do what it is asking about.
fn permission(theme: &Theme, session: SessionId, ask: &Ask) -> Div<Message> {
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

    v_flex()
        .w_full()
        .px(1.75)
        .py(1)
        .gap(0.75)
        .bg(theme.colors.surface)
        .child(
            text(format!("{BULLET}{}", ask.tool.title))
                .text_xs()
                .font_mono()
                .color(theme.colors.warning),
        )
        .child(h_flex().gap(0.75).children(choices))
}

/// Builds the box the next prompt is written in, and what it takes.
fn prompt_bar(theme: &Theme, talk: &Talk, typing: bool) -> Div<Message> {
    let id = talk.id();

    v_flex()
        .w_full()
        .px(1.75)
        .py(1)
        .gap(0.5)
        .bg(theme.colors.background)
        .child(
            h_flex()
                .w_full()
                .h_px(theme.size.control * PROMPT_LINES)
                .px(1)
                .py(0.5)
                .gap(0.75)
                .overflow_hidden()
                .rounded(theme.radius.md)
                .border_1(match typing {
                    true => theme.colors.border_focused,
                    false => theme.colors.border,
                })
                .child(
                    text(">")
                        .text_xs()
                        .font_mono()
                        .color(theme.colors.text_subtle),
                )
                .child(plain_view(talk.prompt(), typing).caret(typing).on_select(
                    move |phase, from, to| Message::WriteAgentPrompt(id, phase, from, to),
                )),
        )
        .child(
            h_flex()
                .gap(2)
                .children(hints(talk).into_iter().map(|hint| {
                    text(hint)
                        .text_xs()
                        .font_mono()
                        .color(theme.colors.text_subtle)
                })),
        )
}

/// What the line under the prompt says the keyboard does.
fn hints(talk: &Talk) -> [&'static str; 3] {
    if !talk.offered().is_empty() {
        return ["↑↓ choose", "⏎ take the command", "esc leave the prompt"];
    }
    match talk.is_busy() {
        true => ["⏎ send", "⇧⏎ newline", "esc interrupt"],
        false => ["⏎ send", "⇧⏎ newline", "esc leave the prompt"],
    }
}

/// How many characters of the conversation's type fit across `width`.
fn columns(theme: &Theme, width: f32) -> usize {
    let advance = (theme.text.code.size * ADVANCE).max(1.0);
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
    Piece { text, tone }
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
        Tone::Rule => theme.colors.border_selected,
        Tone::Said => theme.colors.text,
        Tone::Spoken => theme.colors.text_muted,
        Tone::Quiet => theme.colors.text_subtle,
        Tone::Tool => theme.syntax.function,
        Tone::Argument => theme.syntax.string,
        Tone::Done => theme.colors.success,
        Tone::Failed => theme.colors.danger,
        Tone::Note => theme.colors.warning,
    }
}

/// The colour the header's mark is drawn in, for how the session is doing.
fn standing(theme: &Theme, talk: &Talk) -> Rgba {
    match (talk.is_running(), talk.asks().is_empty(), talk.is_busy()) {
        (false, ..) => theme.colors.danger,
        (_, false, _) => theme.colors.warning,
        (_, _, true) => theme.colors.success,
        _ => theme.colors.text_subtle,
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
