//! The button: one component with a look per variant and a measure per size.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, animate_spin, bg, border, border_color, cursor_not_allowed,
    cursor_pointer, flex, flex_1, flex_none, font_medium, font_mono, gap, h, items_center,
    justify_between, justify_center, leading_ratio, min_w_px, opacity, px, ring, ring_color,
    rounded, rounded_full, size, text_color, text_px, underline, w_px,
};
use pm_ui::{Node, node, text};

use crate::parts::{clipped, glyph};
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
    /// A control that shows a value and opens its choices: a filled box with an outline.
    Field,
}

/// How far a button reaches across its row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Reach {
    /// As far as its words and icons need.
    #[default]
    Content,
    /// A fixed number of logical pixels, its words cut short at the edge.
    Fixed(f32),
    /// All the room its row has left, its words cut short at the edge.
    Fill,
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
    /// A node before the words in place of an icon, if any.
    mark: Option<Node>,
    /// The icon after the words, if any.
    trailing: Option<IconName>,
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
    /// Whether it is filled to say it is the one chosen, or open.
    selected: bool,
    /// The colour of its words and icons in place of its variant's, if any.
    tone: Option<Role>,
    /// How far it reaches across its row.
    reach: Reach,
}

impl Button {
    /// A button saying `label`, answering to `id`.
    pub fn new(label: impl Into<String>, id: FocusId) -> Self {
        Self {
            label: Some(label.into()),
            icon: None,
            mark: None,
            trailing: None,
            variant: Variant::Default,
            size: Size::Md,
            id,
            disabled: false,
            round: false,
            monospace: false,
            spinning: false,
            selected: false,
            tone: None,
            reach: Reach::Content,
        }
    }

    /// A button showing only `icon`, answering to `id`.
    pub fn icon(icon: IconName, id: FocusId) -> Self {
        Self {
            label: None,
            icon: Some(icon),
            mark: None,
            trailing: None,
            variant: Variant::Ghost,
            size: Size::Sm,
            id,
            disabled: false,
            round: false,
            monospace: false,
            spinning: false,
            selected: false,
            tone: None,
            reach: Reach::Content,
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

    /// The button at `size`: 28, 32, 36 or 40px tall, or a square 24, 28, 32 or 36px on a side while it holds only an icon.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The button with `icon` before its words.
    pub fn with_icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The button with `mark` before its words in place of an icon.
    pub fn mark(mut self, mark: Node) -> Self {
        self.mark = Some(mark);
        self
    }

    /// The button with `icon` after its words.
    pub fn trailing_icon(mut self, icon: IconName) -> Self {
        self.trailing = Some(icon);
        self
    }

    /// The button filled while `selected` is true, to say it is the one chosen or that what it opens is open.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The button with its words and icons in `tone` in place of its variant's.
    pub fn tone(mut self, tone: Role) -> Self {
        self.tone = Some(tone);
        self
    }

    /// The button `width` logical pixels wide, its words cut short at the edge and its trailing icon at the far end.
    pub fn width(mut self, width: f32) -> Self {
        self.reach = Reach::Fixed(width);
        self
    }

    /// The button as wide as the room its row has left, its words cut short at the edge and its trailing icon at the far end.
    pub fn fill(mut self) -> Self {
        self.reach = Reach::Fill;
        self
    }

    /// The button switched off while `disabled` is true: drawn faint, with
    /// no pointer hand, and neither taking focus nor answering a press.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The button with its trailing icon, or its icon when it has no trailing one, turning while `spinning` is true.
    pub fn spinning(mut self, spinning: bool) -> Self {
        self.spinning = spinning;
        self
    }

    /// The node that draws the button.
    pub fn build(self) -> Node {
        let ink = self.ink();
        let (height, side, font, square) = match self.size {
            Size::Xs => (7.0, 2.5, 12, 6.0),
            Size::Sm => (8.0, 3.0, 12, 7.0),
            Size::Md => (9.0, 4.0, 13, 8.0),
            Size::Lg => (10.0, 5.0, 13, 9.0),
        };
        let spread = self.reach != Reach::Content;
        let mut styles = vec![
            flex(),
            items_center(),
            if spread {
                justify_between()
            } else {
                justify_center()
            },
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
        match self.reach {
            Reach::Content => {}
            Reach::Fixed(width) => styles.extend([w_px(width), flex_none()]),
            Reach::Fill => styles.extend([flex_1(), min_w_px(0)]),
        }
        styles.extend(self.look());
        if self.disabled {
            styles.extend([opacity(40), cursor_not_allowed()]);
        } else {
            styles.push(cursor_pointer());
        }
        let mut button = node().styles(styles);
        if let Some(mark) = self.mark {
            button = button.child(mark);
        } else if let Some(icon) = self.icon {
            let spinning = self.spinning && self.trailing.is_none();
            button = button.child(Self::glyph(icon, ink, spinning));
        }
        if let Some(label) = self.label {
            let mut words = Vec::new();
            if self.variant != Variant::Field {
                words.push(font_medium());
            }
            if self.variant == Variant::Link {
                words.push(underline());
            }
            if self.monospace {
                words.push(font_mono());
            }
            button = button.child(if spread {
                clipped(label, font, ink).styles(words)
            } else {
                words.extend([text_px(font), leading_ratio(1.4), text_color(ink)]);
                text(label).styles(words)
            });
        }
        if let Some(icon) = self.trailing {
            button = button.child(Self::glyph(icon, ink, self.spinning));
        }
        if self.disabled {
            button
        } else {
            button.focusable(self.id)
        }
    }

    /// An icon in `ink`, turning while `spinning` is true.
    fn glyph(icon: IconName, ink: Role, spinning: bool) -> Node {
        let mark = glyph(icon, 4.0, ink);
        if spinning {
            mark.styles([animate_spin()])
        } else {
            mark
        }
    }

    /// The colour of the words and the icon.
    fn ink(&self) -> Role {
        self.tone.unwrap_or(match self.variant {
            Variant::Default => Role::OnAccent,
            Variant::Destructive => Role::OnDanger,
            Variant::Secondary | Variant::Outline | Variant::Link | Variant::Field => Role::Text,
            Variant::Ghost => Role::Muted,
        })
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
                Variant::Field => (
                    Some(Role::Surface.into()),
                    Some(Role::SurfaceHover.into()),
                    Some(Role::SurfaceActive.into()),
                ),
                Variant::Link => (None, None, None),
            };
        let mut utilities = Vec::new();
        match (self.variant, self.selected) {
            (Variant::Outline, true) => utilities.extend([border(), border_color(Role::Muted)]),
            (Variant::Outline, false) => utilities.extend([border(), border_color(Role::Border)]),
            (Variant::Field, true) => utilities.extend([border(), border_color(Role::Focused)]),
            (Variant::Field, false) => utilities.extend([border(), border_color(Role::Border)]),
            _ => {}
        }
        match (self.variant, self.selected) {
            (Variant::Field, _) => utilities.extend(resting.map(bg)),
            (_, true) => utilities.push(bg(Role::SurfaceSelected)),
            _ => utilities.extend(resting.map(bg)),
        }
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
