//! The input: a box holding one editable line, with a look per variant and a measure per size.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_not_allowed, cursor_text, flex, flex_1,
    font_mono, h, h_full, h_px, items_center, items_start, leading_ratio, min_w_px, opacity,
    overflow_hidden, pt, px, ring, ring_color, rounded, text_color, text_px, w_full,
};
use pm_text::Field;
use pm_ui::{Node, node, text};

/// How an input looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// A filled box with an outline.
    #[default]
    Default,
    /// No fill and no outline, for a line edited in place.
    Ghost,
}

/// How big an input is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Size {
    /// 32px tall, 12px text.
    Sm,
    /// 40px tall, 13px text.
    #[default]
    Default,
    /// 48px tall, 14px text.
    Lg,
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
    /// Whether it is switched off.
    disabled: bool,
    /// Whether what it holds is not acceptable.
    invalid: bool,
    /// How many lines tall the box is, with the line at the top when it is more than one.
    rows: u32,
}

impl<'a> Input<'a> {
    /// An input editing `field`, answering to `id`.
    pub fn new(field: &'a Field, id: FocusId) -> Self {
        Self {
            field,
            placeholder: "",
            id,
            variant: Variant::Default,
            size: Size::Default,
            disabled: false,
            invalid: false,
            rows: 1,
        }
    }

    /// The input as tall as `rows` lines, the line it edits at the top.
    pub fn rows(mut self, rows: u32) -> Self {
        self.rows = rows.max(1);
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
        let shown = self.field.shown();
        let (content, tone, caret) = if shown.is_empty() {
            (self.placeholder, Role::Subtle, 0)
        } else {
            (&*shown, Role::Text, self.field.to_shown(self.field.caret()))
        };
        let (height, font) = match self.size {
            Size::Sm => (8, 12),
            Size::Default => (10, 13),
            Size::Lg => (12, 14),
        };
        let mut line = text(content)
            .styles([text_px(font), leading_ratio(1.4), text_color(tone)])
            .caret(caret, self.id);
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
        let tall = self.rows > 1;
        let mut styles = vec![
            flex(),
            items_center(),
            w_full(),
            min_w_px(0),
            h(height),
            px(3),
            rounded(Radius::Lg),
            ring(2).focus(),
            ring_color(Role::Focused).focus(),
        ];
        if self.variant == Variant::Default {
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
        if tall {
            styles.extend([h_px(self.rows as f32 * 22.0 + 28.0), pt(3.5)]);
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
                if tall { items_start() } else { items_center() },
                overflow_hidden(),
            ])
            .scroll_x(self.field.scroll())
            .child(line);
        let input = node().styles(styles).child(view);
        if self.disabled {
            input
        } else {
            input.focusable(self.id)
        }
    }
}

impl From<Input<'_>> for Node {
    /// The node that draws the input.
    fn from(input: Input<'_>) -> Self {
        input.build()
    }
}
