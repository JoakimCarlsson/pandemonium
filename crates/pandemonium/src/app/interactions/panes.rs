//! The file shown and the browser pane: the code taking the keyboard and
//! the view's size told to the screen, and the browser's address field and menu.

use pm_components::areas::{BrowserEvent, EditorEvent};
use pm_components::target::Target;
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
                .field(Target::Editor(EditorEvent::EditorField))
                .map(|field| field.text().to_owned())
        })
    };
    let before = text(&rig).expect("a file is shown");
    rig.click_on(Target::Editor(EditorEvent::EditorField));
    assert_eq!(
        rig.focused(),
        Some(Target::Editor(EditorEvent::EditorField))
    );
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
                .text_field(Target::Browser(BrowserEvent::BrowserAddress))
                .map(rustle::TextFieldState::text)
        })
        .unwrap_or_default()
    };
    let before = address(&rig);
    rig.click_on(Target::Browser(BrowserEvent::BrowserAddress));
    assert_eq!(
        rig.focused(),
        Some(Target::Browser(BrowserEvent::BrowserAddress))
    );
    rig.type_text("x");
    assert_eq!(address(&rig).len(), before.len() + 1);
    rig.key(Key::Escape);
    let at = rig.center(Target::Browser(BrowserEvent::BrowserBody));
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
                .field(Target::Editor(EditorEvent::EditorField))
                .map(|field| field.text().to_owned())
        })
        .expect("a file is shown");
    let word = text.find("HashMap").expect("the word is in the file") + 2;
    let caret = rig
        .read(pm_pages::Screen::code_lines)
        .caret_rect(Target::Editor(EditorEvent::EditorField), word)
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
        .caret_rect(Target::Editor(EditorEvent::EditorField), offset)
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
                .field(Target::Editor(EditorEvent::EditorField))
                .map(|field| field.text().to_owned())
        })
        .unwrap_or_default()
    };
    let before = text(&rig);
    let word = before.find("LIMIT").expect("the word is in the file");
    rig.click_on(Target::Editor(EditorEvent::EditorField));
    std::thread::sleep(double_tap_timeout());
    let at = code_point(&rig, word + 2);
    rig.click(at);
    rig.click(at);
    let selected = rig.read(|screen| {
        screen
            .field(Target::Editor(EditorEvent::EditorField))
            .map(|field| field.selected().to_owned())
    });
    assert_eq!(selected.as_deref(), Some("LIMIT"));
    let target = before
        .find("impl Session")
        .expect("the line is in the file");
    std::thread::sleep(double_tap_timeout());
    let to = code_point(&rig, target);
    rig.drag(at, to, 6);
    let after = text(&rig);
    assert_eq!(after.len(), before.len());
    assert!(after.contains("LIMITimpl Session"), "{after}");
}

/// The code shot with the word `LIMIT` of the file shown selected by a double press, and
/// where the press was.
fn limit_selected() -> (Rig, (f32, f32)) {
    let mut rig = Rig::shot(Shot::Code);
    let word = rig
        .read(|screen| {
            screen
                .field(Target::Editor(EditorEvent::EditorField))
                .and_then(|field| field.text().find("LIMIT"))
        })
        .expect("the word is in the file");
    rig.click_on(Target::Editor(EditorEvent::EditorField));
    std::thread::sleep(double_tap_timeout());
    let at = code_point(&rig, word + 2);
    rig.click(at);
    rig.click(at);
    std::thread::sleep(double_tap_timeout());
    (rig, at)
}

/// The text of the file shown, and what of it is selected.
fn text_and_selection(rig: &Rig) -> (String, String) {
    rig.read(|screen| {
        screen
            .field(Target::Editor(EditorEvent::EditorField))
            .map(|field| (field.text().to_owned(), field.selected().to_owned()))
    })
    .unwrap_or_default()
}

/// A press on the selection let go without moving is no drag: the selection goes and the
/// caret is put where it was pressed, the text unchanged.
#[test]
fn a_press_on_selected_code_without_a_drag_places_the_caret() {
    let (mut rig, at) = limit_selected();
    let (before, selected) = text_and_selection(&rig);
    assert_eq!(selected, "LIMIT");
    rig.click(at);
    let (after, selected) = text_and_selection(&rig);
    assert_eq!(after, before);
    assert_eq!(selected, "");
}

/// Escape while selected code is carried lets go of it: letting the button go afterwards
/// moves nothing.
#[test]
fn escape_gives_up_carrying_selected_code() {
    let (mut rig, at) = limit_selected();
    let (before, _) = text_and_selection(&rig);
    let target = before
        .find("impl Session")
        .expect("the line is in the file");
    let to = code_point(&rig, target);
    rig.move_to(at);
    rig.button_in(None, rustle::PointerButton::Primary, true, at);
    rig.move_to((at.0, at.1 + 20.0));
    rig.move_to(to);
    assert!(rig.state(|state| state.editor_dragging()));
    rig.key(Key::Escape);
    assert!(!rig.state(|state| state.editor_dragging()));
    rig.button_in(None, rustle::PointerButton::Primary, false, to);
    let (after, _) = text_and_selection(&rig);
    assert_eq!(after, before);
}

/// Dragging with Alt and Shift held selects a rectangle of code, a stretch on each line.
#[test]
fn alt_shift_drag_selects_columns() {
    let mut rig = Rig::shot(Shot::Code);
    let text = rig
        .read(|screen| {
            screen
                .field(Target::Editor(EditorEvent::EditorField))
                .map(|field| field.text().to_owned())
        })
        .unwrap_or_default();
    let first = text.find("pub id").expect("the line is in the file");
    let third = text
        .find("names: HashMap")
        .expect("the line is in the file");
    rig.click_on(Target::Editor(EditorEvent::EditorField));
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
            .field(Target::Editor(EditorEvent::EditorField))
            .map(|field| field.selected_all().lines().count())
    });
    assert_eq!(cursors, Some(3));
}

/// How long rustle waits after a press for another to count with it, so a press after this long starts a new row.
fn double_tap_timeout() -> std::time::Duration {
    std::time::Duration::from_millis(rustle::ViewConfiguration::default().double_tap_timeout_millis)
}

/// Holding Control over a word of the file shown underlines it, as what a press takes to
/// its definition, with the pointing hand over the code, and letting go of Control takes
/// the underline away.
#[test]
fn control_reaches_a_word_of_the_code() {
    let mut rig = Rig::shot(Shot::Code);
    let text = rig
        .read(|screen| {
            screen
                .field(Target::Editor(EditorEvent::EditorField))
                .map(|field| field.text().to_owned())
        })
        .expect("a file is shown");
    let word = text.find("HashMap").expect("the word is in the file") + 2;
    let reached = |rig: &Rig| {
        rig.state(|state| {
            state
                .shown_document()
                .and_then(|document| document.reach.clone())
        })
    };
    rig.move_to(code_point(&rig, word));
    assert_eq!(rig.cursor(), rustle::CursorIcon::Text);
    rig.hold(rustle::Modifiers {
        ctrl: true,
        ..rustle::Modifiers::NONE
    });
    let underlined = reached(&rig).expect("the word is underlined");
    assert!(underlined.contains(&word), "{underlined:?}");
    assert_eq!(rig.cursor(), rustle::CursorIcon::Pointer);
    rig.hold(rustle::Modifiers::NONE);
    assert_eq!(reached(&rig), None);
    assert_eq!(rig.cursor(), rustle::CursorIcon::Text);
}

/// The browser's page and its developer tools tell the screen the size each is laid out
/// at, which the pages behind them are given. The browser is the open session's, as it is
/// once the reader opened it there.
#[test]
fn the_browser_surfaces_tell_the_size_they_are_laid_out_at() {
    let Screen::Workspace(state) = Shot::BrowserTabs(false).screen();
    let mut state = state.browser_tools_shown();
    state.browser_adopted_by(0);
    let rig = Rig::new(Screen::Workspace(state), crate::app::headless::SIZE);
    for id in [BrowserEvent::BrowserBody, BrowserEvent::BrowserToolsBody] {
        let id = Target::Browser(id);
        let region = rig.region(id).expect("the surface is laid out");
        assert_eq!(
            rig.read(|screen| screen.places().size(id)),
            Some(region.size)
        );
    }
}

/// The developer tools keep the width they were dragged to beside the page, which takes the
/// rest of the pane after the line between them, and a width that would leave the page too
/// narrow is cut to leave it its least width.
#[test]
fn the_browser_tools_keep_their_width_and_the_page_takes_the_rest() {
    let laid = |tools_width: f32| {
        let Screen::Workspace(state) = Shot::BrowserTabs(false).screen();
        let mut state = state.browser_tools_shown();
        state.browser_adopted_by(0);
        if let Some(browser) = state.browser.as_mut() {
            browser.tools_width = tools_width;
        }
        let rig = Rig::new(Screen::Workspace(state), crate::app::headless::SIZE);
        let page = rig
            .region(Target::Browser(BrowserEvent::BrowserBody))
            .expect("the page is laid out");
        let tools = rig
            .region(Target::Browser(BrowserEvent::BrowserToolsBody))
            .expect("the tools are laid out");
        (page, tools)
    };
    let (page, tools) = laid(300.0);
    assert_eq!(tools.size.width, 300.0);
    assert_eq!(page.origin.x + page.size.width + 1.0, tools.origin.x);
    assert_eq!(page.size.height, tools.size.height);
    let row = page.size.width + 1.0 + tools.size.width;
    let (page, tools) = laid(5000.0);
    assert_eq!(page.size.width, pm_components::browser::TOOLS_LEAST);
    assert_eq!(page.size.width + 1.0 + tools.size.width, row);
}
