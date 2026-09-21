//! The one container: a flex box that stacks children along an axis.

use pm_gfx::{Quad, Rect, Size};

use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::style::{Align, Axis, Justify, Length, Style, Styled};

/// A container that measures its children, stacks them and paints a background.
pub struct Div<M> {
    /// How this container is sized, spaced and filled.
    style: Style,
    /// The children, in stacking order.
    children: Vec<Box<dyn Element<M>>>,
}

/// An empty container stacking children top to bottom.
pub fn div<M>() -> Div<M> {
    Div {
        style: Style::default(),
        children: Vec::new(),
    }
}

/// An empty container stacking children top to bottom.
pub fn v_flex<M>() -> Div<M> {
    div().column()
}

/// An empty container stacking children left to right.
pub fn h_flex<M>() -> Div<M> {
    div().row()
}

impl<M> Div<M> {
    /// Appends one child.
    pub fn child(mut self, child: impl IntoElement<M>) -> Self {
        self.children.push(child.into_element());
        self
    }

    /// Appends every child of `children`.
    pub fn children<I>(mut self, children: I) -> Self
    where
        I: IntoIterator,
        I::Item: IntoElement<M>,
    {
        self.children
            .extend(children.into_iter().map(IntoElement::into_element));
        self
    }

    /// Applies `build` only when `condition` holds.
    pub fn when(self, condition: bool, build: impl FnOnce(Self) -> Self) -> Self {
        if condition { build(self) } else { self }
    }

    /// Measures every child against the space inside the padding.
    ///
    /// Sizing on the stacking axis follows flexbox in two passes. First the
    /// children with an exact length or an `Auto` one are measured as they
    /// are; what is left over is then split between the children that grow —
    /// `Full` or a non-zero `flex_grow` — and each of those is measured
    /// against its own share, so a child that fills its parent's width cannot
    /// widen the parent it is being fitted into.
    fn measure_children(&mut self, content: Size, cx: &mut LayoutContext<'_>) -> Vec<Size> {
        let axis = self.style.axis;
        let align = self.style.align;
        let gaps = self.style.gap * self.children.len().saturating_sub(1) as f32;

        let mut sizes = vec![Size::zero(); self.children.len()];
        let mut weights = vec![0.0; self.children.len()];

        for (index, child) in self.children.iter_mut().enumerate() {
            let style = child.layout_style();
            weights[index] = match main_length(&style, axis) {
                Length::Full => style.flex_grow.max(1.0),
                _ => style.flex_grow,
            };
            if weights[index] > 0.0 {
                continue;
            }

            sizes[index] = match main_length(&style, axis) {
                Length::Px(pixels) => {
                    let mut offer = content;
                    set_main(&mut offer, axis, pixels);
                    child.measure(offer, cx)
                }
                _ => child.measure(content, cx),
            };
        }

        let used: f32 = sizes.iter().map(|size| main_of(*size, axis)).sum();
        let leftover = (main_of(content, axis) - used - gaps).max(0.0);
        let total_weight: f32 = weights.iter().sum();

        for (index, child) in self.children.iter_mut().enumerate() {
            if weights[index] <= 0.0 {
                continue;
            }

            let share = if total_weight > 0.0 {
                leftover * weights[index] / total_weight
            } else {
                0.0
            };
            let mut offer = content;
            set_main(&mut offer, axis, share);
            let mut size = child.measure(offer, cx);
            set_main(&mut size, axis, share);
            sizes[index] = size;
        }

        for (index, size) in sizes.iter_mut().enumerate() {
            let style = self.children[index].layout_style();
            let cross = match cross_length(&style, axis) {
                Length::Px(pixels) => pixels,
                Length::Full => cross_of(content, axis),
                Length::Auto if align == Align::Stretch => cross_of(content, axis),
                Length::Auto => cross_of(*size, axis).min(cross_of(content, axis)),
            };
            set_cross(size, axis, cap_width(&style, axis, cross));
        }

        sizes
    }

    /// The space inside the padding, given the space this container is offered.
    fn content_offer(&self, available: Size) -> Size {
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            _ => self.capped_width(available.width),
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            _ => available.height,
        };

        Size::new(
            (width - self.style.padding.horizontal()).max(0.0),
            (height - self.style.padding.vertical()).max(0.0),
        )
    }

    /// Applies `max_width` to a width offer.
    fn capped_width(&self, width: f32) -> f32 {
        match self.style.max_width {
            Some(max) => width.min(max),
            None => width,
        }
    }
}

impl<M> Styled for Div<M> {
    /// How this container is sized, spaced and filled.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M> Element<M> for Div<M> {
    /// How this container is sized, spaced and filled.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Measures the children, then this container around them.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let content = self.content_offer(available);
        let sizes = self.measure_children(content, cx);
        let axis = self.style.axis;

        let gaps = self.style.gap * sizes.len().saturating_sub(1) as f32;
        let main: f32 = sizes.iter().map(|size| main_of(*size, axis)).sum::<f32>() + gaps;
        let cross = sizes
            .iter()
            .map(|size| cross_of(*size, axis))
            .fold(0.0, f32::max);

        let mut intrinsic = Size::zero();
        set_main(&mut intrinsic, axis, main);
        set_cross(&mut intrinsic, axis, cross);

        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => self.capped_width(available.width),
            Length::Auto => self
                .capped_width(intrinsic.width + self.style.padding.horizontal())
                .min(available.width.max(0.0)),
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            Length::Full => available.height,
            Length::Auto => intrinsic.height + self.style.padding.vertical(),
        };

        Size::new(width, height)
    }

    /// Paints the background, then places and paints every child.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        cx.quad(
            Quad::filled(bounds, self.style.background)
                .corner_radius(self.style.corner_radius)
                .border(self.style.border_width, self.style.border_color),
        );

        let padding = self.style.padding;
        let content = Rect::from_xywh(
            bounds.left() + padding.left,
            bounds.top() + padding.top,
            (bounds.size.width - padding.horizontal()).max(0.0),
            (bounds.size.height - padding.vertical()).max(0.0),
        );

        if self.style.overflow_hidden {
            cx.push_clip(bounds);
        }

        let sizes = self.measure_children(content.size, &mut cx.layout);
        let axis = self.style.axis;
        let gap = self.style.gap;
        let gaps = gap * sizes.len().saturating_sub(1) as f32;
        let used: f32 = sizes.iter().map(|size| main_of(*size, axis)).sum::<f32>() + gaps;
        let leftover = (main_of(content.size, axis) - used).max(0.0);

        let mut main = match self.style.justify {
            Justify::Start | Justify::Between => 0.0,
            Justify::Center => leftover / 2.0,
            Justify::End => leftover,
        };
        let spread = match self.style.justify {
            Justify::Between if sizes.len() > 1 => leftover / (sizes.len() - 1) as f32,
            _ => 0.0,
        };

        for (child, size) in self.children.iter_mut().zip(sizes) {
            let room = cross_of(content.size, axis) - cross_of(size, axis);
            let cross = if child.layout_style().center_horizontally {
                (room / 2.0).max(0.0)
            } else {
                match self.style.align {
                    Align::Start | Align::Stretch => 0.0,
                    Align::Center => (room / 2.0).max(0.0),
                    Align::End => room.max(0.0),
                }
            };

            let origin = match axis {
                Axis::Horizontal => (content.left() + main, content.top() + cross),
                Axis::Vertical => (content.left() + cross, content.top() + main),
            };
            child.paint(
                Rect::from_xywh(origin.0, origin.1, size.width, size.height),
                cx,
            );

            main += main_of(size, axis) + gap + spread;
        }

        if self.style.overflow_hidden {
            cx.pop_clip();
        }
    }
}

/// The length that sizes an element along `axis`.
fn main_length(style: &Style, axis: Axis) -> Length {
    match axis {
        Axis::Horizontal => style.width,
        Axis::Vertical => style.height,
    }
}

/// Applies a child's `max_width` to its cross extent, when that is its width.
fn cap_width(style: &Style, axis: Axis, cross: f32) -> f32 {
    match (axis, style.max_width) {
        (Axis::Vertical, Some(max)) => cross.min(max),
        _ => cross,
    }
}

/// The length that sizes an element across `axis`.
fn cross_length(style: &Style, axis: Axis) -> Length {
    match axis {
        Axis::Horizontal => style.height,
        Axis::Vertical => style.width,
    }
}

/// The extent of `size` along `axis`.
fn main_of(size: Size, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

/// The extent of `size` across `axis`.
fn cross_of(size: Size, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => size.height,
        Axis::Vertical => size.width,
    }
}

/// Sets the extent of `size` along `axis`.
fn set_main(size: &mut Size, axis: Axis, extent: f32) {
    match axis {
        Axis::Horizontal => size.width = extent,
        Axis::Vertical => size.height = extent,
    }
}

/// Sets the extent of `size` across `axis`.
fn set_cross(size: &mut Size, axis: Axis, extent: f32) {
    match axis {
        Axis::Horizontal => size.height = extent,
        Axis::Vertical => size.width = extent,
    }
}
