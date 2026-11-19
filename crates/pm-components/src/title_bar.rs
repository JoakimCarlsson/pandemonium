//! The bar across the top of the window: back and forward, the command center naming where the window is, the button that opens a newer release while one is out, the settings, and the controls of a window the desktop draws no frame around.

use crate::events::Interactive;
use crate::events::reduce;
use crate::focus::FocusId;
use crate::icons::{panel_bottom, panel_right};
use crate::theme;
use crate::theme::Role;
use crate::theme::sp;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{Icon, phosphor};
use std::rc::Rc;

use crate::Button;
use crate::button::Variant;
use crate::listing::{LISTING_WIDTH, QUICK_INSET};
use crate::quick_input::FIELD_HEIGHT;
use crate::size::Size;
use crate::state::{Page, REVIEW, SETTINGS, SIDEBAR_TOGGLE, TERMINAL, WorkspaceState};

/// How tall the bar is, in logical pixels.
pub const TITLE_BAR_HEIGHT: f32 = 40.0;

/// The bar itself, which moves the window when it is dragged and maximizes it when it is pressed twice.
pub const TITLE_BAR: FocusId = FocusId(7_405_000);

/// Goes back to the place shown before.
pub const GO_BACK: FocusId = FocusId(7_405_001);

/// Goes forward to the place gone back from.
pub const GO_FORWARD: FocusId = FocusId(7_405_002);

/// The command center, which lists every command.
pub const COMMAND_CENTER: FocusId = FocusId(7_405_003);

/// Minimizes the window.
pub const WINDOW_MINIMIZE: FocusId = FocusId(7_405_004);

/// Maximizes the window, or restores it.
pub const WINDOW_MAXIMIZE: FocusId = FocusId(7_405_005);

/// Closes the window.
pub const WINDOW_CLOSE: FocusId = FocusId(7_405_006);

/// Opens the page of the newer release that is out.
pub const UPDATE: FocusId = FocusId(7_405_007);

/// Opens the extensions page, where a newer language or language server is installed.
pub const LANGUAGE_UPDATE: FocusId = FocusId(7_405_008);

/// How wide the command center is while the bar has room, in logical pixels: as wide as the field of the quick input that opens over it.
const COMMAND_CENTER_WIDTH: f32 = LISTING_WIDTH - 2.0 * QUICK_INSET;

/// How narrow the command center gets before it is cut short, in logical pixels.
const COMMAND_CENTER_MIN_WIDTH: f32 = 160.0;

/// How tall the command center is, in logical pixels: as tall as the field of the quick input.
const COMMAND_CENTER_HEIGHT: f32 = FIELD_HEIGHT;

/// Whether the window draws its own controls, the desktop drawing no frame around it.
pub const FRAMELESS: bool = cfg!(not(target_os = "macos"));

/// The room left at the start of the bar for the controls the desktop draws over it, in logical pixels.
const DESKTOP_CONTROLS: f32 = if FRAMELESS { 0.0 } else { 72.0 };

/// A title bar being described: the place the command center names, when the window shows one.
#[derive(Clone, Debug, Default)]
pub struct TitleBar {
    /// What the command center says the window is pointed at; nothing leaves out everything but the controls of the window.
    place: Option<String>,
    /// Whether closing is the only control of the window the bar holds.
    close_only: bool,
    /// Whether the bottom pane is showing, which fills the button that shows and hides it.
    panel_open: bool,
    /// Whether the sidebar is showing, which fills the button that shows and hides it.
    sidebar_open: bool,
    /// Whether the review panel is showing, which fills the button that shows and hides it.
    review_open: bool,
    /// Whether a newer release is out, which the bar holds a button for.
    update: bool,
    /// Whether an installed language or language server has a newer version.
    language_update: bool,
}

impl TitleBar {
    /// A bar holding only the controls of the window.
    pub fn new() -> Self {
        Self::default()
    }

    /// A bar holding only the control that closes the window.
    pub fn closing() -> Self {
        Self {
            close_only: true,
            ..Self::default()
        }
    }

    /// The bar for the workspace `state`, naming the project and the session it shows.
    pub fn of(state: &WorkspaceState) -> Self {
        Self {
            place: Some(whereabouts(state)),
            panel_open: state.terminal,
            sidebar_open: state.sidebar_open,
            review_open: state.review && state.review_session().is_some(),
            update: state.update_available(),
            language_update: state.language_update_available(),
            ..Self::default()
        }
    }

    /// Emits the bar.
    #[track_caller]
    pub fn build(self) {
        let navigation = self.place.is_some();
        let own = Modifier::new()
            .fill_max_width()
            .height(TITLE_BAR_HEIGHT)
            .background(theme::color(Role::Sidebar))
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .control(TITLE_BAR, reduce(TITLE_BAR));
        rustle::layout(own, BarLayout, move || {
            row_arranged(
                Modifier::new()
                    .fill_max_height()
                    .padding_xy(theme::sp(2), 0.0),
                Arrangement::SpacedBy(sp(0.5)),
                Alignment::Center,
                || {
                    rustle::spacer(Modifier::new().width(DESKTOP_CONTROLS));
                    if navigation {
                        let back = crate::events::operations();
                        Button::icon(phosphor::ARROW_LEFT, GO_BACK)
                            .on_click(move || back.navigate(-1))
                            .tooltip("Go Back")
                            .build();
                        let forward = crate::events::operations();
                        Button::icon(phosphor::ARROW_RIGHT, GO_FORWARD)
                            .on_click(move || forward.navigate(1))
                            .tooltip("Go Forward")
                            .build();
                    }
                },
            );
            if let Some(place) = self.place {
                command_center(place);
            }
            row_arranged(
                Modifier::new().fill_max_height(),
                Arrangement::SpacedByAligned(sp(0.5), Alignment::End),
                Alignment::Center,
                || {
                    if self.language_update {
                        Button::new("Update languages", LANGUAGE_UPDATE)
                            .variant(Variant::Ghost)
                            .tone(Role::Accent)
                            .size(Size::Sm)
                            .modifier(Modifier::new().padding_each(0.0, 0.0, theme::sp(1), 0.0))
                            .tooltip("A newer language or language server is available")
                            .build();
                    }
                    if self.update {
                        Button::new("Update", UPDATE)
                            .variant(Variant::Ghost)
                            .tone(Role::Accent)
                            .size(Size::Sm)
                            .modifier(Modifier::new().padding_each(0.0, 0.0, theme::sp(1), 0.0))
                            .tooltip("Update Available")
                            .build();
                    }
                    if navigation {
                        layout_toggle(
                            phosphor::SIDEBAR_SIMPLE,
                            sidebar_mark,
                            SIDEBAR_TOGGLE,
                            "Toggle Primary Side Bar",
                            self.sidebar_open,
                        );
                        layout_toggle(
                            phosphor::BROWSER,
                            panel_mark,
                            TERMINAL,
                            "Toggle Panel",
                            self.panel_open,
                        );
                        layout_toggle(
                            phosphor::SIDEBAR_SIMPLE,
                            review_mark,
                            REVIEW,
                            "Toggle Review Panel",
                            self.review_open,
                        );
                        Button::icon(phosphor::GEAR, SETTINGS)
                            .tooltip("Open Settings")
                            .build();
                    }
                    if FRAMELESS {
                        window_controls(self.close_only);
                    }
                },
            );
        });
    }
}

/// How the bar lays out its children: the group at the start, the command center when there is
/// one, and the group at the end.
///
/// The command center gets its own width; the two groups share what is left, the one at the
/// start also keeping its side padding, so the command center sits in the middle. A group whose
/// content is wider than its share keeps its content's width and the other group gives up the
/// difference; when both groups need more than is left, the bar runs past its end.
struct BarLayout;

impl BarLayout {
    /// The widths of the start group, the command center and the end group in a bar `room`
    /// logical pixels across, for groups whose content is `start` and `end` wide and a command
    /// center when `centered`.
    fn widths(room: f32, start: f32, end: f32, centered: bool) -> (f32, f32, f32) {
        let padding = sp(4);
        let center = if centered { COMMAND_CENTER_WIDTH } else { 0.0 };
        let free = room - center - padding;
        let mut left = padding + free / 2.0;
        let mut right = free / 2.0;
        if right < end {
            right = end;
            left = padding + free - end;
        }
        if left < start {
            left = start;
            right = (padding + free - start).max(end);
        }
        (left.max(0.0), center, right.max(0.0))
    }
}

impl rustle::Layout for BarLayout {
    /// Measures the groups and the command center by [`BarLayout::widths`], each as tall as the
    /// bar or less, and centres each down the bar.
    fn measure(
        &self,
        children: &mut rustle::Children,
        constraints: rustle::Constraints,
    ) -> rustle::LayoutResult {
        let count = children.len();
        let height = constraints.max_height;
        let last = count.saturating_sub(1);
        let start = children.max_intrinsic_width(0, height);
        let end = children.max_intrinsic_width(last, height);
        let (left, center, right) = Self::widths(constraints.max_width, start, end, count == 3);
        let widths: Vec<f32> = match count {
            3 => vec![left, center, right],
            2 => vec![left, right],
            _ => vec![constraints.max_width; count],
        };
        let mut positions = Vec::with_capacity(count);
        let mut along = 0.0;
        for (index, width) in widths.iter().enumerate() {
            let size = children.measure(
                index,
                rustle::Constraints {
                    min_width: *width,
                    max_width: *width,
                    min_height: 0.0,
                    max_height: height,
                },
            );
            positions.push(rustle::Offset::new(along, (height - size.height) / 2.0));
            along += width;
        }
        rustle::LayoutResult {
            size: rustle::Size::new(constraints.max_width, height),
            positions,
        }
    }
}

/// Emits the button that shows or hides a panel, standing for `icon`, its
/// look drawn by `mark`, which is told whether the panel is `open`.
#[track_caller]
fn layout_toggle(icon: Icon, mark: fn(bool), id: FocusId, words: &str, open: bool) {
    Button::icon(icon, id)
        .mark(move || mark(open))
        .tooltip(words)
        .build();
}

/// Emits the primary side bar's toggle: Phosphor's sidebar, filled while `open`.
#[track_caller]
fn sidebar_mark(open: bool) {
    let sidebar = if open {
        phosphor::fill::SIDEBAR_SIMPLE
    } else {
        phosphor::SIDEBAR_SIMPLE
    };
    crate::parts::glyph(sidebar, 5.0, Role::Muted);
}

/// Emits the bottom panel's toggle: a window with a band along its bottom,
/// the bottom half filled while `open`.
#[track_caller]
fn panel_mark(open: bool) {
    if open {
        crate::parts::glyph(phosphor::fill::SQUARE_HALF_BOTTOM, 5.0, Role::Muted);
    } else {
        panel_bottom(5.0, Role::Muted);
    }
}

/// Emits the review panel's toggle: a window with a panel on its right, filled while `open`.
#[track_caller]
fn review_mark(open: bool) {
    panel_right(open, 5.0, Role::Muted);
}

/// Emits the box in the middle of the bar that says where the window is pointed and lists every command when it is pressed.
#[track_caller]
fn command_center(place: String) {
    let operations = crate::events::operations();
    Button::new(place, COMMAND_CENTER)
        .on_click(move || operations.open_command_palette())
        .variant(Variant::Field)
        .with_icon(phosphor::MAGNIFYING_GLASS)
        .tone(Role::Subtle)
        .width(COMMAND_CENTER_WIDTH)
        .resting(Role::Background)
        .arranged(
            Arrangement::SpacedByAligned(sp(2), Alignment::Center),
            Alignment::Center,
        )
        .modifier(
            Modifier::new()
                .width_in(COMMAND_CENTER_MIN_WIDTH, COMMAND_CENTER_WIDTH)
                .height(COMMAND_CENTER_HEIGHT)
                .chain(crate::bounds::placed(COMMAND_CENTER, Modifier::new())),
        )
        .tooltip("Show All Commands")
        .build();
}

/// Emits the buttons that minimize, maximize and close a window the desktop draws no frame around, or only the one that closes it when `close_only` is set.
#[track_caller]
fn window_controls(close_only: bool) {
    row_arranged(
        Modifier::new()
            .padding_xy(theme::sp(1.5), 0.0)
            .fill_max_height(),
        Arrangement::SpacedBy(sp(0.5)),
        Alignment::Center,
        || {
            let operations = crate::events::operations();
            if !close_only {
                let minimize = Rc::clone(&operations);
                control(phosphor::MINUS, WINDOW_MINIMIZE, "Minimize", move || {
                    minimize.minimize_window()
                });
                let maximize = Rc::clone(&operations);
                control(
                    phosphor::ARROWS_OUT_SIMPLE,
                    WINDOW_MAXIMIZE,
                    "Maximize",
                    move || maximize.toggle_maximized(),
                );
            }
            control(phosphor::X, WINDOW_CLOSE, "Close", move || {
                operations.close_window()
            });
        },
    );
}

/// Emits one control of the window, showing `icon`, answering to `id`, named `words` and
/// running `on_click` when clicked.
#[track_caller]
fn control(icon: Icon, id: FocusId, words: &str, on_click: impl Fn() + 'static) {
    Button::icon(icon, id)
        .on_click(on_click)
        .modifier(Modifier::new().width(32.0))
        .tooltip(words)
        .build();
}

/// What the command center says the window is pointed at: the project shown, and the session in it or the submodule new work starts in when there is one.
fn whereabouts(state: &WorkspaceState) -> String {
    let project = |index: usize| state.projects.get(index).map(|held| held.name.clone());
    let named = match state.page {
        Page::Session(index) => {
            state
                .sessions
                .get(index)
                .and_then(|session| match session.project {
                    Some(held) => Some(format!("{} · {}", project(held)?, session.title)),
                    None => Some(format!("Chat · {}", session.title)),
                })
        }
        Page::Chat => Some("New chat".to_owned()),
        Page::New(index) => project(index).map(|name| match &state.new_in {
            Some(submodule) => format!("{name} · {submodule}"),
            None => name,
        }),
        Page::Project(index) => project(index),
        Page::History => Some("History".to_owned()),
        Page::Extensions => Some("Extensions".to_owned()),
    };
    named.unwrap_or_else(|| "Search commands".to_owned())
}
