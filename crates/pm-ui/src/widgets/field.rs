//! One line of text being typed into, with a caret in it.
//!
//! The field draws and nothing more: what the text is, where the caret sits,
//! which characters are selected and which field the keyboard is going to
//! are the caller's, because a search bar, a palette and a rename prompt are
//! three callers with one widget between them. A click reports where in the
//! text it landed, so the caller can move its own caret there.

use std::ops::Range;
use std::sync::Arc;

use pm_gfx::{Point, Quad, Rect, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Length, Style, Styled};
use crate::theme::{Font, TextSize};

/// Width of the caret drawn while the field has the keyboard.
const CARET_WIDTH: f32 = 1.5;

/// One line of editable text, drawn with a caret where the cursor is.
pub struct Field<M> {
    /// What is in the field.
    value: String,
    /// What is shown instead while it is empty.
    placeholder: String,
    /// How many characters into the value the caret sits.
    caret: usize,
    /// The characters washed in the selection colour, when a span is selected.
    selection: Option<Range<usize>>,
    /// Whether keystrokes are going to this field.
    focused: bool,
    /// The step of the scale the text is drawn at.
    font: Font,
    /// What a press in the field sends, given the character it landed before.
    on_press: Option<Arc<dyn Fn(usize) -> M>>,
    /// How the field is sized and padded.
    style: Style,
}

/// A field holding `value`, with the caret `caret` characters into it.
pub fn field<M>(value: impl Into<String>, caret: usize, focused: bool) -> Field<M> {
    Field {
        value: value.into(),
        placeholder: String::new(),
        caret,
        selection: None,
        focused,
        font: Font::new(TextSize::Sm),
        on_press: None,
        style: Style::default(),
    }
}

impl<M> Field<M> {
    /// Returns this field showing `placeholder` while it is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Returns this field drawn in the monospaced family.
    pub fn font_mono(mut self) -> Self {
        self.font = self.font.mono();
        self
    }

    /// Returns this field reporting presses through `on_press`.
    pub fn on_press(mut self, on_press: impl Fn(usize) -> M + 'static) -> Self {
        self.on_press = Some(Arc::new(on_press));
        self
    }

    /// Returns this field with characters `selection` washed in the selection
    /// colour.
    pub fn selection(mut self, selection: Option<Range<usize>>) -> Self {
        self.selection = selection;
        self
    }
}

impl<M> Styled for Field<M> {
    /// How the field is sized and padded.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: 'static> Element<M> for Field<M> {
    /// How the field is sized and padded.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Reports the line box the text occupies, within what it was offered.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let font = self.font.resolve(&cx.theme.text);
        let run = cx.measure(&self.value, font);
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => available.width,
            Length::Auto => run.width + self.style.padding.horizontal(),
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            Length::Full => available.height,
            Length::Auto => run.height + self.style.padding.vertical(),
        };
        Size::new(width, height)
    }

    /// Paints the text, the caret after it, and takes the press on it.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        cx.quad(
            Quad::filled(bounds, self.style.background)
                .corner_radii(self.style.corner_radii)
                .border(self.style.border_width, self.style.border_color),
        );

        let theme = *cx.theme();
        let font = self.font.resolve(&theme.text);
        let empty = self.value.is_empty();
        let shown = if empty {
            self.placeholder.clone()
        } else {
            self.value.clone()
        };
        let origin = Point::new(
            bounds.left() + self.style.padding.left,
            bounds.top() + self.style.padding.top,
        );

        let widths = if self.selection.is_some() || self.focused || self.on_press.is_some() {
            self.caret_offsets(font, cx)
        } else {
            Vec::new()
        };

        if let Some(selected) = &self.selection
            && !widths.is_empty()
        {
            let last = widths.len().saturating_sub(1);
            let start = widths[selected.start.min(last)];
            let end = widths[selected.end.min(last)];
            if end > start {
                let color = theme.colors.selection.alpha(theme.emphasis.selection);
                cx.quad(Quad::filled(
                    Rect::from_xywh(origin.x + start, origin.y, end - start, font.line_height),
                    color,
                ));
            }
        }

        if !shown.is_empty() {
            let color = if empty {
                theme.colors.text_subtle
            } else {
                theme.colors.text
            };
            let run = cx.shape(&shown, font);
            cx.text(origin, run, color);
        }

        if self.focused && !widths.is_empty() {
            let last = widths.len().saturating_sub(1);
            let offset = widths[self.caret.min(last)];
            cx.quad(Quad::filled(
                Rect::from_xywh(origin.x + offset, origin.y, CARET_WIDTH, font.line_height),
                theme.colors.accent,
            ));
        }

        let Some(on_press) = self.on_press.clone() else {
            return;
        };
        let pointer = cx.input().pointer;
        let pressed = pointer.map(|pointer| {
            let x = pointer.x - origin.x;
            let caret = widths
                .iter()
                .position(|offset| *offset >= x)
                .unwrap_or(widths.len().saturating_sub(1));
            on_press(caret)
        });
        cx.clickable(bounds, pressed, None);
    }
}

impl<M> Field<M> {
    /// How far into the line the caret sits before each character, and after
    /// the last one.
    fn caret_offsets(&self, font: pm_gfx::FontStyle, cx: &mut PaintContext<'_, '_, M>) -> Vec<f32> {
        let mut offsets = Vec::with_capacity(self.value.chars().count() + 1);
        let mut prefix = String::new();
        offsets.push(0.0);
        for ch in self.value.chars() {
            prefix.push(ch);
            offsets.push(cx.measure(&prefix, font).width);
        }
        offsets
    }
}
