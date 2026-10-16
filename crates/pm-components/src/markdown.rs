//! Markdown: a source laid out as paragraphs, headings, lists, code, quotes, fold-outs, rules and tables.

use std::hash::{DefaultHasher, Hash, Hasher};

use pm_icon::IconName;
use pm_input::FocusId;
use pm_markdown::{Block, Inline, Span};
use pm_style::{
    Radius, Role, Sides, Style, Utility, bg, border, border_color, border_t, border_width,
    cursor_pointer, flex, flex_1, flex_col, flex_none, font_mono, font_semibold, gap, gap_x, h_px,
    italic, items_center, leading_ratio, line_through, min_w_px, ml, my, overflow_hidden, padding,
    pl, px, py, rounded, rounded_full, size, text_color, text_px, underline, w_full, w_px,
};
use pm_text::Shaper;
use pm_ui::{Node, node, retained, text};

use crate::CodeBox;
use crate::parts::{glyph, line};

/// The line height of running text, as a share of its size.
const LEADING: f32 = 1.57;

/// The line height of inline code, as a share of the size of the text around it.
const CODE_LEADING: f32 = 1.35;

/// How far list items are set in from their markers, in logical pixels.
const INDENT: f32 = 22.0;

/// The room inline code keeps on each side of its text, in logical pixels.
const CODE_PAD: f32 = 4.0;

/// How far a quote is set in past its bar, in logical pixels.
const QUOTE_INDENT: f32 = 16.0;

/// A height no code block reaches, so none is cut off.
const UNCUT: f32 = 1.0e6;

/// A markdown source being described: what it says, how wide it is set and in what type.
///
/// Build it with [`Markdown::new`], change it with [`Markdown::size`] and
/// [`Markdown::tone`], and turn it into a node with [`Markdown::build`].
#[derive(Clone, Debug)]
pub struct Markdown<'a> {
    /// The markdown.
    source: &'a str,
    /// How wide the text is set, in logical pixels.
    width: f32,
    /// How tall running text is, in logical pixels.
    pixels: u32,
    /// The colour of running text.
    tone: Role,
    /// The name of the first fold-out's summary, the rest following it, when they can be pressed.
    details: Option<u64>,
    /// The fold-outs that are open, by their place in the source; all are when none can be pressed.
    unfolded: Vec<usize>,
    /// The name of the first link, the rest following it in the order [`pm_markdown::links`] gives their addresses, when they can be pressed.
    links: Option<u64>,
}

impl<'a> Markdown<'a> {
    /// `source` set `width` logical pixels wide, in 14 pixel type in the text colour.
    pub fn new(source: &'a str, width: f32) -> Self {
        Self {
            source,
            width,
            pixels: 14,
            tone: Role::Text,
            details: None,
            unfolded: Vec::new(),
            links: None,
        }
    }

    /// The text set in `pixels` tall type, headings and code scaled from it.
    pub fn size(mut self, pixels: u32) -> Self {
        self.pixels = pixels;
        self
    }

    /// Running text drawn in `tone`.
    pub fn tone(mut self, tone: Role) -> Self {
        self.tone = tone;
        self
    }

    /// Links that can be pressed, the first answering to `base` and the rest following it in the order [`pm_markdown::links`] gives their addresses.
    pub fn links(mut self, base: u64) -> Self {
        self.links = Some(base);
        self
    }

    /// Fold-outs whose summaries are named from `base` up, in the order the
    /// source has them, closed but for those placed in `unfolded`.
    pub fn details(mut self, base: u64, unfolded: Vec<usize>) -> Self {
        self.details = Some(base);
        self.unfolded = unfolded;
        self
    }

    /// The node that draws the source, its lines broken as `shaper` measures
    /// them. The source is parsed and laid out once for as long as nothing
    /// about it changes, and each block once for as long as it stays the
    /// same, so a reply that grows only lays out its last block again.
    pub fn build(self, shaper: &mut Shaper) -> Node {
        let width = self.width;
        let whole = hash((
            self.source,
            self.pixels,
            self.tone,
            self.details,
            &self.unfolded,
            self.links,
        ));
        retained(whole, width, shaper, |shaper| {
            let blocks = pm_markdown::parse(self.source);
            let mut seen = 0;
            node()
                .styles([flex(), flex_col(), gap(3)])
                .children(blocks.iter().map(|block| {
                    let first = seen;
                    seen += count_details(std::slice::from_ref(block));
                    let unfolded: Vec<usize> = self
                        .unfolded
                        .iter()
                        .copied()
                        .filter(|place| (first..seen).contains(place))
                        .collect();
                    let key = hash((
                        block,
                        self.pixels,
                        self.tone,
                        self.details,
                        first,
                        &unfolded,
                        self.links,
                    ));
                    retained(key, width, shaper, |shaper| {
                        Layout {
                            pixels: self.pixels,
                            shaper,
                            details: self.details,
                            unfolded,
                            seen: first,
                            links: self.links,
                        }
                        .block(block, self.tone, width)
                    })
                }))
        })
    }
}

/// `value` hashed, to tell one retained subtree from another.
fn hash(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// One pass of laying blocks out.
struct Layout<'s> {
    /// How tall running text is, in logical pixels.
    pixels: u32,
    /// Measures the text as it will be drawn.
    shaper: &'s mut Shaper,
    /// The name of the first fold-out's summary, when they can be pressed.
    details: Option<u64>,
    /// The fold-outs that are open, by their place in the source.
    unfolded: Vec<usize>,
    /// How many fold-outs were laid out or passed over so far.
    seen: usize,
    /// The name of the first link, when links can be pressed.
    links: Option<u64>,
}

impl Layout<'_> {
    /// How tall one line of running text is, in logical pixels.
    fn line_height(&self) -> f32 {
        self.pixels as f32 * LEADING
    }

    /// `blocks` one under the other, `width` wide, running text in `tone`.
    fn blocks(&mut self, blocks: &[Block], tone: Role, width: f32) -> Node {
        node()
            .styles([flex(), flex_col(), gap(3)])
            .children(blocks.iter().map(|block| self.block(block, tone, width)))
    }

    /// One block, `width` wide, running text in `tone`.
    fn block(&mut self, block: &Block, tone: Role, width: f32) -> Node {
        match block {
            Block::Paragraph(spans) => self.text(spans, self.pixels, false, tone, width),
            Block::Heading { level, spans } => {
                let pixels = match level {
                    1 => self.pixels + 6,
                    2 => self.pixels + 3,
                    3 => self.pixels + 1,
                    _ => self.pixels,
                };
                self.text(spans, pixels, true, tone, width)
            }
            Block::List { start, items } => self.list(*start, items, tone, width),
            Block::Code { text, .. } => self.code(text, width),
            Block::Quote(blocks) => node()
                .styles([
                    flex(),
                    border_width(Sides::Left, 2.0),
                    border_color(Role::Border),
                    pl(QUOTE_INDENT / 4.0 - 0.5),
                ])
                .child(self.blocks(blocks, Role::Muted, width - QUOTE_INDENT)),
            Block::Details { summary, blocks } => self.fold_out(summary, blocks, tone, width),
            Block::Rule => node().styles([h_px(1), w_full(), my(1), bg(Role::Border)]),
            Block::Table { head, rows } => self.table(head, rows, tone, width),
        }
    }

    /// A fold-out: a chevron and `summary`, over `blocks` set in past a bar under the chevron when it is open.
    fn fold_out(&mut self, summary: &[Span], blocks: &[Block], tone: Role, width: f32) -> Node {
        let place = self.seen;
        self.seen += 1;
        let open = self.details.is_none() || self.unfolded.contains(&place);
        let height = self.line_height();
        let fallback = [Span {
            text: "Details".to_owned(),
            inline: Inline::default(),
        }];
        let summary = if summary.is_empty() {
            &fallback[..]
        } else {
            summary
        };
        let mut head = node()
            .styles([flex(), gap_x(1.5), cursor_pointer()])
            .child(
                node()
                    .styles([h_px(height), flex(), items_center(), flex_none()])
                    .child(glyph(
                        if open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        },
                        3.5,
                        Role::Subtle,
                    )),
            )
            .child(
                self.text(summary, self.pixels, true, tone, width - 20.0)
                    .styles([flex_1(), min_w_px(0)]),
            );
        if let Some(base) = self.details {
            head = head.focusable(FocusId(base + place as u64));
        }
        let fold = node().styles([flex(), flex_col(), gap(2)]).child(head);
        if !open {
            self.seen += count_details(blocks);
            return fold;
        }
        fold.child(
            node()
                .styles([
                    flex(),
                    border_width(Sides::Left, 2.0),
                    border_color(Role::Border),
                    ml(1.5),
                    padding(Sides::Left, INDENT - 8.0),
                ])
                .child(self.blocks(blocks, tone, width - INDENT)),
        )
    }

    /// A list of `items`, numbered from `start` when it is ordered and dotted when not.
    fn list(&mut self, start: Option<u64>, items: &[Vec<Block>], tone: Role, width: f32) -> Node {
        let height = self.line_height();
        let pixels = self.pixels;
        node()
            .styles([flex(), flex_col(), gap(1.5)])
            .children(items.iter().enumerate().map(|(index, item)| {
                let marker = match start {
                    Some(first) => line(
                        format!("{}.", first + index as u64),
                        pixels,
                        LEADING,
                        Role::Muted,
                    ),
                    None => node().styles([size(1), rounded_full(), bg(Role::Subtle)]),
                };
                node()
                    .styles([flex()])
                    .child(
                        node()
                            .styles([
                                w_px(INDENT),
                                h_px(height),
                                flex(),
                                items_center(),
                                flex_none(),
                            ])
                            .child(marker),
                    )
                    .child(self.blocks(item, tone, width - INDENT).styles([
                        flex_1(),
                        min_w_px(0),
                        gap(1.5),
                    ]))
            }))
    }

    /// A block of code in a bordered box, its long lines broken to fit `width`.
    fn code(&mut self, source: &str, width: f32) -> Node {
        let styles = [
            text_px(self.pixels - 1),
            leading_ratio(1.6),
            font_mono(),
            text_color(Role::Text),
        ];
        let room = width - 30.0;
        let mut lines = Vec::new();
        for source_line in source.split('\n') {
            let expanded = source_line.replace('\t', "    ");
            for piece in pm_ui::wrap(self.shaper, &expanded, styles, room) {
                lines.push(
                    node()
                        .styles([flex(), px(3.5)])
                        .child(text(&expanded[piece.start..piece.end]).styles(styles)),
                );
            }
        }
        CodeBox::new(lines)
            .max_height(UNCUT)
            .build()
            .styles([py(2), my(0)])
    }

    /// A table: a shaded heading row over its rows, the columns sharing `width` evenly.
    fn table(
        &mut self,
        head: &[Vec<Span>],
        rows: &[Vec<Vec<Span>>],
        tone: Role,
        width: f32,
    ) -> Node {
        let columns = rows
            .iter()
            .map(Vec::len)
            .chain([head.len()])
            .max()
            .unwrap_or(1)
            .max(1);
        let cell_width = ((width - 2.0) / columns as f32).floor();
        let pixels = self.pixels - 1;
        let mut table = node().styles([
            flex(),
            flex_col(),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            overflow_hidden(),
        ]);
        table = table.child(
            node().styles([flex(), bg(Role::Surface)]).children(
                head.iter()
                    .map(|cell| self.cell(cell, pixels, true, tone, cell_width)),
            ),
        );
        for row in rows {
            table = table.child(
                node()
                    .styles([flex(), border_t(), border_color(Role::Border)])
                    .children(
                        row.iter()
                            .map(|cell| self.cell(cell, pixels, false, tone, cell_width)),
                    ),
            );
        }
        table
    }

    /// One cell of a table, `width` wide, set heavier when it is a heading.
    fn cell(&mut self, spans: &[Span], pixels: u32, strong: bool, tone: Role, width: f32) -> Node {
        node()
            .styles([w_px(width), flex_none(), px(2.5), py(1.5)])
            .child(self.text(spans, pixels, strong, tone, width - 20.0))
    }

    /// `spans` broken into lines that fit `width`, set in `pixels` tall type,
    /// heavier throughout when `strong`, running text in `tone`.
    fn text(&mut self, spans: &[Span], pixels: u32, strong: bool, tone: Role, width: f32) -> Node {
        let mut lines: Vec<Vec<(String, Inline)>> = vec![Vec::new()];
        let mut used = 0.0;
        for span in spans {
            if span.text == "\n" {
                lines.push(Vec::new());
                used = 0.0;
                continue;
            }
            for word in span.text.split_inclusive(' ') {
                let room = width - CODE_PAD * 2.0;
                for piece in self.cut(word, span.inline, pixels, strong, room) {
                    let fitted = self.measure(piece.trim_end(), span.inline, pixels, strong);
                    let advance = self.measure(&piece, span.inline, pixels, strong);
                    let current = lines.last().expect("there is always a line");
                    if !current.is_empty() && used + opening(current, span.inline) + fitted > width
                    {
                        lines.push(Vec::new());
                        used = 0.0;
                    }
                    let current = lines.last_mut().expect("there is always a line");
                    if current.is_empty() && piece.trim().is_empty() {
                        continue;
                    }
                    used += opening(current, span.inline) + advance;
                    match current.last_mut() {
                        Some((text, inline)) if *inline == span.inline => text.push_str(&piece),
                        _ => current.push((piece, span.inline)),
                    }
                }
            }
        }
        let height = pixels as f32 * LEADING;
        node()
            .styles([flex(), flex_col()])
            .children(lines.into_iter().map(|runs| {
                node()
                    .styles([flex(), items_center(), h_px(height)])
                    .children(runs.into_iter().map(|(content, inline)| {
                        let piece = run(content, inline, pixels, strong, tone);
                        match (self.links, inline.target) {
                            (Some(base), target) if target > 0 => piece
                                .styles([cursor_pointer()])
                                .focusable(FocusId(base + u64::from(target) - 1)),
                            _ => piece,
                        }
                    }))
            }))
    }

    /// `word` cut into pieces no wider than `width`; one piece when it fits.
    fn cut(
        &mut self,
        word: &str,
        inline: Inline,
        pixels: u32,
        strong: bool,
        width: f32,
    ) -> Vec<String> {
        if self.measure(word.trim_end(), inline, pixels, strong) <= width {
            return vec![word.to_owned()];
        }
        let mut pieces = vec![String::new()];
        for character in word.chars() {
            let current = pieces.last_mut().expect("there is always a piece");
            current.push(character);
            if current.chars().count() > 1 && self.measure(current, inline, pixels, strong) > width
            {
                current.pop();
                pieces.push(character.to_string());
            }
        }
        pieces
    }

    /// How wide `content` is when set as `inline` in `pixels` tall type, heavier when `strong`.
    fn measure(&mut self, content: &str, inline: Inline, pixels: u32, strong: bool) -> f32 {
        let mut style = Style::default();
        style.extend(setting(inline, pixels, strong));
        self.shaper.measure(content, &style.base().typography).width
    }
}

/// How many fold-outs `blocks` hold, at any depth.
fn count_details(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|block| match block {
            Block::Details { blocks, .. } => 1 + count_details(blocks),
            Block::Quote(blocks) => count_details(blocks),
            Block::List { items, .. } => items.iter().map(|item| count_details(item)).sum(),
            _ => 0,
        })
        .sum()
}

/// The room a run set as `inline` takes besides its text when it starts
/// after `line`: the padding of inline code that opens a new run.
fn opening(line: &[(String, Inline)], inline: Inline) -> f32 {
    let opens = line.last().is_none_or(|(_, last)| *last != inline);
    if inline.code && opens {
        CODE_PAD * 2.0
    } else {
        0.0
    }
}

/// The typography of text set as `inline` in `pixels` tall type, heavier when `strong`.
fn setting(inline: Inline, pixels: u32, strong: bool) -> Vec<Utility> {
    let mut utilities = Vec::new();
    if inline.code {
        utilities.push(text_px(pixels - 1));
        utilities.push(leading_ratio(
            pixels as f32 * CODE_LEADING / (pixels - 1) as f32,
        ));
        utilities.push(font_mono());
    } else {
        utilities.push(text_px(pixels));
        utilities.push(leading_ratio(LEADING));
    }
    if strong || inline.strong {
        utilities.push(font_semibold());
    }
    if inline.emphasis {
        utilities.push(italic());
    }
    if inline.strike {
        utilities.push(line_through());
    }
    if inline.link {
        utilities.push(underline());
    }
    utilities
}

/// One run of `content` set as `inline`, in `tone` unless it is a link or struck.
fn run(content: String, inline: Inline, pixels: u32, strong: bool, tone: Role) -> Node {
    let colour = match inline {
        Inline { link: true, .. } => Role::Accent,
        Inline { strike: true, .. } => Role::Muted,
        _ => tone,
    };
    let piece = text(content)
        .styles(setting(inline, pixels, strong))
        .styles([text_color(colour), flex_none()]);
    match inline.code {
        true => piece.styles([
            padding(Sides::Horizontal, CODE_PAD),
            rounded(Radius::Sm),
            bg(Role::SurfaceHover),
        ]),
        false => piece,
    }
}
