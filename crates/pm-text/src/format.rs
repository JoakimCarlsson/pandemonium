//! Paragraph wrapping shared by modal editing and Markdown formatting.

use std::ops::Range;

use tree_sitter::{Node, Parser};

use crate::{Buffer, Position};

/// Comment and prose markers kept by wrapping in languages other than Markdown.
const LEADERS: [&str; 11] = [
    "///", "//!", "//", "#", "--", ";;", ";", "*", ">", "- ", "%",
];

/// Wraps prose in a Markdown buffer, leaving every other block untouched.
pub fn reflow_markdown(buffer: &mut Buffer, width: usize) {
    if buffer
        .language()
        .is_some_and(|language| language.name() == "Markdown")
    {
        rewrap(buffer, 0, buffer.line_count().saturating_sub(1), width);
    }
}

/// Wraps paragraphs between `first` and `last`, keeping indentation and markers.
/// Markdown uses its block tree to select prose and preserves hard line breaks.
pub fn rewrap(buffer: &mut Buffer, first: usize, last: usize, width: usize) {
    if buffer
        .language()
        .is_some_and(|language| language.name() == "Markdown")
    {
        let edits = buffer
            .syntax_nodes(&|kind| kind == "paragraph")
            .into_iter()
            .filter(|node| node.range.start.line >= first && node.range.start.line <= last)
            .filter(|node| {
                buffer
                    .syntax_around(node.range.start, &|kind| kind == "setext_heading")
                    .is_empty()
            })
            .filter_map(|node| markdown_edit(buffer, node.range, width))
            .collect::<Vec<_>>();
        if !edits.is_empty() {
            buffer.apply_edits(edits);
        }
        return;
    }
    let mut out = Vec::new();
    let mut paragraph = Vec::new();
    let mut prefix = String::new();
    for line in first..=last {
        let text = buffer.line_text(line);
        let (lead, body) = split_leader(&text);
        if body.trim().is_empty() || (!paragraph.is_empty() && lead != prefix) {
            wrap_paragraph(&mut paragraph, &prefix, &prefix, width, false, &mut out);
        }
        if body.trim().is_empty() {
            out.push(text.trim_end().to_owned());
        } else {
            prefix = lead;
            paragraph.push(body.to_owned());
        }
    }
    wrap_paragraph(&mut paragraph, &prefix, &prefix, width, false, &mut out);
    let range = Position::new(first, 0)..Position::new(last, buffer.line_len(last));
    let wrapped = out.join("\n");
    if buffer.text_in(range.clone()) != wrapped {
        buffer.grouped(|buffer| buffer.replace(range, &wrapped));
    }
}

/// Replaces one parsed Markdown paragraph, including its container prefixes.
fn markdown_edit(
    buffer: &Buffer,
    span: Range<Position>,
    width: usize,
) -> Option<(Range<Position>, String)> {
    let first = span.start.line;
    let ends_in_prefix = buffer
        .line_text(span.end.line)
        .chars()
        .take(span.end.column)
        .all(|ch| matches!(ch, ' ' | '\t' | '>'));
    let last = if ends_in_prefix && span.end.line > first {
        span.end.line - 1
    } else {
        span.end.line
    };
    let text = buffer.line_text(first);
    let prefix = text.chars().take(span.start.column).collect::<String>();
    let continuation = continuation_prefix(&prefix);
    let mut bodies = Vec::new();
    for line in first..=last {
        let text = buffer.line_text(line);
        let body = if line == first {
            text.chars().skip(span.start.column).collect::<String>()
        } else {
            text.trim_start_matches([' ', '\t', '>']).to_owned()
        };
        bodies.push(body);
    }
    let inline = inline_ranges(&bodies.join("\n"));
    let mut offset = 0;
    let mut paragraph = Vec::new();
    let mut out = Vec::new();
    for body in &bodies {
        let end = offset + body.len();
        let inside = inline
            .iter()
            .any(|span| span.start <= end && end < span.end);
        offset = end + 1;
        let trimmed = if inside {
            body
        } else {
            body.trim_end_matches([' ', '\t'])
        };
        let spaces = body.len() - trimmed.len();
        let backslashes = trimmed.chars().rev().take_while(|ch| *ch == '\\').count();
        let hard = !inside && (backslashes % 2 == 1 || body.ends_with("  "));
        paragraph.push(trimmed.to_owned());
        if hard {
            let lead = if out.is_empty() {
                &prefix
            } else {
                &continuation
            };
            wrap_paragraph(
                &mut paragraph,
                lead,
                &continuation,
                width.saturating_sub(if body.ends_with("  ") { spaces } else { 0 }),
                true,
                &mut out,
            );
            if body.ends_with("  ")
                && let Some(line) = out.last_mut()
            {
                line.push_str(&" ".repeat(spaces));
            }
        }
    }
    let lead = if out.is_empty() {
        &prefix
    } else {
        &continuation
    };
    wrap_paragraph(&mut paragraph, lead, &continuation, width, true, &mut out);
    let range = Position::new(first, 0)..Position::new(last, buffer.line_len(last));
    let original = buffer.text_in(range.clone());
    let crlf = first + 1 < buffer.line_count()
        && buffer
            .text_in(Position::new(first, 0)..Position::new(first + 1, 0))
            .ends_with("\r\n");
    let newline = if crlf { "\r\n" } else { "\n" };
    let wrapped = out.join(newline);
    (original != wrapped).then_some((range, wrapped))
}

/// Keeps quote markers and replaces a list marker with its indentation width.
fn continuation_prefix(prefix: &str) -> String {
    prefix
        .chars()
        .map(|ch| {
            if ch == '>' || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect()
}

/// Writes a paragraph with its first and subsequent line prefixes.
fn wrap_paragraph(
    paragraph: &mut Vec<String>,
    first: &str,
    continuation: &str,
    width: usize,
    markdown: bool,
    out: &mut Vec<String>,
) {
    if paragraph.is_empty() {
        return;
    }
    let text = paragraph.join(" ");
    let words = words(&text, markdown);
    let mut line = first.to_owned();
    let mut has_word = false;
    for word in words {
        if has_word && line.chars().count() + 1 + word.chars().count() > width {
            out.push(line);
            line = continuation.to_owned();
            has_word = false;
        }
        if has_word {
            line.push(' ');
        }
        line.push_str(word);
        has_word = true;
    }
    if has_word {
        out.push(line);
    }
    paragraph.clear();
}

/// Splits at whitespace outside Markdown code spans, links and images.
fn words(text: &str, markdown: bool) -> Vec<&str> {
    let protected = if markdown {
        inline_ranges(text)
    } else {
        Vec::new()
    };
    let mut words = Vec::new();
    let mut start = None;
    let mut spans = protected.into_iter().peekable();
    for (byte, ch) in text.char_indices() {
        while spans.peek().is_some_and(|span| span.end <= byte) {
            spans.next();
        }
        let inside = spans
            .peek()
            .is_some_and(|span| span.start <= byte && byte < span.end);
        if ch.is_whitespace() && !inside {
            if let Some(from) = start.take() {
                words.push(&text[from..byte]);
            }
        } else {
            start.get_or_insert(byte);
        }
    }
    if let Some(from) = start {
        words.push(&text[from..]);
    }
    words
}

/// Parses the inline ranges whose whitespace is part of an indivisible word.
fn inline_ranges(text: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_md::INLINE_LANGUAGE.into())
        .is_ok()
        && let Some(tree) = parser.parse(text, None)
    {
        inline_spans(tree.root_node(), &mut spans);
    }
    spans
}

/// Collects outermost inline constructs that must remain on one line.
fn inline_spans(node: Node<'_>, spans: &mut Vec<Range<usize>>) {
    if matches!(
        node.kind(),
        "code_span"
            | "inline_link"
            | "full_reference_link"
            | "collapsed_reference_link"
            | "shortcut_link"
            | "image"
    ) {
        spans.push(node.byte_range());
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        inline_spans(child, spans);
    }
}

/// Splits generic prose into its indentation and marker, and its body.
fn split_leader(text: &str) -> (String, &str) {
    let indent = text.len() - text.trim_start().len();
    let rest = &text[indent..];
    let marker = LEADERS
        .iter()
        .find(|leader| rest.starts_with(**leader))
        .map_or(0, |leader| leader.len());
    let after = &rest[marker..];
    let spaces = after.len() - after.trim_start().len();
    let split = indent + marker + spaces;
    let lead = if marker > 0 && spaces == 0 && !after.is_empty() {
        format!("{} ", &text[..split])
    } else {
        text[..split].to_owned()
    };
    (lead, &text[split..])
}
