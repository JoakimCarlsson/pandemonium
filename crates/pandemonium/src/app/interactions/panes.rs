//! The file shown and the browser pane: the code taking the keyboard and
//! the view's size told to the screen, and the browser's address field and menu.

use pm_components::browser::{BROWSER_ADDRESS, BROWSER_BODY};
use pm_components::open_files::EDITOR_FIELD;
use pm_pages::Screen;
use rustle::Key;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// The file shown takes the keyboard when pressed, and keys typed change it.
#[test]
fn the_code_takes_typing() {
    let mut rig = Rig::shot(Shot::Code);
    let text = |rig: &Rig| {
        rig.read(|screen| {
            screen
                .field(EDITOR_FIELD)
                .map(|field| field.text().to_owned())
        })
    };
    let before = text(&rig).expect("a file is shown");
    rig.click_on(EDITOR_FIELD);
    assert_eq!(rig.focused(), Some(EDITOR_FIELD));
    rig.type_text("zz");
    let after = text(&rig).expect("a file is shown");
    assert_eq!(after.len(), before.len() + 2);
    assert!(after.contains("zz"));
}

/// The screen is told how tall the view of the file shown was drawn, which
/// paging and wrapping go by.
#[test]
fn the_code_view_size_reaches_the_screen() {
    let rig = Rig::shot(Shot::Code);
    let drawn = rig.area(Screen::code_area).map(|view| view.size.height);
    assert!(drawn.is_some());
    assert_eq!(rig.read(Screen::code_height), drawn);
}

/// The browser's address takes the keyboard and typing; the secondary
/// button over the page opens the browser's menu. The browser is the open
/// session's, as it is once the reader opened it there.
#[test]
fn the_browser_address_and_menu() {
    let mut screen = Shot::BrowserTabs(false).screen();
    let Screen::Workspace(state) = &mut screen;
    state.browser_adopted_by(0);
    let mut rig = Rig::new(screen, crate::app::headless::SIZE);
    let address = |rig: &Rig| {
        rig.read(|screen| {
            screen
                .text_field(BROWSER_ADDRESS)
                .map(rustle::TextFieldState::text)
        })
        .unwrap_or_default()
    };
    let before = address(&rig);
    rig.click_on(BROWSER_ADDRESS);
    assert_eq!(rig.focused(), Some(BROWSER_ADDRESS));
    rig.type_text("x");
    assert_eq!(address(&rig).len(), before.len() + 1);
    rig.key(Key::Escape);
    let at = rig.center(BROWSER_BODY);
    rig.right_click(at);
    assert!(rig.state(|state| state.browser_menu.is_some()));
}

/// The pointer resting on a word of the file shown starts the wait before
/// the language server is asked about it, and leaving the code ends it. The
/// asking itself waits for a language server that has the file, which the
/// app headless has none of.
#[test]
fn resting_on_code_waits_to_ask_about_it() {
    let mut rig = Rig::shot(Shot::Code);
    let text = rig
        .read(|screen| {
            screen
                .field(EDITOR_FIELD)
                .map(|field| field.text().to_owned())
        })
        .expect("a file is shown");
    let word = text.find("HashMap").expect("the word is in the file") + 2;
    let caret = rig
        .read(pm_pages::Screen::code_lines)
        .caret_rect(EDITOR_FIELD, word)
        .expect("the word is drawn");
    let at = (caret.origin.x, caret.origin.y + caret.size.height / 2.0);
    rig.move_to(at);
    assert!(rig.app.borrow_mut().hover_rest.is_some());
    rig.move_to((at.0 + 1.0, at.1));
    assert!(rig.app.borrow_mut().hover_rest.is_some());
    rig.leave();
    rig.move_to((5.0, 400.0));
    assert!(rig.app.borrow_mut().hover_rest.is_none());
}

/// Where byte `offset` of the file shown is drawn, halfway down its line.
fn code_point(rig: &Rig, offset: usize) -> (f32, f32) {
    let caret = rig
        .read(pm_pages::Screen::code_lines)
        .caret_rect(EDITOR_FIELD, offset)
        .expect("the place is drawn");
    (
        caret.origin.x + 1.0,
        caret.origin.y + caret.size.height / 2.0,
    )
}

/// Dragging selected code and letting go elsewhere moves it there.
#[test]
fn selected_code_drags_to_another_place() {
    let mut rig = Rig::shot(Shot::Code);
    let text = |rig: &Rig| {
        rig.read(|screen| {
            screen
                .field(EDITOR_FIELD)
                .map(|field| field.text().to_owned())
        })
        .unwrap_or_default()
    };
    let before = text(&rig);
    let word = before.find("LIMIT").expect("the word is in the file");
    rig.click_on(EDITOR_FIELD);
    std::thread::sleep(crate::app::DOUBLE_CLICK);
    let at = code_point(&rig, word + 2);
    rig.click(at);
    rig.click(at);
    let selected = rig.read(|screen| {
        screen
            .field(EDITOR_FIELD)
            .map(|field| field.selected().to_owned())
    });
    assert_eq!(selected.as_deref(), Some("LIMIT"));
    let target = before
        .find("impl Session")
        .expect("the line is in the file");
    std::thread::sleep(crate::app::DOUBLE_CLICK);
    let to = code_point(&rig, target);
    rig.drag(at, to, 6);
    let after = text(&rig);
    assert_eq!(after.len(), before.len());
    assert!(after.contains("LIMITimpl Session"), "{after}");
}

/// Dragging with Alt and Shift held selects a rectangle of code, a stretch on each line.
#[test]
fn alt_shift_drag_selects_columns() {
    let mut rig = Rig::shot(Shot::Code);
    let text = rig
        .read(|screen| {
            screen
                .field(EDITOR_FIELD)
                .map(|field| field.text().to_owned())
        })
        .unwrap_or_default();
    let first = text.find("pub id").expect("the line is in the file");
    let third = text
        .find("names: HashMap")
        .expect("the line is in the file");
    rig.click_on(EDITOR_FIELD);
    let from = code_point(&rig, first);
    let to = code_point(&rig, third + 3);
    rig.hold(rustle::Modifiers {
        alt: true,
        shift: true,
        ..Default::default()
    });
    rig.drag(from, to, 4);
    rig.hold(rustle::Modifiers::default());
    let cursors = rig.read(|screen| {
        screen
            .field(EDITOR_FIELD)
            .map(|field| field.selected_all().lines().count())
    });
    assert_eq!(cursors, Some(3));
}
