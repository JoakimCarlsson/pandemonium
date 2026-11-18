//! The button: one component with a look per variant and a measure per size.

use std::fmt;
use std::rc::Rc;

use crate::kit::{
    Flex, FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Styled, Tone, Type, column,
    icon, label, label_with, row, spinner, spinning_icon, wrap_lines,
};
use rustle::{CursorIcon, Modifier};

use crate::size::Size;

/// Content drawn before a button's words in place of an icon: a closure that emits it.
#[derive(Clone)]
struct Mark(Rc<dyn Fn()>);

impl fmt::Debug for Mark {
    /// Writes a placeholder, since the content is a closure.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Mark(..)")
    }
}

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
/// methods that follow, and draw it with [`Button::build`].
#[derive(Clone, Debug)]
pub struct Button {
    /// The words on the button, if any.
    label: Option<String>,
    /// The icon before the words, or alone.
    icon: Option<IconName>,
    /// What is drawn before the words in place of an icon, if anything.
    mark: Option<Mark>,
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
    /// The lines its label is broken into when it is set to wrap, one text line each.
    lines: Vec<String>,
    /// What the caller puts around the button's box, outside its own.
    modifier: Modifier,
    /// The padding that replaces the button's own, when the caller gave one.
    inner: Option<Modifier>,
    /// How the button lays out what it holds, in place of its own, when the caller gave one.
    flex: Option<Flex>,
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
            lines: Vec::new(),
            modifier: Modifier::new(),
            inner: None,
            flex: None,
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
            lines: Vec::new(),
            modifier: Modifier::new(),
            inner: None,
            flex: None,
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

    /// The button with what `mark` emits before its words in place of an icon.
    pub fn mark(mut self, mark: impl Fn() + 'static) -> Self {
        self.mark = Some(Mark(Rc::new(mark)));
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

    /// The height, the side padding, the type size and the side of a square button at this size, all but the type size in steps.
    fn measure(&self) -> (f32, f32, u32, f32) {
        match self.size {
            Size::Xs => (7.0, 2.5, 12, 6.0),
            Size::Sm => (8.0, 3.0, 12, 7.0),
            Size::Md => (9.0, 4.0, 13, 8.0),
            Size::Lg => (10.0, 5.0, 13, 9.0),
        }
    }

    /// The button with `modifier` put around its box, outside its own: sizes, growth, margins, placement, a tooltip.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The button with `modifier` in place of its own padding, innermost on its box.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = Some(modifier);
        self
    }

    /// The button laying out what it holds by `flex` in place of its own.
    pub fn flex(mut self, flex: Flex) -> Self {
        self.flex = Some(flex);
        self
    }

    /// The button with its label broken into lines that fit `width` logical pixels, so that a long label grows the button taller instead of running out of it.
    pub fn wrapped(mut self, width: f32) -> Self {
        let Some(label) = self.label.as_deref() else {
            return self;
        };
        let (_, side, font, _) = self.measure();
        let inner = (width - 2.0 * side * 4.0).max(32.0);
        let kind = Type::new(font).ratio(1.4).medium();
        self.lines = wrap_lines(label, kind, inner)
            .into_iter()
            .map(|part| label[part.start..part.end].trim_end().to_owned())
            .collect();
        self
    }

    /// Emits the button.
    #[track_caller]
    pub fn build(self) {
        let ink = self.ink();
        let (height, side, font, square) = self.measure();
        let spread = self.reach != Reach::Content;
        let radius = if self.round { Radius::Full } else { Radius::Md };
        let several = self.lines.len() > 1;
        let mut own = match self.reach {
            Reach::Content => Modifier::new(),
            Reach::Fixed(width) => Modifier::new().width(width),
            Reach::Fill => Modifier::new().grow(),
        };
        own = if self.label.is_none() {
            own.square(square)
        } else if several {
            own.min_h(height)
        } else {
            own.h(height)
        };
        own = own.looks_of(self.looks(radius), self.id);
        own = match (self.variant, self.selected) {
            (Variant::Outline, true) => own.stroke(Role::Muted, radius),
            (Variant::Outline, false) | (Variant::Field, false) => own.stroke(Role::Border, radius),
            (Variant::Field, true) => own.stroke(Role::Focused, radius),
            _ => own,
        };
        own = own.focus_ring(self.id, 2.0, radius);
        own = if self.disabled {
            own.opacity(40.0).cursor(CursorIcon::NotAllowed)
        } else {
            own.cursor(CursorIcon::Pointer).target(self.id)
        };
        own = match (self.inner, self.label.is_some(), several) {
            (Some(inner), _, _) => own.chain(inner),
            (None, true, true) => own.pad_x(side).pad_y(1.5),
            (None, true, false) => own.pad_x(side),
            (None, false, _) => own,
        };
        let flex = self.flex.unwrap_or(if spread {
            Flex::new().center().between().gap(2)
        } else {
            Flex::new().center().middle().gap(2)
        });
        let mut kind = Type::new(font).ratio(1.4).tone(ink);
        if self.variant != Variant::Field {
            kind = kind.medium();
        }
        if self.variant == Variant::Link {
            kind = kind.underline();
        }
        if self.monospace {
            kind = kind.mono();
        }
        let spins_first = self.spinning && self.trailing.is_none();
        let (mark, leading, trailing, spinning, words, lines) = (
            self.mark,
            self.icon,
            self.trailing,
            self.spinning,
            self.label,
            self.lines,
        );
        row(self.modifier.chain(own), flex, move || {
            if let Some(mark) = mark {
                (mark.0)();
            } else if let Some(name) = leading {
                Self::glyph(name, ink, spins_first);
            }
            if let Some(words) = words {
                if lines.len() > 1 {
                    column(Modifier::new(), Flex::new(), || {
                        for part in &lines {
                            label(part, kind);
                        }
                    });
                } else if spread {
                    label_with(Modifier::new().grow(), words, kind.truncate());
                } else {
                    label(words, kind);
                }
            }
            if let Some(name) = trailing {
                Self::glyph(name, ink, spinning);
            }
        });
    }

    /// Emits `name` four steps across in `ink`, turning while `spinning` is true.
    #[track_caller]
    fn glyph(name: IconName, ink: Role, spinning: bool) {
        match (spinning, name) {
            (true, IconName::Loading) => spinner(4.0, ink),
            (true, name) => spinning_icon(name, 4.0, ink),
            (false, name) => icon(name, 4.0, ink),
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

    /// The fill of the button at rest and, while it is on, under the pointer and pressed, with its corners `radius` round.
    fn looks(&self, radius: Radius) -> Looks {
        let (resting, hovered, pressed): (Option<Tone>, Option<Tone>, Option<Tone>) =
            match self.variant {
                Variant::Default => (
                    Some(Role::Accent.into()),
                    Some(Role::AccentHover.into()),
                    Some(Tone::from(Role::Accent).alpha(0.8)),
                ),
                Variant::Secondary => (
                    Some(Role::SurfaceHover.into()),
                    Some(Role::SurfaceActive.into()),
                    Some(Role::SurfaceSelected.into()),
                ),
                Variant::Destructive => (
                    Some(Role::Danger.into()),
                    Some(Tone::from(Role::Danger).alpha(0.9)),
                    Some(Tone::from(Role::Danger).alpha(0.8)),
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
        let resting = match (self.variant, self.selected) {
            (Variant::Field, _) => resting,
            (_, true) => Some(Role::SurfaceSelected.into()),
            _ => resting,
        };
        let mut looks = Looks::new().radius(radius);
        if let Some(tone) = resting {
            looks = looks.idle(tone);
        }
        if !self.disabled {
            if let Some(tone) = hovered {
                looks = looks.hover(tone);
            }
            if let Some(tone) = pressed {
                looks = looks.active(tone);
            }
        }
        looks
    }
}
