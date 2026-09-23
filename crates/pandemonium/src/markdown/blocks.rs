//! A markdown document as the blocks it reads as.
//!
//! The parser speaks in events — a heading starts, some text, the heading
//! ends — and a pane is built from things, so the events are gathered here
//! into the blocks and runs a screen is made of. Only what a reader of a
//! README meets is kept: headings, paragraphs, lists, quotes, code, tables,
//! rules and pictures. The rest reads as the text it holds.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// How a run of text within a block is set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Emphasis {
    /// Set heavier, as `**strong**` asks.
    pub strong: bool,
    /// Set slanted, as `*emphasis*` asks.
    pub italic: bool,
    /// Set in the monospaced family, as `` `code` `` asks.
    pub code: bool,
    /// Struck through, as `~~this~~` asks.
    pub struck: bool,
    /// Part of a link.
    pub link: bool,
}

/// One run of text within a block, all of it set one way.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Run {
    /// The characters of the run.
    pub text: String,
    /// How they are set.
    pub emphasis: Emphasis,
}

/// One block of a document, in the order it is read.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// A heading, at a depth from one to six, and what it says.
    Heading(usize, Vec<Run>),
    /// A paragraph of prose.
    Paragraph(Vec<Run>),
    /// A block of code, with the language its fence named.
    Code(Option<String>, String),
    /// A passage quoted from elsewhere, and the blocks it holds.
    Quote(Vec<Block>),
    /// A list, numbered from its first number or bulleted, and its items.
    List(Option<u64>, Vec<Item>),
    /// A table: its heading row, then the rows of its body.
    Table(Vec<Vec<Vec<Run>>>),
    /// A line across the page between one part and the next.
    Rule,
    /// A picture, where it is and what it shows in words.
    Picture(String, String),
}

/// One item of a list: whether it is a task and done, and what it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// Whether the item is a task, and if so whether it is done.
    pub task: Option<bool>,
    /// The blocks it holds.
    pub blocks: Vec<Block>,
}

/// The blocks `source` reads as.
pub fn blocks(source: &str) -> Vec<Block> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let mut reader = Reader {
        events: Parser::new_ext(source, options).collect(),
        at: 0,
    };
    reader.blocks_until(None)
}

/// The events of a document, read one block at a time.
struct Reader<'a> {
    /// Every event the parser gave.
    events: Vec<Event<'a>>,
    /// The next one to read.
    at: usize,
}

impl<'a> Reader<'a> {
    /// Reads blocks until the tag `end` closes, or to the end of the document.
    ///
    /// Text that stands in no paragraph of its own — the items of a tight
    /// list hold theirs bare — is gathered into one all the same, emphasis
    /// and all.
    fn blocks_until(&mut self, end: Option<TagEnd>) -> Vec<Block> {
        let mut blocks = Vec::new();
        while let Some(event) = self.next() {
            match event {
                Event::End(closed) if Some(closed) == end => break,
                Event::Start(Tag::Paragraph) => match self.lone_picture() {
                    Some(picture) => blocks.push(picture),
                    None => blocks.push(Block::Paragraph(self.runs_until(TagEnd::Paragraph))),
                },
                Event::Start(Tag::Heading { level, .. }) => {
                    let runs = self.runs_until(TagEnd::Heading(level));
                    blocks.push(Block::Heading(depth(level), runs));
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    let language = match kind {
                        CodeBlockKind::Fenced(tag) if !tag.trim().is_empty() => {
                            Some(tag.split_whitespace().next().unwrap_or_default().to_owned())
                        }
                        _ => None,
                    };
                    blocks.push(Block::Code(language, self.text_until(TagEnd::CodeBlock)));
                }
                Event::Start(Tag::BlockQuote(kind)) => {
                    blocks.push(Block::Quote(
                        self.blocks_until(Some(TagEnd::BlockQuote(kind))),
                    ));
                }
                Event::Start(Tag::List(first)) => {
                    blocks.push(Block::List(first, self.items_until(first.is_some())));
                }
                Event::Start(Tag::Table(_)) => blocks.push(Block::Table(self.rows())),
                Event::Start(Tag::HtmlBlock) => {
                    let html = self.text_until(TagEnd::HtmlBlock);
                    blocks.push(Block::Code(Some("html".to_owned()), html));
                }
                Event::Rule => blocks.push(Block::Rule),
                event if is_inline(&event) => {
                    self.at -= 1;
                    let runs = self.runs(&|next| !is_inline(next));
                    blocks.push(Block::Paragraph(runs));
                }
                _ => {}
            }
        }
        blocks
    }

    /// The picture a paragraph just begun holds and nothing else, taking
    /// the whole paragraph, or nothing when it holds more than that.
    ///
    /// A picture on a line of its own is drawn; one in the middle of a
    /// sentence is read as what it shows in words.
    fn lone_picture(&mut self) -> Option<Block> {
        let Some(Event::Start(Tag::Image { dest_url, .. })) = self.events.get(self.at).cloned()
        else {
            return None;
        };
        let closes = self.events[self.at..]
            .iter()
            .position(|event| matches!(event, Event::End(TagEnd::Image)))?;
        let after = self.events.get(self.at + closes + 1)?;
        if !matches!(after, Event::End(TagEnd::Paragraph)) {
            return None;
        }
        self.at += 1;
        let said = self.text_until(TagEnd::Image);
        self.at += 1;
        Some(Block::Picture(dest_url.into_string(), said))
    }

    /// Reads the items of a list until it closes.
    ///
    /// A tight list's items hold their text bare rather than in paragraphs,
    /// which [`Self::blocks_until`] gathers into one all the same.
    fn items_until(&mut self, ordered: bool) -> Vec<Item> {
        let mut items = Vec::new();
        while let Some(event) = self.next() {
            match event {
                Event::End(TagEnd::List(closed)) if closed == ordered => break,
                Event::Start(Tag::Item) => {
                    let task = match self.events.get(self.at) {
                        Some(Event::TaskListMarker(done)) => {
                            let done = *done;
                            self.at += 1;
                            Some(done)
                        }
                        _ => None,
                    };
                    let blocks = self.blocks_until(Some(TagEnd::Item));
                    items.push(Item { task, blocks });
                }
                _ => {}
            }
        }
        items
    }

    /// Reads the rows of a table until it closes, the heading row first.
    fn rows(&mut self) -> Vec<Vec<Vec<Run>>> {
        let mut rows = Vec::new();
        let mut row = Vec::new();
        while let Some(event) = self.next() {
            match event {
                Event::End(TagEnd::Table) => break,
                Event::Start(Tag::TableCell) => row.push(self.runs_until(TagEnd::TableCell)),
                Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                    rows.push(std::mem::take(&mut row));
                }
                _ => {}
            }
        }
        rows
    }

    /// Reads the runs of an inline passage until the tag `end` closes it,
    /// taking the close as well.
    fn runs_until(&mut self, end: TagEnd) -> Vec<Run> {
        let runs = self.runs(&|next| matches!(next, Event::End(closed) if *closed == end));
        self.at += 1;
        runs
    }

    /// Reads the runs of an inline passage up to the first event `stop`
    /// accepts, leaving that event to be read next.
    fn runs(&mut self, stop: &dyn Fn(&Event<'_>) -> bool) -> Vec<Run> {
        let mut runs: Vec<Run> = Vec::new();
        let mut emphasis = Emphasis::default();
        while let Some(event) = self.events.get(self.at).cloned() {
            if stop(&event) {
                break;
            }
            self.at += 1;
            let (text, set) = match event {
                Event::Start(Tag::Strong) => {
                    emphasis.strong = true;
                    continue;
                }
                Event::End(TagEnd::Strong) => {
                    emphasis.strong = false;
                    continue;
                }
                Event::Start(Tag::Emphasis) => {
                    emphasis.italic = true;
                    continue;
                }
                Event::End(TagEnd::Emphasis) => {
                    emphasis.italic = false;
                    continue;
                }
                Event::Start(Tag::Strikethrough) => {
                    emphasis.struck = true;
                    continue;
                }
                Event::End(TagEnd::Strikethrough) => {
                    emphasis.struck = false;
                    continue;
                }
                Event::Start(Tag::Link { .. }) => {
                    emphasis.link = true;
                    continue;
                }
                Event::End(TagEnd::Link) => {
                    emphasis.link = false;
                    continue;
                }
                Event::Start(Tag::Image { .. }) => (self.text_until(TagEnd::Image), emphasis),
                Event::Code(code) => (
                    code.into_string(),
                    Emphasis {
                        code: true,
                        ..emphasis
                    },
                ),
                Event::Text(text) | Event::InlineHtml(text) | Event::InlineMath(text) => {
                    (text.into_string(), emphasis)
                }
                Event::SoftBreak => (" ".to_owned(), emphasis),
                Event::HardBreak => ("\n".to_owned(), emphasis),
                Event::TaskListMarker(done) => (marker(done).to_owned(), emphasis),
                _ => continue,
            };
            match runs.last_mut() {
                Some(last) if last.emphasis == set => last.text.push_str(&text),
                _ => runs.push(Run {
                    text,
                    emphasis: set,
                }),
            }
        }
        runs
    }

    /// Reads the plain text of a passage until the tag `end` closes it.
    fn text_until(&mut self, end: TagEnd) -> String {
        let mut text = String::new();
        while let Some(event) = self.next() {
            match event {
                Event::End(closed) if closed == end => break,
                Event::Text(said) | Event::Code(said) | Event::Html(said) => text.push_str(&said),
                Event::SoftBreak | Event::HardBreak => text.push('\n'),
                _ => {}
            }
        }
        text
    }

    /// The next event, taking it.
    fn next(&mut self) -> Option<Event<'a>> {
        let event = self.events.get(self.at).cloned()?;
        self.at += 1;
        Some(event)
    }
}

/// Whether `event` is part of a passage of text rather than a block.
fn is_inline(event: &Event<'_>) -> bool {
    matches!(
        event,
        Event::Text(_)
            | Event::Code(_)
            | Event::InlineHtml(_)
            | Event::InlineMath(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::TaskListMarker(_)
            | Event::Start(
                Tag::Strong
                    | Tag::Emphasis
                    | Tag::Strikethrough
                    | Tag::Link { .. }
                    | Tag::Image { .. }
            )
            | Event::End(
                TagEnd::Strong
                    | TagEnd::Emphasis
                    | TagEnd::Strikethrough
                    | TagEnd::Link
                    | TagEnd::Image
            )
    )
}

/// How deep a heading of `level` is, one being the top.
fn depth(level: HeadingLevel) -> usize {
    level as usize
}

/// What a task in a list is drawn with, done or not.
fn marker(done: bool) -> &'static str {
    match done {
        true => "☑ ",
        false => "☐ ",
    }
}
