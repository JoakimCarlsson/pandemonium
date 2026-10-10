//! The paragraph element: runs of text in their own faces, wrapped to fit.
//!
//! [`crate::Text`] is one run on one line, which is what a label is. Prose is
//! not: a sentence runs in several faces — a word in bold, a name in the
//! monospaced family — and breaks where the room it is given runs out. The
//! paragraph is measured against the width it is offered, broken between
//! words, and painted from the lines measurement settled on, so what is
//! drawn is exactly what was measured.

use pm_gfx::{FontStyle, Point, Quad, Rect, Rgba, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Style, Styled};
use crate::theme::Font;

/// Thickness of the line drawn under an underlined run.
const UNDERLINE: f32 = 1.0;

/// One run of a paragraph: its text, its face and how it is drawn.
#[derive(Clone, Debug)]
struct Span<M> {
    /// The characters of the run.
    content: String,
    /// The step of the scale and the variations it is set in.
    font: Font,
    /// The colour it is drawn in.
    color: Rgba,
    /// Whether a line is drawn under it, as under a link.
    underline: bool,
    /// What it sits on, when it sits on anything, as inline code does.
    background: Option<Rgba>,
    /// The ordinary click action of a linked span.
    on_click: Option<M>,
}

/// One word or one stretch of space, placed on a line.
#[derive(Clone, Debug)]
struct Piece {
    /// The characters placed.
    content: String,
    /// The face they were measured in.
    font: FontStyle,
    /// Where on the line the piece begins.
    x: f32,
    /// How wide it came out.
    width: f32,
    /// Which span it came from, for its colour and its marks.
    span: usize,
    /// The character boundary in the unwrapped paragraph.
    column: usize,
}

/// One line the paragraph broke into.
#[derive(Clone, Debug, Default)]
struct Line {
    /// The pieces on it, left to right.
    pieces: Vec<Piece>,
    /// How tall the tallest face on it is.
    height: f32,
}

/// Runs of text in their own faces, broken into lines at the width offered.
pub struct Paragraph<M> {
    /// The runs, in reading order.
    spans: Vec<Span<M>>,
    /// How the paragraph is sized and padded.
    style: Style,
    /// The lines the last measurement broke the runs into.
    lines: Vec<Line>,
    /// Whether a word wider than the offered width may break between characters.
    break_long_words: bool,
    /// Whether indentation and spaces survive line wrapping.
    preserve_whitespace: bool,
    /// An explicit copy boundary before this paragraph.
    separator: Option<&'static str>,
}

/// An empty paragraph, to add runs to.
pub fn paragraph<M>() -> Paragraph<M> {
    Paragraph {
        spans: Vec::new(),
        style: Style::default(),
        lines: Vec::new(),
        break_long_words: false,
        preserve_whitespace: false,
        separator: None,
    }
}

impl<M> Paragraph<M> {
    /// Sets the boundary used when copying this paragraph after another.
    pub fn copy_separator(mut self, separator: &'static str) -> Self {
        self.separator = Some(separator);
        self
    }

    /// Allows words wider than the paragraph to break between characters.
    pub fn break_long_words(mut self) -> Self {
        self.break_long_words = true;
        self
    }

    /// Preserves indentation and spaces when displaying wrapped source code.
    pub fn preserve_whitespace(mut self) -> Self {
        self.preserve_whitespace = true;
        self
    }

    /// Returns this paragraph with `content` added in `font` and `color`.
    pub fn span(mut self, content: impl Into<String>, font: Font, color: Rgba) -> Self {
        self.spans.push(Span {
            content: content.into(),
            font,
            color,
            underline: false,
            background: None,
            on_click: None,
        });
        self
    }

    /// Returns this paragraph with `content` added underlined, as a link is.
    pub fn underlined(mut self, content: impl Into<String>, font: Font, color: Rgba) -> Self {
        self = self.span(content, font, color);
        if let Some(span) = self.spans.last_mut() {
            span.underline = true;
        }
        self
    }

    /// Returns this paragraph with `content` added on a wash of `background`,
    /// as inline code is.
    pub fn marked(
        mut self,
        content: impl Into<String>,
        font: Font,
        color: Rgba,
        background: Rgba,
    ) -> Self {
        self = self.span(content, font, color);
        if let Some(span) = self.spans.last_mut() {
            span.background = Some(background);
        }
        self
    }

    /// Gives the last span a click action, preserved when a text press is released.
    pub fn on_span_click(mut self, message: M) -> Self {
        if let Some(span) = self.spans.last_mut() {
            span.on_click = Some(message);
        }
        self
    }

    /// Whether the paragraph holds nothing but space.
    pub fn is_blank(&self) -> bool {
        self.spans.iter().all(|span| span.content.trim().is_empty())
    }

    /// Breaks the runs into lines no wider than `width`.
    ///
    /// A line breaks before the word that would not fit. A wider word stays
    /// whole unless character breaking was requested. The space a line
    /// breaks at is not carried onto the next.
    fn break_lines(&self, width: f32, cx: &mut LayoutContext<'_>) -> Vec<Line> {
        let mut lines = vec![Line::default()];
        let mut x = 0.0;
        let mut column = 0;

        for (index, span) in self.spans.iter().enumerate() {
            let font = span.font.resolve(&cx.theme.text);
            for token in tokens(&span.content) {
                let token_start = column;
                column += token.chars().count();
                if token == "\n" {
                    lines.push(Line::default());
                    x = 0.0;
                    continue;
                }
                let blank = token.trim().is_empty();
                let size = cx.measure(token, font);
                if self.break_long_words
                    && (!blank || self.preserve_whitespace)
                    && size.width > width
                {
                    for (offset, character) in token.chars().enumerate() {
                        let character = character.to_string();
                        let size = cx.measure(&character, font);
                        if x + size.width > width
                            && !lines.last().is_some_and(|line| line.pieces.is_empty())
                        {
                            if !self.preserve_whitespace {
                                trim_trailing_space(
                                    lines.last_mut().expect("there is always a line"),
                                );
                            }
                            lines.push(Line::default());
                            x = 0.0;
                        }
                        let line = lines.last_mut().expect("there is always a line");
                        line.height = line.height.max(font.line_height);
                        line.pieces.push(Piece {
                            content: character,
                            font,
                            x,
                            width: size.width,
                            span: index,
                            column: token_start + offset,
                        });
                        x += size.width;
                    }
                    continue;
                }
                let line = lines.last_mut().expect("there is always a line");
                if blank && line.pieces.is_empty() && !self.preserve_whitespace {
                    line.height = line.height.max(font.line_height);
                    continue;
                }
                if (!blank || self.preserve_whitespace)
                    && x + size.width > width
                    && !line.pieces.is_empty()
                {
                    if !self.preserve_whitespace {
                        trim_trailing_space(line);
                    }
                    lines.push(Line::default());
                    x = 0.0;
                }
                let line = lines.last_mut().expect("there is always a line");
                line.height = line.height.max(font.line_height);
                line.pieces.push(Piece {
                    content: token.to_owned(),
                    font,
                    x,
                    width: size.width,
                    span: index,
                    column: token_start,
                });
                x += size.width;
            }
        }
        if !self.preserve_whitespace {
            for line in &mut lines {
                trim_trailing_space(line);
            }
        }
        lines
    }
}

impl<M> Styled for Paragraph<M> {
    /// How the paragraph is sized and padded.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: Clone> Element<M> for Paragraph<M> {
    /// How the paragraph is sized and padded.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Breaks the runs at the width offered and reports how tall that is.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let inner = (available.width - self.style.padding.horizontal()).max(1.0);
        self.lines = self.break_lines(inner, cx);
        let widest = self
            .lines
            .iter()
            .filter_map(|line| line.pieces.last().map(|piece| piece.x + piece.width))
            .fold(0.0_f32, f32::max);
        let height = self.lines.iter().map(|line| line.height).sum::<f32>();
        Size::new(
            match self.style.width {
                crate::Length::Px(pixels) => pixels,
                crate::Length::Full => available.width,
                crate::Length::Auto => widest + self.style.padding.horizontal(),
            },
            height + self.style.padding.vertical(),
        )
    }

    /// Paints every line measurement settled on, a run at a time.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let left = bounds.left() + self.style.padding.left;
        let mut top = bounds.top() + self.style.padding.top;

        let content = self
            .spans
            .iter()
            .map(|span| span.content.as_str())
            .collect::<String>();
        let row = cx.selection_row(content, bounds, self.separator);
        for line in &self.lines {
            for piece in &line.pieces {
                let span = &self.spans[piece.span];
                let run = cx.shape(&piece.content, piece.font);
                let drop = line.height - piece.font.line_height;
                let origin = Point::new(left + piece.x, top + drop);
                if let Some(background) = span.background {
                    cx.quad(Quad::filled(
                        Rect::from_xywh(origin.x, origin.y, piece.width, piece.font.line_height),
                        background,
                    ));
                }
                if span.underline && !piece.content.trim().is_empty() {
                    cx.quad(Quad::filled(
                        Rect::from_xywh(
                            origin.x,
                            origin.y + run.baseline + UNDERLINE * 2.0,
                            piece.width,
                            UNDERLINE,
                        ),
                        span.color,
                    ));
                }
                if let Some(message) = &span.on_click {
                    cx.interactive(
                        Rect::from_xywh(origin.x, origin.y, run.width, run.height),
                        message.clone(),
                    );
                }
                cx.selectable_run(
                    &piece.content,
                    origin,
                    &run,
                    row.map(|row| crate::Spot {
                        row,
                        column: piece.column,
                    }),
                );
                cx.text(origin, run, span.color);
            }
            top += line.height;
        }
    }
}

/// `content` cut into words, the stretches of space between them and the
/// line breaks it holds, in order.
fn tokens(content: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut blank = None;
    for (at, ch) in content.char_indices() {
        if ch == '\n' {
            if start < at {
                tokens.push(&content[start..at]);
            }
            tokens.push("\n");
            start = at + 1;
            blank = None;
            continue;
        }
        let is_blank = ch.is_whitespace();
        if blank.is_some_and(|was| was != is_blank) {
            tokens.push(&content[start..at]);
            start = at;
        }
        blank = Some(is_blank);
    }
    if start < content.len() {
        tokens.push(&content[start..]);
    }
    tokens
}

/// Takes the space off the end of `line`, which only pushed it wider.
fn trim_trailing_space(line: &mut Line) {
    while line
        .pieces
        .last()
        .is_some_and(|piece| piece.content.trim().is_empty())
    {
        line.pieces.pop();
    }
}
