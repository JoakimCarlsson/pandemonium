//! The menu: a floating list of actions opened at a point.

use crate::events::reduce;
use crate::theme;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{CursorIcon, Modifier};
use std::rc::Rc;

use crate::Surface;
use crate::events::Interactive;
use crate::focus::FocusId;
use crate::select::styled_text;
use crate::theme::{Radius, Role, sp};

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
    /// What picking a row does, told the row's id, when the caller said; the workspace's
    /// reducer takes the press of the row otherwise.
    on_pick: Option<Pick>,
}

/// What picking a row of a menu does, told the row's id, as the menu's builder keeps it.
#[derive(Clone)]
struct Pick(Rc<dyn Fn(FocusId)>);

impl std::fmt::Debug for Pick {
    /// Writes that there is a lambda, which has nothing else to show.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Pick")
    }
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
            on_pick: None,
        }
    }

    /// The menu with `modifier` put around its box, outside its own.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The menu running `on_pick` with the id of the row picked, in place of handing the
    /// press of the row to the workspace's reducer.
    pub fn on_pick(mut self, on_pick: impl Fn(FocusId) + 'static) -> Self {
        self.on_pick = Some(Pick(Rc::new(on_pick)));
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
        let step = 4.0 * crate::theme::step_scale();
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

    /// Emits the menu over everything, with its corner at its point, kept inside the window; a
    /// press outside it asks the app to close what a press elsewhere closes.
    #[track_caller]
    pub fn build(self) {
        let (x, y) = self.at;
        let (width, height) = (self.width, self.row);
        let entries = self.entries;
        let modifier = self.modifier;
        let on_pick = self.on_pick;
        crate::popups::at_window(x, y, true, move || {
            Surface::new()
                .modifier(modifier.chain(Modifier::new().width(theme::sp(width))))
                .inner(Modifier::new().padding(theme::sp(1)))
                .build_with(move || {
                    for entry in entries {
                        menu_row(entry, height, on_pick.clone());
                    }
                });
        });
    }
}

/// What clicking the row `id` does: `on_pick` told the row, or the workspace's reducer
/// taking the press without one.
fn picked(id: FocusId, on_pick: Option<Pick>) -> impl Fn() + 'static {
    let reduced = reduce(id);
    move || match &on_pick {
        Some(Pick(on_pick)) => on_pick(id),
        None => reduced(),
    }
}

/// Emits one entry, `height` style units tall when it is a row: a hairline reaching the menu's
/// edges with a step of room above and below it, or a row with the label and its shortcut,
/// whose click runs `on_pick`, or hands the press to the workspace's reducer without one.
#[track_caller]
fn menu_row(entry: Entry, height: f32, on_pick: Option<Pick>) {
    match entry {
        Entry::Separator => {
            let color = crate::theme::role(Role::Border);
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
            let modifier = Modifier::new().fill_max_width().height(theme::sp(height));
            let modifier = if disabled {
                modifier.alpha(0.4).cursor(CursorIcon::NotAllowed)
            } else {
                modifier
                    .interaction_background(
                        theme::InteractionTones::new()
                            .hover(Role::SurfaceHover)
                            .active(Role::SurfaceActive)
                            .radius(Radius::Sm)
                            .colors(),
                    )
                    .cursor(CursorIcon::Pointer)
                    .control(id, picked(id, on_pick))
            };
            row_arranged(
                modifier.padding_xy(theme::sp(2), 0.0),
                Arrangement::SpacedBy(sp(4)),
                Alignment::Center,
                move || {
                    styled_text(
                        Modifier::new().weight(1.0),
                        words,
                        theme::text_style(13, 1.4).color(theme::color(Role::Text)),
                    );
                    if let Some(shortcut) = shortcut {
                        styled_text(
                            Modifier::new(),
                            shortcut,
                            theme::text_style(12, 1.4).color(theme::color(Role::Subtle)),
                        );
                    }
                },
            );
        }
    }
}
