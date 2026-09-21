//! A frame's worth of single-character runs, shaped once each.
//!
//! A pane of monospaced text — a terminal grid, a file being edited — draws
//! a character at a time rather than a line at a time: the lines are never
//! seen twice, so shaping them as runs fills the text system's cache with
//! strings that will not come back, while the characters themselves are a
//! hundred-odd shapes that repeat all day.
//!
//! One cache belongs to one pane and one type size: the key is the character
//! and the weight and slant it is drawn in, and the size is whatever the
//! pane asked for.

use std::collections::HashMap;
use std::sync::Arc;

use pm_gfx::{FontStyle, ShapedRun};

use crate::element::PaintContext;

/// The characters shaped so far, in the styles they were asked for.
#[derive(Default)]
pub struct Glyphs {
    /// Runs already shaped, by the character, its weight and its slant.
    runs: HashMap<(char, u16, bool), Arc<ShapedRun>>,
}

impl Glyphs {
    /// The shaped run for `ch` in `font`, shaping it the first time only.
    pub fn shape<M>(
        &mut self,
        ch: char,
        font: FontStyle,
        cx: &mut PaintContext<'_, '_, M>,
    ) -> Arc<ShapedRun> {
        self.runs
            .entry((ch, font.weight, font.italic))
            .or_insert_with(|| cx.shape(&ch.to_string(), font))
            .clone()
    }
}
