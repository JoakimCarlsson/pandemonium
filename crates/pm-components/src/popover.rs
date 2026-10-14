//! The popover: a raised surface that floats over a screen, with a heading and rows of choices.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::primitives::{Length, Sides};
use pm_style::{
    Radius, Role, ShadowLayer, absolute, bg, border, border_color, cursor_pointer, flex, flex_1,
    flex_col, flex_none, font_mono, gap, h, inset_length, items_center, justify_between,
    leading_ratio, left, max_w_px, mb, min_w_px, p, px, py, relative, rounded, shadow_layers,
    text_color, text_px, truncate, w_full, w_px, z,
};
use pm_ui::{Node, node, text};

use crate::parts::{glyph, line};

/// The shadow under a popover: a tall, soft one that lifts it off what is behind.
const SHADOW: [ShadowLayer; 1] = [ShadowLayer::new(0.0, 12.0, 40.0, 0.0, 0.35, false)];

/// A popover being described: the rows and sections it holds.
///
/// Build it with [`Popover::new`], fill it with [`Popover::heading`],
/// [`Popover::row`], [`Popover::child`] and [`Popover::children`], and turn it
/// into a node with [`Popover::build`], or with one of the methods that also
/// place it over what it belongs to.
#[derive(Clone, Debug)]
pub struct Popover {
    /// The frame holding everything in it.
    body: Node,
}

impl Default for Popover {
    /// An empty popover.
    fn default() -> Self {
        Self::new()
    }
}

impl Popover {
    /// An empty popover.
    pub fn new() -> Self {
        Self {
            body: node().styles([
                flex(),
                flex_col(),
                p(1.5),
                rounded(Radius::Xl),
                border(),
                border_color(Role::Border),
                bg(Role::Surface),
                shadow_layers(&SHADOW),
            ]),
        }
    }

    /// The popover `pixels` wide, and no wider.
    pub fn width(mut self, pixels: f32) -> Self {
        self.body = self.body.styles([w_px(pixels), max_w_px(pixels)]);
        self
    }

    /// The popover as wide as what it is placed in.
    pub fn full_width(mut self) -> Self {
        self.body = self.body.styles([w_full()]);
        self
    }

    /// The popover with a heading line saying `title`, and `hint` at its right.
    pub fn heading(self, title: &str, hint: Option<(&str, &str)>) -> Self {
        self.child(heading(title, hint))
    }

    /// The popover with a row of `name` over `description`, ticked while `current`, answering to `id`.
    pub fn row(self, name: &str, description: &str, current: bool, id: FocusId) -> Self {
        self.child(row(name, description, current, id))
    }

    /// The popover with `child` after what it holds.
    pub fn child(mut self, child: Node) -> Self {
        self.body = self.body.child(child);
        self
    }

    /// The popover with every node of `children` after what it holds.
    pub fn children(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.body = self.body.children(children);
        self
    }

    /// The node that draws the popover.
    pub fn build(self) -> Node {
        self.body
    }

    /// The popover placed above the button it is a child of, a little way off, with their left edges together,
    /// and answering to `id` so that pressing it does not count as pressing outside.
    pub fn over_button(self, id: FocusId) -> Node {
        placed_above(self.body).focusable(id)
    }

    /// The popover placed just above the box it is a child of, across its width.
    pub fn above_box(self) -> Node {
        placed_above(self.body)
    }
}

/// `body` hanging above the box it is a child of.
fn placed_above(body: Node) -> Node {
    body.styles([
        absolute(),
        inset_length(Sides::Bottom, Length::Fraction(1.0)),
        left(0),
        mb(2),
        z(40),
    ])
}

/// `button` with `popover`, if there is one, hanging above it.
pub fn anchored(button: Node, popover: Option<Node>) -> Node {
    node().styles([relative()]).child(button).children(popover)
}

/// One row of a popover: a name over what choosing it means, with a tick on the one that is current.
pub fn row(name: &str, description: &str, current: bool, id: FocusId) -> Node {
    let mut styles = vec![
        flex(),
        flex_none(),
        items_center(),
        gap(3),
        px(2.5),
        py(1.5),
        rounded(Radius::Lg),
        cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ];
    if current {
        styles.push(bg(Role::SurfaceSelected));
    }
    let mut texts = node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0)])
        .child(line(name, 13, 1.4, Role::Text).styles([truncate()]));
    if !description.is_empty() {
        texts = texts.child(line(description, 12, 1.4, Role::Subtle).styles([truncate()]));
    }
    let mut row = node().styles(styles).child(texts);
    if current {
        row = row.child(glyph(IconName::Check, 4.0, Role::Text));
    }
    row.focusable(id)
}

/// The small heading line of a popover, with `hint` at its right: a key to press, and what it does.
pub fn heading(title: &str, hint: Option<(&str, &str)>) -> Node {
    let mut row = node()
        .styles([flex(), items_center(), justify_between(), h(8), px(2.5)])
        .child(line(title, 12, 1.4, Role::Subtle));
    if let Some((key, does)) = hint {
        row = row.child(key_hint(key, does));
    }
    row
}

/// A key to press, in a small chip, and what it does.
pub fn key_hint(key: &str, does: &str) -> Node {
    node()
        .styles([flex(), items_center(), gap(1.5)])
        .child(
            node()
                .styles([px(1.5), rounded(Radius::Md), bg(Role::SurfaceHover)])
                .child(text(key.to_owned()).styles([
                    text_px(11),
                    leading_ratio(1.6),
                    font_mono(),
                    text_color(Role::Muted),
                ])),
        )
        .child(line(does, 12, 1.4, Role::Subtle))
}
