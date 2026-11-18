//! The code box: lines in the code font inside a bordered box that cuts them off at a height.

use crate::kit::{Flex, Radius, Role, Side, Styled, column};
use rustle::Modifier;

/// A code box being described: the lines it holds and how tall it may grow.
///
/// Build it with [`CodeBox::new`], change it with [`CodeBox::max_height`],
/// and emit it with [`CodeBox::build`].
pub struct CodeBox<'a> {
    /// Emits the lines, top to bottom.
    lines: Box<dyn FnOnce() + 'a>,
    /// The height past which the lines are cut off, in logical pixels.
    height: f32,
    /// The room kept above and below the box, in steps.
    margin: f32,
    /// What the caller puts around the box.
    modifier: Modifier,
    /// What replaces the box's own padding, when the caller set it.
    inner: Option<Modifier>,
}

impl std::fmt::Debug for CodeBox<'_> {
    /// Writes the box's height, margin and modifiers; the lines are a closure and are left out.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodeBox")
            .field("height", &self.height)
            .field("margin", &self.margin)
            .field("modifier", &self.modifier)
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<'a> CodeBox<'a> {
    /// A box of the lines `lines` emits, cut off at 280 logical pixels.
    pub fn new(lines: impl FnOnce() + 'a) -> Self {
        Self {
            lines: Box::new(lines),
            height: 280.0,
            margin: 1.0,
            modifier: Modifier::new(),
            inner: None,
        }
    }

    /// The box cut off at `pixels` logical pixels tall.
    pub fn max_height(mut self, pixels: f32) -> Self {
        self.height = pixels;
        self
    }

    /// The box with `steps` of room above and below it instead of one step.
    pub fn margin(mut self, steps: f32) -> Self {
        self.margin = steps;
        self
    }

    /// The box with `m` put around it, outside what it draws itself.
    pub fn modifier(mut self, m: Modifier) -> Self {
        self.modifier = self.modifier.chain(m);
        self
    }

    /// The box with `m` inside its border, in place of its own padding (it has none of its own).
    pub fn inner(mut self, m: Modifier) -> Self {
        self.inner = Some(m);
        self
    }

    /// Emits the box holding its lines.
    #[track_caller]
    pub fn build(self) {
        let own = Modifier::new()
            .pad_side(Side::Top, self.margin)
            .pad_side(Side::Bottom, self.margin)
            .max_h_px(self.height)
            .fill_rounded(Role::Background, Radius::Lg)
            .stroke(Role::Border, Radius::Lg)
            .clip_to(Radius::Lg)
            .chain(self.inner.unwrap_or_default());
        column(self.modifier.chain(own), Flex::new(), self.lines);
    }
}
