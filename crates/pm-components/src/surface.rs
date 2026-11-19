//! The surface: the raised, bordered column that menus, popovers and modals are drawn on,
//! a rustle surface in the theme's surface colour, border and corner, with the app's shadow.

use crate::theme;
use crate::theme::ShadowLayers;
use crate::theme::{Radius, Role};
use rustle::Modifier;
use rustle::{Alignment, Arrangement, column_arranged};

/// How a surface looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// Small corners and a short shadow, for a list of actions.
    #[default]
    Compact,
    /// Large corners and a tall, soft shadow, for a panel of choices that floats over a screen.
    Floating,
    /// Large corners, a short shadow and a clip at its edge, for a panel that holds a whole task.
    Dialog,
}

/// A surface being described: how it looks.
///
/// Build it with [`Surface::new`], change it with [`Surface::variant`], and
/// emit it with [`Surface::build`] or [`Surface::build_with`]; the caller
/// gives what it holds, the room around it ([`Surface::inner`]) and its place
/// ([`Surface::modifier`]).
#[derive(Clone, Debug, Default)]
pub struct Surface {
    /// How it looks.
    variant: Variant,
    /// The padding the caller gave in place of the surface's own, which has none.
    inner: Modifier,
    /// What the caller puts around its box.
    modifier: Modifier,
    /// How its children are laid out, when the caller restyled the column.
    arranged: Option<(Arrangement, Alignment)>,
}

impl Surface {
    /// A compact surface.
    pub fn new() -> Self {
        Self::default()
    }

    /// The surface looking like `variant`.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The surface with `modifier`, padding, in place of its own padding, inside its fill and border.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = modifier;
        self
    }

    /// The surface with `modifier` put around its box, outside its own: sizes, weight, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The surface spreading its children by `arrangement` and placing them across by `alignment` in place of its own.
    pub fn arranged(mut self, arrangement: Arrangement, alignment: Alignment) -> Self {
        self.arranged = Some((arrangement, alignment));
        self
    }

    /// Emits the surface holding `content`: a column on the surface colour, bordered and shadowed.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let (shadow, radius) = match self.variant {
            Variant::Compact => (
                Modifier::new().shadow_layers(&theme::MEDIUM_SHADOW, Radius::Md.px()),
                Radius::Md,
            ),
            Variant::Floating => (
                Modifier::new().shadow_layers(&theme::FLOATING_SHADOW, Radius::Xl.px()),
                Radius::Xl,
            ),
            Variant::Dialog => (
                Modifier::new().shadow_layers(&theme::MEDIUM_SHADOW, Radius::Xl.px()),
                Radius::Xl,
            ),
        };
        let mut inner = Modifier::new().padding(1.0);
        if self.variant == Variant::Dialog {
            inner = inner.clip();
        }
        let inner = inner.chain(self.inner);
        let (arrangement, alignment) = self
            .arranged
            .unwrap_or((Arrangement::Start, Alignment::Start));
        rustle::surface_custom(
            self.modifier.chain(shadow),
            rustle::Shape::rounded(radius.px()),
            theme::color(Role::Surface),
            theme::color(Role::Text),
            Some(rustle::BorderStroke::new(1.0, theme::color(Role::Border))),
            || column_arranged(inner, arrangement, alignment, content),
        );
    }

    /// Emits the empty surface.
    #[track_caller]
    pub fn build(self) {
        self.build_with(|| {});
    }
}
