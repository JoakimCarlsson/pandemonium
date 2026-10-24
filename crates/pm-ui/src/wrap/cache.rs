//! Bounded paragraph wrapping shared across input edits and caret moves.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use pm_text::Shaper;
use pm_typography::{FontFamily, FontSlant, TypographyStyle};

use super::Line;

/// The estimated retained bytes in one generation.
const BYTES: usize = 4 * 1024 * 1024;

/// The maximum paragraphs in one generation.
const ENTRIES: usize = 4096;

/// Font identity, width and the typography values the shaper measures.
type Setting = (u64, u32, FontFamily, u32, u32, u16, FontSlant);

/// Paragraphs grouped by setting so a hit can use borrowed source text.
type Paragraphs = HashMap<Setting, HashMap<String, Arc<[Line]>>>;

/// Two bounded generations, promoting hot paragraphs while old edits expire.
#[derive(Default)]
struct Cache {
    /// Paragraphs built or reused recently.
    newer: Paragraphs,
    /// Paragraphs from the previous generation.
    older: Paragraphs,
    /// Estimated bytes in the newer generation.
    bytes: usize,
    /// Paragraphs in the newer generation.
    entries: usize,
}

thread_local! {
    /// Per-thread wrapping cache, independently bounded from the shaped glyph cache.
    static CACHE: RefCell<Cache> = RefCell::default();
}

/// Returns paragraph-local line ranges, measuring only paragraphs whose content or settings changed.
pub(super) fn paragraph(
    shaper: &mut Shaper,
    source: &str,
    typography: TypographyStyle,
    width: f32,
) -> Arc<[Line]> {
    let key = (
        shaper.font_key(),
        width.to_bits(),
        typography.family,
        typography.size.to_bits(),
        typography.line_height_px().to_bits(),
        typography.weight,
        typography.slant,
    );
    if let Some(lines) = CACHE.with_borrow(|cache| {
        cache
            .newer
            .get(&key)
            .and_then(|items| items.get(source))
            .cloned()
    }) {
        return lines;
    }
    let previous = CACHE.with_borrow_mut(|cache| {
        cache
            .older
            .get_mut(&key)
            .and_then(|items| items.remove(source))
    });
    let lines = previous.unwrap_or_else(|| {
        let mut lines = Vec::new();
        let mut fits = |piece: &str| shaper.measure(piece.trim_end(), &typography).width <= width;
        super::cut(source, 0, &mut fits, &mut lines);
        lines.into()
    });
    let bytes = source.len() + std::mem::size_of_val(lines.as_ref());
    if bytes <= BYTES {
        CACHE.with_borrow_mut(|cache| {
            if cache.bytes + bytes > BYTES || cache.entries >= ENTRIES {
                cache.older = std::mem::take(&mut cache.newer);
                cache.bytes = 0;
                cache.entries = 0;
            }
            cache
                .newer
                .entry(key)
                .or_default()
                .insert(source.to_owned(), lines.clone());
            cache.bytes += bytes;
            cache.entries += 1;
        });
    }
    lines
}
