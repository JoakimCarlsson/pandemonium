//! The tab chip: one tab of a strip inside a pane, with its glyph, its words, marks for what it is and a button that closes it.

use crate::kit::{
    Flex, FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Styled, Tooltip, row,
};
use rustle::{CursorIcon, Modifier};

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

/// A tab chip being described: build it with [`TabChip::new`], change it with the methods that follow, and emit it with [`TabChip::build`].
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
    /// What the caller puts around its box.
    modifier: Modifier,
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
            modifier: Modifier::new(),
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

    /// The tab with `modifier` put around its box, outside its own: sizes, weight, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the tab.
    #[track_caller]
    pub fn build(self) {
        let tone = if self.selected {
            Role::Text
        } else {
            Role::Muted
        };
        let height = if self.size == Size::Xs { 24.0 } else { 28.0 };
        let looks = if self.selected {
            Looks::new().idle(Role::SurfaceSelected)
        } else {
            Looks::new().hover(Role::SurfaceHover)
        }
        .radius(Radius::Md);
        let mut own = Modifier::new()
            .width(self.width)
            .height(height)
            .looks_of(looks, self.select)
            .cursor(CursorIcon::Pointer)
            .target(self.select);
        if let Some(hint) = self.hint {
            own = own.tooltip(hint);
        }
        let own = own.pad_x(2);
        let (variant, icon, label, locked, close) =
            (self.variant, self.icon, self.label, self.locked, self.close);
        row(
            self.modifier.chain(own),
            Flex::new().center().gap(1),
            move || {
                match variant {
                    TabVariant::Agent => glyph(IconName::Sparkles, 3.5, Role::Accent),
                    TabVariant::Page => glyph(icon, 3.5, tone),
                }
                clipped(label, 12, tone);
                if locked {
                    glyph(IconName::Lock, 3.0, Role::Warning);
                }
                Button::icon(IconName::Close, close)
                    .variant(crate::button::Variant::Ghost)
                    .size(Size::Xs)
                    .build();
            },
        );
    }
}
