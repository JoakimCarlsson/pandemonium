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
/// emit it with [`Surface::build_with`]; the caller
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

    /// Emits the surface holding `content`: a column on the surface colour, bordered and shadowed.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let (layers, radius): (&[rustle::Shadow], Radius) = match self.variant {
            Variant::Compact => (&theme::MEDIUM_SHADOW, Radius::Md),
            Variant::Floating => (&theme::FLOATING_SHADOW, Radius::Xl),
            Variant::Dialog => (&theme::MEDIUM_SHADOW, Radius::Xl),
        };
        let shadow = Modifier::new().shadow_layers(layers, radius.px());
        let mut inner = Modifier::new().padding(1.0);
        if self.variant == Variant::Dialog {
            inner = inner.clip();
        }
        let inner = inner.chain(self.inner);
        rustle::surface_custom(
            self.modifier.chain(shadow),
            rustle::Shape::rounded(radius.px()),
            theme::color(Role::Surface),
            theme::color(Role::Text),
            Some(rustle::BorderStroke::new(1.0, theme::color(Role::Border))),
            || column_arranged(inner, Arrangement::Start, Alignment::Start, content),
        );
    }
}
