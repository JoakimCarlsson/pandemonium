//! Markdown: a source laid out as paragraphs, headings, lists, code, quotes, fold-outs, rules and tables.

use crate::events::reduce;
use crate::theme;
use crate::theme::sp;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use std::hash::{DefaultHasher, Hash, Hasher};

use pm_markdown::{Block, Inline, Span};
use rustle::{CursorIcon, Modifier};

use crate::CodeBox;
use crate::events::Interactive;
use crate::figures::{
    Figures, diagram_key, diagram_node, figure_marks, formula_key, image_key, images_node,
    is_diagram,
};
use crate::focus::FocusId;
use crate::icons::IconName;
use crate::parts::{glyph, line};
use crate::select::styled_text;
use crate::theme::{Radius, Role};

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
/// [`Markdown::tone`], and emit it with [`Markdown::build`].
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
    /// The figures drawn or fetched so far, when diagrams and images are shown as pictures.
    figures: Option<&'a Figures>,
    /// The markdown file the source is, from the root of its repository, which the paths of its images start from; nothing for text in no file.
    place: Option<&'a str>,
    /// What the caller puts around the column of blocks.
    modifier: Modifier,
    /// How the column of blocks spreads them, and where it places them across.
    arranged: (Arrangement, Alignment),
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
            figures: None,
            place: None,
            modifier: Modifier::new(),
            arranged: (Arrangement::SpacedBy(sp(3)), Alignment::Start),
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

    /// Diagrams and images shown as the drawings `figures` holds for them, a diagram as its code and an image as its words until one arrives; the paths of images start from the markdown file at `place`, or are not followed for text in no file.
    pub fn figures(mut self, figures: &'a Figures, place: Option<&'a str>) -> Self {
        self.figures = Some(figures);
        self.place = place;
        self
    }

    /// Fold-outs whose summaries are named from `base` up, in the order the
    /// source has them, closed but for those placed in `unfolded`.
    pub fn details(mut self, base: u64, unfolded: Vec<usize>) -> Self {
        self.details = Some(base);
        self.unfolded = unfolded;
        self
    }

    /// The source with `modifier` put around its column of blocks (size, growth, margins), on top of what earlier calls put.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The column of blocks spread by `arrangement` and placed across by `alignment` instead of one under the other three steps apart.
    pub fn arranged(mut self, arrangement: Arrangement, alignment: Alignment) -> Self {
        self.arranged = (arrangement, alignment);
        self
    }

    /// Emits the source, its lines broken as the type measures them. The
    /// source is parsed and laid out once for as long as nothing about it
    /// changes, and each block once for as long as it stays the same, so a
    /// reply that grows only lays out its last block again.
    #[track_caller]
    pub fn build(self) {
        let width = self.width;
        let drawn = self
            .figures
            .map(|figures| figure_marks(self.source, self.place, figures))
            .unwrap_or_default();
        let whole = hash((
            self.source,
            self.pixels,
            self.tone,
            self.details,
            &self.unfolded,
            self.links,
            &drawn,
            self.place,
        ));
        let modifier = self.modifier.clone();
        let (arrangement, alignment) = self.arranged;
        column_arranged(modifier, arrangement, alignment, || {
            rustle::key(whole, || {
                rustle::skippable((whole, width.to_bits()), || {
                    let blocks = pm_markdown::parse(self.source);
                    let mut seen = 0;
                    for block in &blocks {
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
                            &drawn,
                        ));
                        rustle::key(key, || {
                            rustle::skippable((key, width.to_bits()), || {
                                Layout {
                                    pixels: self.pixels,
                                    details: self.details,
                                    unfolded,
                                    seen: first,
                                    links: self.links,
                                    figures: self.figures,
                                    place: self.place,
                                }
                                .block(block, self.tone, width);
                            });
                        });
                    }
                });
            });
        });
    }
}

/// `value` hashed, to tell one kept stretch of the drawing from another.
fn hash(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// One pass of laying blocks out.
struct Layout<'s> {
    /// How tall running text is, in logical pixels.
    pixels: u32,
    /// The name of the first fold-out's summary, when they can be pressed.
    details: Option<u64>,
    /// The fold-outs that are open, by their place in the source.
    unfolded: Vec<usize>,
    /// How many fold-outs were laid out or passed over so far.
    seen: usize,
    /// The name of the first link, when links can be pressed.
    links: Option<u64>,
    /// The figures drawn or fetched so far, when diagrams and images are shown as pictures.
    figures: Option<&'s Figures>,
    /// The markdown file the source is, which the paths of its images start from.
    place: Option<&'s str>,
}

impl Layout<'_> {
    /// How tall one line of running text is, in logical pixels.
    fn line_height(&self) -> f32 {
        self.pixels as f32 * LEADING
    }

    /// Emits `blocks` one under the other, `gap` steps apart, in a column with
    /// `modifier`, `width` wide, running text in `tone`.
    #[track_caller]
    fn blocks(&mut self, modifier: Modifier, gap: f32, blocks: &[Block], tone: Role, width: f32) {
        column_arranged(
            modifier,
            Arrangement::SpacedBy(sp(gap)),
            Alignment::Start,
            || {
                for block in blocks {
                    self.block(block, tone, width);
                }
            },
        );
    }

    /// Emits one block, `width` wide, running text in `tone`.
    #[track_caller]
    fn block(&mut self, block: &Block, tone: Role, width: f32) {
        match block {
            Block::Paragraph(spans) => {
                self.text(Modifier::new(), spans, self.pixels, false, tone, width)
            }
            Block::Heading { level, spans } => {
                let pixels = match level {
                    1 => self.pixels + 6,
                    2 => self.pixels + 3,
                    3 => self.pixels + 1,
                    _ => self.pixels,
                };
                self.text(Modifier::new(), spans, pixels, true, tone, width);
            }
            Block::List { start, items } => self.list(*start, items, tone, width),
            Block::Code { language, text } if is_diagram(language) => self.figure(
                text,
                diagram_key(text),
                "This diagram could not be drawn.",
                width,
            ),
            Block::Code { text, .. } => self.code(text, width),
            Block::Quote(blocks) => {
                let own = Modifier::new()
                    .draw_behind(theme::edge(
                        theme::Side::Left,
                        2.0,
                        theme::color(Role::Border),
                    ))
                    .padding_each(2.0, 0.0, 0.0, 0.0)
                    .padding_each(theme::sp(QUOTE_INDENT / 4.0 - 0.5), 0.0, 0.0, 0.0);
                row_arranged(own, Arrangement::Start, Alignment::Start, || {
                    self.blocks(
                        Modifier::new(),
                        3.0,
                        blocks,
                        Role::Muted,
                        width - QUOTE_INDENT,
                    );
                });
            }
            Block::Details { summary, blocks } => self.fold_out(summary, blocks, tone, width),
            Block::Rule => rustle::horizontal_divider(
                Modifier::new().padding_xy(0.0, theme::sp(1)),
                rustle::DividerDefaults::THICKNESS,
                rustle::DividerDefaults::color(),
            ),
            Block::Table { head, rows } => self.table(head, rows, tone, width),
            Block::Math(text) => self.figure(
                text,
                formula_key(text),
                "This formula could not be set.",
                width,
            ),
            Block::Images { images, centered } => images_node(
                images,
                *centered,
                self.figures,
                |image| image_key(self.place, &image.source),
                (self.links, self.pixels, width),
            ),
        }
    }

    /// Emits a fold-out: a chevron and `summary`, over `blocks` set in past a bar under the chevron when it is open.
    #[track_caller]
    fn fold_out(&mut self, summary: &[Span], blocks: &[Block], tone: Role, width: f32) {
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
        let mut head = Modifier::new().cursor(CursorIcon::Pointer);
        if let Some(base) = self.details {
            head = head.control(
                FocusId(base + place as u64),
                reduce(FocusId(base + place as u64)),
            );
        }
        let pixels = self.pixels;
        column_arranged(
            Modifier::new().fill_max_width().width_in(0.0, width),
            Arrangement::SpacedBy(sp(2)),
            Alignment::Start,
            || {
                row_arranged(
                    Modifier::new().fill_max_width().chain(head),
                    Arrangement::SpacedBy(sp(1.5)),
                    Alignment::Start,
                    || {
                        row_arranged(
                            Modifier::new().height(height),
                            Arrangement::Start,
                            Alignment::Center,
                            || {
                                glyph(
                                    if open {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    },
                                    3.5,
                                    Role::Subtle,
                                );
                            },
                        );
                        self.text(
                            Modifier::new().weight(1.0),
                            summary,
                            pixels,
                            true,
                            tone,
                            width - 20.0,
                        );
                    },
                );
                if !open {
                    self.seen += count_details(blocks);
                    return;
                }
                let bar = Modifier::new()
                    .padding_each(theme::sp(1.5), 0.0, 0.0, 0.0)
                    .draw_behind(theme::edge(
                        theme::Side::Left,
                        2.0,
                        theme::color(Role::Border),
                    ))
                    .padding_each(2.0, 0.0, 0.0, 0.0)
                    .padding_each(INDENT - 8.0, 0.0, 0.0, 0.0);
                row_arranged(bar, Arrangement::Start, Alignment::Start, || {
                    self.blocks(Modifier::new(), 3.0, blocks, tone, width - INDENT);
                });
            },
        );
    }

    /// Emits a list of `items`, numbered from `start` when it is ordered and dotted when not.
    #[track_caller]
    fn list(&mut self, start: Option<u64>, items: &[Vec<Block>], tone: Role, width: f32) {
        let height = self.line_height();
        let pixels = self.pixels;
        column_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(sp(1.5)),
            Alignment::Start,
            || {
                for (index, item) in items.iter().enumerate() {
                    row_arranged(
                        Modifier::new(),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            row_arranged(
                                Modifier::new().width(INDENT).height(height),
                                Arrangement::Start,
                                Alignment::Center,
                                || match start {
                                    Some(first) => line(
                                        format!("{}.", first + index as u64),
                                        pixels,
                                        LEADING,
                                        Role::Muted,
                                    ),
                                    None => rustle::spacer(
                                        Modifier::new()
                                            .size(theme::sp(1), theme::sp(1))
                                            .background_rounded(
                                                theme::color(Role::Subtle),
                                                Radius::Full.px(),
                                            ),
                                    ),
                                },
                            );
                            self.blocks(
                                Modifier::new().weight(1.0),
                                1.5,
                                item,
                                tone,
                                width - INDENT,
                            );
                        },
                    );
                }
            },
        );
    }

    /// Emits a block of code in a bordered box, its long lines broken to fit `width`.
    #[track_caller]
    fn code(&mut self, source: &str, width: f32) {
        let kind = theme::mono_style(self.pixels - 1, 1.6).color(theme::color(Role::Text));
        let room = width - 30.0;
        let mut lines = Vec::new();
        for source_line in source.split('\n') {
            let expanded = source_line.replace('\t', "    ");
            for piece in rustle::wrap_at_spaces(&expanded, &kind, room) {
                lines.push(expanded[piece.start..piece.end].to_owned());
            }
        }
        CodeBox::new(move || {
            for content in lines {
                row_arranged(
                    Modifier::new().padding_xy(theme::sp(3.5), 0.0),
                    Arrangement::Start,
                    Alignment::Start,
                    move || {
                        styled_text(Modifier::new(), content, kind);
                    },
                );
            }
        })
        .max_height(UNCUT)
        .modifier(Modifier::new().fill_max_width().width_in(0.0, width))
        .margin(0.0)
        .inner(Modifier::new().padding_xy(0.0, theme::sp(2)))
        .build();
    }

    /// Emits the figure written as `source` and kept under `key`, drawn when its drawing has arrived, and otherwise its source as code, with `failed` under it when it could not be drawn.
    #[track_caller]
    fn figure(&mut self, source: &str, key: u64, failed: &str, width: f32) {
        match self.figures.and_then(|figures| figures.get(&key)) {
            Some(Ok(drawing)) => diagram_node(drawing, width),
            Some(Err(_)) => {
                let pixels = self.pixels;
                column_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(sp(1.5)),
                    Alignment::Start,
                    || {
                        self.code(source, width);
                        line(failed, pixels - 2, 1.4, Role::Subtle);
                    },
                );
            }
            None => self.code(source, width),
        }
    }

    /// Emits a table: a shaded heading row over its rows, the columns sharing `width` evenly.
    #[track_caller]
    fn table(&mut self, head: &[Vec<Span>], rows: &[Vec<Vec<Span>>], tone: Role, width: f32) {
        let columns = rows
            .iter()
            .map(Vec::len)
            .chain([head.len()])
            .max()
            .unwrap_or(1)
            .max(1);
        let cell_width = ((width - 2.0) / columns as f32).floor();
        let pixels = self.pixels - 1;
        let own = Modifier::new()
            .border(1.0, theme::color(Role::Border), Radius::Lg.px())
            .padding(1.0)
            .clip();
        column_arranged(own, Arrangement::Start, Alignment::Start, || {
            row_arranged(
                Modifier::new().background(theme::color(Role::Surface)),
                Arrangement::Start,
                Alignment::Start,
                || {
                    for cell in head {
                        self.cell(cell, pixels, true, tone, cell_width);
                    }
                },
            );
            for cells in rows {
                row_arranged(
                    Modifier::new()
                        .draw_behind(theme::edge(
                            theme::Side::Top,
                            1.0,
                            theme::color(Role::Border),
                        ))
                        .padding_each(0.0, 1.0, 0.0, 0.0),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        for cell in cells {
                            self.cell(cell, pixels, false, tone, cell_width);
                        }
                    },
                );
            }
        });
    }

    /// Emits one cell of a table, `width` wide, set heavier when it is a heading.
    #[track_caller]
    fn cell(&mut self, spans: &[Span], pixels: u32, strong: bool, tone: Role, width: f32) {
        column_arranged(
            Modifier::new()
                .width(width)
                .padding_xy(theme::sp(2.5), 0.0)
                .padding_xy(0.0, theme::sp(1.5)),
            Arrangement::Start,
            Alignment::Start,
            || self.text(Modifier::new(), spans, pixels, strong, tone, width - 20.0),
        );
    }

    /// Emits `spans` broken into lines that fit `width`, set in `pixels` tall
    /// type, heavier throughout when `strong`, running text in `tone`, in a
    /// column with `modifier`.
    #[track_caller]
    fn text(
        &mut self,
        modifier: Modifier,
        spans: &[Span],
        pixels: u32,
        strong: bool,
        tone: Role,
        width: f32,
    ) {
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
        let links = self.links;
        column_arranged(modifier, Arrangement::Start, Alignment::Start, move || {
            for runs in lines {
                row_arranged(
                    Modifier::new().height(height),
                    Arrangement::Start,
                    Alignment::Center,
                    move || {
                        for (content, inline) in runs {
                            let pressed = match (links, inline.target) {
                                (Some(base), target) if target > 0 => {
                                    Modifier::new().cursor(CursorIcon::Pointer).control(
                                        FocusId(base + u64::from(target) - 1),
                                        reduce(FocusId(base + u64::from(target) - 1)),
                                    )
                                }
                                _ => Modifier::new(),
                            };
                            run(pressed, content, inline, pixels, strong, tone);
                        }
                    },
                );
            }
        });
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
        rustle::layout_text(content, &setting(inline, pixels, strong), None)
            .size()
            .width
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

/// The type of text set as `inline` in `pixels` tall type, heavier when `strong`.
fn setting(inline: Inline, pixels: u32, strong: bool) -> rustle::TextStyle {
    let mut kind = if inline.code {
        theme::mono_style(
            pixels - 1,
            pixels as f32 * CODE_LEADING / (pixels - 1) as f32,
        )
    } else {
        theme::text_style(pixels, LEADING)
    };
    if strong || inline.strong {
        kind = kind.font_weight(rustle::FontWeight::SEMI_BOLD);
    }
    if inline.emphasis {
        kind = kind.italic();
    }
    if inline.strike {
        kind = kind.strikethrough();
    }
    if inline.link {
        kind = kind.underline();
    }
    kind
}

/// Emits one run of `content` set as `inline`, in `tone` unless it is a link or struck, inside
/// `modifier` (what makes a link pressable), on a shaded rounded backing when it is code.
#[track_caller]
fn run(modifier: Modifier, content: String, inline: Inline, pixels: u32, strong: bool, tone: Role) {
    let colour = match inline {
        Inline { link: true, .. } => Role::Accent,
        Inline { strike: true, .. } => Role::Muted,
        _ => tone,
    };
    let kind = setting(inline, pixels, strong).color(theme::color(colour));
    let modifier = match inline.code {
        true => Modifier::new()
            .background_rounded(theme::color(Role::SurfaceHover), Radius::Sm.px())
            .chain(modifier)
            .padding_xy(CODE_PAD, 0.0),
        false => modifier,
    };
    styled_text(modifier, content, kind);
}
