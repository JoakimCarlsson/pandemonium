//! The popover: a raised surface that floats over a screen, with a heading and rows of choices.

use crate::kit::sp;
use crate::kit::{
    FocusId, Hovered, IconName, Interactive, Looks, Placement, Radius, Role, Styled, Type, label,
};
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::parts::{glyph, line};
use crate::{Surface, surface};

/// One entry of a popover, kept until it is built.
#[derive(Clone, Debug)]
enum Entry {
    /// A heading line saying `title`, with `hint` at its right: a key to press, and what it does.
    Heading {
        /// The words of the heading.
        title: String,
        /// The key to press and what it does, shown at the heading's right.
        hint: Option<(String, String)>,
    },
    /// A row of `name` over `description`, ticked while `current`, answering to `id`.
    Row {
        /// The name of the choice.
        name: String,
        /// What choosing it means.
        description: String,
        /// Whether it is the current choice.
        current: bool,
        /// What the row answers to.
        id: FocusId,
    },
}

/// A popover being described: the rows and sections it holds.
///
/// Build it with [`Popover::new`], fill it with [`Popover::heading`] and
/// [`Popover::row`], and emit it with [`Popover::build`], or with
/// [`Popover::build_with`] to put more after its entries. Place it over what
/// it belongs to with [`anchored`] or [`anchored_end`], emitting it there with
/// [`Popover::build_target`] so pressing it does not count as pressing outside.
#[derive(Clone, Debug, Default)]
pub struct Popover {
    /// What it holds, in order.
    entries: Vec<Entry>,
    /// How wide it is, when fixed.
    width: Option<f32>,
    /// Whether it is as wide as what it is placed in.
    full_width: bool,
    /// The padding inside its border, in place of its own, when the caller gave one.
    inner: Option<Modifier>,
    /// What the caller puts around its box.
    modifier: Modifier,
}

impl Popover {
    /// An empty popover.
    pub fn new() -> Self {
        Self::default()
    }

    /// The popover `pixels` wide, and no wider.
    pub fn width(mut self, pixels: f32) -> Self {
        self.width = Some(pixels);
        self
    }

    /// The popover as wide as what it is placed in.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// The popover with `padding` inside its border in place of its own.
    pub fn inner(mut self, padding: Modifier) -> Self {
        self.inner = Some(padding);
        self
    }

    /// The popover with a heading line saying `title`, and `hint` at its right.
    pub fn heading(mut self, title: &str, hint: Option<(&str, &str)>) -> Self {
        self.entries.push(Entry::Heading {
            title: title.to_owned(),
            hint: hint.map(|(key, does)| (key.to_owned(), does.to_owned())),
        });
        self
    }

    /// The popover with a row of `name` over `description`, ticked while `current`, answering to `id`.
    pub fn row(mut self, name: &str, description: &str, current: bool, id: FocusId) -> Self {
        self.entries.push(Entry::Row {
            name: name.to_owned(),
            description: description.to_owned(),
            current,
            id,
        });
        self
    }

    /// `modifier` put around the popover's box, outside its own: sizes, weight, placement, a target.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the popover holding its entries, then what `content` emits.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let mut own = Modifier::new();
        if let Some(pixels) = self.width {
            own = own.width(pixels).max_w_px(pixels);
        }
        if self.full_width {
            own = own.fill_max_width();
        }
        if self.width.is_none() && !self.full_width {
            own = own.width_intrinsic(rustle::IntrinsicSize::Max);
        }
        let entries = self.entries;
        let inner = self.inner.unwrap_or_else(|| Modifier::new().pad(1.5));
        Surface::new()
            .variant(surface::Variant::Floating)
            .inner(inner)
            .modifier(self.modifier.chain(own))
            .build_with(move || {
                for entry in entries {
                    match entry {
                        Entry::Heading { title, hint } => heading(
                            &title,
                            hint.as_ref()
                                .map(|(key, does)| (key.as_str(), does.as_str())),
                        ),
                        Entry::Row {
                            name,
                            description,
                            current,
                            id,
                        } => rustle::key(id.0, || row(&name, &description, current, id)),
                    }
                }
                content();
            });
    }

    /// Emits the popover.
    #[track_caller]
    pub fn build(self) {
        self.build_with(|| {});
    }

    /// Emits the popover answering to `id`, so that pressing it does not count as pressing outside.
    /// Placed over its trigger by [`anchored`] or [`anchored_end`].
    #[track_caller]
    pub fn build_target(self, id: FocusId) {
        self.modifier(Modifier::new().target(id)).build();
    }

    /// Emits the popover answering to `id`, so that pressing it does not count as pressing outside;
    /// placed above its button, left edges together, by [`anchored`].
    #[track_caller]
    pub fn over_button(self, id: FocusId) {
        self.build_target(id);
    }

    /// Emits the popover answering to `id`, so that pressing it does not count as pressing outside;
    /// placed above its button, right edges together, by [`anchored_end`].
    #[track_caller]
    pub fn over_button_end(self, id: FocusId) {
        self.build_target(id);
    }

    /// Emits the popover across the width of the box it hangs over; placed just above that box by [`anchored`].
    #[track_caller]
    pub fn above_box(self) {
        self.modifier(Modifier::new().fill_max_width()).build();
    }
}

/// Emits `button` with `popover`, when there is one, hanging a little way above it with their left edges together.
#[track_caller]
pub fn anchored(button: impl FnOnce(), popover: Option<impl FnOnce()>) {
    crate::kit::anchored(Modifier::new(), Placement::Above, 2.0, button, popover);
}

/// Emits `button` with `popover`, when there is one, hanging a little way above it with their right edges together.
#[track_caller]
pub fn anchored_end(button: impl FnOnce(), popover: Option<impl FnOnce()>) {
    crate::kit::anchored(Modifier::new(), Placement::AboveEnd, 2.0, button, popover);
}

/// Emits one row of a popover: a name over what choosing it means, with a tick on the one that is current.
#[track_caller]
pub fn row(name: &str, description: &str, current: bool, id: FocusId) {
    let looks = if current {
        Looks::new().idle(Role::SurfaceSelected)
    } else {
        Looks::new().hover(Role::SurfaceHover)
    }
    .radius(Radius::Lg);
    let modifier = Modifier::new()
        .fill_max_width()
        .looks_of(looks, id)
        .cursor(CursorIcon::Pointer)
        .target(id)
        .pad_x(2.5)
        .pad_y(1.5);
    row_arranged(
        modifier,
        Arrangement::SpacedBy(sp(3)),
        Alignment::Center,
        || {
            column_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    line(name, 13, 1.4, Role::Text);
                    if !description.is_empty() {
                        line(description, 12, 1.4, Role::Subtle);
                    }
                },
            );
            if current {
                glyph(IconName::Check, 4.0, Role::Text);
            }
        },
    );
}

/// Emits the small heading line of a popover, with `hint` at its right: a key to press, and what it does.
#[track_caller]
pub fn heading(title: &str, hint: Option<(&str, &str)>) {
    row_arranged(
        Modifier::new().fill_max_width().h(8).pad_x(2.5),
        Arrangement::SpaceBetween,
        Alignment::Center,
        || {
            line(title, 12, 1.4, Role::Subtle);
            if let Some((key, does)) = hint {
                key_hint(key, does);
            }
        },
    );
}

/// Emits a key to press, in a small chip, and what it does.
#[track_caller]
pub fn key_hint(key: &str, does: &str) {
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(sp(1.5)),
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new()
                    .fill_rounded(Role::SurfaceHover, Radius::Md)
                    .pad_x(1.5),
                Arrangement::Start,
                Alignment::Start,
                || label(key, Type::new(11).ratio(1.6).mono().tone(Role::Muted)),
            );
            line(does, 12, 1.4, Role::Subtle);
        },
    );
}
