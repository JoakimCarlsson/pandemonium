//! Chainable utility setters expressed in spacing-scale steps.

use pm_gfx::Rgba;

use crate::style::Style;
use crate::style::units::{Align, Axis, Justify, Length, space};

/// The utility setters every element is styled with.
///
/// Implementors expose their [`Style`] through [`Styled::style`]; everything
/// else is a default method, so a new element gets the whole vocabulary by
/// handing over one field.
pub trait Styled: Sized {
    /// The style this element is laid out and painted from.
    fn style(&mut self) -> &mut Style;

    /// Sets padding on every edge in spacing-scale steps.
    fn p(mut self, steps: impl Into<f64>) -> Self {
        self.style().set_padding(space(steps.into() as f32));
        self
    }

    /// Sets horizontal padding in spacing-scale steps.
    fn px(mut self, steps: impl Into<f64>) -> Self {
        self.style().set_padding_x(space(steps.into() as f32));
        self
    }

    /// Sets vertical padding in spacing-scale steps.
    fn py(mut self, steps: impl Into<f64>) -> Self {
        self.style().set_padding_y(space(steps.into() as f32));
        self
    }

    /// Sets top padding in spacing-scale steps.
    fn pt(mut self, steps: impl Into<f64>) -> Self {
        self.style().set_padding_top(space(steps.into() as f32));
        self
    }

    /// Sets bottom padding in spacing-scale steps.
    fn pb(mut self, steps: impl Into<f64>) -> Self {
        self.style().set_padding_bottom(space(steps.into() as f32));
        self
    }

    /// Sets the gap between children in spacing-scale steps.
    fn gap(mut self, steps: impl Into<f64>) -> Self {
        self.style().set_gap(space(steps.into() as f32));
        self
    }

    /// Sets width in spacing-scale steps.
    fn w(mut self, steps: impl Into<f64>) -> Self {
        self.style().set_width(space(steps.into() as f32));
        self
    }

    /// Sets height in spacing-scale steps.
    fn h(mut self, steps: impl Into<f64>) -> Self {
        self.style().set_height(space(steps.into() as f32));
        self
    }

    /// Stacks children left to right instead of top to bottom.
    fn row(mut self) -> Self {
        self.style().axis = Axis::Horizontal;
        self
    }

    /// Stacks children top to bottom, which is the default.
    fn column(mut self) -> Self {
        self.style().axis = Axis::Vertical;
        self
    }

    /// Takes the full width the parent offers.
    fn w_full(mut self) -> Self {
        self.style().width = Length::Full;
        self
    }

    /// Takes the full height the parent offers.
    fn h_full(mut self) -> Self {
        self.style().height = Length::Full;
        self
    }

    /// Takes exactly `pixels` of width.
    fn w_px(mut self, pixels: f32) -> Self {
        self.style().set_width(pixels);
        self
    }

    /// Takes exactly `pixels` of height.
    fn h_px(mut self, pixels: f32) -> Self {
        self.style().set_height(pixels);
        self
    }

    /// Takes exactly `pixels` on both axes.
    fn size_px(mut self, pixels: f32) -> Self {
        self.style().set_width(pixels);
        self.style().set_height(pixels);
        self
    }

    /// Caps the width at `pixels`, however much the parent offers.
    fn max_w_px(mut self, pixels: f32) -> Self {
        self.style().max_width = Some(pixels);
        self
    }

    /// Takes an equal share of the leftover space on the parent's axis.
    fn flex_1(mut self) -> Self {
        self.style().flex_grow = 1.0;
        self
    }

    /// Splits leftover horizontal space either side of this element.
    fn mx_auto(mut self) -> Self {
        self.style().center_horizontally = true;
        self
    }

    /// Packs children against the cross-axis start edge.
    fn items_start(mut self) -> Self {
        self.style().align = Align::Start;
        self
    }

    /// Centres children on the cross axis.
    fn items_center(mut self) -> Self {
        self.style().align = Align::Center;
        self
    }

    /// Packs children against the cross-axis end edge.
    fn items_end(mut self) -> Self {
        self.style().align = Align::End;
        self
    }

    /// Stretches children across the cross axis.
    fn items_stretch(mut self) -> Self {
        self.style().align = Align::Stretch;
        self
    }

    /// Centres children on the stacking axis.
    fn justify_center(mut self) -> Self {
        self.style().justify = Justify::Center;
        self
    }

    /// Packs children against the stacking-axis end edge.
    fn justify_end(mut self) -> Self {
        self.style().justify = Justify::End;
        self
    }

    /// Spreads children evenly along the stacking axis.
    fn justify_between(mut self) -> Self {
        self.style().justify = Justify::Between;
        self
    }

    /// Fills the element with `color`.
    fn bg(mut self, color: Rgba) -> Self {
        self.style().background = color;
        self
    }

    /// Draws a one-pixel border in `color`.
    fn border_1(mut self, color: Rgba) -> Self {
        self.style().border_width = 1.0;
        self.style().border_color = color;
        self
    }

    /// Draws a two-pixel border in `color`.
    fn border_2(mut self, color: Rgba) -> Self {
        self.style().border_width = 2.0;
        self.style().border_color = color;
        self
    }

    /// Rounds the corners by `radius` logical pixels.
    fn rounded(mut self, radius: f32) -> Self {
        self.style().corner_radius = radius;
        self
    }

    /// Clips descendants to this element's bounds.
    fn overflow_hidden(mut self) -> Self {
        self.style().overflow_hidden = true;
        self
    }
}
