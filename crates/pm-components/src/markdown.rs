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
        let width = self.width;
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
        rustle::column_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(theme::sp(3)),
            Alignment::Start,
            || {
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
                            let key = crate::figures::key_of((
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
            },
        );
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
                rustle::row_arranged(
                    Modifier::new().padding_xy(theme::sp(3.5), 0.0),
                    Arrangement::Start,
                    Alignment::Start,
                    move || {
                        rustle::basic_text(Modifier::new(), content, kind, TextOverflow::Ellipsis);
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
    /// own cut where it reaches the edge.
    fn chip_pieces(
        &mut self,
        code: &str,
        inline: Inline,
        pixels: u32,
        strong: bool,
        width: f32,
    ) -> Vec<String> {
        let room = width - CODE_PAD * 2.0;
        if self.measure(code, inline, pixels, strong) <= room {
            return vec![code.to_owned()];
        }
        let mut pieces: Vec<String> = Vec::new();
        for word in code.split_inclusive(' ') {
            for cut in self.cut(word, inline, pixels, strong, room) {
                match pieces.last_mut() {
                    Some(last)
                        if self.measure(&format!("{last}{cut}"), inline, pixels, strong)
                            <= room =>
                    {
                        last.push_str(&cut)
                    }
                    _ => pieces.push(cut),
                }
            }
        }
        pieces
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
