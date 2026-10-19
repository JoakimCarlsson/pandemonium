//! The chip: a small control with a mark and a label that is filled while it is chosen or open.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, gap, h, items_center, px,
    rounded, rounded_full,
};
use pm_ui::{Node, node};

use crate::parts::{glyph, line};
use crate::size::Size;

/// How a chip looks, which says what it is for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// An outlined pill that stands alone: it names a choice or opens a panel.
    #[default]
    Outline,
    /// A borderless button in a strip of controls: it names a value and opens its menu.
    Ghost,
}

/// A chip being described: what it says, how it looks and what it does.
///
/// Build it with [`Chip::new`], change it with the methods that follow, and
/// turn it into a node with [`Chip::build`].
#[derive(Clone, Debug)]
pub struct Chip {
    /// The words on the chip.
    label: String,
    /// The glyph before the words, if any.
    icon: Option<IconName>,
    /// A node before the words in place of a glyph, if any.
    mark: Option<Node>,
    /// The name it answers to.
    id: FocusId,
    /// Whether it is filled.
    selected: bool,
    /// How it looks.
    variant: Variant,
    /// How big it is.
    size: Size,
}

impl Chip {
    /// A chip saying `label` that answers to `id`.
    pub fn new(label: impl Into<String>, id: FocusId) -> Self {
        Self {
            label: label.into(),
            icon: None,
            mark: None,
            id,
            selected: false,
            variant: Variant::default(),
            size: Size::default(),
        }
    }

    /// The chip with `icon` before its words.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The chip with `mark` before its words.
    pub fn mark(mut self, mark: Node) -> Self {
        self.mark = Some(mark);
        self
    }

    /// The chip filled while `selected` is true.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The chip looking like `variant`.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The chip measuring `size`: 28, 36 or 40px tall.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The node that draws the chip.
    pub fn build(self) -> Node {
        let outlined = self.variant == Variant::Outline;
        let large = self.size != Size::Sm;
        let tone = if outlined && self.selected {
            Role::Text
        } else {
            Role::Muted
        };
        let mut styles = vec![
            flex(),
            items_center(),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ];
        match (self.variant, self.size) {
            (Variant::Outline, Size::Sm) => styles.extend([gap(1.5), h(7), px(3)]),
            (Variant::Outline, Size::Md) => styles.extend([gap(2), h(9), px(3.5)]),
            (Variant::Outline, Size::Lg) => styles.extend([gap(2), h(10), px(4)]),
            (Variant::Ghost, _) => styles.extend([gap(1), h(7), px(2)]),
        }
        if outlined {
            styles.extend([rounded_full(), border()]);
            if self.selected {
                styles.extend([bg(Role::SurfaceSelected), border_color(Role::Muted)]);
            } else {
                styles.push(border_color(Role::Border));
            }
        } else {
            styles.push(rounded(Radius::Md));
            if self.selected {
                styles.push(bg(Role::SurfaceSelected));
            }
        }
        let mut chip = node().styles(styles);
        if let Some(mark) = self.mark {
            chip = chip.child(mark);
        } else if let Some(icon) = self.icon {
            chip = chip.child(glyph(icon, if large { 4.0 } else { 3.5 }, tone));
        }
        chip.child(line(self.label, if large { 13 } else { 12 }, 1.4, tone))
            .focusable(self.id)
    }
}
