//! The window's own frame, and the windows beside it: moving and resizing by
//! the frame, the buttons that minimize, maximize and close, and the
//! settings window and the folder picker's opening, taking input and closing.

use pm_components::git_prompts::{PROMPT_CONFIRM, Prompt};
use pm_components::kit::{Key, Modifiers};
use pm_components::state::SETTINGS;
use pm_components::title_bar::{
    FRAMELESS, TITLE_BAR, WINDOW_CLOSE, WINDOW_MAXIMIZE, WINDOW_MINIMIZE,
};
use winit::event::MouseButton;
use winit::window::ResizeDirection;

use crate::app::asides::Aside;
use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// Control held.
const CONTROL: Modifiers = Modifiers {
    shift: false,
    control: true,
    alt: false,
    logo: false,
};

/// A place on the title bar with nothing else on it.
fn bare_title_bar(rig: &Rig) -> (f32, f32) {
    let bar = rig
        .report()
        .region(TITLE_BAR)
        .expect("the title bar is drawn");
    (bar.origin.x + 200.0, bar.origin.y + bar.size.height / 2.0)
}

/// A press on the title bar moves the window; a second one at once maximizes
/// it, and another pair restores it.
#[test]
fn the_title_bar_moves_and_maximizes() {
    if !FRAMELESS {
        return;
    }
    let mut rig = Rig::shot(Shot::Workspace);
    let at = bare_title_bar(&rig);
    rig.click(at);
    assert_eq!(rig.window().moved.get(), 1);
    rig.click(at);
    assert!(rig.window().maximized.get());
    rig.wait(1000);
    rig.click(at);
    rig.click(at);
    assert!(!rig.window().maximized.get());
}

/// Two presses on the title bar far apart in time both move the window.
#[test]
fn slow_presses_on_the_title_bar_only_move() {
    if !FRAMELESS {
        return;
    }
    let mut rig = Rig::shot(Shot::Workspace);
    let at = bare_title_bar(&rig);
    rig.click(at);
    rig.wait(1000);
    rig.click(at);
    assert_eq!(rig.window().moved.get(), 2);
    assert!(!rig.window().maximized.get());
}

/// The edges and corners of the frame resize the window and show the
/// matching cursor; a maximized window has none.
#[test]
fn the_edges_resize() {
    if !FRAMELESS {
        return;
    }
    let mut rig = Rig::shot(Shot::Workspace);
    rig.move_to((1.0, 400.0));
    assert_eq!(rig.cursor(), rustle::CursorIcon::EwResize);
    rig.button_in(None, MouseButton::Left, true, (1.0, 400.0));
    rig.button_in(None, MouseButton::Left, false, (1.0, 400.0));
    rig.move_to((1279.0, 799.0));
    assert_eq!(rig.cursor(), rustle::CursorIcon::NwseResize);
    rig.button_in(None, MouseButton::Left, true, (1279.0, 799.0));
    rig.button_in(None, MouseButton::Left, false, (1279.0, 799.0));
    assert_eq!(
        *rig.window().resized.borrow(),
        vec![ResizeDirection::West, ResizeDirection::SouthEast]
    );
    rig.window().maximized.set(true);
    rig.move_to((1.0, 400.0));
    rig.button_in(None, MouseButton::Left, true, (1.0, 400.0));
    rig.button_in(None, MouseButton::Left, false, (1.0, 400.0));
    assert_eq!(rig.window().resized.borrow().len(), 2);
}

/// The buttons of the frame minimize, maximize and close the window.
#[test]
fn the_frame_buttons() {
    if !FRAMELESS {
        return;
    }
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(WINDOW_MAXIMIZE);
    assert!(rig.window().maximized.get());
    rig.click_on(WINDOW_MAXIMIZE);
    assert!(!rig.window().maximized.get());
    rig.click_on(WINDOW_MINIMIZE);
    assert!(rig.window().minimized.get());
    rig.click_on(WINDOW_CLOSE);
    assert_eq!(
        rig.state(|state| state.git.prompt.clone()),
        Some(Prompt::Quit)
    );
}

/// The desktop asking the window to close while an agent works asks first,
/// and confirming quits the app.
#[test]
fn closing_the_window_asks_then_quits() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.close(None);
    assert!(!rig.app.come_to_rest().1);
    assert_eq!(
        rig.state(|state| state.git.prompt.clone()),
        Some(Prompt::Quit)
    );
    rig.click_on(PROMPT_CONFIRM);
    assert!(rig.app.come_to_rest().1);
}

/// The settings button opens settings in their own window, which takes
/// presses and keys, and Escape there closes it.
#[test]
fn settings_open_in_their_window_and_escape_closes_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(SETTINGS);
    assert!(rig.aside_open(Aside::Settings));
    assert!(rig.aside_report(Aside::Settings).regions.len() > 1);
    rig.key_in(Some(Aside::Settings), Key::Escape);
    assert!(!rig.aside_open(Aside::Settings));
}

/// The shortcut for settings, pressed while they are open, brings their
/// window forward; the desktop closing the window closes settings.
#[test]
fn the_settings_window_closes_and_comes_forward() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(SETTINGS);
    rig.chord(CONTROL, Key::Character(','));
    assert_eq!(rig.aside_window(Aside::Settings).raised.get(), 1);
    rig.close(Some(Aside::Settings));
    assert!(!rig.aside_open(Aside::Settings));
    assert!(!rig.read(pm_pages::Screen::settings_open));
}

/// The desktop resizing the window lays the frame out again at its size.
#[test]
fn resizing_lays_out_again() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.resize((900.0, 600.0));
    let bar = rig
        .report()
        .region(TITLE_BAR)
        .expect("the title bar is drawn");
    assert!((bar.size.width - 900.0).abs() < 1.0, "{bar:?}");
}

/// The panel that adds a project opens from its button, takes the arrows, and Escape closes it.
#[test]
fn the_project_adder_opens_and_closes() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(pm_components::state::ADD_PROJECT);
    assert!(rig.read(pm_pages::Screen::adder_open));
    rig.key(Key::ArrowDown);
    rig.key(Key::Escape);
    assert!(!rig.read(pm_pages::Screen::adder_open));
}

/// Browsing for a folder opens the folder picker's window, brought forward
/// with its filter holding the keyboard; typing filters, and Escape there
/// closes it.
#[test]
fn the_folder_picker_window() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(pm_components::state::ADD_PROJECT);
    rig.click_on(pm_components::adder::ADDER_BROWSE);
    assert!(rig.aside_open(Aside::Picker));
    assert_eq!(rig.aside_window(Aside::Picker).attention.get(), 1);
    assert_eq!(
        rig.focused(),
        Some(pm_components::folder_picker::PICKER_FIELD)
    );
    rig.type_in(Some(Aside::Picker), "src");
    let filter = rig.read(|screen| {
        screen
            .field(pm_components::folder_picker::PICKER_FIELD)
            .map(|field| field.text().to_owned())
    });
    assert_eq!(filter.as_deref(), Some("src"));
    rig.key_in(Some(Aside::Picker), Key::Escape);
    assert!(!rig.aside_open(Aside::Picker));
}

/// A press on a section in the settings window's list shows that section,
/// and Tab there walks the window's own controls, from the first it drew.
#[test]
fn the_settings_list_picks_a_section() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(SETTINGS);
    let section = pm_components::settings::Section::Terminal;
    let at = rig.aside_center(Aside::Settings, section.id());
    rig.click_in(Some(Aside::Settings), at);
    assert_eq!(rig.state(|state| state.settings), Some(section));
    rig.key_in(Some(Aside::Settings), Key::Tab);
    let focused = rig.focused().expect("Tab gave something the keyboard");
    assert_eq!(
        rig.aside_report(Aside::Settings).focus_order().first(),
        Some(&focused)
    );
    assert!(!rig.report().drawn(focused) || focused == TITLE_BAR);
}

/// A tooltip of the main window shows in the main window only, not in the
/// settings window open beside it.
#[test]
fn tooltips_stay_in_their_window() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(SETTINGS);
    let on = rig.center(SETTINGS);
    rig.move_to(on);
    rig.wait(400);
    assert!(rig.shows("Open Settings"));
    assert!(
        rig.text_at(Some(Aside::Settings), "Open Settings")
            .is_none()
    );
}
