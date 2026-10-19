//! The button: one component with a look per variant and a measure per size.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, animate_spin, bg, border, border_color, cursor_not_allowed,
    cursor_pointer, flex, font_medium, font_mono, gap, h, items_center, justify_center,
    leading_ratio, opacity, px, ring, ring_color, rounded, rounded_full, size, text_color, text_px,
    underline,
};
use pm_ui::{Node, node, text};

use crate::size::Size;

/// How a button looks, which says what it is for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// The main action: the accent fill.
    #[default]
    Default,
    /// A quieter action: a muted fill.
    Secondary,
    /// An action that cannot be undone: the danger fill.
    Destructive,
    /// An alternative: an outline that fills under the pointer.
    Outline,
    /// A quiet action: bare text that fills under the pointer.
    Ghost,
    /// An action that reads as a link: underlined text.
    Link,
}

/// A button being described: what it says, how it looks, and what it does.
///
/// Build it with [`Button::new`] or [`Button::icon`], change it with the
/// methods that follow, and turn it into a node with [`Button::build`].
#[derive(Clone, Debug)]
pub struct Button {
    /// The words on the button, if any.
    label: Option<String>,
    /// The icon before the words, or alone.
    icon: Option<IconName>,
    /// How it looks.
    variant: Variant,
    /// How big it is.
    size: Size,
    /// The name it answers to.
    id: FocusId,
    /// Whether it is switched off.
    disabled: bool,
    /// Whether its corners are fully round.
    round: bool,
    /// Whether its words are set in the code font.
    monospace: bool,
    /// Whether its icon turns, to say something is under way.
    spinning: bool,
}

impl Button {
    /// A button saying `label`, answering to `id`.
    pub fn new(label: impl Into<String>, id: FocusId) -> Self {
        Self {
            label: Some(label.into()),
            icon: None,
            variant: Variant::Default,
            size: Size::Md,
            id,
            disabled: false,
            round: false,
            monospace: false,
            spinning: false,
        }
    }

    /// A button showing only `icon`, answering to `id`.
    pub fn icon(icon: IconName, id: FocusId) -> Self {
        Self {
            label: None,
            icon: Some(icon),
            variant: Variant::Ghost,
            size: Size::Sm,
            id,
            disabled: false,
            round: false,
            monospace: false,
            spinning: false,
        }
    }

    /// The button with `variant`'s look.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The button with fully round corners.
    pub fn round(mut self) -> Self {
        self.round = true;
        self
    }

    /// The button with its words set in the code font.
    pub fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }

    /// The button at `size`: 32, 36 or 40px tall, or a square 28, 32 or 36px on a side while it holds only an icon.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The button with `icon` before its words.
    pub fn with_icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The button switched off while `disabled` is true: drawn faint, with
    /// no pointer hand, and neither taking focus nor answering a press.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The button with its icon turning while `spinning` is true.
    pub fn spinning(mut self, spinning: bool) -> Self {
        self.spinning = spinning;
        self
    }

    /// The node that draws the button.
    pub fn build(self) -> Node {
        let ink = self.ink();
        let (height, side, font) = match self.size {
            Size::Sm => (8.0, 3.0, 12),
            Size::Md => (9.0, 4.0, 13),
            Size::Lg => (10.0, 5.0, 13),
        };
        let square = match self.size {
            Size::Sm => 7.0,
            Size::Md => 8.0,
            Size::Lg => 9.0,
        };
        let mut styles = vec![
            flex(),
            items_center(),
            justify_center(),
            gap(2),
            if self.round {
                rounded_full()
            } else {
                rounded(Radius::Md)
            },
            ring(2).focus(),
            ring_color(Role::Focused).focus(),
        ];
        if self.label.is_none() {
            styles.push(size(square));
        } else {
            styles.extend([h(height), px(side)]);
        }
        styles.extend(self.look());
        if self.disabled {
            styles.extend([opacity(40), cursor_not_allowed()]);
        } else {
            styles.push(cursor_pointer());
        }
        let mut button = node().styles(styles);
        if let Some(icon) = self.icon {
            let mut mark = node().styles([size(4), text_color(ink)]).icon(icon);
            if self.spinning {
                mark = mark.styles([animate_spin()]);
            }
            button = button.child(mark);
        }
        if let Some(label) = self.label {
            let mut words = vec![
                text_px(font),
                leading_ratio(1.4),
                font_medium(),
                text_color(ink),
            ];
            if self.variant == Variant::Link {
                words.push(underline());
            }
            if self.monospace {
                words.push(font_mono());
            }
            button = button.child(text(label).styles(words));
        }
        if self.disabled {
            button
        } else {
            button.focusable(self.id)
        }
    }

    /// The colour of the words and the icon.
    fn ink(&self) -> Role {
        match self.variant {
            Variant::Default => Role::OnAccent,
            Variant::Destructive => Role::OnDanger,
            Variant::Secondary | Variant::Outline | Variant::Link => Role::Text,
            Variant::Ghost => Role::Muted,
        }
    }

    /// The fill, the outline, and, while the button is on, how they change
    /// under the pointer.
    fn look(&self) -> Vec<pm_style::Utility> {
        let (resting, hovered, pressed): (Option<Color>, Option<Color>, Option<Color>) =
            match self.variant {
                Variant::Default => (
                    Some(Role::Accent.into()),
                    Some(Role::AccentHover.into()),
                    Some(Color::from(Role::Accent).alpha(0.8)),
                ),
                Variant::Secondary => (
                    Some(Role::SurfaceHover.into()),
                    Some(Role::SurfaceActive.into()),
                    Some(Role::SurfaceSelected.into()),
                ),
                Variant::Destructive => (
                    Some(Role::Danger.into()),
                    Some(Color::from(Role::Danger).alpha(0.9)),
                    Some(Color::from(Role::Danger).alpha(0.8)),
                ),
                Variant::Outline | Variant::Ghost => (
                    None,
                    Some(Role::SurfaceHover.into()),
                    Some(Role::SurfaceActive.into()),
                ),
                Variant::Link => (None, None, None),
            };
        let mut utilities = Vec::new();
        if self.variant == Variant::Outline {
            utilities.extend([border(), border_color(Role::Border)]);
        }
        utilities.extend(resting.map(bg));
        if !self.disabled {
            utilities.extend(hovered.map(|color| bg(color).hover()));
            utilities.extend(pressed.map(|color| bg(color).active()));
        }
        utilities
    }
}

impl From<Button> for Node {
    /// The node that draws the button.
    fn from(button: Button) -> Self {
        button.build()
    }
}
