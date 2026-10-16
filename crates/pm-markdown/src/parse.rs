//! Reading a markdown source into blocks.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::block::{Block, Inline, Span};

/// The blocks `source` is made of, with tables, strikethrough and task lists understood.
///
/// A source cut off part way, as a streamed reply is, parses as though every
/// open block closed where it ends.
pub fn parse(source: &str) -> Vec<Block> {
    read(source).root
}

/// The addresses of the links of `source`, in the order the text of each is numbered by [`Inline::target`].
pub fn links(source: &str) -> Vec<String> {
    read(source).targets
}

/// `source` read through.
fn read(source: &str) -> Builder {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut builder = Builder::default();
    for event in Parser::new_ext(source, options) {
        builder.event(event);
    }
    builder.flush_prose();
    builder.close_text();
    builder.fold_details();
    builder
}

/// A block that holds other blocks and is still being read.
enum Container {
    /// A quote, and the blocks read into it so far.
    Quote(Vec<Block>),
    /// A list, and the items read into it so far.
    List {
        /// The number of its first item when it is ordered.
        start: Option<u64>,
        /// The items read so far.
        items: Vec<Vec<Block>>,
    },
    /// An HTML `<details>`, and its summary and the blocks read into it so far.
    Details {
        /// The text of its `<summary>`.
        summary: Vec<Span>,
        /// The blocks read so far.
        blocks: Vec<Block>,
    },
    /// An item of a list, and the blocks read into it so far.
    Item(Vec<Block>),
    /// A table, and the cells read into it so far.
    Table {
        /// The cells of the heading row.
        head: Vec<Vec<Span>>,
        /// The rows read so far.
        rows: Vec<Vec<Vec<Span>>>,
        /// Whether the heading row is being read.
        in_head: bool,
    },
}

/// The block whose text is being read.
enum Open {
    /// A paragraph, said or implied by a tight list item.
    Paragraph,
    /// A heading of this level.
    Heading(u8),
    /// A code block in this language.
    Code(String),
    /// The `<summary>` of a details block.
    Summary,
    /// A cell of a table.
    Cell,
}

/// The state of reading one source.
#[derive(Default)]
struct Builder {
    /// The top-level blocks read so far.
    root: Vec<Block>,
    /// The containers open, innermost last.
    stack: Vec<Container>,
    /// The block whose text is being read, if any.
    open: Option<Open>,
    /// The text read into the open block so far.
    spans: Vec<Span>,
    /// The code read into the open code block so far.
    code: String,
    /// How many strong runs are open.
    strong: u32,
    /// How many emphasised runs are open.
    emphasis: u32,
    /// How many struck runs are open.
    strike: u32,
    /// How many links are open.
    link: u32,
    /// The addresses of the links read so far, in order.
    targets: Vec<String>,
    /// The links open, innermost last, by their number among `targets`.
    open_links: Vec<u16>,
    /// How many HTML code or key tags are open.
    monospace: u32,
    /// Running text read but not yet added, gathered so that a web address the parser split is found whole.
    prose: String,
}

impl Builder {
    /// Takes one event of the parser into the blocks.
    fn event(&mut self, event: Event) {
        if let Event::Text(text) = &event
            && !matches!(self.open, Some(Open::Code(_)))
        {
            return self.prose.push_str(text);
        }
        self.flush_prose();
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.code.push_str(&text),
            Event::Code(text) => self.text(
                &text,
                Inline {
                    code: true,
                    ..self.inline()
                },
            ),
            Event::Html(text) | Event::InlineHtml(text) => self.html(&text),
            Event::SoftBreak => self.text(" ", self.inline()),
            Event::HardBreak => self.text("\n", self.inline()),
            Event::Rule => {
                self.close_text();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(done) => self.text(
                if done { "[x] " } else { "[ ] " },
                Inline {
                    code: true,
                    ..Inline::default()
                },
            ),
            _ => {}
        }
    }

    /// Opens what `tag` starts.
    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                self.close_text();
                self.open = Some(Open::Paragraph);
            }
            Tag::Heading { level, .. } => {
                self.close_text();
                self.open = Some(Open::Heading(level as u8));
            }
            Tag::CodeBlock(kind) => {
                self.close_text();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_owned()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                self.open = Some(Open::Code(language));
            }
            Tag::BlockQuote(_) => self.open_container(Container::Quote(Vec::new())),
            Tag::List(start) => self.open_container(Container::List {
                start,
                items: Vec::new(),
            }),
            Tag::Item => self.open_container(Container::Item(Vec::new())),
            Tag::Table(_) => self.open_container(Container::Table {
                head: Vec::new(),
                rows: Vec::new(),
                in_head: false,
            }),
            Tag::TableHead => {
                if let Some(Container::Table { in_head, .. }) = self.stack.last_mut() {
                    *in_head = true;
                }
            }
            Tag::TableRow => {
                if let Some(Container::Table { rows, .. }) = self.stack.last_mut() {
                    rows.push(Vec::new());
                }
            }
            Tag::TableCell => self.open = Some(Open::Cell),
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { dest_url, .. } => {
                self.link += 1;
                self.targets.push(dest_url.into_string());
                self.open_links.push(self.targets.len() as u16);
            }
            _ => {}
        }
    }

    /// Closes what `tag` ends.
    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.close_text(),
            TagEnd::CodeBlock => {
                if let Some(Open::Code(language)) = self.open.take() {
                    let mut text = std::mem::take(&mut self.code);
                    if text.ends_with('\n') {
                        text.pop();
                    }
                    self.push_block(Block::Code { language, text });
                }
            }
            TagEnd::BlockQuote(_) => {
                self.fold_details();
                self.close_text();
                if let Some(Container::Quote(blocks)) = self.stack.pop() {
                    self.push_block(Block::Quote(blocks));
                }
            }
            TagEnd::Item => {
                self.fold_details();
                self.close_text();
                if let Some(Container::Item(blocks)) = self.stack.pop()
                    && let Some(Container::List { items, .. }) = self.stack.last_mut()
                {
                    items.push(blocks);
                }
            }
            TagEnd::List(_) => {
                self.fold_details();
                self.close_text();
                if let Some(Container::List { start, items }) = self.stack.pop() {
                    self.push_block(Block::List { start, items });
                }
            }
            TagEnd::TableHead => {
                if let Some(Container::Table { in_head, .. }) = self.stack.last_mut() {
                    *in_head = false;
                }
            }
            TagEnd::TableCell => {
                self.open = None;
                let cell = std::mem::take(&mut self.spans);
                if let Some(Container::Table {
                    head,
                    rows,
                    in_head,
                }) = self.stack.last_mut()
                {
                    match (*in_head, rows.last_mut()) {
                        (true, _) | (false, None) => head.push(cell),
                        (false, Some(row)) => row.push(cell),
                    }
                }
            }
            TagEnd::Table => {
                self.fold_details();
                if let Some(Container::Table { head, rows, .. }) = self.stack.pop() {
                    self.push_block(Block::Table { head, rows });
                }
            }
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link => {
                self.link = self.link.saturating_sub(1);
                self.open_links.pop();
            }
            _ => {}
        }
    }

    /// How text read now is set, from the runs open around it.
    fn inline(&self) -> Inline {
        Inline {
            strong: self.strong > 0,
            emphasis: self.emphasis > 0,
            code: self.monospace > 0,
            strike: self.strike > 0,
            link: self.link > 0,
            target: self.open_links.last().copied().unwrap_or(0),
        }
    }

    /// Reads embedded HTML: its tags set the text between them as markdown
    /// would, or break it into blocks, and are never shown themselves.
    fn html(&mut self, source: &str) {
        let mut rest = source;
        while let Some(open) = rest.find('<') {
            self.html_text(&rest[..open]);
            let tag = &rest[open + 1..];
            if let Some(comment) = tag.strip_prefix("!--") {
                rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
                continue;
            }
            let Some(close) = tag.find('>') else {
                self.html_text(&rest[open..]);
                return;
            };
            self.tag(&tag[..close]);
            rest = &tag[close + 1..];
        }
        self.html_text(rest);
    }

    /// Adds the text between HTML tags, its line breaks read as spaces, and
    /// keeps the room between tags only inside text already open.
    fn html_text(&mut self, text: &str) {
        let text = text.replace('\n', " ");
        if text.trim().is_empty() {
            if !text.is_empty() && self.open.is_some() {
                self.text(" ", self.inline());
            }
            return;
        }
        let text = match self.open {
            Some(_) => text.as_str(),
            None => text.trim_start(),
        };
        self.text(text, self.inline());
    }

    /// Takes one HTML tag, the text between its angle brackets: inline tags
    /// set the text they hold, a break breaks the line, a details block holds
    /// the blocks up to its end under its summary and any other tag ends the text open.
    fn tag(&mut self, inside: &str) {
        let closing = inside.starts_with('/');
        let name = inside
            .trim_start_matches('/')
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        let count = match name.as_str() {
            "b" | "strong" => &mut self.strong,
            "i" | "em" => &mut self.emphasis,
            "s" | "del" | "strike" => &mut self.strike,
            "a" => &mut self.link,
            "code" | "kbd" | "tt" => &mut self.monospace,
            "br" => return self.text("\n", self.inline()),
            "details" if closing => {
                self.close_text();
                if let Some(Container::Details { summary, blocks }) = self
                    .stack
                    .pop_if(|top| matches!(top, Container::Details { .. }))
                {
                    self.push_block(Block::Details { summary, blocks });
                }
                return;
            }
            "details" => {
                return self.open_container(Container::Details {
                    summary: Vec::new(),
                    blocks: Vec::new(),
                });
            }
            "summary" if closing => {
                if let Some(Open::Summary) = self.open {
                    self.open = None;
                    let spans = std::mem::take(&mut self.spans);
                    if let Some(Container::Details { summary, .. }) = self.stack.last_mut() {
                        *summary = spans;
                    }
                }
                return;
            }
            "summary" => {
                self.close_text();
                self.open = Some(Open::Summary);
                return;
            }
            "span" | "sup" | "sub" | "u" | "mark" | "small" | "abbr" => return,
            _ => return self.close_text(),
        };
        *count = if closing {
            count.saturating_sub(1)
        } else {
            *count + 1
        };
    }

    /// Adds the running text gathered to the open block, each bare web
    /// address in it outside a link made a link to itself.
    fn flush_prose(&mut self) {
        let text = std::mem::take(&mut self.prose);
        let mut rest = text.as_str();
        while self.link == 0
            && let Some((start, end)) = bare_address(rest)
        {
            self.text(&rest[..start], self.inline());
            self.targets.push(rest[start..end].to_owned());
            let inline = Inline {
                link: true,
                target: self.targets.len() as u16,
                ..self.inline()
            };
            self.text(&rest[start..end], inline);
            rest = &rest[end..];
        }
        if !rest.is_empty() {
            self.text(rest, self.inline());
        }
    }

    /// Adds `text` set as `inline` to the open block, opening a paragraph
    /// when none is, as a tight list item's text needs.
    fn text(&mut self, text: &str, inline: Inline) {
        if self.open.is_none() {
            self.open = Some(Open::Paragraph);
        }
        match self.spans.last_mut() {
            Some(last) if last.inline == inline && last.text != "\n" && text != "\n" => {
                last.text.push_str(text)
            }
            _ => self.spans.push(Span {
                text: text.to_owned(),
                inline,
            }),
        }
    }

    /// Ends the open text and opens `container`, where blocks now go.
    fn open_container(&mut self, container: Container) {
        self.close_text();
        self.stack.push(container);
    }

    /// Ends the open paragraph or heading, if one is, adding it to the blocks.
    fn close_text(&mut self) {
        let level = match self.open {
            Some(Open::Paragraph) => None,
            Some(Open::Heading(level)) => Some(level),
            _ => return,
        };
        self.open = None;
        let spans = std::mem::take(&mut self.spans);
        if spans.is_empty() {
            return;
        }
        self.push_block(match level {
            Some(level) => Block::Heading { level, spans },
            None => Block::Paragraph(spans),
        });
    }

    /// Closes every `<details>` left open on top of the containers, as the
    /// end of a markdown container or of the source does.
    fn fold_details(&mut self) {
        self.close_text();
        while let Some(Container::Details { summary, blocks }) = self
            .stack
            .pop_if(|top| matches!(top, Container::Details { .. }))
        {
            self.push_block(Block::Details { summary, blocks });
        }
    }

    /// Adds `block` to the innermost container that holds blocks.
    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(
                Container::Quote(blocks)
                | Container::Item(blocks)
                | Container::Details { blocks, .. },
            ) => blocks.push(block),
            _ => self.root.push(block),
        }
    }
}

/// Where the first bare `http://` or `https://` address of `text` starts and
/// ends, the punctuation after it and a closing bracket it did not open left out.
pub fn bare_address(text: &str) -> Option<(usize, usize)> {
    let start = [text.find("https://"), text.find("http://")]
        .into_iter()
        .flatten()
        .min()?;
    let length = text[start..]
        .find(|character: char| character.is_whitespace() || "<>\"`".contains(character))
        .unwrap_or(text.len() - start);
    let mut address = &text[start..start + length];
    loop {
        let trimmed = address.trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', '*', '_']);
        let unopened =
            trimmed.ends_with(')') && trimmed.matches('(').count() < trimmed.matches(')').count();
        address = match unopened {
            true => &trimmed[..trimmed.len() - 1],
            false => trimmed,
        };
        if address.len() == trimmed.len() {
            break;
        }
    }
    let scheme = if address.starts_with("https://") {
        8
    } else {
        7
    };
    (address.len() > scheme).then_some((start, start + address.len()))
}
