//! The menu: a floating list of actions opened at a point.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, absolute, bg, cursor_not_allowed, cursor_pointer, flex, flex_1, gap, h,
    items_center, leading_ratio, left_px, min_w_px, opacity, p, px, rounded, text_color, text_px,
    top_px, w, z,
};
use pm_ui::{Node, node, text};

use crate::Surface;

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
/// [`Menu::separator`], and turn it into a node with [`Menu::build`]. The
/// node is positioned over whatever it is placed in, so it belongs last in
/// the root of a screen.
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
}

impl Menu {
    /// An empty menu opening with its corner at `x`, `y` logical pixels.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            at: (x, y),
            entries: Vec::new(),
            width: 52.0,
            row: 8.0,
        }
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
        let step = 4.0 * pm_style::step_scale();
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

    /// The node that draws the menu.
    pub fn build(self) -> Node {
        Surface::new()
            .build()
            .styles([
                absolute(),
                left_px(self.at.0),
                top_px(self.at.1),
                z(50),
                w(self.width),
                min_w_px(0),
                p(1),
            ])
            .keep_in_view()
            .children(self.entries.into_iter().map(|entry| row(entry, self.row)))
    }
}

/// The node for one entry, `height` style units tall when it is a row.
fn row(entry: Entry, height: f32) -> Node {
    match entry {
        Entry::Separator => node().styles([
            h(0.25),
            bg(Role::Border),
            pm_style::mx(-1.0),
            pm_style::my(1),
        ]),
        Entry::Item {
            label,
            shortcut,
            id,
            disabled,
        } => {
            let mut styles = vec![
                flex(),
                items_center(),
                gap(4),
                h(height),
                px(2),
                rounded(Radius::Sm),
            ];
            if disabled {
                styles.extend([opacity(40), cursor_not_allowed()]);
            } else {
                styles.extend([
                    cursor_pointer(),
                    bg(Role::SurfaceHover).hover(),
                    bg(Role::SurfaceActive).active(),
                ]);
            }
            let mut item =
                node()
                    .styles(styles)
                    .child(node().styles([flex_1()]).child(text(label).styles([
                        text_px(13),
                        leading_ratio(1.4),
                        text_color(Role::Text),
                    ])));
            if let Some(shortcut) = shortcut {
                item = item.child(text(shortcut).styles([
                    text_px(12),
                    leading_ratio(1.4),
                    text_color(Role::Subtle),
                ]));
            }
            if disabled { item } else { item.focusable(id) }
        }
    }
}

impl From<Menu> for Node {
    /// The node that draws the menu.
    fn from(menu: Menu) -> Self {
        menu.build()
    }
}
