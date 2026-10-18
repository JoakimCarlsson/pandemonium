//! The bar across the top of the window: back and forward, the command center naming where the window is, the settings, and the controls of a window the desktop draws no frame around.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_b, border_color, cursor_pointer, flex, flex_1, flex_none, gap,
    h_px, items_center, justify_center, justify_end, min_w_px, overflow_hidden, px, rounded,
    w_full, w_px,
};
use pm_ui::{Node, node};

use crate::Button;
use crate::listing::{LISTING_WIDTH, QUICK_INSET};
use crate::parts::{clipped, glyph};
use crate::quick_input::FIELD_HEIGHT;
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
            ..Self::default()
        }
    }

    /// The node that draws the bar.
    pub fn build(self) -> Node {
        let navigation = self.place.is_some();
        let leading = node()
            .styles([flex(), flex_1(), items_center(), gap(0.5), px(2)])
            .child(node().styles([flex_none(), w_px(DESKTOP_CONTROLS)]))
            .children(navigation.then(|| {
                Button::icon(IconName::ArrowLeft, GO_BACK)
                    .build()
                    .tooltip("Go Back")
            }))
            .children(navigation.then(|| {
                Button::icon(IconName::ArrowRight, GO_FORWARD)
                    .build()
                    .tooltip("Go Forward")
            }));
        let trailing = node()
            .styles([
                flex(),
                flex_1(),
                items_center(),
                justify_end(),
                gap(0.5),
                pm_style::h_full(),
            ])
            .children(navigation.then(|| {
                layout_toggle(
                    IconName::PanelLeft,
                    SIDEBAR_TOGGLE,
                    "Toggle Primary Side Bar",
                    self.sidebar_open,
                )
            }))
            .children(navigation.then(|| {
                layout_toggle(
                    IconName::PanelBottom,
                    TERMINAL,
                    "Toggle Panel",
                    self.panel_open,
                )
            }))
            .children(navigation.then(|| {
                layout_toggle(
                    IconName::PanelRight,
                    REVIEW,
                    "Toggle Review Panel",
                    self.review_open,
                )
            }))
            .children(navigation.then(|| {
                Button::icon(IconName::Settings, SETTINGS)
                    .build()
                    .tooltip("Open Settings")
            }))
            .children(FRAMELESS.then(|| window_controls(self.close_only)));
        node()
            .styles([
                flex(),
                flex_none(),
                items_center(),
                w_full(),
                h_px(TITLE_BAR_HEIGHT),
                bg(Role::Sidebar),
                border_b(),
                border_color(Role::Border),
            ])
            .focusable(TITLE_BAR)
            .child(leading)
            .children(self.place.map(command_center))
            .child(trailing)
    }
}

/// The button of the title bar that shows or hides the part of the window `icon` pictures, filled while it shows.
fn layout_toggle(icon: IconName, id: FocusId, words: &str, open: bool) -> Node {
    let toggle = Button::icon(icon, id).build().tooltip(words);
    if open {
        toggle.styles([bg(Role::SurfaceSelected)])
    } else {
        toggle
    }
}

/// The box in the middle of the bar that says where the window is pointed and lists every command when it is pressed.
fn command_center(place: String) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            justify_center(),
            gap(1.5),
            px(2),
            w_px(COMMAND_CENTER_WIDTH),
            min_w_px(COMMAND_CENTER_MIN_WIDTH),
            h_px(COMMAND_CENTER_HEIGHT),
            overflow_hidden(),
            rounded(Radius::Md),
            border(),
            border_color(Role::Border),
            bg(Role::Background),
            bg(Role::SurfaceHover).hover(),
            cursor_pointer(),
        ])
        .focusable(COMMAND_CENTER)
        .tooltip("Show All Commands")
        .child(glyph(IconName::Search, 3.5, Role::Subtle))
        .child(
            node()
                .styles([flex_none(), overflow_hidden()])
                .child(clipped(place, 12, Role::Subtle)),
        )
}

/// The buttons that minimize, maximize and close a window the desktop draws no frame around, or only the one that closes it when `close_only` is set.
fn window_controls(close_only: bool) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            gap(0.5),
            px(1.5),
            pm_style::h_full(),
        ])
        .children((!close_only).then(|| control(IconName::Minus, WINDOW_MINIMIZE, "Minimize")))
        .children((!close_only).then(|| control(IconName::Maximize, WINDOW_MAXIMIZE, "Maximize")))
        .child(control(IconName::Close, WINDOW_CLOSE, "Close"))
}

/// One control of the window, showing `icon`, answering to `id` and named `words`.
fn control(icon: IconName, id: FocusId, words: &str) -> Node {
    Button::icon(icon, id)
        .build()
        .styles([w_px(32.0)])
        .tooltip(words)
}

/// What the command center says the window is pointed at: the project shown, and the session in it when one is.
fn whereabouts(state: &WorkspaceState) -> String {
    let project = |index: usize| state.projects.get(index).map(|held| held.name.clone());
    let named = match state.page {
        Page::Session(index) => state.sessions.get(index).and_then(|session| {
            let project = project(session.project)?;
            Some(format!("{project} · {}", session.title))
        }),
        Page::New(index) | Page::Project(index) => project(index),
        Page::History => Some("History".to_owned()),
        Page::Extensions => Some("Extensions".to_owned()),
    };
    named.unwrap_or_else(|| "Search commands".to_owned())
}

/// Whether `id` is one of the controls of the window.
pub fn is_window_control(id: FocusId) -> bool {
    [WINDOW_MINIMIZE, WINDOW_MAXIMIZE, WINDOW_CLOSE].contains(&id)
}
