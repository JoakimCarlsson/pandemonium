//! The utility setters, one per step of the scale, in one trait.

use pm_gfx::Rgba;

use crate::style::Style;
use crate::style::units::{Align, Axis, Justify, Length, space};

/// Declares one setter per step of the spacing scale.
macro_rules! scale_setters {
    ($property:expr, $apply:ident, [$(($name:ident, $steps:expr)),* $(,)?]) => {
        $(
            #[doc = concat!("Sets ", $property, " to ", stringify!($steps), " steps of the spacing scale.")]
            fn $name(mut self) -> Self {
                self.style().$apply(space($steps));
                self
            }
        )*
    };
}

/// The utility setters every element is styled with.
///
/// Implementors expose their [`Style`] through [`Styled::style`]; everything
/// else is a default method, so a new element gets the whole vocabulary by
/// handing over one field.
pub trait Styled: Sized {
    /// The style this element is laid out and painted from.
    fn style(&mut self) -> &mut Style;

    scale_setters!(
        "padding on every edge",
        set_padding,
        [
            (p_0, 0.0),
            (p_1, 1.0),
            (p_2, 2.0),
            (p_3, 3.0),
            (p_4, 4.0),
            (p_5, 5.0),
            (p_6, 6.0),
            (p_8, 8.0),
            (p_10, 10.0),
            (p_12, 12.0),
        ]
    );

    scale_setters!(
        "horizontal padding",
        set_padding_x,
        [
            (px_1, 1.0),
            (px_2, 2.0),
            (px_3, 3.0),
            (px_4, 4.0),
            (px_5, 5.0),
            (px_6, 6.0),
            (px_8, 8.0),
        ]
    );

    scale_setters!(
        "vertical padding",
        set_padding_y,
        [
            (py_0, 0.0),
            (py_1, 1.0),
            (py_2, 2.0),
            (py_3, 3.0),
            (py_4, 4.0),
            (py_6, 6.0),
            (py_8, 8.0),
        ]
    );

    scale_setters!(
        "top padding",
        set_padding_top,
        [(pt_1, 1.0), (pt_2, 2.0), (pt_4, 4.0), (pt_6, 6.0)]
    );

    scale_setters!(
        "bottom padding",
        set_padding_bottom,
        [(pb_1, 1.0), (pb_2, 2.0), (pb_4, 4.0), (pb_6, 6.0)]
    );

    scale_setters!(
        "the gap between children",
        set_gap,
        [
            (gap_0p5, 0.5),
            (gap_1, 1.0),
            (gap_1p5, 1.5),
            (gap_2, 2.0),
            (gap_3, 3.0),
            (gap_4, 4.0),
            (gap_6, 6.0),
            (gap_8, 8.0),
        ]
    );

    scale_setters!(
        "width",
        set_width,
        [
            (w_4, 4.0),
            (w_6, 6.0),
            (w_8, 8.0),
            (w_10, 10.0),
            (w_12, 12.0)
        ]
    );

    scale_setters!(
        "height",
        set_height,
        [
            (h_4, 4.0),
            (h_5, 5.0),
            (h_6, 6.0),
            (h_8, 8.0),
            (h_10, 10.0),
            (h_12, 12.0)
        ]
    );

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
}
