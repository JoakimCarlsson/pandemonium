//! The popover: a raised surface that floats over a screen, with a heading and rows of choices.

use crate::events;
use crate::events::Interactive;
use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use crate::theme::{Radius, Role};
use rustle::Modifier;
use rustle::TextOverflow;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, row_arranged};

use crate::parts::{glyph, line};
use crate::popups::Placement;
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
        id: Target,
    },
}

/// A popover being described: the rows and sections it holds.
///
/// Build it with [`Popover::new`], fill it with [`Popover::heading`] and
/// [`Popover::row`], and emit it with [`Popover::build_with`], giving what follows its entries. Place it over what
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
    pub fn row(mut self, name: &str, description: &str, current: bool, id: Target) -> Self {
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
            own = own.width(pixels).width_in(0.0, pixels);
        }
        if self.full_width {
            own = own.fill_max_width();
        }
        if self.width.is_none() && !self.full_width {
            own = own.width_intrinsic(rustle::IntrinsicSize::Max);
        }
        let entries = self.entries;
        let inner = self
            .inner
            .unwrap_or_else(|| Modifier::new().padding(theme::sp(1.5)));
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
                        } => rustle::key(id, || row(&name, &description, current, id)),
                    }
                }
                content();
            });
    }

    /// Emits the popover answering to `id`, so that pressing it does not count as pressing outside.
    /// Placed over its trigger by [`anchored`] or [`anchored_end`].
    #[track_caller]
    pub fn build_target(self, id: Target) {
        self.modifier(Modifier::new().control(id, send(Event::pressing(id))))
            .build_with(|| {});
    }
}

/// Emits `button` with `popover`, when there is one, hanging a little way above it with their
/// left edges together; a press elsewhere asks the app to close it.
#[track_caller]
pub fn anchored(button: impl FnOnce(), popover: Option<impl FnOnce()>) {
    hang(Placement::Above, button, popover);
}

/// Emits `button` with `popover`, when there is one, hanging a little way above it with their
/// right edges together; a press elsewhere asks the app to close it.
pub fn anchored_end(button: impl FnOnce(), popover: Option<impl FnOnce()>) {
    hang(Placement::AboveEnd, button, popover);
}

/// Emits `button` with `popover`, when there is one, hanging a little way from it at `placement`; a press elsewhere asks the app to close it.
#[track_caller]
fn hang(placement: Placement, button: impl FnOnce(), popover: Option<impl FnOnce()>) {
    crate::popups::anchored(Modifier::new(), placement, 2.0, true, button, popover);
}

/// Emits one row of a popover: a name over what choosing it means, with a tick on the one that is current.
#[track_caller]
pub fn row(name: &str, description: &str, current: bool, id: Target) {
    let looks = theme::selectable(current).radius(Radius::Lg);
    let modifier = events::pressed(
        Modifier::new()
            .fill_max_width()
            .indicated(looks, theme::Focused::Control(crate::focus::is_focused(id))),
        id,
    )
    .padding_xy(theme::sp(2.5), theme::sp(1.5));
    rustle::row_arranged(
        modifier,
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Center,
        || {
            rustle::column_arranged(
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
                glyph(phosphor::CHECK, 4.0, Role::Text);
            }
        },
    );
}

/// Emits the small heading line of a popover, with `hint` at its right: a key to press, and what it does.
#[track_caller]
pub fn heading(title: &str, hint: Option<(&str, &str)>) {
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(8))
            .padding_xy(theme::sp(2.5), 0.0),
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
    rustle::row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(theme::sp(1.5)),
        Alignment::Center,
        || {
            rustle::row_arranged(
                Modifier::new()
                    .background_rounded(theme::color(Role::SurfaceHover), Radius::Md.px())
                    .padding_xy(theme::sp(1.5), 0.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        key,
                        theme::mono_style(11, 1.6).color(theme::color(Role::Muted)),
                        TextOverflow::Ellipsis,
                    )
                },
            );
            line(does, 12, 1.4, Role::Subtle);
        },
    );
}
