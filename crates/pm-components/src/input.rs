//! The input: a box holding editable text, one line or several, with a look per variant and a measure per size.

use pm_field::Field;
use rustle::{CursorIcon, Modifier};

use crate::kit::scroll::bar_of;
use crate::kit::{
    FieldText, Flex, FocusId, Hovered, IconName, Interactive, Radius, Role, Side, Styled, Type,
    block, column, row, theme,
};
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

/// The width the lines of a multiline input wrap at when no [`Input::wrap`] gave one, in logical pixels.
const UNWRAPPED_WIDTH: f32 = 400.0;

/// The size and leading every line of a multiline input is set in.
fn typography() -> Type {
    Type::new(14).ratio(LINE_RATIO)
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
/// emit it with [`Input::build`].
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
    /// The width in logical pixels the lines of a multiline box wrap at, once given.
    width: Option<f32>,
    /// What the caller puts around its box: sizes, growth, margins, placement.
    modifier: Modifier,
    /// The padding that replaces the box's own, when the caller gave one.
    inner: Option<Modifier>,
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
            width: None,
            modifier: Modifier::new(),
            inner: None,
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
    pub fn wrap(mut self, width: f32) -> Self {
        self.width = Some(width);
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

    /// The input with `modifier` put around its box, outside its own look:
    /// sizes, growth, margins, placement, tooltips. Calls accumulate.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The input with `modifier` in place of its own padding, innermost in its box.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = Some(modifier);
        self
    }

    /// Emits the input: a ring and a blinking caret while it holds keyboard
    /// focus, and the selection highlighted.
    #[track_caller]
    pub fn build(self) {
        if self.variant == Variant::Multiline {
            return self.build_multiline();
        }
        let (height, font) = match (self.variant, self.size) {
            (Variant::Bordered, Size::Xs) => (7.0, 12),
            (Variant::Bordered, Size::Sm) => (8.0, 12),
            (Variant::Bordered, Size::Md) => (9.0, 12),
            (Variant::Bordered, Size::Lg) => (10.0, 12),
            (_, Size::Xs) => (7.0, 12),
            (_, Size::Sm) => (8.0, 12),
            (_, Size::Md) => (10.0, 13),
            (_, Size::Lg) => (12.0, 14),
        };
        let id = self.id;
        let invalid = self.invalid;
        let boxed = matches!(self.variant, Variant::Default | Variant::Bordered);
        let mut own = Modifier::new().fill_max_width().h(height);
        if boxed {
            own = own.fill_rounded(Role::Surface, Radius::Lg);
        }
        let (rest, focused, danger) = border_colors();
        own = own.focus_of(id).styled_by(move |canvas, interaction| {
            let tone = match (invalid, interaction.focused) {
                (true, _) => danger,
                (false, true) => focused,
                (false, false) => rest,
            };
            if boxed || invalid {
                let (width, height) = (canvas.width(), canvas.height());
                canvas.stroke_rect(0.0, 0.0, width, height, Radius::Lg.px(), 1.0, tone);
            }
        });
        own = own.focus_ring(id, 2.0, Radius::Lg);
        own = if self.disabled {
            own.opacity(40.0).cursor(CursorIcon::NotAllowed)
        } else {
            own.cursor(CursorIcon::Text).target(id)
        };
        own = own.chain(self.inner.unwrap_or_else(|| Modifier::new().pad_x(3)));
        let mut kind = Type::new(font).ratio(1.4).tone(Role::Text);
        kind = if self.variant == Variant::Default {
            kind.mono()
        } else {
            kind.ratio(1.57)
        };
        let (field, placeholder, leading) = (self.field, self.placeholder, self.leading);
        row(
            self.modifier.chain(own),
            Flex::new().center().gap(2),
            move || {
                if let Some(icon) = leading {
                    glyph(icon, 4.0, Role::Muted);
                }
                FieldText::new(field, id, kind)
                    .placeholder(placeholder, Role::Subtle)
                    .build(Modifier::new().grow());
            },
        );
    }

    /// Emits a multiline input: its wrapped lines in a column at least
    /// [`Input::rows`] lines tall, showing [`MAX_LINES`] at most and scrolled
    /// to keep the caret in view, the caret and selection placed in them.
    #[track_caller]
    fn build_multiline(self) {
        let minimum = self.rows as f32 * LINE + PADDING;
        let wrap_at = self.width.unwrap_or(UNWRAPPED_WIDTH);
        let (field, id, placeholder) = (self.field, self.id, self.placeholder);
        let text = FieldText::new(field, id, typography().tone(Role::Text))
            .placeholder(placeholder, Role::Subtle)
            .wrap(wrap_at, Some(MAX_LINES));
        let scrolls = self.inner.is_none() && text.layout().line_count() > MAX_LINES;
        let mut own = Modifier::new().fill_max_width().min_h(minimum / 4.0);
        own = if self.disabled {
            own.opacity(40.0).cursor(CursorIcon::NotAllowed)
        } else {
            own.cursor(CursorIcon::Text).target(self.id)
        };
        own = own.chain(self.inner.unwrap_or_else(|| {
            let padded = Modifier::new()
                .pad_side(Side::Top, 3.5)
                .pad_side(Side::Bottom, 1);
            if scrolls { padded } else { padded.pad_x(4) }
        }));
        column(self.modifier.chain(own), Flex::new(), move || {
            if scrolls {
                block(
                    bar_of(Modifier::new().fill_max_width(), id).pad_x(4),
                    || text.build(Modifier::new().fill_max_width()),
                );
            } else {
                text.build(Modifier::new().fill_max_width());
            }
        });
    }
}

/// The border colours of a boxed input at rest, focused and holding something not acceptable.
fn border_colors() -> (rustle::Color, rustle::Color, rustle::Color) {
    (
        theme::role(Role::Border),
        theme::role(Role::Focused),
        theme::role(Role::Danger),
    )
}

#[cfg(test)]
mod tests;
