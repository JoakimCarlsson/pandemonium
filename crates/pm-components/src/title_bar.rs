//! The bar across the top of the window: back and forward, the command center naming where the window is, the button that opens a newer release while one is out, the settings, and the controls of a window the desktop draws no frame around.

use crate::kit::{Flex, FocusId, IconName, Interactive, Role, Side, Styled, Tooltip, gap_box, row};
use rustle::Modifier;

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
            .fill(Role::Sidebar)
            .stroke_side(Side::Bottom, 1.0, Role::Border)
            .target(TITLE_BAR);
        row(own, Flex::new().center(), move || {
            row(
                Modifier::new().grow().min_width_auto().pad_x(2),
                Flex::new().center().gap(0.5),
                || {
                    gap_box(Modifier::new().width(DESKTOP_CONTROLS));
                    if navigation {
                        Button::icon(IconName::ArrowLeft, GO_BACK)
                            .modifier(Modifier::new().tooltip("Go Back"))
                            .build();
                        Button::icon(IconName::ArrowRight, GO_FORWARD)
                            .modifier(Modifier::new().tooltip("Go Forward"))
                            .build();
                    }
                },
            );
            if let Some(place) = self.place {
                command_center(place);
            }
            row(
                Modifier::new().grow().min_width_auto().fill_max_height(),
                Flex::new().center().trailing().gap(0.5),
                || {
                    if self.language_update {
                        Button::new("Update languages", LANGUAGE_UPDATE)
                            .variant(Variant::Ghost)
                            .tone(Role::Accent)
                            .size(Size::Sm)
                            .modifier(
                                Modifier::new()
                                    .pad_side(Side::Right, 1)
                                    .tooltip("A newer language or language server is available"),
                            )
                            .build();
                    }
                    if self.update {
                        Button::new("Update", UPDATE)
                            .variant(Variant::Ghost)
                            .tone(Role::Accent)
                            .size(Size::Sm)
                            .modifier(
                                Modifier::new()
                                    .pad_side(Side::Right, 1)
                                    .tooltip("Update Available"),
                            )
                            .build();
                    }
                    if navigation {
                        layout_toggle(
                            IconName::PanelLeft,
                            SIDEBAR_TOGGLE,
                            "Toggle Primary Side Bar",
                            self.sidebar_open,
                        );
                        layout_toggle(
                            IconName::PanelBottom,
                            TERMINAL,
                            "Toggle Panel",
                            self.panel_open,
                        );
                        layout_toggle(
                            IconName::PanelRight,
                            REVIEW,
                            "Toggle Review Panel",
                            self.review_open,
                        );
                        Button::icon(IconName::Settings, SETTINGS)
                            .modifier(Modifier::new().tooltip("Open Settings"))
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

/// Emits the button that shows or hides a panel, filling its part of the icon while open.
#[track_caller]
fn layout_toggle(icon: IconName, id: FocusId, words: &str, open: bool) {
    let mark = match (icon, open) {
        (IconName::PanelLeft, true) => IconName::PanelLeftFilled,
        (IconName::PanelRight, true) => IconName::PanelRightFilled,
        (IconName::PanelBottom, true) => IconName::PanelBottomFilled,
        _ => icon,
    };
    Button::icon(icon, id)
        .mark(move || crate::parts::glyph(mark, 5.0, Role::Muted))
        .modifier(Modifier::new().tooltip(words))
        .build();
}

/// Emits the box in the middle of the bar that says where the window is pointed and lists every command when it is pressed.
#[track_caller]
fn command_center(place: String) {
    Button::new(place, COMMAND_CENTER)
        .variant(Variant::Field)
        .with_icon(IconName::Search)
        .tone(Role::Subtle)
        .width(COMMAND_CENTER_WIDTH)
        .resting(Role::Background)
        .flex(Flex::new().center().middle().gap(2))
        .modifier(
            Modifier::new()
                .width_in(COMMAND_CENTER_MIN_WIDTH, COMMAND_CENTER_WIDTH)
                .height(COMMAND_CENTER_HEIGHT)
                .tooltip("Show All Commands"),
        )
        .build();
}

/// Emits the buttons that minimize, maximize and close a window the desktop draws no frame around, or only the one that closes it when `close_only` is set.
#[track_caller]
fn window_controls(close_only: bool) {
    row(
        Modifier::new().pad_x(1.5).fill_max_height(),
        Flex::new().center().gap(0.5),
        || {
            if !close_only {
                control(IconName::Minus, WINDOW_MINIMIZE, "Minimize");
                control(IconName::Maximize, WINDOW_MAXIMIZE, "Maximize");
            }
            control(IconName::Close, WINDOW_CLOSE, "Close");
        },
    );
}

/// Emits one control of the window, showing `icon`, answering to `id` and named `words`.
#[track_caller]
fn control(icon: IconName, id: FocusId, words: &str) {
    Button::icon(icon, id)
        .modifier(Modifier::new().width(32.0).tooltip(words))
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

/// Whether `id` is one of the controls of the window.
pub fn is_window_control(id: FocusId) -> bool {
    [WINDOW_MINIMIZE, WINDOW_MAXIMIZE, WINDOW_CLOSE].contains(&id)
}
