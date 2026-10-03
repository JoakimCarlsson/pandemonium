//! Markdown rendered as an HTML fragment for the clipboard.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Parser, Tag, html};

use super::blocks::parser_options;

/// Renders `source` with the document's extensions, without generated styles or classes.
pub fn render_html(source: &str) -> String {
    let events = Parser::new_ext(source, parser_options()).map(|event| match event {
        Event::Start(Tag::CodeBlock(_)) => Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)),
        Event::Start(Tag::Table(alignments)) => {
            Event::Start(Tag::Table(vec![Alignment::None; alignments.len()]))
        }
        Event::Start(Tag::BlockQuote(_)) => Event::Start(Tag::BlockQuote(None)),
        event => event,
    });
    let mut rendered = String::new();
    html::push_html(&mut rendered, events);
    rendered
}
