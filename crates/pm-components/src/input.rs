//! The input: a box holding editable text, one line or several, with a look per variant and a measure per size.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, Utility, bg, border, border_color, cursor_not_allowed, cursor_text, flex, flex_1,
    flex_col, flex_none, font_mono, gap, h, h_full, items_center, leading_ratio, min_h, min_w_px,
    opacity, overflow_hidden, pb, pt, px, ring, ring_color, rounded, text_color, text_px, truncate,
    w_full,
};
use pm_text::{Field, Shaper};
use pm_ui::{Line, Node, node, text, wrap};

use crate::parts::glyph;
use crate::size::Size;

/// How tall a line of a multiline input is as a share of its type size.
const LINE_RATIO: f32 = 1.57;

/// How tall a line of a multiline input is, in logical pixels.
const LINE: f32 = 22.0;

/// How tall a multiline input is above and below its lines together, in logical pixels.
const PADDING: f32 = 18.0;

/// How many lines a multiline input shows at most; longer text scrolls to keep the caret in view.
const MAX_LINES: usize = 8;

/// The size and leading every line of a multiline input is set in.
fn typography() -> [Utility; 2] {
    [text_px(14), leading_ratio(LINE_RATIO)]
}

/// How wide the lines of a multiline input `outer` logical pixels wide are, inside its border and padding.
pub fn inner_width(outer: f32) -> f32 {
    (outer - 2.0 - 32.0).max(40.0)
}

/// How an input looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// A filled box with an outline.
    #[default]
    Default,
    /// A filled box with an outline for a line typed in ordinary text, which a [`Input::leading_icon`] stands before.
    Bordered,
    /// No fill and no outline, for a line edited in place.
    Ghost,
    /// Several wrapped lines with no fill and no outline, for a box that draws its own; give the lines with [`Input::wrap`].
    Multiline,
}

/// An input being described: the line it edits, what it says while empty,
/// how it looks and whether it is switched off.
///
/// Build it with [`Input::new`], change it with the methods that follow, and
/// turn it into a node with [`Input::build`].
#[derive(Clone, Debug)]
pub struct Input<'a> {
    /// The line being edited.
    field: &'a Field,
    /// What shows while the line is empty.
    placeholder: &'a str,
    /// The name it answers to.
    id: FocusId,
    /// How it looks.
    variant: Variant,
    /// How big it is.
    size: Size,
    /// The glyph before the line, if any.
    leading: Option<IconName>,
    /// Whether it is switched off.
    disabled: bool,
    /// Whether what it holds is not acceptable.
    invalid: bool,
    /// How many lines tall a multiline box is at least.
    rows: u32,
    /// The text cut into the lines a multiline box shows.
    lines: Vec<Line>,
}

impl<'a> Input<'a> {
    /// An input editing `field`, answering to `id`.
    pub fn new(field: &'a Field, id: FocusId) -> Self {
        Self {
            field,
            placeholder: "",
            id,
            variant: Variant::Default,
            size: Size::Md,
            leading: None,
            disabled: false,
            invalid: false,
            rows: 1,
            lines: Vec::new(),
        }
    }

    /// A bordered input editing `field` with a magnifying glass before the line, for what is looked for.
    pub fn search(field: &'a Field, id: FocusId) -> Self {
        Self::new(field, id)
            .variant(Variant::Bordered)
            .leading_icon(IconName::Search)
    }

    /// The input with `icon` before its line.
    pub fn leading_icon(mut self, icon: IconName) -> Self {
        self.leading = Some(icon);
        self
    }

    /// The multiline input at least `rows` lines tall.
    pub fn rows(mut self, rows: u32) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// The multiline input with its text wrapped to `width` logical pixels.
    pub fn wrap(mut self, width: f32, shaper: &mut Shaper) -> Self {
        self.lines = wrap(shaper, &self.field.shown(), typography(), width);
        self
    }

    /// The input saying `placeholder` while it is empty.
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// The input with `variant`'s look.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The input at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The input switched off while `disabled` is true: drawn faint, and
    /// neither taking focus nor answering a press.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The input outlined as holding something that is not acceptable.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// The node that draws the input: a ring and a blinking caret while it
    /// holds keyboard focus, and the selection highlighted.
    pub fn build(self) -> Node {
        if self.variant == Variant::Multiline {
            return self.build_multiline();
        }
        let shown = self.field.shown();
        let (content, tone, caret) = if shown.is_empty() {
            (self.placeholder, Role::Subtle, 0)
        } else {
            (&*shown, Role::Text, self.field.to_shown(self.field.caret()))
        };
        let (height, font) = match (self.variant, self.size) {
            (Variant::Bordered, Size::Sm) => (8, 12),
            (Variant::Bordered, Size::Md) => (9, 12),
            (Variant::Bordered, Size::Lg) => (10, 12),
            (_, Size::Sm) => (8, 12),
            (_, Size::Md) => (10, 13),
            (_, Size::Lg) => (12, 14),
        };
        let mut line = text(content)
            .styles([text_px(font), leading_ratio(1.4), text_color(tone)])
            .caret(caret, self.id);
        if shown.is_empty() {
            line = line.styles([truncate(), min_w_px(0)]);
        }
        if self.variant == Variant::Default {
            line = line.styles([font_mono()]);
        } else {
            line = line.styles([leading_ratio(1.57)]);
        }
        if let Some(range) = self.field.selection() {
            line = line.selection(
                self.field.to_shown(range.start)..self.field.to_shown(range.end),
                self.id,
            );
        }
        if let Some(range) = self.field.marked() {
            line = line.marked(range, self.id);
        }
        let mut styles = vec![
            flex(),
            items_center(),
            w_full(),
            min_w_px(0),
            h(height),
            gap(2),
            px(3),
            rounded(Radius::Lg),
            ring(2).focus(),
            ring_color(Role::Focused).focus(),
        ];
        if matches!(self.variant, Variant::Default | Variant::Bordered) {
            styles.extend([
                bg(Role::Surface),
                border(),
                border_color(Role::Border),
                border_color(Role::Focused).focus(),
            ]);
        }
        if self.invalid {
            styles.extend([
                border(),
                border_color(Role::Danger),
                ring_color(Role::Danger).focus(),
            ]);
        }
        if self.disabled {
            styles.extend([opacity(40), cursor_not_allowed()]);
        } else {
            styles.push(cursor_text());
        }
        let view = node()
            .styles([
                flex(),
                flex_1(),
                h_full(),
                items_center(),
                overflow_hidden(),
            ])
            .scroll_x(self.field.scroll())
            .child(line);
        let mut input = node().styles(styles);
        if let Some(icon) = self.leading {
            input = input.child(glyph(icon, 4.0, Role::Muted));
        }
        let input = input.child(view);
        if self.disabled {
            input
        } else {
            input.focusable(self.id)
        }
    }

    /// The node that draws a multiline input: its wrapped lines in a column, the caret and selection placed in them.
    fn build_multiline(self) -> Node {
        let content = self.field.shown();
        let caret = self.field.to_shown(self.field.caret());
        let caret_line = self
            .lines
            .iter()
            .position(|line| caret < line.next)
            .unwrap_or(self.lines.len().saturating_sub(1));
        let first = (caret_line + 1).saturating_sub(MAX_LINES);
        let selection = self
            .field
            .selection()
            .map(|range| self.field.to_shown(range.start)..self.field.to_shown(range.end));
        let marked = self.field.marked();
        let column = node()
            .styles([
                flex(),
                flex_col(),
                w_full(),
                min_w_px(0),
                px(4),
                pt(3.5),
                pb(1),
                min_h((self.rows as f32 * LINE + PADDING) / 4.0),
                cursor_text(),
            ])
            .focusable(self.id);
        if content.is_empty() {
            return column.child(
                text(self.placeholder)
                    .styles(typography())
                    .styles([text_color(Role::Subtle), truncate(), min_w_px(0)])
                    .caret(0, self.id)
                    .field_line(self.id, 0),
            );
        }
        column.children(
            self.lines
                .iter()
                .enumerate()
                .skip(first)
                .take(MAX_LINES)
                .map(|(index, line)| {
                    let mut row = text(&content[line.start..line.end])
                        .styles(typography())
                        .styles([text_color(Role::Text), flex_none()])
                        .field_line(self.id, line.start);
                    if index == caret_line {
                        row = row.caret(caret - line.start, self.id);
                    }
                    if let Some(range) = clipped(&selection, line) {
                        row = row.selection(range, self.id);
                    }
                    if let Some(range) = clipped(&marked, line) {
                        row = row.marked(range, self.id);
                    }
                    row
                }),
        )
    }
}

/// The part of `range` that falls on `line`, as offsets into the line, or
/// nothing when none of it does.
fn clipped(range: &Option<std::ops::Range<usize>>, line: &Line) -> Option<std::ops::Range<usize>> {
    let range = range.as_ref()?;
    let (low, high) = (range.start.max(line.start), range.end.min(line.end));
    (low < high).then(|| low - line.start..high - line.start)
}

impl From<Input<'_>> for Node {
    /// The node that draws the input.
    fn from(input: Input<'_>) -> Self {
        input.build()
    }
}
