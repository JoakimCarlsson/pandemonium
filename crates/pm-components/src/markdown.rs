//! Markdown: a source laid out as paragraphs, headings, lists, code, quotes, fold-outs, rules and tables.

use crate::events::{Event, send};
use crate::icons;
use crate::theme;
use crate::theme::Frame;
use rustle::{Alignment, Arrangement, TextOverflow};

use pm_markdown::{Block, Inline, Span};
use rustle::{CursorIcon, Modifier};

use crate::CodeBox;
use crate::events::Interactive;
use crate::figures::{
    Figures, diagram_key, diagram_node, figure_marks, formula_key, image_key, images_node,
    is_diagram,
};
use crate::parts::{glyph, line};
use crate::target::Target;
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
    details: Option<Target>,
    /// The fold-outs that are open, by their place in the source; all are when none can be pressed.
    unfolded: Vec<usize>,
    /// The name of the first link, the rest following it in the order [`pm_markdown::links`] gives their addresses, when they can be pressed.
    links: Option<Target>,
    /// The figures drawn or fetched so far, when diagrams and images are shown as pictures.
    figures: Option<&'a Figures>,
    /// The markdown file the source is, from the root of its repository, which the paths of its images start from; nothing for text in no file.
    place: Option<&'a str>,
    /// What the caller knows the source by, such as a message and its piece, when it keeps
    /// its parse across builds so a source that grows is parsed again only from its last block.
    identity: Option<u64>,
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
            identity: None,
        }
    }

    /// The source known by `identity`, which stays the same while the source grows, as a reply
    /// being written does: its parse is kept, and a source that grew is parsed again only from
    /// its last top-level block, and the source is told apart from the last one by a
    /// generation rather than by hashing it whole.
    pub fn identity(mut self, identity: u64) -> Self {
        self.identity = Some(identity);
        self
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

    /// Links that can be pressed, the first named by `base` and the rest following it in the order [`pm_markdown::links`] gives their addresses, as [`Target::offset`] counts them.
    pub fn links(mut self, base: Target) -> Self {
        self.links = Some(base);
        self
    }

    /// Diagrams and images shown as the drawings `figures` holds for them, a diagram as its code and an image as its words until one arrives; the paths of images start from the markdown file at `place`, or are not followed for text in no file.
    pub fn figures(mut self, figures: &'a Figures, place: Option<&'a str>) -> Self {
        self.figures = Some(figures);
        self.place = place;
        self
    }

    /// Fold-outs whose summaries are named from `base` on, in the order the
    /// source has them as [`Target::offset`] counts them, closed but for those placed in `unfolded`.
    pub fn details(mut self, base: Target, unfolded: Vec<usize>) -> Self {
        self.details = Some(base);
        self.unfolded = unfolded;
        self
    }

    /// Emits the source, its running text set as rustle's annotated text, which
    /// the text layout breaks into lines; inline code is set in the lines as
    /// chips and links are pressed through the text's own link areas. The
    /// source is parsed and laid out once for as long as nothing about it
    /// changes, and each block once for as long as it stays the same, so a
    /// reply that grows only lays out its last block again.
    #[track_caller]
    pub fn build(self) {
        match self.identity {
            Some(identity) => self.build_kept(identity),
            None => self.build_once(),
        }
    }

    /// Emits the source as [`Markdown::build`] does, parsing it each time it changed and
    /// telling it apart by a hash of it whole.
    #[track_caller]
    fn build_once(self) {
        let drawn = self
            .figures
            .map(|figures| figure_marks(self.source, self.place, figures))
            .unwrap_or_default();
        let whole = crate::figures::key_of((
            self.source,
            self.pixels,
            self.tone,
            self.details,
            &self.unfolded,
            self.links,
            &drawn,
            self.place,
        ));
        let width = self.width;
        rustle::column_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(theme::sp(3)),
            Alignment::Start,
            || {
                rustle::skippable((whole, width.to_bits()), || {
                    let blocks = pm_markdown::parse(self.source);
                    self.emit_blocks(
                        blocks
                            .iter()
                            .map(|block| (block, crate::figures::key_of(block))),
                        &drawn,
                    );
                });
            },
        );
    }

    /// Emits the source as [`Markdown::build`] does, its parse kept under `identity` and
    /// told apart from the last one by its generation, its figures found in the blocks parsed.
    #[track_caller]
    fn build_kept(self, identity: u64) {
        let parsed = parsed::parse(identity, self.source);
        let drawn = self
            .figures
            .map(|figures| {
                crate::figures::block_figure_marks(
                    parsed.blocks().map(|(block, _)| block),
                    self.place,
                    figures,
                )
            })
            .unwrap_or_default();
        let whole = crate::figures::key_of((
            identity,
            parsed.generation,
            self.pixels,
            self.tone,
            self.details,
            &self.unfolded,
            self.links,
            &drawn,
            self.place,
        ));
        let width = self.width;
        rustle::column_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(theme::sp(3)),
            Alignment::Start,
            || {
                rustle::skippable((whole, width.to_bits()), || {
                    self.emit_blocks(parsed.blocks(), &drawn);
                });
            },
        );
    }

    /// Emits `blocks`, each with a hash of itself, as the blocks of the source, each kept for
    /// as long as it, how it is set and the figures `drawn` stay the same.
    #[track_caller]
    fn emit_blocks<'b>(&self, blocks: impl Iterator<Item = (&'b Block, u64)>, drawn: &[u64]) {
        let width = self.width;
        let mut seen = 0;
        for (block, hash) in blocks {
            let first = seen;
            seen += count_details(std::slice::from_ref(block));
            let unfolded: Vec<usize> = self
                .unfolded
                .iter()
                .copied()
                .filter(|place| (first..seen).contains(place))
                .collect();
            let key = crate::figures::key_of((
                hash,
                self.pixels,
                self.tone,
                self.details,
                first,
                &unfolded,
                self.links,
                drawn,
            ));
            rustle::key(key, || {
                rustle::kept((key, width.to_bits()), || {
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
    }
}

/// One pass of laying blocks out.
struct Layout<'s> {
    /// How tall running text is, in logical pixels.
    pixels: u32,
    /// The name of the first fold-out's summary, when they can be pressed.
    details: Option<Target>,
    /// The fold-outs that are open, by their place in the source.
    unfolded: Vec<usize>,
    /// How many fold-outs were laid out or passed over so far.
    seen: usize,
    /// The name of the first link, when links can be pressed.
    links: Option<Target>,
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
        rustle::column_arranged(
            modifier,
            Arrangement::SpacedBy(theme::sp(gap)),
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
                let inset = theme::sp(QUOTE_INDENT / 4.0 - 0.5);
                rustle::row_arranged(
                    Modifier::new().height_intrinsic(rustle::IntrinsicSize::Min),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        rustle::vertical_divider(Modifier::new(), 2.0, theme::color(Role::Border));
                        self.blocks(
                            Modifier::new().padding_each(inset, 0.0, 0.0, 0.0),
                            3.0,
                            blocks,
                            Role::Muted,
                            width - QUOTE_INDENT,
                        );
                    },
                );
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
                base.offset(place),
                send(Event::pressing(base.offset(place))),
            );
        }
        let pixels = self.pixels;
        rustle::column_arranged(
            Modifier::new().fill_max_width().width_in(0.0, width),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Start,
            || {
                rustle::row_arranged(
                    Modifier::new().fill_max_width().chain(head),
                    Arrangement::SpacedBy(theme::sp(1.5)),
                    Alignment::Start,
                    || {
                        rustle::row_arranged(
                            Modifier::new().height(height),
                            Arrangement::Start,
                            Alignment::Center,
                            || {
                                glyph(icons::chevron(open), 3.5, Role::Subtle);
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
                    .height_intrinsic(rustle::IntrinsicSize::Min);
                rustle::row_arranged(bar, Arrangement::Start, Alignment::Start, || {
                    rustle::vertical_divider(Modifier::new(), 2.0, theme::color(Role::Border));
                    self.blocks(
                        Modifier::new().padding_each(INDENT - 8.0, 0.0, 0.0, 0.0),
                        3.0,
                        blocks,
                        tone,
                        width - INDENT,
                    );
                });
            },
        );
    }

    /// Emits a list of `items`, numbered from `start` when it is ordered and dotted when not.
    #[track_caller]
    fn list(&mut self, start: Option<u64>, items: &[Vec<Block>], tone: Role, width: f32) {
        let height = self.line_height();
        let pixels = self.pixels;
        rustle::column_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(theme::sp(1.5)),
            Alignment::Start,
            || {
                for (index, item) in items.iter().enumerate() {
                    rustle::row_arranged(
                        Modifier::new(),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            rustle::row_arranged(
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

    /// Emits a block of code in a bordered box, its long lines broken to fit `width`, all its
    /// lines set as one text.
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
        let content = lines.join("\n");
        CodeBox::new(move || {
            rustle::basic_text(
                Modifier::new().padding_xy(theme::sp(3.5), 0.0),
                content,
                kind,
                TextOverflow::Ellipsis,
            );
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
                rustle::column_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(theme::sp(1.5)),
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
        let own = Modifier::new().outlined(Radius::Lg).clip();
        rustle::column_arranged(own, Arrangement::Start, Alignment::Start, || {
            rustle::row_arranged(
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
                rustle::horizontal_divider(
                    Modifier::new().width(cells.len() as f32 * cell_width),
                    1.0,
                    theme::color(Role::Border),
                );
                rustle::row_arranged(
                    Modifier::new(),
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
        rustle::column_arranged(
            Modifier::new()
                .width(width)
                .padding_xy(theme::sp(2.5), theme::sp(1.5)),
            Arrangement::Start,
            Alignment::Start,
            || self.text(Modifier::new(), spans, pixels, strong, tone, width - 20.0),
        );
    }

    /// Emits `spans` as one text that wraps at `width`, whatever room its parent has, set in `pixels` tall type,
    /// heavier throughout when `strong`, running text in `tone`, with `modifier`: inline
    /// code as chips set in the lines, and links that can be pressed when links can.
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
        let mut builder = rustle::AnnotatedStringBuilder::new();
        let mut chips = std::collections::HashMap::new();
        for span in spans {
            if span.inline.code {
                let index = self
                    .link(span.inline, tone)
                    .map(|link| builder.push_link(link));
                for piece in self.chip_pieces(&span.text, span.inline, pixels, strong, width) {
                    let wide = self.measure(&piece, span.inline, pixels, strong) + CODE_PAD * 2.0;
                    let id = chip_id(span.inline, wide);
                    chips
                        .entry(id.clone())
                        .or_insert_with(|| chip(span.inline, pixels, strong, tone, wide));
                    rustle::AppendInlineContent::append_inline_content(&mut builder, &id, &piece);
                }
                if let Some(index) = index {
                    builder.pop_to(index);
                }
                continue;
            }
            match self.link(span.inline, tone) {
                Some(link) => builder.with_link(link, |builder| builder.append(&span.text)),
                None => builder.with_style(running(span.inline, tone), |builder| {
                    builder.append(&span.text)
                }),
            }
        }
        let text = builder.to_annotated_string();
        let mut kind = theme::text_style(pixels, LEADING)
            .color(theme::color(tone))
            .wrap(rustle::TextWrap::Word);
        if strong {
            kind = kind.font_weight(rustle::FontWeight::SEMI_BOLD);
        }
        let wrapped_at = modifier
            .wrap_content_width(rustle::BoxAlignment::TopStart, true)
            .width_in(0.0, width);
        rustle::basic_text_annotated(wrapped_at, &text, kind, &chips);
    }

    /// The link `inline` is the text of, drawn as running text set as `inline` over text in
    /// `tone`, or as its chip draws it when it is code, when links can be pressed and it has
    /// an address; nothing otherwise.
    fn link(&self, inline: Inline, tone: Role) -> Option<rustle::LinkAnnotation> {
        let id = crate::figures::link_target(self.links, inline.target)?;
        let press = crate::events::link_pressed(id);
        let style = if inline.code {
            rustle::SpanStyle::new()
        } else {
            running(inline, tone)
        };
        Some(
            rustle::LinkAnnotation::clickable(id.tag(), move |_| press())
                .with_styles(rustle::TextLinkStyles::new(style)),
        )
    }

    /// The pieces inline code saying `code` is set in as chips: all of it when its chip fits
    /// `width`, and otherwise its words gathered into pieces that fit, a word too wide on its
    /// own cut where it reaches the edge. The code is shaped once, and the pieces are measured
    /// by the advances of its characters.
    fn chip_pieces(
        &mut self,
        code: &str,
        inline: Inline,
        pixels: u32,
        strong: bool,
        width: f32,
    ) -> Vec<String> {
        let room = width - CODE_PAD * 2.0;
        let layout = rustle::layout_text(code, &setting(inline, pixels, strong), None);
        if layout.size().width <= room {
            return vec![code.to_owned()];
        }
        let advances = Advances::of(code, &layout);
        let mut pieces: Vec<(usize, usize)> = Vec::new();
        let mut start = 0;
        for word in code.split_inclusive(' ') {
            let end = start + word.len();
            for (from, to) in advances.cut(code, start, end, room) {
                match pieces.last_mut() {
                    Some(last) if advances.width(last.0, to) <= room => last.1 = to,
                    _ => pieces.push((from, to)),
                }
            }
            start = end;
        }
        pieces
            .into_iter()
            .map(|(from, to)| code[from..to].to_owned())
            .collect()
    }

    /// How wide `content` is when set as `inline` in `pixels` tall type, heavier when `strong`.
    fn measure(&mut self, content: &str, inline: Inline, pixels: u32, strong: bool) -> f32 {
        rustle::layout_text(content, &setting(inline, pixels, strong), None)
            .size()
            .width
    }
}

/// Where each character of a line of text starts, as one shaping of it placed them, to
/// measure any run of it without shaping that run again.
struct Advances {
    /// The x of every character boundary, by byte offset into the text; boundaries inside a
    /// character take the x of the one before.
    at: Vec<f32>,
}

impl Advances {
    /// The boundaries of `text` as `layout`, its shaping on one line, placed them.
    fn of(text: &str, layout: &rustle::TextLayout) -> Self {
        let positions = layout.char_positions();
        let mut at = vec![0.0; text.len() + 1];
        let mut last = 0.0;
        let mut characters = text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain([text.len()])
            .zip(positions.iter().map(|(x, _)| *x))
            .peekable();
        for (offset, slot) in at.iter_mut().enumerate() {
            while let Some((boundary, x)) = characters.next_if(|(boundary, _)| *boundary <= offset)
            {
                if boundary == offset {
                    last = x;
                }
            }
            *slot = last;
        }
        Self { at }
    }

    /// How wide the text from byte `from` to byte `to` is.
    fn width(&self, from: usize, to: usize) -> f32 {
        self.at[to] - self.at[from]
    }

    /// The word of `text` from byte `start` to byte `end` cut into runs no wider than `room`,
    /// as byte ranges; the whole word when it fits without the spaces it ends with.
    fn cut(&self, text: &str, start: usize, end: usize, room: f32) -> Vec<(usize, usize)> {
        let trimmed = start + text[start..end].trim_end().len();
        if self.width(start, trimmed) <= room {
            return vec![(start, end)];
        }
        let mut pieces = vec![(start, start)];
        for (offset, character) in text[start..end].char_indices() {
            let from = start + offset;
            let to = from + character.len_utf8();
            let current = pieces.last_mut().expect("there is always a piece");
            current.1 = to;
            if current.1 - current.0 > character.len_utf8() && self.width(current.0, to) > room {
                current.1 = from;
                pieces.push((from, to));
            }
        }
        pieces
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

/// The name a chip of inline code set as `inline`, `wide` logical pixels wide, has among a
/// text's inline contents.
fn chip_id(inline: Inline, wide: f32) -> String {
    format!(
        "code {} {} {} {} {}",
        inline.strong,
        inline.emphasis,
        inline.strike,
        inline.link,
        wide.to_bits()
    )
}

/// The chip of inline code set as `inline` in a text of `pixels` tall type, heavier when
/// `strong`, running text in `tone`, `wide` logical pixels wide: room as tall as a line of
/// code in the middle of the line, holding its words on a shaded rounded backing, left out of
/// the selection, which takes the code from the text the chip stands for.
fn chip(
    inline: Inline,
    pixels: u32,
    strong: bool,
    tone: Role,
    wide: f32,
) -> rustle::InlineTextContent {
    let room = rustle::Placeholder::new(
        rustle::TextUnit::Sp(wide),
        rustle::TextUnit::Sp(pixels as f32 * CODE_LEADING),
        rustle::PlaceholderVerticalAlign::Center,
    );
    rustle::InlineTextContent::new(room, move |code| {
        let code = code.to_owned();
        rustle::disable_selection(|| chip_text(code, inline, pixels, strong, tone));
    })
}

/// The style of running text set as `inline` over text in `tone`: heavier, leaning, struck
/// through in the muted colour or underlined in the accent colour of a link, as it says.
fn running(inline: Inline, tone: Role) -> rustle::SpanStyle {
    let mut style = rustle::SpanStyle::new();
    if inline.strong {
        style = style.font_weight(rustle::FontWeight::SEMI_BOLD);
    }
    if inline.emphasis {
        style = style.italic();
    }
    if inline.strike {
        style = style.strikethrough(true).color(theme::color(Role::Muted));
    }
    if inline.link {
        style = style.underline(true).color(theme::color(Role::Accent));
    }
    if !inline.strike && !inline.link {
        style = style.color(theme::color(tone));
    }
    style
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

/// Emits the words `content` of a chip of inline code set as `inline`, in `tone` unless it
/// is a link or struck, on a shaded rounded backing.
#[track_caller]
fn chip_text(content: String, inline: Inline, pixels: u32, strong: bool, tone: Role) {
    let colour = match inline {
        Inline { link: true, .. } => Role::Accent,
        Inline { strike: true, .. } => Role::Muted,
        _ => tone,
    };
    let kind = setting(inline, pixels, strong).color(theme::color(colour));
    let modifier = Modifier::new()
        .background_rounded(theme::color(Role::SurfaceHover), Radius::Sm.px())
        .padding_xy(CODE_PAD, 0.0);
    rustle::basic_text(modifier, content, kind, TextOverflow::Ellipsis);
}

/// Markdown sources parsed once and kept by an identity their caller gives them, so a source
/// that grows, as a reply being streamed does, is parsed again only from its last top-level
/// block rather than from its start.
mod parsed {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use pm_markdown::{Block, Span};

    /// The most source the cache keeps across all identities before it forgets the older half.
    const KEPT_BYTES: usize = 8 * 1024 * 1024;

    /// The most identities the cache keeps in its newer half.
    const KEPT_ENTRIES: usize = 512;

    /// The HTML tags whose meaning reaches past a top-level block or depends on the source
    /// around it, which a source must not hold to be parsed in pieces.
    const SPANNING_TAGS: &[&str] = &[
        "a", "abbr", "b", "br", "center", "code", "del", "details", "div", "em", "h1", "h2", "h3",
        "h4", "h5", "h6", "hr", "i", "img", "kbd", "mark", "p", "picture", "s", "small", "source",
        "span", "strike", "strong", "sub", "summary", "sup", "tt", "u", "style", "script", "html",
    ];

    /// A source as parsed: its blocks, split into those before the last cut and those after,
    /// each with a hash of itself, and a generation that changes whenever the source does.
    pub(super) struct Parsed {
        /// A number no other source parsed has, unchanged for as long as the source is.
        pub(super) generation: u64,
        /// The blocks before the last cut, which a source that grows keeps.
        head: Rc<Vec<(Block, u64)>>,
        /// The blocks after the last cut, parsed again whenever the source grows.
        tail: Rc<Vec<(Block, u64)>>,
    }

    impl Parsed {
        /// Every block of the source in order, each with the hash of itself.
        pub(super) fn blocks(&self) -> impl Iterator<Item = (&Block, u64)> {
            self.head
                .iter()
                .chain(self.tail.iter())
                .map(|(block, hash)| (block, *hash))
        }
    }

    /// What is kept of one identity's source.
    struct Entry {
        /// The source last parsed.
        source: String,
        /// The generation it was given.
        generation: u64,
        /// Where in the source the head ends and the tail begins, at the start of a line.
        cut: usize,
        /// How many link addresses the head holds, which the links of the tail count on from.
        head_links: u16,
        /// The blocks before the cut.
        head: Rc<Vec<(Block, u64)>>,
        /// The blocks after the cut.
        tail: Rc<Vec<(Block, u64)>>,
    }

    impl Entry {
        /// The parse this entry holds.
        fn parsed(&self) -> Parsed {
            Parsed {
                generation: self.generation,
                head: Rc::clone(&self.head),
                tail: Rc::clone(&self.tail),
            }
        }
    }

    /// The sources kept, in two halves so the older one can be forgotten at once.
    #[derive(Default)]
    struct Cache {
        /// The entries used since the halves last turned over.
        newer: HashMap<u64, Entry>,
        /// The entries used before that.
        older: HashMap<u64, Entry>,
        /// The bytes of source the newer half holds.
        bytes: usize,
        /// The generation the next source parsed is given.
        next: u64,
    }

    thread_local! {
        /// The sources parsed on this thread, by identity.
        static CACHE: RefCell<Cache> = RefCell::default();
    }

    /// `source` parsed, kept under `identity`: what was kept when it is the same source as
    /// last time, its head kept and only the rest parsed when it grew past the last cut, and
    /// all of it parsed otherwise. The blocks are what [`pm_markdown::parse`] gives.
    pub(super) fn parse(identity: u64, source: &str) -> Parsed {
        CACHE.with_borrow_mut(|cache| {
            let kept = cache
                .newer
                .remove(&identity)
                .or_else(|| cache.older.remove(&identity));
            let entry = match kept {
                Some(entry) if entry.source == source => entry,
                kept => {
                    cache.next += 1;
                    let generation = cache.next;
                    let grown = kept.filter(|entry| {
                        source.len() >= entry.cut
                            && source.as_bytes()[..entry.cut]
                                == entry.source.as_bytes()[..entry.cut]
                    });
                    reparsed(grown, source, generation)
                }
            };
            let parsed = entry.parsed();
            if entry.source.len() <= KEPT_BYTES {
                if cache.bytes + entry.source.len() > KEPT_BYTES
                    || cache.newer.len() >= KEPT_ENTRIES
                {
                    cache.older = std::mem::take(&mut cache.newer);
                    cache.bytes = 0;
                }
                cache.bytes += entry.source.len();
                cache.newer.insert(identity, entry);
            }
            parsed
        })
    }

    /// The entry for `source` given `generation`: `grown`'s head kept and extended to the last
    /// cut the source allows, and the rest parsed after it, when there is a grown entry and
    /// the source can be parsed in pieces; all of it parsed as one otherwise.
    fn reparsed(grown: Option<Entry>, source: &str, generation: u64) -> Entry {
        if !in_pieces(source) {
            return Entry {
                source: source.to_owned(),
                generation,
                cut: 0,
                head_links: 0,
                head: Rc::default(),
                tail: Rc::new(hashed(pm_markdown::parse(source))),
            };
        }
        let (mut head, mut cut, mut head_links) = match grown {
            Some(entry) => (entry.head, entry.cut, entry.head_links),
            None => (Rc::default(), 0, 0),
        };
        if let Some(next) = last_cut(source, cut) {
            let piece = &source[cut..next];
            let mut blocks = pm_markdown::parse(piece);
            renumber(&mut blocks, head_links);
            Rc::make_mut(&mut head).extend(hashed(blocks));
            head_links = head_links.saturating_add(pm_markdown::links(piece).len() as u16);
            cut = next;
        }
        let mut tail = pm_markdown::parse(&source[cut..]);
        renumber(&mut tail, head_links);
        Entry {
            source: source.to_owned(),
            generation,
            cut,
            head_links,
            head,
            tail: Rc::new(hashed(tail)),
        }
    }

    /// `blocks` each paired with a hash of itself.
    fn hashed(blocks: Vec<Block>) -> Vec<(Block, u64)> {
        blocks
            .into_iter()
            .map(|block| {
                let hash = crate::figures::key_of(&block);
                (block, hash)
            })
            .collect()
    }

    /// Whether `source` parses the same in pieces cut between its top-level blocks as it does
    /// whole: it defines no link references and holds no HTML whose meaning could reach
    /// across blocks.
    fn in_pieces(source: &str) -> bool {
        if source.contains("]:") {
            return false;
        }
        let bytes = source.as_bytes();
        let mut from = 0;
        while let Some(found) = source[from..].find('<') {
            let at = from + found;
            let rest = &source[at + 1..];
            if rest.starts_with('!') {
                return false;
            }
            let name: String = rest
                .trim_start_matches('/')
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_ascii_lowercase();
            if SPANNING_TAGS.contains(&name.as_str()) {
                return false;
            }
            let line_start = source[..at].rfind('\n').map_or(0, |newline| newline + 1);
            if bytes[line_start..at]
                .iter()
                .all(|byte| *byte == b' ' || *byte == b'\t')
            {
                return false;
            }
            from = at + 1;
        }
        true
    }

    /// The last place after `from` where `source` can be cut between top-level blocks: the
    /// start of a line that follows a blank line outside any code fence and starts a block
    /// that cannot continue one before it, as running text or a heading at the margin does.
    fn last_cut(source: &str, from: usize) -> Option<usize> {
        let mut fence: Option<(char, usize)> = None;
        let mut blank_before = false;
        let mut found = None;
        let mut at = from;
        for line in source[from..].split_inclusive('\n') {
            let start = at;
            at += line.len();
            if let Some(open) = fence {
                if closes_fence(line, open) {
                    fence = None;
                }
                blank_before = false;
                continue;
            }
            if blank_before && start > from && starts_apart(line) {
                found = Some(start);
            }
            fence = opens_fence(line);
            blank_before = line.trim().is_empty();
        }
        found
    }

    /// Whether `line`, after a blank line, starts a block of its own that nothing before it
    /// can take in: it starts at the margin with a character that opens no list, quote,
    /// table, fence, code or HTML.
    fn starts_apart(line: &str) -> bool {
        line.chars().next().is_some_and(|first| {
            !first.is_whitespace()
                && !first.is_ascii_digit()
                && !matches!(
                    first,
                    '-' | '*' | '+' | '>' | '|' | '`' | '~' | '<' | '[' | '$' | '\\' | '!'
                )
        })
    }

    /// The character and length of the code fence `line` opens, if it opens one, as
    /// [`pm_markdown::parse`] reads fences.
    fn opens_fence(line: &str) -> Option<(char, usize)> {
        let indent = line.len() - line.trim_start_matches(' ').len();
        let rest = line.trim_start_matches(' ');
        let mark = rest
            .chars()
            .next()
            .filter(|mark| matches!(mark, '`' | '~'))?;
        let length = rest.len() - rest.trim_start_matches(mark).len();
        let info = &rest[length..];
        let valid = indent < 4 && length >= 3 && !(mark == '`' && info.contains('`'));
        valid.then_some((mark, length))
    }

    /// Whether `line` closes the code fence `open`, made of its character repeated its length
    /// of times.
    fn closes_fence(line: &str, open: (char, usize)) -> bool {
        let indent = line.len() - line.trim_start_matches(' ').len();
        let rest = line.trim_start_matches(' ');
        let length = rest.len() - rest.trim_start_matches(open.0).len();
        indent < 4 && length >= open.1 && rest[length..].trim().is_empty()
    }

    /// `blocks`, parsed from a piece after `offset` link addresses, with their links counted
    /// on from those, as they are when the whole source is parsed.
    fn renumber(blocks: &mut [Block], offset: u16) {
        if offset == 0 {
            return;
        }
        for block in blocks {
            match block {
                Block::Paragraph(spans) | Block::Heading { spans, .. } => {
                    renumber_spans(spans, offset)
                }
                Block::List { items, .. } => {
                    for item in items {
                        renumber(item, offset);
                    }
                }
                Block::Quote(inner) => renumber(inner, offset),
                Block::Details { summary, blocks } => {
                    renumber_spans(summary, offset);
                    renumber(blocks, offset);
                }
                Block::Images { images, .. } => {
                    for image in images {
                        if image.target != 0 {
                            image.target = image.target.saturating_add(offset);
                        }
                    }
                }
                Block::Table { head, rows } => {
                    for cell in head {
                        renumber_spans(cell, offset);
                    }
                    for row in rows {
                        for cell in row {
                            renumber_spans(cell, offset);
                        }
                    }
                }
                Block::Code { .. } | Block::Rule | Block::Math(_) => {}
            }
        }
    }

    /// `spans` with their links counted on from `offset`.
    fn renumber_spans(spans: &mut [Span], offset: u16) {
        for span in spans {
            if span.inline.target != 0 {
                span.inline.target = span.inline.target.saturating_add(offset);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// A source streamed a few characters at a time parses, at every length, into the
        /// blocks a whole parse of it gives.
        #[test]
        fn growing_sources_parse_as_whole() {
            let sources = [
                "Intro with a [link](https://a.example).\n\n# Heading\n\nText `code` and https://b.example here.\n\n- one\n- two\n\nAfter the list.\n\n```rust\nfn main() {}\n\nlet x = 1;\n```\n\nTail [again](https://c.example).\n\n> quote\n\nEnd.",
                "Para one.\n\n1. first\n\n2. second\n\nPara two.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nPara three ![pic](https://p.example/x.png).\n\n***\n\nLast.",
                "A\n\n    indented code\n\nB\n\n~~~\nfenced\n\n~~~\nC **strong\n\nD** e.\n\n$$\nx^2\n$$\n\nF.",
            ];
            for (number, source) in sources.iter().enumerate() {
                let identity = 0xfeed_0000 + number as u64;
                let mut end = 0;
                while end < source.len() {
                    end = (end + 3).min(source.len());
                    while !source.is_char_boundary(end) {
                        end += 1;
                    }
                    let piece = &source[..end];
                    let parsed = parse(identity, piece);
                    let blocks: Vec<Block> =
                        parsed.blocks().map(|(block, _)| block.clone()).collect();
                    assert_eq!(
                        blocks,
                        pm_markdown::parse(piece),
                        "at {end} of source {number}"
                    );
                }
            }
        }

        /// The same source parsed twice keeps its generation, and a changed one gets a new one.
        #[test]
        fn generations_follow_the_source() {
            let first = parse(0xbeef, "Hello.").generation;
            assert_eq!(parse(0xbeef, "Hello.").generation, first);
            assert_ne!(parse(0xbeef, "Hello. More.").generation, first);
        }
    }
}
