//! The picture element: an image in its own colours, fitted to its room.

use pm_gfx::{Image, Rect, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Length, Style, Styled};

/// An image drawn as large as its room allows, keeping its shape.
///
/// A picture is never drawn larger than `zoom` times its own size: an icon
/// of sixteen pixels stretched over a whole pane is a blur of four colours,
/// not a better look at the icon.
pub struct Picture {
    /// The image drawn.
    image: Image,
    /// Largest the image is scaled up to, against its own size.
    zoom: f32,
    /// How the picture is sized in its parent.
    style: Style,
}

/// `image`, drawn no larger than its own size.
pub fn picture(image: Image) -> Picture {
    Picture {
        image,
        zoom: 1.0,
        style: Style::default(),
    }
}

impl Picture {
    /// Returns this picture allowed to grow to `zoom` times its own size.
    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom.max(f32::EPSILON);
        self
    }

    /// The size the image comes to inside `room`.
    fn fitted(&self, room: Size) -> Size {
        let (width, height) = (
            self.image.width().max(1) as f32,
            self.image.height().max(1) as f32,
        );
        let scale = (room.width / width)
            .min(room.height / height)
            .min(self.zoom)
            .max(0.0);
        Size::new(width * scale, height * scale)
    }
}

impl Styled for Picture {
    /// How the picture is sized in its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M> Element<M> for Picture {
    /// How the picture is sized in its parent.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// The size it is set to, or the image fitted to what it is offered.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let room = Size::new(
            match self.style.width {
                Length::Px(pixels) => pixels,
                Length::Full | Length::Auto => available.width,
            },
            match self.style.height {
                Length::Px(pixels) => pixels,
                Length::Full | Length::Auto => available.height,
            },
        );
        match (self.style.width, self.style.height) {
            (Length::Auto, Length::Auto) => self.fitted(room),
            _ => room,
        }
    }

    /// Draws the image fitted to `bounds`, centred in what it leaves over.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let size = self.fitted(bounds.size);
        let left = bounds.left() + (bounds.size.width - size.width) / 2.0;
        let top = bounds.top() + (bounds.size.height - size.height) / 2.0;
        cx.image(
            Rect::from_xywh(left, top, size.width, size.height),
            self.image.clone(),
        );
    }
}
