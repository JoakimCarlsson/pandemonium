//! A picture shown whole in the middle of its area, decoded once from the
//! bytes of its file, or a note when the bytes are not a picture.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a picture view sits on the page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PictureViewVariant {
    /// On the page's background.
    #[default]
    Plain,
    /// On a checkerboard-free raised surface with a border, as a framed preview.
    Framed,
}

/// The decoded picture a view remembers, with the bytes it came from.
type Decoded = Option<(Rc<Vec<u8>>, Result<ImageData, String>)>;

/// A picture view with a variant and a size.
#[derive(Clone)]
pub struct PictureView {
    /// The encoded bytes.
    bytes: Rc<Vec<u8>>,
    /// How it sits on the page.
    variant: PictureViewVariant,
    /// Its size, which sets the caption's.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
}

impl PictureView {
    /// A view of the picture encoded in `bytes`.
    pub fn new(bytes: Rc<Vec<u8>>) -> Self {
        Self {
            bytes,
            variant: PictureViewVariant::Plain,
            size: Size::Sm,
            modifier: Modifier::new(),
        }
    }

    /// The view in `variant`.
    pub fn variant(mut self, variant: PictureViewVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The view at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The view laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// Emits the view.
    pub fn show(self) {
        let scheme = color_scheme();
        let decoded = remember(|| -> Decoded { None });
        let fresh = decoded
            .get()
            .filter(|(bytes, _)| Rc::ptr_eq(bytes, &self.bytes));
        let picture = match fresh {
            Some((_, picture)) => picture,
            None => {
                let picture = ImageData::decode(&self.bytes);
                decoded.set(Some((self.bytes.clone(), picture.clone())));
                picture
            }
        };
        let modifier = match self.variant {
            PictureViewVariant::Plain => self.modifier.background(scheme.background),
            PictureViewVariant::Framed => self
                .modifier
                .background(scheme.surface_container)
                .border(1.0, scheme.outline_variant, 6.0),
        };
        let size = self.size;
        column_arranged(
            modifier.padding(16.0),
            Arrangement::SpacedBy(crate::spacing(10.0)),
            Alignment::Center,
            move || match &picture {
                Ok(picture) => {
                    image(
                        Modifier::new().fill_max_width().weight(1.0),
                        picture,
                        ContentScale::Fit,
                    );
                    Label::new(format!("{} × {}", picture.width(), picture.height()))
                        .variant(LabelVariant::Subtle)
                        .size(size)
                        .show();
                }
                Err(problem) => Label::new(format!("This picture cannot be shown: {problem}"))
                    .variant(LabelVariant::Muted)
                    .size(size)
                    .show(),
            },
        );
    }
}
