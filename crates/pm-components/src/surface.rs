//! The surface: the raised, bordered column that menus, popovers and modals are drawn on.

use crate::kit::{Flex, Radius, Role, Styled, column};
use rustle::Modifier;

/// The shadow under a floating surface: a tall, soft one that lifts it off what is behind,
/// as `(x, y, blur, spread, alpha)`.
const FLOATING_SHADOW: [(f32, f32, f32, f32, f32); 1] = [(0.0, 12.0, 40.0, 0.0, 0.35)];

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
    flex: Option<Flex>,
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

    /// The surface laying out its children by `flex` instead of the plain column's default.
    pub fn flex(mut self, flex: Flex) -> Self {
        self.flex = Some(flex);
        self
    }

    /// Emits the surface holding `content`: a column on the surface colour, bordered and shadowed.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let (shadow, radius) = match self.variant {
            Variant::Compact => (Modifier::new().shadow_md(Radius::Md), Radius::Md),
            Variant::Floating => (
                Modifier::new().shadows(&FLOATING_SHADOW, Radius::Xl),
                Radius::Xl,
            ),
            Variant::Dialog => (Modifier::new().shadow_md(Radius::Xl), Radius::Xl),
        };
        let mut own = shadow
            .fill_rounded(Role::Surface, radius)
            .stroke(Role::Border, radius);
        if self.variant == Variant::Dialog {
            own = own.clip_to(Radius::None);
        }
        column(
            self.modifier.chain(own.chain(self.inner)),
            self.flex.unwrap_or_default(),
            content,
        );
    }

    /// Emits the empty surface.
    #[track_caller]
    pub fn build(self) {
        self.build_with(|| {});
    }
}
