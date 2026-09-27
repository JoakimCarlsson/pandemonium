//! The one container: a flex box that stacks children along an axis.

use std::sync::Arc;

use pm_gfx::{Quad, Rect, Rgba, Size};

use crate::element::{Element, Interaction, IntoElement, LayoutContext, PaintContext};
use crate::resize::ResizeEvent;
use crate::style::{Align, Axis, Justify, Length, Side, Style, Styled};
use crate::ui::PointerCursor;

/// A container that measures its children, stacks them and paints a background.
pub struct Div<M> {
    /// How this container is sized, spaced and filled.
    style: Style,
    /// The children, in stacking order.
    children: Vec<Box<dyn Element<M>>>,
    /// What a click on this container sends, when it answers to one at all.
    on_click: Option<M>,
    /// What a secondary click on it sends, when it answers to one at all.
    on_secondary_click: Option<M>,
    /// What dragging it sends, when it is something that can be carried.
    on_drag: Option<Arc<dyn Fn(ResizeEvent) -> M>>,
    /// The shape the pointer takes over it while it can be dragged.
    drag_cursor: PointerCursor,
    /// What this element tells the reader while it is hovered.
    tooltip: Option<String>,
}

/// An empty container stacking children top to bottom.
pub fn div<M>() -> Div<M> {
    Div {
        style: Style::default(),
        children: Vec::new(),
        on_click: None,
        on_secondary_click: None,
        on_drag: None,
        drag_cursor: PointerCursor::Pointer,
        tooltip: None,
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

    /// Makes this container something the pointer carries, through `on_drag`.
    ///
    /// A container that is dragged is no longer clicked: every press on it
    /// is a drag, and a press that travels nowhere before it is let go is
    /// what the caller reads as a click. That is the caller's to decide,
    /// because only it knows how far a tab may slip and still have been
    /// tapped rather than carried.
    pub fn on_drag(mut self, on_drag: impl Fn(ResizeEvent) -> M + 'static) -> Self {
        self.on_drag = Some(Arc::new(on_drag));
        self
    }

    /// Returns this container showing `cursor` while the pointer is over it,
    /// where it can be dragged: a hand for a thing carried, a beam for text
    /// picked out.
    pub fn drag_cursor(mut self, cursor: PointerCursor) -> Self {
        self.drag_cursor = cursor;
        self
    }

    /// Makes this container answer to a click by sending `message`.
    pub fn on_click(mut self, message: M) -> Self {
        self.on_click = Some(message);
        self
    }

    /// Makes this container answer to a secondary click by sending `message`.
    pub fn on_secondary_click(mut self, message: M) -> Self {
        self.on_secondary_click = Some(message);
        self
    }

    /// Shows `text` over this element while the pointer rests on it.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// The fill for this container in `interaction`.
    fn background(&self, interaction: Interaction) -> Rgba {
        let style = &self.style;
        match (interaction.pressed, interaction.hovered) {
            (true, _) => style
                .background_active
                .or(style.background_hovered)
                .unwrap_or(style.background),
            (_, true) => style.background_hovered.unwrap_or(style.background),
            _ => style.background,
        }
    }

    /// Applies `build` only when `condition` holds.
    pub fn when(self, condition: bool, build: impl FnOnce(Self) -> Self) -> Self {
        if condition { build(self) } else { self }
    }

    /// Applies `build` with the value only when there is one.
    pub fn when_some<T>(self, value: Option<T>, build: impl FnOnce(Self, T) -> Self) -> Self {
        match value {
            Some(value) => build(self, value),
            None => self,
        }
    }

    /// Measures every child against the space inside the padding.
    ///
    /// Sizing on the stacking axis follows flexbox in two passes. First the
    /// children with an exact length or an `Auto` one are measured as they
    /// are; what is left over is then split between the children that grow —
    /// `Full` or a non-zero `flex_grow` — and each of those is measured
    /// against its own share, so a child that fills its parent's width cannot
    /// widen the parent it is being fitted into.
    fn measure_children(
        &mut self,
        content: Size,
        stretch: bool,
        cx: &mut LayoutContext<'_>,
    ) -> Vec<Size> {
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
                    axis.set_main(&mut offer, pixels);
                    child.measure(offer, cx)
                }
                _ => child.measure(content, cx),
            };
        }

        let used: f32 = sizes.iter().map(|size| axis.main_of(*size)).sum();
        let leftover = (axis.main_of(content) - used - gaps).max(0.0);
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
            axis.set_main(&mut offer, share);
            let mut size = child.measure(offer, cx);
            axis.set_main(&mut size, share);
            sizes[index] = size;
        }

        for (index, size) in sizes.iter_mut().enumerate() {
            let style = self.children[index].layout_style();
            let cross = match cross_length(&style, axis) {
                Length::Px(pixels) => pixels,
                Length::Full => axis.cross_of(content),
                Length::Auto if align == Align::Stretch && stretch => axis.cross_of(content),
                Length::Auto => axis.cross_of(*size).min(axis.cross_of(content)),
            };
            axis.set_cross(size, cap_width(&style, axis, cross));
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

    /// Applies `min_width` and `max_width` to a width.
    fn capped_width(&self, width: f32) -> f32 {
        let width = match self.style.max_width {
            Some(max) => width.min(max),
            None => width,
        };
        match self.style.min_width {
            Some(min) => width.max(min),
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

impl<M: Clone> Element<M> for Div<M> {
    /// How this container is sized, spaced and filled.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Measures the children, then this container around them.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let content = self.content_offer(available);
        let sizes = self.measure_children(content, !self.style.fit_width, cx);
        let axis = self.style.axis;

        let gaps = self.style.gap * sizes.len().saturating_sub(1) as f32;
        let main: f32 = sizes.iter().map(|size| axis.main_of(*size)).sum::<f32>() + gaps;
        let cross = sizes
            .iter()
            .map(|size| axis.cross_of(*size))
            .fold(0.0, f32::max);

        let mut intrinsic = Size::zero();
        axis.set_main(&mut intrinsic, main);
        axis.set_cross(&mut intrinsic, cross);

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
        let interaction = match (
            self.on_drag.clone(),
            self.on_click.clone(),
            self.on_secondary_click.clone(),
        ) {
            (Some(on_drag), _, on_secondary) => {
                cx.draggable(bounds, self.drag_cursor, on_drag, on_secondary)
            }
            (None, None, None) => Interaction::default(),
            (None, on_click, on_secondary) => cx.clickable(bounds, on_click, on_secondary),
        };

        if interaction.hovered
            && let Some(text) = &self.tooltip
        {
            cx.tooltip(bounds, text.clone());
        }

        cx.quad(
            Quad::filled(bounds, self.background(interaction))
                .corner_radius(self.style.corner_radius)
                .border(self.style.border_width, self.style.border_color),
        );
        for (side, (width, color)) in Side::ALL.into_iter().zip(self.style.sides) {
            if width > 0.0 && !color.is_transparent() {
                cx.quad(Quad::filled(side.strip(bounds, width), color));
            }
        }

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

        let sizes = self.measure_children(content.size, true, &mut cx.layout);
        let axis = self.style.axis;
        let gap = self.style.gap;
        let gaps = gap * sizes.len().saturating_sub(1) as f32;
        let used: f32 = sizes.iter().map(|size| axis.main_of(*size)).sum::<f32>() + gaps;
        let leftover = (axis.main_of(content.size) - used).max(0.0);

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
            let room = axis.cross_of(content.size) - axis.cross_of(size);
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

            main += axis.main_of(size) + gap + spread;
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
