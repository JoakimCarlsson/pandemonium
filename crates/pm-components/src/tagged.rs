//! Views laid out with no window, and their controls found by the tags their
//! [`Target`]s give them, as a Compose test finds nodes with `onNodeWithTag`.
//!
//! Every control named with [`Interactive::control`](crate::events::Interactive::control),
//! [`Interactive::pressable`](crate::events::Interactive::pressable) or their variants carries the
//! [`test_tag`](rustle::Modifier::test_tag) of its id ([`Target::tag`]). Tests and
//! snapshots lay views out with [`lay_out`] and ask the [`Laid`] where a control went.

use crate::target::Target;
use rustle::{Embedded, Offset, PixelRounding, Rect};

use crate::focus::{self, FocusKeys};

/// A control or area laid out, found by the tag its [`Target`] gave it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tagged {
    /// What names it.
    pub id: Target,
    /// Where it was laid out, whether or not it can be seen: Compose's `boundsInRoot`.
    pub bounds: Rect,
    /// The part of it that can be seen; empty when it is scrolled or clipped away.
    pub visible: Rect,
    /// Its place among the focus targets of its window, the order Tab walks them in.
    pub focusable: Option<usize>,
}

impl Tagged {
    /// The middle of the part of it that can be seen, where a test presses it.
    pub fn center(&self) -> (f32, f32) {
        (
            self.visible.origin.x + self.visible.size.width / 2.0,
            self.visible.origin.y + self.visible.size.height / 2.0,
        )
    }
}

/// The controls and areas `ui` laid out last that are named by a [`Target`], in paint
/// order, moved `dx` right and `dy` down.
pub fn tagged_in(ui: &Embedded, dx: f32, dy: f32) -> Vec<Tagged> {
    let moved = |rect: Rect| {
        Rect::new(
            Offset::new(rect.origin.x + dx, rect.origin.y + dy),
            rect.size,
        )
    };
    ui.tagged_nodes()
        .iter()
        .filter_map(|node| {
            let id = Target::from_tag(&node.tag)?;
            Some(Tagged {
                id,
                bounds: moved(node.bounds),
                visible: moved(node.visible),
                focusable: node.focusable,
            })
        })
        .collect()
}

/// The ids of `nodes` that are focus targets, in the order Tab walks them: one id for each
/// focus target, the first named on it, and each id once.
pub fn focus_order(nodes: &[Tagged]) -> Vec<Target> {
    let mut order: Vec<(usize, Target)> = nodes
        .iter()
        .filter_map(|node| Some((node.focusable?, node.id)))
        .collect();
    order.sort_by_key(|(index, _)| *index);
    order.dedup_by_key(|(index, _)| *index);
    let mut ids: Vec<Target> = Vec::new();
    for (_, id) in order {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// What a view laid out with no window holds: its controls and areas by tag.
#[derive(Clone, Debug, Default)]
pub struct Laid {
    /// The controls and areas, in paint order.
    pub nodes: Vec<Tagged>,
}

impl Laid {
    /// The first control or area named `id`.
    pub fn node(&self, id: Target) -> Option<Tagged> {
        self.nodes.iter().copied().find(|node| node.id == id)
    }

    /// The part of what `id` names that can be seen, if it was laid out.
    pub fn region(&self, id: Target) -> Option<Rect> {
        self.node(id).map(|node| node.visible)
    }

    /// Whether something named `id` was laid out.
    pub fn drawn(&self, id: Target) -> bool {
        self.node(id).is_some()
    }

    /// The ids laid out, in paint order.
    pub fn ids(&self) -> Vec<Target> {
        self.nodes.iter().map(|node| node.id).collect()
    }

    /// The controls in the order Tab walks them.
    pub fn focus_order(&self) -> Vec<Target> {
        focus_order(&self.nodes)
    }
}

/// An interface of `content` with no window, `width` by `height` logical pixels, with the
/// app's focus kept on `focused`, laid out at whole pixels in the default theme.
pub fn interface(
    width: f32,
    height: f32,
    focused: Option<Target>,
    content: impl Fn() + 'static,
) -> Embedded {
    rustle_wgpu::set_legible_text(false);
    let tokens = crate::theme::Tokens::default();
    let root = move || {
        focus::provide(FocusKeys::resting(focused), || {
            crate::theme::provide(tokens.clone(), &content)
        })
    };
    let mut ui = Embedded::without_renderer(root, || {});
    ui.set_pixel_rounding(PixelRounding::Logical);
    ui.resize((width.round() as u32, height.round() as u32), 1.0);
    ui.update();
    ui.invalidate();
    ui.update();
    ui
}

/// Lays `content` out at `width` by `height` logical pixels without drawing it, with the
/// app's focus kept on `focused`, and returns its controls by tag.
///
/// For tests and snapshots that ask which controls were laid out and where: no GPU is
/// needed, text is measured with the fonts of this thread.
pub fn lay_out(
    width: f32,
    height: f32,
    focused: Option<Target>,
    content: impl Fn() + 'static,
) -> Laid {
    let ui = interface(width, height, focused, content);
    Laid {
        nodes: tagged_in(&ui, 0.0, 0.0),
    }
}
