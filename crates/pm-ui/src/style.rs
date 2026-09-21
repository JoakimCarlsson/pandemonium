//! The style a layout reads, and the utility setters elements are built with.
//!
//! The vocabulary is Tailwind's: a fixed spacing scale, one step per setter,
//! and a chain of setters instead of a style object filled in by hand. The
//! scale is four logical pixels a step, so `p_4` is sixteen pixels of padding,
//! exactly as `p-4` is in Tailwind.

use pm_gfx::Rgba;

/// Logical pixels in one step of the spacing scale.
pub const STEP: f32 = 4.0;

/// The size of `steps` on the spacing scale, in logical pixels.
pub const fn space(steps: f32) -> f32 {
    steps * STEP
}

/// The axis a container stacks its children along.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    /// Children are placed left to right.
    Horizontal,
    /// Children are placed top to bottom.
    Vertical,
}

/// How an element is sized along one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// As large as the element's own content needs.
    Auto,
    /// Exactly this many logical pixels.
    Px(f32),
    /// The whole extent the parent offers.
    Full,
}

/// How children are aligned on the axis they are *not* stacked along.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Align {
    /// Packed against the start edge.
    Start,
    /// Centred.
    Center,
    /// Packed against the end edge.
    End,
    /// Stretched to fill the cross axis.
    Stretch,
}

/// How leftover space on the stacking axis is distributed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Justify {
    /// Children sit at the start, leftover space after them.
    Start,
    /// Children sit in the middle of the leftover space.
    Center,
    /// Children sit at the end, leftover space before them.
    End,
    /// Leftover space is split evenly between children.
    Between,
}

/// Padding or inset on each edge, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    /// Space above the content.
    pub top: f32,
    /// Space right of the content.
    pub right: f32,
    /// Space below the content.
    pub bottom: f32,
    /// Space left of the content.
    pub left: f32,
}

impl Edges {
    /// The same amount on every edge.
    pub const fn all(amount: f32) -> Self {
        Self {
            top: amount,
            right: amount,
            bottom: amount,
            left: amount,
        }
    }

    /// Total space taken on the horizontal axis.
    pub const fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total space taken on the vertical axis.
    pub const fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

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
