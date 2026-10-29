//! The tab chip: one tab of a strip inside a pane, with its glyph, its words, marks for what it is and a button that closes it.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, flex, flex_none, gap, h_px, items_center, px, rounded, text_color, w_px,
};
use pm_ui::{Node, node};

use crate::Button;
use crate::parts::{clipped, glyph};
use crate::size::Size;

/// What a tab chip says about whose tab it is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TabVariant {
    /// A tab of the reader's own.
    #[default]
    Page,
    /// A tab an agent opened: marked with sparkles in the accent colour.
    Agent,
}

/// A tab chip being described: build it with [`TabChip::new`], change it with the methods that follow, and turn it into a node with [`TabChip::build`].
#[derive(Clone, Debug)]
pub struct TabChip {
    /// The words on the tab.
    label: String,
    /// The target that selects the tab.
    select: FocusId,
    /// The target of the button that closes it.
    close: FocusId,
    /// The glyph before the words.
    icon: IconName,
    /// Whether it is the tab being shown.
    selected: bool,
    /// Whose tab it is.
    variant: TabVariant,
    /// How big it is: `Sm` is the strip's usual height, `Xs` a tighter one.
    size: Size,
    /// Whether a padlock marks it as kept from agents.
    locked: bool,
    /// How wide it is, in logical pixels.
    width: f32,
    /// What it says when the pointer rests on it, if anything.
    hint: Option<String>,
}

impl TabChip {
    /// A tab saying `label`, selected by `select` and closed by `close`.
    pub fn new(label: impl Into<String>, select: FocusId, close: FocusId) -> Self {
        Self {
            label: label.into(),
            select,
            close,
            icon: IconName::Globe,
            selected: false,
            variant: TabVariant::Page,
            size: Size::Sm,
            locked: false,
            width: 160.0,
            hint: None,
        }
    }

    /// Whose tab it is.
    pub fn variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    /// How big it is.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Whether it is the tab being shown.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Whether a padlock marks it as kept from agents.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// How wide it is, in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// What it says when the pointer rests on it.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The tab as a node.
    pub fn build(self) -> Node {
        let tone = if self.selected {
            Role::Text
        } else {
            Role::Muted
        };
        let height = if self.size == Size::Xs { 24.0 } else { 28.0 };
        let mut chip = node().styles([
            if self.selected {
                bg(Role::SurfaceSelected)
            } else {
                bg(Role::SurfaceHover).hover()
            },
            flex(),
            flex_none(),
            items_center(),
            gap(1),
            px(2),
            h_px(height),
            w_px(self.width),
            rounded(Radius::Md),
            pm_style::cursor_pointer(),
            text_color(tone),
        ]);
        chip = chip.child(match self.variant {
            TabVariant::Agent => glyph(IconName::Sparkles, 3.5, Role::Accent),
            TabVariant::Page => glyph(self.icon, 3.5, tone),
        });
        chip = chip.child(clipped(self.label, 12, tone));
        if self.locked {
            chip = chip.child(glyph(IconName::Lock, 3.0, Role::Warning));
        }
        chip = chip.child(
            Button::icon(IconName::Close, self.close)
                .variant(crate::button::Variant::Ghost)
                .size(Size::Xs)
                .build(),
        );
        if let Some(hint) = self.hint {
            chip = chip.tooltip(hint);
        }
        chip.focusable(self.select)
    }
}
