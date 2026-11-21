//! The menu: a floating list of actions opened at a point or hanging from the control that
//! opened it.

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use rustle::{Alignment, Arrangement, TextOverflow};
use rustle::{CursorIcon, Modifier, Rect};
use std::rc::Rc;

use crate::Surface;
use crate::events::Interactive;
use crate::popups::{Dismissal, at_window, dropdown};
use crate::state::WorkspaceState;
use crate::theme::{Radius, Role, sp};

/// How far a menu opened from a control hangs from it, in logical pixels.
const MENU_GAP: f32 = 4.0;

/// Where a menu opens.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    /// With its corner at a point of the window, in logical pixels, moved by an offset.
    At {
        /// The point, in the window.
        point: (f32, f32),
        /// How far the corner is moved from the point.
        offset: (f32, f32),
    },
    /// Hanging from the control at a rectangle of the window, as a dropdown menu does.
    Dropdown(Rect),
}

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
        id: Target,
        /// Whether the row is switched off.
        disabled: bool,
    },
    /// A hairline between two groups of rows.
    Separator,
}

/// A menu being described: where it opens and what it holds.
///
/// Build it with [`Menu::new`] or [`Menu::dropdown`], add rows with [`Menu::item`] and
/// [`Menu::separator`], and emit it with [`Menu::build`]. It floats over everything at its
/// point, or under the control that opened it, kept inside the window; Escape or a
/// press outside it closes it.
#[derive(Clone, Debug)]
pub struct Menu {
    /// Where it opens.
    place: Place,
    /// The rows, top to bottom.
    entries: Vec<Entry>,
    /// How wide it is, in style units.
    width: f32,
    /// How tall each row is, in style units.
    row: f32,
    /// What picking a row does, told the row's id, when the caller said; the workspace's
    /// reducer takes the press of the row otherwise.
    on_pick: Option<Pick>,
}

/// What picking a row of a menu does, told the row's id, as the menu's builder keeps it.
#[derive(Clone)]
struct Pick(Rc<dyn Fn(Target)>);

impl std::fmt::Debug for Pick {
    /// Writes that there is a lambda, which has nothing else to show.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Pick")
    }
}

impl Menu {
    /// An empty menu opening with its corner at `x`, `y` logical pixels in the window.
    pub fn new(x: f32, y: f32) -> Self {
        Self::placed(Place::At {
            point: (x, y),
            offset: (0.0, 0.0),
        })
    }

    /// An empty menu hanging from the control at `control`, a rectangle of the window: below
    /// it with their left edges together, their right edges together when it would leave the
    /// window on the right, and above the control when there is no room below.
    pub fn dropdown(control: Rect) -> Self {
        Self::placed(Place::Dropdown(control))
    }

    /// The menu with its corner moved `dx`, `dy` logical pixels from the point it opens at.
    pub fn offset(mut self, dx: f32, dy: f32) -> Self {
        if let Place::At { offset, .. } = &mut self.place {
            *offset = (dx, dy);
        }
        self
    }

    /// An empty menu opening at `place`.
    fn placed(place: Place) -> Self {
        Self {
            place,
            entries: Vec::new(),
            width: 52.0,
            row: 8.0,
            on_pick: None,
        }
    }

    /// The menu running `on_pick` with the id of the row picked, in place of handing the
    /// press of the row to the workspace's reducer.
    pub fn on_pick(mut self, on_pick: impl Fn(Target) + 'static) -> Self {
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
    pub fn item(mut self, label: &str, shortcut: Option<&str>, id: Target, disabled: bool) -> Self {
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

    /// The menu with each of `rows` added in turn: what the row says, the keys that do the same, the name it answers to, whether it is switched on, and whether a rule follows it.
    pub fn rows<'a>(
        self,
        rows: impl IntoIterator<Item = (&'a str, Option<String>, Target, bool, bool)>,
    ) -> Self {
        rows.into_iter()
            .fold(self, |menu, (label, keys, id, on, rule)| {
                let menu = menu.item(label, keys.as_deref(), id, !on);
                if rule { menu.separator() } else { menu }
            })
    }

    /// Emits the menu over everything where it opens, kept inside the window, as a focusable
    /// popup that Escape or a press outside it closes.
    #[track_caller]
    pub fn build(self) {
        let (width, height) = (self.width, self.row);
        let entries = self.entries;
        let on_pick = self.on_pick;
        let content = move || {
            Surface::new()
                .modifier(Modifier::new().width(theme::sp(width)))
                .inner(Modifier::new().padding(theme::sp(1)))
                .build_with(move || {
                    for entry in entries {
                        menu_row(entry, height, on_pick.clone());
                    }
                });
        };
        let dismissal = Dismissal::focusable(WorkspaceState::without_menu);
        match self.place {
            Place::At { point, offset } => at_window(point, offset, dismissal, content),
            Place::Dropdown(control) => dropdown(control, MENU_GAP, dismissal, content),
        }
    }
}

/// What clicking the row `id` does: `on_pick` told the row, or the workspace's reducer
/// taking the press without one.
fn picked(id: Target, on_pick: Option<Pick>) -> impl Fn() + 'static {
    let reduced = send(Event::pressing(id));
    move || match &on_pick {
        Some(Pick(on_pick)) => on_pick(id),
        None => reduced(),
    }
}

/// Emits one entry, `height` style units tall when it is a row: rustle's horizontal divider
/// across the menu inside its padding with a step of room above and below it, or a row with
/// the label and its shortcut, whose click runs `on_pick`, or hands the press to the
/// workspace's reducer without one.
fn menu_row(entry: Entry, height: f32, on_pick: Option<Pick>) {
    match entry {
        Entry::Separator => {
            rustle::horizontal_divider(
                Modifier::new().padding_xy(0.0, sp(1)),
                sp(0.25),
                crate::theme::role(Role::Border),
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
                    .indicated(
                        theme::InteractionTones::new()
                            .hover(Role::SurfaceHover)
                            .active(Role::SurfaceActive)
                            .radius(Radius::Sm),
                        theme::Focused::Rustle,
                    )
                    .cursor(CursorIcon::Pointer)
                    .control(id, picked(id, on_pick))
            };
            rustle::row_arranged(
                modifier.padding_xy(theme::sp(2), 0.0),
                Arrangement::SpacedBy(theme::sp(4)),
                Alignment::Center,
                move || {
                    rustle::basic_text(
                        Modifier::new().weight(1.0),
                        words,
                        theme::text_style(13, 1.4).color(theme::color(Role::Text)),
                        TextOverflow::Ellipsis,
                    );
                    if let Some(shortcut) = shortcut {
                        rustle::basic_text(
                            Modifier::new(),
                            shortcut,
                            theme::text_style(12, 1.4).color(theme::color(Role::Subtle)),
                            TextOverflow::Ellipsis,
                        );
                    }
                },
            );
        }
    }
}
