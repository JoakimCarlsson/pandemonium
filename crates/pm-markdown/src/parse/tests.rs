//! Tests for how raw HTML in a markdown source is read.

use super::*;

/// The text of every span of `spans`, joined.
fn words(spans: &[Span]) -> String {
    spans.iter().map(|span| span.text.as_str()).collect()
}

/// A whole HTML document, with blank lines inside it, is one `html` code block holding its source, and the markdown after it is read as markdown.
#[test]
fn a_whole_document_shows_as_its_source() {
    let document = "<!doctype html>\n<html>\n<head>\n<style>\nbody { color: red; }\n\np { margin: 0; }\n</style>\n</head>\n\n<body>\n<p>Hello</p>\n</body>\n</html>";
    let source = format!("Here it is:\n\n{document}\n\nDone.");
    let blocks = parse(&source);
    assert_eq!(blocks.len(), 3);
    assert_eq!(
        blocks[1],
        Block::Code {
            language: "html".to_owned(),
            text: document.to_owned(),
        }
    );
    assert!(matches!(&blocks[2], Block::Paragraph(spans) if words(spans) == "Done."));
}

/// A document cut off before its `</html>`, as a streamed reply is, is shown as source up to where it ends.
#[test]
fn a_document_cut_off_shows_as_source() {
    let blocks = parse("<html>\n<head><style>\nbody {}\n\n.a {}");
    assert_eq!(
        blocks,
        [Block::Code {
            language: "html".to_owned(),
            text: "<html>\n<head><style>\nbody {}\n\n.a {}".to_owned(),
        }]
    );
}

/// A document inside a fenced code block stays in that block as it was written.
#[test]
fn a_document_in_a_fence_is_left_alone() {
    let blocks = parse("```xml\n<html>\n</html>\n```");
    assert_eq!(
        blocks,
        [Block::Code {
            language: "xml".to_owned(),
            text: "<html>\n</html>".to_owned(),
        }]
    );
}

/// A block of HTML with a tag beyond those understood, such as a `<style>` block, shows as its source, never as prose.
#[test]
fn a_foreign_block_shows_as_its_source() {
    let blocks = parse("<style>\n.card { display: grid; }\n</style>\n\nAfter.");
    assert_eq!(
        blocks[0],
        Block::Code {
            language: "html".to_owned(),
            text: "<style>\n.card { display: grid; }\n</style>".to_owned(),
        }
    );
    assert!(matches!(&blocks[1], Block::Paragraph(spans) if words(spans) == "After."));
}

/// What a `<style>` or `<script>` holds within running text is left out, and the words around it kept.
#[test]
fn inline_style_and_script_are_left_out() {
    let blocks = parse("Before <style>p{}</style>middle <script>run()</script>after.");
    assert!(
        matches!(&blocks[..], [Block::Paragraph(spans)] if words(spans) == "Before middle after.")
    );
}

/// Blocks of the HTML tags understood keep being read as text and pictures, and inline tags still set the words they hold.
#[test]
fn understood_html_is_still_read() {
    let blocks =
        parse("<p align=\"center\"><img src=\"logo.png\" alt=\"Logo\"></p>\n\nSay <b>hi</b>.");
    assert!(matches!(&blocks[0], Block::Images { centered: true, .. }));
    let Block::Paragraph(spans) = &blocks[1] else {
        panic!("expected a paragraph, got {:?}", blocks[1]);
    };
    assert_eq!(words(spans), "Say hi.");
    assert!(
        spans
            .iter()
            .any(|span| span.text == "hi" && span.inline.strong)
    );
}
