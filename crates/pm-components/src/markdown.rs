//! Markdown as a message shows it: headings, paragraphs with strong,
//! emphasised, struck, linked and `code` runs, nested lists, code blocks,
//! quotes, fold-outs, tables, rules, formulas, diagrams and images.

use pm_markdown::{Block, Image, Span};
use rustle::prelude::*;

use crate::card::{Card, CardVariant};
use crate::figures::{Figure, FigureSource, FigureVariant, is_diagram, is_formula};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::{ColorScheme, color_scheme};

/// The visual treatment of markdown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkdownVariant {
    /// Body text, as the agent's replies show.
    #[default]
    Body,
    /// Faint text, as thoughts show.
    Muted,
}

/// A column of markdown blocks at a size.
#[derive(Clone, Debug)]
pub struct Markdown {
    /// The blocks in order.
    blocks: Vec<Block>,
    /// Its visual treatment.
    variant: MarkdownVariant,
    /// Its text size.
    size: Size,
}

impl Markdown {
    /// Markdown made of the parsed `blocks`.
    pub fn new(blocks: Vec<Block>) -> Self {
        Self {
            blocks,
            variant: MarkdownVariant::Body,
            size: Size::Sm,
        }
    }

    /// The markdown with `variant`.
    pub fn variant(mut self, variant: MarkdownVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The markdown at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the markdown.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let label = match self.variant {
            MarkdownVariant::Body => Label::new(""),
            MarkdownVariant::Muted => Label::new("").variant(LabelVariant::Muted),
        };
        let style = label.size(self.size).wrap().style();
        let size = self.size;
        LOCAL_TEXT_STYLE.provide(style, move || {
            blocks(&self.blocks, size, &scheme);
        });
    }
}

/// Emits `list` as a column, spaced as paragraphs are.
fn blocks(list: &[Block], size: Size, scheme: &ColorScheme) {
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(10.0),
        Alignment::Start,
        || {
            for block in list {
                one_block(block, size, scheme);
            }
        },
    );
}

/// Emits one `block`.
fn one_block(block: &Block, size: Size, scheme: &ColorScheme) {
    match block {
        Block::Paragraph(content) => {
            rich_text_with(Modifier::new().fill_max_width(), spans(content, scheme));
        }
        Block::Heading {
            level,
            spans: content,
        } => heading(*level, content, size, scheme),
        Block::List { start, items } => list(*start, items, size, scheme),
        Block::Code { language, text } if is_diagram(language) => {
            Figure::new(FigureSource::Diagram(text.clone()))
                .variant(FigureVariant::Framed)
                .size(size)
                .show();
        }
        Block::Code { language, text } if is_formula(language) => formula(text, size),
        Block::Code { language, text } => code_block(language, text),
        Block::Quote(inner) => quote(inner, size, scheme),
        Block::Details {
            summary,
            blocks: inner,
        } => fold_out(summary, inner, size, scheme),
        Block::Rule => spacer(
            Modifier::new()
                .fill_max_width()
                .height(1.0)
                .background(scheme.outline_variant),
        ),
        Block::Math(text) => formula(text, size),
        Block::Images { images, centered } => pictures(images, *centered, size),
        Block::Table { head, rows } => table(head, rows, scheme),
    }
}

/// Emits a heading of `level` saying `content`.
fn heading(level: u8, content: &[Span], size: Size, scheme: &ColorScheme) {
    let base = Label::new("")
        .variant(LabelVariant::Heading)
        .size(size)
        .style();
    let grown = match level {
        1 => 1.4,
        2 => 1.2,
        _ => 1.05,
    };
    let style = TextStyle {
        font_size: base.font_size * grown,
        ..base
    };
    LOCAL_TEXT_STYLE.provide(style, || {
        rich_text_with(
            Modifier::new()
                .fill_max_width()
                .padding_each(0.0, 4.0, 0.0, 0.0),
            spans(content, scheme),
        );
    });
}

/// Emits `items` as a list, numbered from `start` when it is ordered.
fn list(start: Option<u64>, items: &[Vec<Block>], size: Size, scheme: &ColorScheme) {
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(4.0),
        Alignment::Start,
        || {
            for (index, item) in items.iter().enumerate() {
                row_arranged(
                    Modifier::new().fill_max_width(),
                    Arrangement::SpacedBy(8.0),
                    Alignment::Start,
                    || {
                        let marker = match start {
                            Some(first) => format!("{}.", first + index as u64),
                            None => "•".to_owned(),
                        };
                        Label::new(marker)
                            .variant(LabelVariant::Subtle)
                            .size(size)
                            .modifier(Modifier::new().width(16.0))
                            .show();
                        column_with(Modifier::new().weight(1.0), || blocks(item, size, scheme));
                    },
                );
            }
        },
    );
}

/// Emits `text` in the monospace face on a sunken card, under the name of its `language`.
fn code_block(language: &str, text: &str) {
    let (language, text) = (language.to_owned(), text.to_owned());
    Card::new()
        .variant(CardVariant::Sunken)
        .size(Size::Md)
        .modifier(Modifier::new().fill_max_width())
        .show(move || {
            if !language.is_empty() {
                Label::new(language)
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .modifier(Modifier::new().padding_each(0.0, 0.0, 0.0, 6.0))
                    .show();
            }
            Label::new(text)
                .variant(LabelVariant::Code)
                .size(Size::Xs)
                .wrap()
                .show();
        });
}

/// Emits the TeX formula `text` set on lines of its own.
fn formula(text: &str, size: Size) {
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::Center,
        Alignment::Center,
        || {
            Figure::new(FigureSource::Formula(text.to_owned()))
                .size(size)
                .show()
        },
    );
}

/// Emits `inner` behind a bar.
fn quote(inner: &[Block], size: Size, scheme: &ColorScheme) {
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height_intrinsic(IntrinsicSize::Min),
        Arrangement::SpacedBy(10.0),
        Alignment::Start,
        || {
            spacer(
                Modifier::new()
                    .width(3.0)
                    .fill_max_height()
                    .background_rounded(scheme.outline_variant, 1.5),
            );
            column_with(Modifier::new().weight(1.0), || blocks(inner, size, scheme));
        },
    );
}

/// Emits a fold-out: `summary` that opens and closes `inner` when clicked.
fn fold_out(summary: &[Span], inner: &[Block], size: Size, scheme: &ColorScheme) {
    let open = remember(|| false);
    let toggle = open.clone();
    Card::new()
        .variant(CardVariant::Outlined)
        .size(Size::Sm)
        .modifier(Modifier::new().fill_max_width())
        .show(|| {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .cursor(CursorIcon::Pointer)
                    .clickable(move || toggle.set(!toggle.get())),
                Arrangement::SpacedBy(6.0),
                Alignment::Center,
                || {
                    icon(
                        if open.get() {
                            phosphor::CARET_DOWN
                        } else {
                            phosphor::CARET_RIGHT
                        },
                        12.0,
                        scheme.on_surface_variant,
                    );
                    let named: Vec<Span> = if summary.is_empty() {
                        vec![Span {
                            text: "Details".to_owned(),
                            inline: pm_markdown::Inline::default(),
                        }]
                    } else {
                        summary.to_vec()
                    };
                    rich_text_with(Modifier::new().weight(1.0), spans(&named, scheme));
                },
            );
            if open.get() {
                spacer(Modifier::new().height(8.0));
                blocks(inner, size, scheme);
            }
        });
}

/// Emits `images` side by side, in the middle when `centered`.
fn pictures(images: &[Image], centered: bool, size: Size) {
    row_arranged(
        Modifier::new().fill_max_width(),
        if centered {
            Arrangement::Center
        } else {
            Arrangement::SpacedBy(8.0)
        },
        Alignment::Center,
        || {
            for picture in images {
                Figure::new(FigureSource::Image {
                    source: picture.source.clone(),
                    alt: picture.alt.clone(),
                })
                .size(size)
                .show();
            }
        },
    );
}

/// Emits a table of `head` cells over `rows`, its columns sharing the width.
fn table(head: &[Vec<Span>], rows: &[Vec<Vec<Span>>], scheme: &ColorScheme) {
    let columns = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    let row = |cells: &[Vec<Span>], header: bool| {
        row_arranged(
            Modifier::new()
                .fill_max_width()
                .background(if header {
                    scheme.surface_container_high
                } else {
                    Color::TRANSPARENT
                })
                .border_bottom_hairline(scheme.outline_variant),
            Arrangement::Start,
            Alignment::Start,
            || {
                for column in 0..columns {
                    let mut content = cells.get(column).cloned().unwrap_or_default();
                    if header {
                        for span in &mut content {
                            span.inline.strong = true;
                        }
                    }
                    rich_text_with(
                        Modifier::new().weight(1.0).padding_xy(10.0, 6.0),
                        spans(&content, scheme),
                    );
                }
            },
        );
    };
    column_with(
        Modifier::new()
            .fill_max_width()
            .border(1.0, scheme.outline_variant, 6.0)
            .clip(),
        || {
            row(head, true);
            for cells in rows {
                row(cells, false);
            }
        },
    );
}

/// The text spans of `content`, each set as its markdown says.
pub fn spans(content: &[Span], scheme: &ColorScheme) -> Vec<TextSpan> {
    content
        .iter()
        .map(|span| {
            let mut text = TextSpan::new(span.text.clone());
            let inline = span.inline;
            if inline.strong {
                text = text.bold();
            }
            if inline.emphasis {
                text.italic = Some(true);
            }
            if inline.strike {
                text.strikethrough = Some(true);
            }
            if inline.link {
                text.color = Some(scheme.primary);
                text.underline = Some(true);
            }
            if inline.code {
                text.font_family = Some(FontFamily::Monospace);
                text.background = Some(scheme.surface_container_high);
                text.font_size = Some(12.0);
            }
            text
        })
        .collect()
}
