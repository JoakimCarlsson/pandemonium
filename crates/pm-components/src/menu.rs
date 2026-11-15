//! A menu of commands in a popover: rows with an icon, a label and a hint,
//! split by separators, some of them destructive or disabled.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::popover::Popover;
use crate::size::Size;
use rustle::color_scheme;

/// How a menu is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuVariant {
    /// A floating popover dismissed by a click outside.
    #[default]
    Floating,
    /// Rows only, for a menu inside another surface.
    Inline,
}

/// One entry of a menu.
#[derive(Clone)]
enum Entry {
    /// A command.
    Item {
        /// What it reads.
        label: String,
        /// The icon before it.
        icon: Option<Icon>,
        /// Faint text at its end, such as a key.
        hint: Option<String>,
        /// Whether it is drawn as a warning.
        destructive: bool,
        /// Whether it is greyed out and does nothing.
        disabled: bool,
        /// What choosing it does.
        on_select: Rc<dyn Fn()>,
    },
    /// A hairline between groups of commands.
    Separator,
    /// A small heading over a group of commands.
    Heading(String),
}

/// A menu with a variant and a size.
#[derive(Clone)]
pub struct Menu {
    /// Its entries, in order.
    entries: Vec<Entry>,
    /// How it is drawn.
    variant: MenuVariant,
    /// How tall its rows are.
    size: Size,
    /// How wide it is.
    width: f32,
    /// How far it is moved from where it is emitted, across and down.
    offset: (f32, f32),
    /// What a click outside it does.
    on_dismiss: Option<Rc<dyn Fn()>>,
    /// Whether it opens upward from where it is emitted.
    above: bool,
}

impl Menu {
    /// An empty floating menu.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            variant: MenuVariant::Floating,
            size: Size::Sm,
            width: 220.0,
            offset: (0.0, 4.0),
            on_dismiss: None,
            above: false,
        }
    }

    /// The menu opening upward from where it is emitted when `above`, for a field near the bottom of the window.
    pub fn above(mut self, above: bool) -> Self {
        self.above = above;
        self
    }

    /// The menu with a command reading `label` that calls `on_select`.
    pub fn item(self, label: impl Into<String>, on_select: impl Fn() + 'static) -> Self {
        self.entry(label.into(), None, None, false, false, Rc::new(on_select))
    }

    /// The menu with a command reading `label` after `icon` that calls `on_select`.
    pub fn icon_item(
        self,
        icon: Icon,
        label: impl Into<String>,
        on_select: impl Fn() + 'static,
    ) -> Self {
        self.entry(
            label.into(),
            Some(icon),
            None,
            false,
            false,
            Rc::new(on_select),
        )
    }

    /// The menu with a warning command reading `label` after `icon` that calls `on_select`.
    pub fn destructive_item(
        self,
        icon: Icon,
        label: impl Into<String>,
        on_select: impl Fn() + 'static,
    ) -> Self {
        self.entry(
            label.into(),
            Some(icon),
            None,
            true,
            false,
            Rc::new(on_select),
        )
    }

    /// The menu with a greyed-out command reading `label` after `icon`.
    pub fn disabled_item(self, icon: Icon, label: impl Into<String>) -> Self {
        self.entry(label.into(), Some(icon), None, false, true, Rc::new(|| {}))
    }

    /// The last command shown with `hint` at its end.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        if let Some(Entry::Item { hint: slot, .. }) = self.entries.last_mut() {
            *slot = Some(hint.into());
        }
        self
    }

    /// The menu with a hairline after what it holds so far.
    pub fn separator(mut self) -> Self {
        if !self.entries.is_empty() {
            self.entries.push(Entry::Separator);
        }
        self
    }

    /// The menu with a small heading reading `title`.
    pub fn heading(mut self, title: impl Into<String>) -> Self {
        self.entries.push(Entry::Heading(title.into()));
        self
    }

    /// The menu in `variant`.
    pub fn variant(mut self, variant: MenuVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The menu with rows at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The menu `width` wide.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The menu moved `across` and `down` from where it is emitted.
    pub fn offset(mut self, across: f32, down: f32) -> Self {
        self.offset = (across, down);
        self
    }

    /// The menu calling `on_dismiss` when clicked outside or when a command is chosen.
    pub fn on_dismiss(mut self, on_dismiss: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(on_dismiss));
        self
    }

    /// The menu with `entry` added.
    fn entry(
        mut self,
        label: String,
        icon: Option<Icon>,
        hint: Option<String>,
        destructive: bool,
        disabled: bool,
        on_select: Rc<dyn Fn()>,
    ) -> Self {
        self.entries.push(Entry::Item {
            label,
            icon,
            hint,
            destructive,
            disabled,
            on_select,
        });
        self
    }

    /// Emits the menu.
    #[track_caller]
    pub fn show(self) {
        let height = match self.size {
            Size::Xs => 22.0,
            Size::Sm => 26.0,
            Size::Md => 30.0,
            Size::Lg => 34.0,
        };
        let height = crate::row_height(height);
        let entries = self.entries;
        let dismiss = self.on_dismiss.clone();
        let rows = move || {
            for (place, entry) in entries.into_iter().enumerate() {
                let dismiss = dismiss.clone();
                key(place, move || entry_row(entry, height, dismiss));
            }
        };
        match self.variant {
            MenuVariant::Inline => column_with(Modifier::new().width(self.width), rows),
            MenuVariant::Floating => {
                let mut popover = Popover::new()
                    .width(self.width)
                    .offset(self.offset.0, self.offset.1)
                    .above(self.above);
                if let Some(dismiss) = self.on_dismiss {
                    popover = popover.on_dismiss(move || dismiss());
                }
                popover.show(rows);
            }
        }
    }
}

impl Default for Menu {
    /// The same as [`Menu::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// Emits `entry` `height` tall, calling `dismiss` after a command is chosen.
#[track_caller]
fn entry_row(entry: Entry, height: f32, dismiss: Option<Rc<dyn Fn()>>) {
    let scheme = color_scheme();
    match entry {
        Entry::Separator => spacer(
            Modifier::new()
                .fill_max_width()
                .padding_xy(0.0, 4.0)
                .height(9.0)
                .border_bottom_hairline(scheme.outline_variant),
        ),
        Entry::Heading(title) => Label::new(title)
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .modifier(Modifier::new().padding_xy(8.0, 4.0))
            .show(),
        Entry::Item {
            label,
            icon: glyph,
            hint,
            destructive,
            disabled,
            on_select,
        } => {
            let hovered = remember(|| false);
            let tone = if disabled {
                scheme.on_surface_variant
            } else if destructive {
                scheme.error
            } else {
                scheme.on_surface
            };
            let fill = if hovered.get() && !disabled {
                scheme.surface_container_high
            } else {
                Color::TRANSPARENT
            };
            let mut modifier = Modifier::new()
                .fill_max_width()
                .height(height)
                .background_rounded(fill, 4.0)
                .on_hover({
                    let hovered = hovered.clone();
                    move |inside| hovered.set(inside)
                })
                .padding_xy(8.0, 0.0);
            if !disabled {
                modifier = modifier.cursor(CursorIcon::Pointer).clickable(move || {
                    if let Some(dismiss) = &dismiss {
                        dismiss();
                    }
                    on_select();
                });
            }
            row_arranged(
                modifier,
                Arrangement::SpacedBy(crate::spacing(8.0)),
                Alignment::Center,
                || {
                    match glyph {
                        Some(glyph) => icon(glyph, 14.0, tone),
                        None => spacer(Modifier::new().width(13.0)),
                    }
                    Label::new(label)
                        .colour(tone)
                        .size(Size::Xs)
                        .modifier(Modifier::new().weight(1.0))
                        .show();
                    if let Some(hint) = hint {
                        Label::new(hint)
                            .variant(LabelVariant::Subtle)
                            .size(Size::Xs)
                            .show();
                    }
                },
            );
        }
    }
}
