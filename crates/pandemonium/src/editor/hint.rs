//! The small panel that says what the editor knows about one place.
//!
//! A hover, a signature and the text of a diagnostic are three things a
//! server says about where the cursor is and one way of showing them: a
//! panel beside the place, holding a few lines of plain text. It is not a
//! document view — a hover that runs to forty lines is a hover the reader
//! reads the top of and then goes to the definition.
//!
//! [`Shown`] is what the panel is drawn from. A fault the editor already
//! knows about goes up the moment the pointer stops on it, and what the
//! server says about the same place joins it when the answer arrives, so
//! pointing at a squiggle does not hide the type it is on.

use std::ops::Range;

use pm_gfx::{Point, Rgba};
use pm_text::{Language, Position};
use pm_ui::{
    Bounds, Div, Measured, Scrolled, Styled, Theme, h_flex, measured, scroll_area, text, v_flex,
};

use crate::editor::FileId;
use crate::editor::view::tint;
use crate::message::Message;

/// Widest the panel is drawn.
const WIDTH: f32 = 640.0;

/// How much of the panel's width its padding and border take.
const INSET: f32 = 16.0;

/// How wide one character of the monospaced family is, against its size.
///
/// The panel is built before anything is shaped, so lines are wrapped
/// against the advance monospaced families share rather than a measured
/// one; erring wide wraps a character early instead of cutting one off.
const ADVANCE: f32 = 0.62;

/// How many columns a tab stands for in code a server wrote.
const TAB: usize = 4;

/// Most lines of it shown at once; the rest are scrolled to.
const LINES: usize = 16;

/// What is being said about one place, and where it is being said.
#[derive(Clone, Debug, Default)]
pub struct Shown {
    /// Where the panel is drawn.
    pub at: Point,
    /// The fault under the place, which the editor knew without asking.
    pub fault: Option<String>,
    /// What a server said about it, once it has said anything.
    pub said: Option<String>,
    /// The word it is about, while it is the pointer that is asking.
    ///
    /// A panel about a word stays up for as long as the pointer is still on
    /// that word or on the panel itself: reading a hover means crossing the
    /// name it is about, and scrolling one means resting on it.
    pub about: Option<(FileId, Range<Position>)>,
    /// The language of the file it is about, for code a server did not tag.
    pub language: Option<Language>,
    /// How far what it says is scrolled, when it says more than fits.
    pub scroll: Scrolled,
    /// Where the panel was drawn last frame.
    pub bounds: Bounds,
}

impl Shown {
    /// A panel at `at`, with nothing in it yet.
    pub fn at(at: Point) -> Self {
        Self {
            at,
            ..Self::default()
        }
    }

    /// Everything the panel has to say, the fault first.
    pub fn text(&self) -> String {
        [self.fault.as_deref(), self.said.as_deref()]
            .into_iter()
            .flatten()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Whether there is nothing to show yet.
    pub fn is_empty(&self) -> bool {
        self.text().trim().is_empty()
    }

    /// Whether `point` is over the panel as it was last drawn.
    pub fn covers(&self, point: Point) -> bool {
        !self.is_empty() && self.bounds.get().contains(point)
    }

    /// Scrolls what the panel says by `delta` logical pixels.
    pub fn scroll_by(&self, delta: f32) {
        let mut scroll = self.scroll.get();
        scroll.by(delta);
        self.scroll.set(scroll);
    }
}

/// One line of the panel, as runs of text in the colour each is drawn in.
type Line = Vec<(String, Rgba)>;

/// Builds the panel saying what `shown` holds, beside the place it is about.
///
/// What runs past [`LINES`] is scrolled to rather than cut off, and the
/// panel records where it was drawn so that the pointer can move onto it
/// without it going away.
pub fn hint(theme: &Theme, shown: &Shown) -> Measured<Message> {
    let columns = ((WIDTH - INSET) / (theme.text.sm.size * ADVANCE)).floor() as usize;
    let rows = lines(theme, &shown.text(), shown.language)
        .into_iter()
        .flat_map(|line| wrap(line, columns.max(1)))
        .map(row)
        .collect::<Vec<_>>();
    let overflows = rows.len() > LINES;
    let body = v_flex().items_stretch().children(rows);
    let body = match overflows {
        true => v_flex().child(
            scroll_area(shown.scroll.clone(), body)
                .w_full()
                .h_px(LINES as f32 * theme.text.sm.line_height),
        ),
        false => body,
    };

    measured(
        shown.bounds.clone(),
        v_flex()
            .max_w_px(WIDTH)
            .px(1.5)
            .py(1)
            .items_stretch()
            .overflow_hidden()
            .bg(theme.colors.surface)
            .border_1(theme.colors.border)
            .rounded(theme.radius.md)
            .on_click(Message::DismissPopup)
            .child(body),
    )
}

/// One line of the panel, its runs set side by side.
fn row(line: Line) -> Div<Message> {
    let runs = match line.is_empty() {
        true => vec![(String::from(" "), Rgba::TRANSPARENT)],
        false => line,
    };
    h_flex().children(
        runs.into_iter()
            .map(|(run, color)| text(run).text_sm().font_mono().color(color)),
    )
}

/// What a server said, as the lines the panel draws.
///
/// It is written in markdown, and only the parts that are read as more than
/// themselves are acted on: a fenced block is code, coloured the way its
/// language is coloured in a file, and the fences, the rules between one
/// section and the next and the blank lines those leave behind go.
fn lines(theme: &Theme, content: &str, language: Option<Language>) -> Vec<Line> {
    let prose = theme.colors.text_muted;
    let mut out = Vec::new();
    let mut fenced: Option<(Option<Language>, Vec<&str>)> = None;

    for line in content.lines().map(str::trim_end) {
        if let Some(tag) = line.trim_start().strip_prefix("```") {
            match fenced.take() {
                Some((fence, code)) => out.extend(code_lines(theme, fence, &code)),
                None => {
                    let fence = match tag.trim().is_empty() {
                        true => language,
                        false => Language::fenced(tag),
                    };
                    fenced = Some((fence, Vec::new()));
                }
            }
            continue;
        }
        match fenced.as_mut() {
            Some((_, code)) => code.push(line),
            None if is_rule(line) => {}
            None => out.push(vec![(plain(line), prose)]),
        }
    }
    if let Some((fence, code)) = fenced {
        out.extend(code_lines(theme, fence, &code));
    }
    collapse_blanks(out)
}

/// The lines of one fenced block, each coloured by its language's grammar.
///
/// A block in a language the editor has no grammar for is drawn the way
/// the text of a file with no grammar is drawn: in the plain text colour.
fn code_lines(theme: &Theme, language: Option<Language>, code: &[&str]) -> Vec<Line> {
    let highlights = language.map(|language| pm_text::highlight(language, &code.join("\n")));
    code.iter()
        .enumerate()
        .map(|(number, line)| {
            let mut runs: Line = Vec::new();
            let mut drawn = String::new();
            for (column, ch) in line.chars().enumerate() {
                let color = highlights
                    .as_ref()
                    .and_then(|found| found.at(number, column))
                    .map_or(theme.colors.text, |highlight| tint(highlight, theme));
                drawn.clear();
                match ch {
                    '\t' => drawn.extend(std::iter::repeat_n(' ', TAB)),
                    ch => drawn.push(ch),
                }
                match runs.last_mut() {
                    Some((run, last)) if *last == color => run.push_str(&drawn),
                    _ => runs.push((drawn.clone(), color)),
                }
            }
            runs
        })
        .collect()
}

/// A line of markdown prose as it reads: links as their text, escapes and
/// the backticks around inline code taken off.
fn plain(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' if chars.peek().is_some_and(char::is_ascii_punctuation) => {
                out.extend(chars.next());
            }
            '`' => {}
            ']' if chars.peek() == Some(&'(') => {
                for skipped in chars.by_ref() {
                    if skipped == ')' {
                        break;
                    }
                }
            }
            '[' => {}
            ch => out.push(ch),
        }
    }
    out
}

/// `line` broken into lines of at most `columns` characters.
///
/// A line breaks at the last space that fits, or where it runs out of room
/// when a word is longer than the panel is wide, and what it carries onto
/// the next line is indented as far as the line itself was, so wrapped code
/// still reads as the block it is in.
fn wrap(line: Line, columns: usize) -> Vec<Line> {
    let chars = line
        .iter()
        .flat_map(|(run, color)| run.chars().map(move |ch| (ch, *color)))
        .collect::<Vec<_>>();
    if chars.len() <= columns {
        return vec![line];
    }
    let indent = chars
        .iter()
        .take_while(|(ch, _)| *ch == ' ')
        .count()
        .min(columns / 2);

    let mut out = Vec::new();
    let mut rest = chars.as_slice();
    let mut first = true;
    while !rest.is_empty() {
        let room = if first { columns } else { columns - indent };
        if rest.len() <= room {
            out.push(runs_of(rest, if first { 0 } else { indent }));
            break;
        }
        let cut = rest[..=room]
            .iter()
            .rposition(|(ch, _)| *ch == ' ')
            .filter(|at| *at > 0 && (!first || *at > indent))
            .unwrap_or(room);
        out.push(runs_of(&rest[..cut], if first { 0 } else { indent }));
        rest = &rest[cut..];
        let spaces = rest.iter().take_while(|(ch, _)| *ch == ' ').count();
        rest = &rest[spaces..];
        first = false;
    }
    out
}

/// `chars` gathered back into runs of one colour, after `indent` spaces.
fn runs_of(chars: &[(char, Rgba)], indent: usize) -> Line {
    let mut runs: Line = Vec::new();
    if indent > 0 {
        runs.push((" ".repeat(indent), Rgba::TRANSPARENT));
    }
    for (ch, color) in chars {
        match runs.last_mut() {
            Some((run, last)) if last == color => run.push(*ch),
            _ => runs.push((ch.to_string(), *color)),
        }
    }
    runs
}

/// `lines` with no two blank lines together, and none at either end.
fn collapse_blanks(lines: Vec<Line>) -> Vec<Line> {
    let blank = |line: &Line| line.iter().all(|(run, _)| run.trim().is_empty());
    let mut out: Vec<Line> = Vec::new();
    for line in lines {
        if blank(&line) && out.last().is_none_or(blank) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(blank) {
        out.pop();
    }
    out
}

/// Whether `line` is a rule between sections rather than something to read.
fn is_rule(line: &str) -> bool {
    let line = line.trim();
    line.len() >= 3 && line.chars().all(|ch| matches!(ch, '-' | '_' | '*'))
}
