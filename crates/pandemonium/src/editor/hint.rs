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
use pm_text::{Highlight, Language, Position};
use pm_ui::{
    Bounds, Font, Measured, Scrolled, Styled, TextSize, Theme, measured, paragraph, scroll_area,
    v_flex,
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
    /// The signature of the call the cursor is inside, when that is what
    /// is being said, with the parameter being written lit.
    pub signature: Option<pm_text::Signature>,
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
        self.text().trim().is_empty() && self.signature.is_none()
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

/// One line of code, as runs of text in the colour each is drawn in.
pub type Line = Vec<(String, Rgba)>;

/// Builds the panel saying what `shown` holds, beside the place it is about.
///
/// What runs past [`LINES`] is scrolled to rather than cut off, and the
/// panel records where it was drawn so that the pointer can move onto it
/// without it going away.
pub fn hint(theme: &Theme, shown: &Shown) -> Measured<Message> {
    let columns = ((WIDTH - INSET) / (theme.text.sm.size * ADVANCE)).floor() as usize;
    let mut said = shown
        .signature
        .as_ref()
        .map(|signature| signature_lines(theme, signature, shown.language))
        .unwrap_or_default();
    said.extend(lines(theme, &shown.text(), shown.language));
    let height = said
        .iter()
        .map(|line| {
            line.iter()
                .map(|(run, _)| run.chars().count())
                .sum::<usize>()
                .max(1)
                .div_ceil(columns.max(1))
        })
        .sum::<usize>()
        .clamp(1, LINES) as f32
        * theme.text.sm.line_height;
    let rows = said.into_iter().map(|line| {
        let line = if line.is_empty() {
            vec![(String::from(" "), Rgba::TRANSPARENT)]
        } else {
            line
        };
        line.into_iter().fold(
            paragraph().break_long_words().copy_separator("\n"),
            |row, (run, color)| row.span(run, Font::new(TextSize::Sm).mono(), color),
        )
    });
    let body = scroll_area(
        shown.scroll.clone(),
        v_flex().items_stretch().children(rows),
    )
    .selectable()
    .w_full()
    .h_px(height);

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

/// A signature as the panel draws it: the signature itself, the parameter
/// being written lit in the accent colour, and what the server says about
/// that parameter and the call beneath it.
fn signature_lines(
    theme: &Theme,
    signature: &pm_text::Signature,
    language: Option<Language>,
) -> Vec<Line> {
    let label = signature.label.chars().collect::<Vec<_>>();
    let active = signature
        .active
        .clone()
        .filter(|span| span.start <= span.end && span.end <= label.len())
        .unwrap_or(0..0);
    let run = |span: Range<usize>| label[span].iter().collect::<String>();
    let mut first: Line = vec![(run(0..active.start), theme.colors.text)];
    if !active.is_empty() {
        first.push((run(active.clone()), theme.colors.accent));
    }
    first.push((run(active.end..label.len()), theme.colors.text));
    first.retain(|(text, _)| !text.is_empty());
    let notes = [
        signature.parameter.as_str(),
        signature.documentation.as_str(),
    ]
    .into_iter()
    .filter(|note| !note.is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");
    let mut out = vec![first];
    if !notes.is_empty() {
        out.push(Vec::new());
        out.extend(lines(theme, &notes, language));
    }
    out
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
/// A rendered document's code blocks are coloured by the same hand.
pub fn code_lines(theme: &Theme, language: Option<Language>, code: &[&str]) -> Vec<Line> {
    code_highlights(language, code)
        .into_iter()
        .map(|line| {
            let mut runs: Line = Vec::new();
            for (run, highlight) in line {
                let color = highlight.map_or(theme.colors.text, |highlight| tint(highlight, theme));
                match runs.last_mut() {
                    Some((drawn, last)) if *last == color => drawn.push_str(&run),
                    _ => runs.push((run, color)),
                }
            }
            runs
        })
        .collect()
}

/// The lines of one fenced block as runs of what each character is to its
/// language's grammar, before any theme has said what colour that is.
///
/// A tab is set out as the spaces it stands for, so a column counted in the
/// runs is a column drawn on screen.
pub fn code_highlights(
    language: Option<Language>,
    code: &[&str],
) -> Vec<Vec<(String, Option<Highlight>)>> {
    let highlights = language.map(|language| pm_text::highlight(language, &code.join("\n")));
    code.iter()
        .enumerate()
        .map(|(number, line)| {
            let mut runs: Vec<(String, Option<Highlight>)> = Vec::new();
            let mut drawn = String::new();
            for (column, ch) in line.chars().enumerate() {
                let highlight = highlights
                    .as_ref()
                    .and_then(|found| found.at(number, column));
                drawn.clear();
                match ch {
                    '\t' => drawn.extend(std::iter::repeat_n(' ', TAB)),
                    ch => drawn.push(ch),
                }
                match runs.last_mut() {
                    Some((run, last)) if *last == highlight => run.push_str(&drawn),
                    _ => runs.push((drawn.clone(), highlight)),
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
