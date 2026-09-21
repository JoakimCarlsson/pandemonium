//! The style a layout reads, and the utility setters elements are built with.
//!
//! The vocabulary follows Tailwind's spacing scale through parameterized,
//! chainable setters instead of a style object filled in by hand. [`Style`] is
//! what the layout and paint passes read; [`Styled`] is how a caller writes it,
//! and an element joins that vocabulary by handing over one field.

mod styled;
mod units;

pub use styled::Styled;
pub use units::{Align, Axis, Edges, Justify, Length, STEP, space};

use pm_gfx::Rgba;

/// Everything the layout and the painter need to know about one element.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    /// The axis children are stacked along.
    pub axis: Axis,
    /// Width along the horizontal axis.
    pub width: Length,
    /// Height along the vertical axis.
    pub height: Length,
    /// Upper bound on width, applied after `width`.
    pub max_width: Option<f32>,
    /// Share of the leftover space on the parent's stacking axis.
    pub flex_grow: f32,
    /// Space between the element's border and its children.
    pub padding: Edges,
    /// Space between adjacent children.
    pub gap: f32,
    /// Cross-axis alignment of children.
    pub align: Align,
    /// Stacking-axis distribution of children.
    pub justify: Justify,
    /// Whether leftover horizontal space is split either side of the element.
    pub center_horizontally: bool,
    /// Fill colour behind the children.
    pub background: Rgba,
    /// Border thickness, drawn inside the element's bounds.
    pub border_width: f32,
    /// Border colour.
    pub border_color: Rgba,
    /// Corner radius of the background and border.
    pub corner_radius: f32,
}

impl Style {
    /// Sets padding on every edge.
    pub fn set_padding(&mut self, amount: f32) {
        self.padding = Edges::all(amount);
    }

    /// Sets padding on the left and right edges.
    pub fn set_padding_x(&mut self, amount: f32) {
        self.padding.left = amount;
        self.padding.right = amount;
    }

    /// Sets padding on the top and bottom edges.
    pub fn set_padding_y(&mut self, amount: f32) {
        self.padding.top = amount;
        self.padding.bottom = amount;
    }

    /// Sets padding on the top edge.
    pub fn set_padding_top(&mut self, amount: f32) {
        self.padding.top = amount;
    }

    /// Sets padding on the bottom edge.
    pub fn set_padding_bottom(&mut self, amount: f32) {
        self.padding.bottom = amount;
    }

    /// Sets the space between adjacent children.
    pub fn set_gap(&mut self, amount: f32) {
        self.gap = amount;
    }

    /// Sets an exact width.
    pub fn set_width(&mut self, amount: f32) {
        self.width = Length::Px(amount);
    }

    /// Sets an exact height.
    pub fn set_height(&mut self, amount: f32) {
        self.height = Length::Px(amount);
    }
}

impl Default for Style {
    /// A vertical, content-sized, transparent element with no padding.
    fn default() -> Self {
        Self {
            axis: Axis::Vertical,
            width: Length::Auto,
            height: Length::Auto,
            max_width: None,
            flex_grow: 0.0,
            padding: Edges::default(),
            gap: 0.0,
            align: Align::Start,
            justify: Justify::Start,
            center_horizontally: false,
            background: Rgba::TRANSPARENT,
            border_width: 0.0,
            border_color: Rgba::TRANSPARENT,
            corner_radius: 0.0,
        }
    }
}
