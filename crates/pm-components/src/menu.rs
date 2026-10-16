//! The menu: a floating list of actions opened at a point.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, absolute, bg, border, border_color, cursor_not_allowed, cursor_pointer, flex,
    flex_1, flex_col, gap, h, items_center, leading_ratio, left, min_w_px, opacity, p, px, rounded,
    shadow_md, text_color, text_px, top, w, z,
};
use pm_ui::{Node, node, text};

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

    /// The node that draws the menu.
    pub fn build(self) -> Node {
        node()
            .styles([
                absolute(),
                left(self.at.0 / 4.0),
                top(self.at.1 / 4.0),
                z(50),
                flex(),
                flex_col(),
                w(self.width),
                min_w_px(0),
                p(1),
                rounded(Radius::Md),
                bg(Role::Surface),
                border(),
                border_color(Role::Border),
                shadow_md(),
            ])
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
