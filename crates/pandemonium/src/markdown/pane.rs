//! The pane a markdown document is read in as it renders.
//!
//! The page is a column no wider than prose reads well at, scrolled inside
//! the pane: headings in the interface's own faces, paragraphs wrapped to the
//! column, code in the colours its language has in an editor pane, and the
//! pictures a document names drawn where it names them.

use std::path::Path;

use pm_gfx::Rgba;
use pm_text::Language;
use pm_ui::{
    Div, Font, Paragraph, Scrolled, Styled, TextSize, Theme, h_flex, paragraph, picture,
    scroll_area, text, v_flex,
};

use crate::editor::code_lines;
use crate::markdown::Renders;
use crate::markdown::blocks::{Block, Emphasis, Item, Run};
use crate::message::Message;

/// Widest the column of prose is drawn.
const COLUMN: f32 = 820.0;

/// How wide the mark before an item of a list is.
const MARKER: f32 = 24.0;

/// How wide the bar down the side of a quotation is.
const QUOTE_BAR: f32 = 3.0;

/// Builds the pane rendering `source`, the text of the document at `path`.
///
/// `blocks` is what the source parsed into and `scroll` how far down it the
/// pane is; `renders` hands over the pictures the document names, read from
/// beside it.
pub fn rendered_pane(
    theme: &Theme,
    blocks: &[Block],
    scroll: Scrolled,
    path: &Path,
    renders: &Renders,
) -> Div<Message> {
    let folder = path.parent().unwrap_or(Path::new(""));
    let page = Page {
        theme,
        folder,
        renders,
    };
    let column = v_flex()
        .w_full()
        .max_w_px(COLUMN)
        .mx_auto()
        .px(4)
        .py(3)
        .gap(1.5)
        .items_stretch()
        .children(blocks.iter().map(|block| page.block(block)));

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(
            scroll_area(scroll, v_flex().w_full().child(column))
                .w_full()
                .flex_1(),
        )
}

/// What every block of one page is drawn against.
struct Page<'a> {
    /// The tokens the page is drawn from.
    theme: &'a Theme,
    /// Where the document is, which is where the pictures it names are
    /// found from.
    folder: &'a Path,
    /// Where those pictures are read and kept.
    renders: &'a Renders,
}

impl Page<'_> {
    /// Builds one block, whichever kind of block it is.
    fn block(&self, block: &Block) -> Div<Message> {
        match block {
            Block::Heading(depth, runs) => self.heading(*depth, runs),
            Block::Paragraph(runs) => v_flex().w_full().child(self.prose(
                runs,
                Font::new(TextSize::Base),
                self.theme.colors.text,
            )),
            Block::Code(language, code) => self.code(language.as_deref(), code),
            Block::Quote(blocks) => self.quote(blocks),
            Block::List(first, items) => self.list(*first, items),
            Block::Table(rows) => self.table(rows),
            Block::Rule => v_flex()
                .w_full()
                .h_px(1.0)
                .bg(self.theme.colors.border_variant),
            Block::Picture(target, said) => self.picture(target, said),
        }
    }

    /// Builds a heading, in the step of the scale its depth calls for, with a
    /// rule under the two that divide a document into parts.
    fn heading(&self, depth: usize, runs: &[Run]) -> Div<Message> {
        let size = match depth {
            1 => TextSize::Xxl,
            2 => TextSize::Xl,
            3 => TextSize::Lg,
            _ => TextSize::Base,
        };
        let font = Font::new(size).weight(600);

        v_flex()
            .w_full()
            .pt(1)
            .gap(0.75)
            .child(self.prose(runs, font, self.theme.colors.text))
            .when(depth <= 2, |heading| {
                heading.child(
                    v_flex()
                        .w_full()
                        .h_px(1.0)
                        .bg(self.theme.colors.border_variant),
                )
            })
    }

    /// Builds the runs of a passage as one paragraph wrapped to the column.
    fn prose(&self, runs: &[Run], font: Font, color: Rgba) -> Paragraph {
        let theme = self.theme;
        runs.iter().fold(paragraph().w_full(), |paragraph, run| {
            let Emphasis {
                strong,
                italic,
                code,
                struck,
                link,
            } = run.emphasis;
            let mut face = font;
            if strong {
                face = face.weight(700);
            }
            if italic {
                face = face.italic();
            }
            let color = match (link, struck) {
                (true, _) => theme.colors.link,
                (_, true) => theme.colors.text_subtle,
                _ => color,
            };
            match (code, link) {
                (true, _) => paragraph.marked(
                    run.text.clone(),
                    face.mono().size(TextSize::Sm),
                    color,
                    theme.colors.surface,
                ),
                (false, true) => paragraph.underlined(run.text.clone(), face, color),
                (false, false) => paragraph.span(run.text.clone(), face, color),
            }
        })
    }

    /// Builds a block of code, coloured the way its language is.
    fn code(&self, language: Option<&str>, code: &str) -> Div<Message> {
        let language = language.and_then(Language::fenced);
        let lines = code.trim_end_matches('\n').lines().collect::<Vec<_>>();
        let rows = code_lines(self.theme, language, &lines)
            .into_iter()
            .map(|line| {
                let runs = match line.is_empty() {
                    true => vec![(String::from(" "), Rgba::TRANSPARENT)],
                    false => line,
                };
                h_flex().children(
                    runs.into_iter()
                        .map(|(run, color)| text(run).text_sm().font_mono().color(color)),
                )
            });

        v_flex()
            .w_full()
            .px(1.5)
            .py(1)
            .overflow_hidden()
            .rounded(self.theme.radius.md)
            .bg(self.theme.colors.surface)
            .children(rows)
    }

    /// Builds a quotation: its blocks, beside a bar that says it is quoted.
    fn quote(&self, blocks: &[Block]) -> Div<Message> {
        h_flex()
            .w_full()
            .gap(1.5)
            .items_stretch()
            .child(
                v_flex()
                    .w_px(QUOTE_BAR)
                    .rounded(self.theme.radius.sm)
                    .bg(self.theme.colors.border),
            )
            .child(
                v_flex()
                    .flex_1()
                    .gap(1)
                    .children(blocks.iter().map(|block| self.block(block))),
            )
    }

    /// Builds a list, numbered from `first` or bulleted, item by item.
    fn list(&self, first: Option<u64>, items: &[Item]) -> Div<Message> {
        v_flex()
            .w_full()
            .gap(0.5)
            .children(items.iter().enumerate().map(|(index, item)| {
                let mark = match (item.task, first) {
                    (Some(true), _) => "☑".to_owned(),
                    (Some(false), _) => "☐".to_owned(),
                    (None, Some(first)) => format!("{}.", first + index as u64),
                    (None, None) => "•".to_owned(),
                };
                h_flex()
                    .w_full()
                    .items_start()
                    .child(
                        h_flex()
                            .w_px(MARKER)
                            .child(text(mark).text_base().color(self.theme.colors.text_muted)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap(0.5)
                            .children(item.blocks.iter().map(|block| self.block(block))),
                    )
            }))
    }

    /// Builds a table, its heading row set apart from the rows under it.
    fn table(&self, rows: &[Vec<Vec<Run>>]) -> Div<Message> {
        let theme = self.theme;
        v_flex()
            .w_full()
            .border_1(theme.colors.border_variant)
            .rounded(theme.radius.md)
            .overflow_hidden()
            .children(rows.iter().enumerate().map(|(index, row)| {
                let heading = index == 0;
                let font = match heading {
                    true => Font::new(TextSize::Sm).weight(600),
                    false => Font::new(TextSize::Sm),
                };
                h_flex()
                    .w_full()
                    .items_stretch()
                    .when(heading, |row| row.bg(theme.colors.surface))
                    .children(row.iter().map(|cell| {
                        v_flex()
                            .flex_1()
                            .px(1)
                            .py(0.5)
                            .border_1(theme.colors.border_variant)
                            .child(self.prose(cell, font, theme.colors.text))
                    }))
            }))
    }

    /// Builds a picture the document names, or what it shows in words when
    /// it is somewhere the editor does not read from.
    fn picture(&self, target: &str, said: &str) -> Div<Message> {
        let local = !target.contains("://");
        let found = local
            .then(|| self.renders.picture(&self.folder.join(target)))
            .flatten();
        match found {
            Some(image) => h_flex().w_full().child(picture(image)),
            None => v_flex().w_full().child(
                paragraph()
                    .span(
                        match said.is_empty() {
                            true => target.to_owned(),
                            false => said.to_owned(),
                        },
                        Font::new(TextSize::Sm).italic(),
                        self.theme.colors.text_subtle,
                    )
                    .w_full(),
            ),
        }
    }
}
