//! What waits for a pause: each wait an effect of the window's root keyed on what it waits
//! on, delaying on the frame clock the rig moves on, so each is seen to fire once its time
//! has passed and not before.

use pm_components::areas::{ComposerEvent, EditorEvent, SidebarEvent, WindowEvent};
use pm_components::target::Target;
use rustle::{Key, PointerButton};

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// The file shown in the code shot, with the code editor holding the keyboard.
fn editing() -> Rig {
    let mut rig = Rig::shot(Shot::Code);
    rig.click_on(Target::Editor(EditorEvent::EditorField));
    rig
}

/// Where the middle of the word `word` of the file shown is drawn, halfway down its line.
fn word_point(rig: &Rig, word: &str) -> (f32, f32) {
    let offset = rig
        .read(|screen| {
            screen
                .field(Target::Editor(EditorEvent::EditorField))
                .and_then(|field| field.text().find(word))
        })
        .expect("the word is in the file")
        + word.len() / 2;
    let caret = rig
        .read(pm_pages::Screen::code_lines)
        .caret_rect(Target::Editor(EditorEvent::EditorField), offset)
        .expect("the word is drawn");
    (caret.origin.x, caret.origin.y + caret.size.height / 2.0)
}

/// The pointer resting on a word asks about it once it rested for the editor's hover delay,
/// and not before.
#[test]
fn resting_on_a_word_asks_after_the_delay() {
    let mut rig = Rig::shot(Shot::Code);
    rig.move_to(word_point(&rig, "HashMap"));
    let (_, rest) = rig
        .app
        .borrow()
        .hover_waiting()
        .expect("the rest on the word starts");
    let rest = rest.as_millis() as u64;
    rig.wait(rest - 20);
    assert!(
        rig.app.borrow().hover_waiting().is_some(),
        "asked too early"
    );
    rig.wait(40);
    assert!(rig.app.borrow().hover_waiting().is_none(), "never asked");
}

/// A card about a word stays a moment after the pointer left the word, so it can cross to
/// the card, and goes once that grace has passed.
#[test]
fn a_card_goes_a_moment_after_the_pointer_left_its_word() {
    let mut rig = Rig::shot(Shot::Hovering);
    let open = |rig: &Rig| rig.state(|state| state.hover_open());
    assert!(open(&rig));
    rig.move_to(word_point(&rig, "main\";"));
    assert!(open(&rig));
    rig.move_to((5.0, 400.0));
    assert!(rig.app.borrow().hover_leaving());
    rig.wait(300);
    assert!(open(&rig), "the card went before the grace passed");
    rig.wait(100);
    assert!(!open(&rig), "the card stayed after the grace passed");
}

/// Once the caret rests in the file shown, the language server is asked where the thing at
/// it is used and whether it offers changes there, each after its own pause.
#[test]
fn a_resting_caret_asks_after_its_pauses() {
    let mut rig = editing();
    assert!(rig.app.borrow().highlight_waiting().is_some());
    assert!(rig.app.borrow().bulb_waiting().is_some());
    rig.wait(200);
    assert!(
        rig.app.borrow().highlight_waiting().is_some(),
        "asked too early"
    );
    rig.wait(100);
    assert!(
        rig.app.borrow().highlight_waiting().is_none(),
        "never asked"
    );
    assert!(
        rig.app.borrow().bulb_waiting().is_none(),
        "the marker never asked"
    );
}

/// What the app asked of the servers since this was last asked, written out.
fn asked(rig: &Rig) -> String {
    format!("{:?}", rig.app.borrow_mut().work.asked())
}

/// A file that saves by itself waits until it went unchanged for its pause, then saves, and
/// asks again after each pause while the save is not yet done.
#[test]
fn a_changed_file_saves_itself_after_a_pause() {
    let mut rig = editing();
    rig.app.borrow().model.change(|mut state| {
        state.editor.autosave = true;
        state
    });
    rig.type_text("x");
    assert!(rig.app.borrow().autosave_waiting().is_some());
    let _ = asked(&rig);
    rig.wait(900);
    let early = asked(&rig);
    rig.wait(200);
    let late = asked(&rig);
    assert!(!early.contains("Format"), "saved too early: {early}");
    assert!(late.contains("Format"), "never saved: {late}");
    rig.wait(1000);
    let again = asked(&rig);
    assert!(
        again.contains("Format"),
        "not saved again while unsaved: {again}"
    );
}

/// Keys recorded for an action on the keymap page are kept once no second chord came within
/// the pause.
#[test]
fn recorded_keys_are_kept_after_a_pause() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.app.borrow().model.change(|mut state| {
        state.recording = Some(pm_editor::Action::OpenSettings);
        state
    });
    rig.key(Key::Character("k".into()));
    assert!(rig.app.borrow().recording_waiting().is_some());
    rig.wait(900);
    assert!(rig.state(|state| state.recording_keys()), "kept too early");
    rig.wait(200);
    assert!(!rig.state(|state| state.recording_keys()), "never kept");
    assert!(rig.app.borrow().recording_waiting().is_none());
}

/// The view is written once changes to it pause.
#[test]
fn the_view_is_written_once_changes_pause() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.wait(2100);
    rig.click_on(Target::Sidebar(SidebarEvent::SidebarToggle));
    assert!(rig.app.borrow().view_unsaved);
    rig.wait(200);
    assert!(rig.app.borrow().view_unsaved, "written too early");
    rig.wait(200);
    assert!(!rig.app.borrow().view_unsaved, "never written");
}

/// A change that only resizes a panel is followed as every change of the screen is: the
/// view is written with the new width once changes of it pause.
#[test]
fn a_resized_panel_is_written_with_the_view() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.wait(2100);
    let from = rig.center(Target::Window(WindowEvent::SidebarSash));
    let to = (from.0 + 60.0, from.1);
    rig.move_to(from);
    rig.button_in(None, PointerButton::Primary, true, from);
    rig.move_to(to);
    rig.button_in(None, PointerButton::Primary, false, to);
    assert!(rig.app.borrow().view_unsaved, "the resize was not followed");
    rig.wait(400);
    assert!(!rig.app.borrow().view_unsaved, "never written");
}

/// While the view keeps changing it is still written once the longest wait for a pause
/// has passed since it first changed.
#[test]
fn a_view_that_keeps_changing_is_written_after_a_while() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(Target::Composer(ComposerEvent::Prompt));
    rig.wait(2100);
    for _ in 0..9 {
        rig.type_text("x");
        rig.wait(200);
    }
    assert!(
        rig.app.borrow().view_unsaved,
        "written while changes kept coming"
    );
    rig.type_text("x");
    rig.wait(200);
    assert!(!rig.app.borrow().view_unsaved, "never written");
}

/// A change of what the screen shows that leaves the view as it would be written, such as
/// the sidebar shown or hidden, does not put off writing a view that changed before it.
#[test]
fn a_change_outside_the_view_does_not_put_off_writing_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(Target::Composer(ComposerEvent::Prompt));
    rig.wait(2100);
    rig.type_text("x");
    rig.wait(200);
    rig.app
        .borrow()
        .model
        .update(|screen| screen.sidebar_open = !screen.sidebar_open);
    rig.settle();
    assert!(rig.app.borrow().view_unsaved);
    rig.wait(150);
    assert!(
        !rig.app.borrow().view_unsaved,
        "the sidebar put the write off"
    );
}

/// While newer releases are looked for, the app asks again each time the wait between
/// looks has passed.
#[test]
fn newer_releases_are_looked_for_again_after_a_while() {
    let mut rig = Rig::shot(Shot::Workspace);
    assert!(rig.app.borrow().release_checked());
    let _ = asked(&rig);
    rig.wait(super::super::RELEASE_RECHECK.as_millis() as u64 - 1000);
    let early = asked(&rig);
    rig.wait(2000);
    let late = asked(&rig);
    assert!(
        !early.contains("LatestRelease"),
        "looked too early: {early}"
    );
    assert!(late.contains("LatestRelease"), "never looked: {late}");
}

/// The ring beside a working session turns as the frame clock moves on.
#[test]
fn the_ring_of_a_working_session_turns() {
    let mut rig = Rig::shot(Shot::SessionStates(false));
    let before = rig.dash_angles();
    assert!(!before.is_empty(), "no ring is drawn");
    rig.wait(120);
    assert_ne!(rig.dash_angles(), before, "the ring stood still");
}
