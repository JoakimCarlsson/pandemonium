//! Diagrams written in markdown as fenced mermaid code, drawn into pictures and shown in place of the code.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use pm_markdown::Block;
use pm_style::{flex, flex_none, h_px, justify_center, w_full, w_px};
use pm_ui::{Node, Picture, node};

use crate::preview::VECTOR_DENSITY;

/// The diagrams drawn so far, by [`diagram_key`] of their source, each a picture or why it could not be drawn.
pub type Diagrams = HashMap<u64, Result<Picture, String>>;

/// Whether a fence naming `language` holds a diagram.
pub fn is_diagram(language: &str) -> bool {
    language.eq_ignore_ascii_case("mermaid")
}

/// The key a diagram written as `source` is kept under.
pub fn diagram_key(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

/// The sources of every diagram in the markdown `source`, nested ones included, in the order they are written.
pub fn diagram_sources(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    collect(&pm_markdown::parse(source), &mut found);
    found
}

/// Adds the sources of the diagrams among `blocks` and the blocks inside them to `found`.
fn collect(blocks: &[Block], found: &mut Vec<String>) {
    for block in blocks {
        match block {
            Block::Code { language, text } if is_diagram(language) => found.push(text.clone()),
            Block::List { items, .. } => {
                for item in items {
                    collect(item, found);
                }
            }
            Block::Quote(inner) | Block::Details { blocks: inner, .. } => collect(inner, found),
            _ => {}
        }
    }
}

/// What `diagrams` holds for each diagram of the markdown `source`, as one number each that changes when its picture arrives or fails, so a layout kept of the source is made again then.
pub(crate) fn diagram_marks(source: &str, diagrams: &Diagrams) -> Vec<u64> {
    diagram_sources(source)
        .iter()
        .map(|text| match diagrams.get(&diagram_key(text)) {
            None => 0,
            Some(Ok(picture)) => picture.key,
            Some(Err(_)) => 1,
        })
        .collect()
}

/// `picture`, a drawn diagram, at its own size or shrunk to `width` logical pixels, in the middle of a row that wide.
pub(crate) fn diagram_node(picture: &Picture, width: f32) -> Node {
    let (across, down) = (
        picture.width as f32 / VECTOR_DENSITY,
        picture.height as f32 / VECTOR_DENSITY,
    );
    let fit = (width / across.max(1.0)).min(1.0);
    node()
        .styles([flex(), justify_center(), w_full(), flex_none()])
        .child(
            node()
                .styles([w_px(across * fit), h_px(down * fit), flex_none()])
                .picture(picture.clone()),
        )
}
