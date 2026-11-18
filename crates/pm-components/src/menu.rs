//! The menu: a floating list of actions opened at a point.

use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::Surface;
use crate::kit::{
    FocusId, Hovered, Interactive, Looks, Radius, Role, Styled, Type, at_window, label, label_with,
    sp,
};

/// How far a menu opened from a control hangs from it, in logical pixels.
const MENU_GAP: f32 = 4.0;

/// One row of a menu, or the rule between groups of rows.
#[derive(Clone, Debug)]
pub enum Entry {
    /// An action the reader can pick.
    Item {
        /// What the row says.
        label: String,
        /// The keys that do the same, written out, if any.
        shortcut: Option<String>,
        /// The name the row answers to.
        id: FocusId,
        /// Whether the row is switched off.
        disabled: bool,
    },
    /// A hairline between two groups of rows.
    Separator,
}

/// A menu being described: where it opens and what it holds.
///
/// Build it with [`Menu::new`], add rows with [`Menu::item`] and
/// [`Menu::separator`], and emit it with [`Menu::build`]. It floats over
/// everything at its point, kept inside the window.
#[derive(Clone, Debug)]
pub struct Menu {
    /// Where its top-left corner is, in logical pixels.
    at: (f32, f32),
    /// The rows, top to bottom.
    entries: Vec<Entry>,
    /// How wide it is, in style units.
    width: f32,
    /// How tall each row is, in style units.
    row: f32,
    /// What the caller puts around its box.
    modifier: Modifier,
}

impl Menu {
    /// An empty menu opening with its corner at `x`, `y` logical pixels.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            at: (x, y),
            entries: Vec::new(),
            width: 52.0,
            row: 8.0,
            modifier: Modifier::new(),
        }
    }

    /// The menu with `modifier` put around its box, outside its own.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The menu with rows `units` style units tall, for a long menu that has to fit the window.
    pub fn row_height(mut self, units: f32) -> Self {
        self.row = units;
        self
    }

    /// The menu `units` style units wide, for rows whose labels and keys need more room.
    pub fn width(mut self, units: f32) -> Self {
        self.width = units;
        self
    }

    /// The menu with a row saying `label`, answering to `id`, written with
    /// `shortcut` beside it, and switched off while `disabled` is true.
    pub fn item(
        mut self,
        label: &str,
        shortcut: Option<&str>,
        id: FocusId,
        disabled: bool,
    ) -> Self {
        self.entries.push(Entry::Item {
            label: label.to_owned(),
            shortcut: shortcut.map(str::to_owned),
            id,
            disabled,
        });
        self
    }

    /// The menu with a rule after the rows so far.
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }

    /// The menu opened from the control occupying `trigger`, as `(x, y, width, height)` in logical pixels, inside a window `window` wide and tall:
    /// below the control with their left edges together, or their right edges together when it would leave the window on the right,
    /// and above the control when there is more room above it than below.
    pub fn below(mut self, trigger: (f32, f32, f32, f32), window: (f32, f32)) -> Self {
        let (x, y, width, height) = trigger;
        let (menu_width, menu_height) = self.extent();
        let left = if x + menu_width <= window.0 {
            x
        } else {
            x + width - menu_width
        };
        let room_below = window.1 - (y + height + MENU_GAP);
        let top = if menu_height <= room_below || room_below >= y - MENU_GAP {
            y + height + MENU_GAP
        } else {
            y - MENU_GAP - menu_height
        };
        self.at = (
            left.min(window.0 - menu_width).max(0.0),
            top.min(window.1 - menu_height).max(0.0),
        );
        self
    }

    /// Where its top-left corner is, in logical pixels.
    pub fn at(&self) -> (f32, f32) {
        self.at
    }

    /// How wide and how tall the menu is, in logical pixels, at the spacing scale being built for.
    fn extent(&self) -> (f32, f32) {
        let step = 4.0 * crate::kit::theme::step_scale();
        let rows: f32 = self
            .entries
            .iter()
            .map(|entry| match entry {
                Entry::Item { .. } => self.row * step,
                Entry::Separator => step * 2.25,
            })
            .sum();
        (self.width * step, rows + step * 0.5)
    }

    /// Emits the menu over everything, with its corner at its point, kept inside the window.
    #[track_caller]
    pub fn build(self) {
        let (x, y) = self.at;
        let (width, height) = (self.width, self.row);
        let entries = self.entries;
        let modifier = self.modifier;
        at_window(x, y, Modifier::new(), move || {
            Surface::new()
                .modifier(modifier.chain(Modifier::new().w(width)))
                .inner(Modifier::new().pad(1))
                .build_with(move || {
                    for entry in entries {
                        menu_row(entry, height);
                    }
                });
        });
    }
}

/// Emits one entry, `height` style units tall when it is a row: a hairline reaching the menu's
/// edges with a step of room above and below it, or a row with the label and its shortcut.
#[track_caller]
fn menu_row(entry: Entry, height: f32) {
    match entry {
        Entry::Separator => {
            let color = crate::kit::role(Role::Border);
            rustle::spacer(
                Modifier::new()
                    .fill_max_width()
                    .height(sp(0.25) + 2.0 * sp(1))
                    .draw_behind(move |canvas| {
                        let width = canvas.width();
                        let height = canvas.height();
                        canvas.rect(
                            -sp(1),
                            (height - sp(0.25)) / 2.0,
                            width + 2.0 * sp(1),
                            sp(0.25),
                            color,
                        );
                    }),
            );
        }
        Entry::Item {
            label: words,
            shortcut,
            id,
            disabled,
        } => {
            let modifier = Modifier::new().fill_max_width().h(height);
            let modifier = if disabled {
                modifier.opacity(40.0).cursor(CursorIcon::NotAllowed)
            } else {
                modifier
                    .looks(
                        Looks::new()
                            .hover(Role::SurfaceHover)
                            .active(Role::SurfaceActive)
                            .radius(Radius::Sm),
                    )
                    .cursor(CursorIcon::Pointer)
                    .target(id)
            };
            row_arranged(
                modifier.pad_x(2),
                Arrangement::SpacedBy(sp(4)),
                Alignment::Center,
                move || {
                    label_with(
                        Modifier::new().weight(1.0),
                        words,
                        Type::new(13).ratio(1.4).tone(Role::Text),
                    );
                    if let Some(shortcut) = shortcut {
                        label(shortcut, Type::new(12).ratio(1.4).tone(Role::Subtle));
                    }
                },
            );
        }
    }
}
