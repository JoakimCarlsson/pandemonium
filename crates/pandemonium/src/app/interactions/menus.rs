//! Context menus: which one the secondary button opens over what, what
//! their rows do, and the press or key that closes them.

use pm_components::areas::{
    ChatEvent, ComposerEvent, DiffEvent, GitEvent, MenuEvent, SidebarEvent,
};
use pm_components::target::Target;

use rustle::Key;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// The text of the prompt.
fn prompt(rig: &Rig) -> String {
    rig.read(|screen| {
        screen
            .text_field(Target::Composer(ComposerEvent::Prompt))
            .map(rustle::TextFieldState::text)
    })
    .unwrap_or_default()
}

/// The secondary button over a field opens its menu; Escape closes it.
#[test]
fn a_field_menu_opens_and_escape_closes_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = rig.center(Target::Composer(ComposerEvent::Prompt));
    rig.right_click(at);
    assert_eq!(
        rig.state(|state| state.menu.as_ref().map(|menu| menu.field)),
        Some(Target::Composer(ComposerEvent::Prompt))
    );
    assert!(rig.drawn(Target::Menu(MenuEvent::MenuPaste)));
    rig.key(Key::Escape);
    assert!(rig.state(|state| state.menu.is_none()));
}

/// The rows of a field's menu act on that field: select everything, then paste over it.
#[test]
fn a_field_menu_selects_and_pastes() {
    let mut rig = Rig::shot(Shot::Workspace);
    *rig.clipboard.text.borrow_mut() = Some("pasted".to_owned());
    rig.click_on(Target::Composer(ComposerEvent::Prompt));
    rig.type_text("draft");
    let at = rig.caret_in_text("draft", 2).expect("the prompt is drawn");
    rig.right_click(at);
    rig.click_on(Target::Menu(MenuEvent::MenuSelectAll));
    assert!(rig.state(|state| state.menu.is_none()));
    assert_eq!(rig.focused(), Some(Target::Composer(ComposerEvent::Prompt)));
    rig.right_click(at);
    rig.click_on(Target::Menu(MenuEvent::MenuPaste));
    assert_eq!(prompt(&rig), "pasted");
}

/// The secondary button over a session's row opens its menu, whose rows act on that session.
#[test]
fn a_session_menu_opens_on_its_row() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = rig.center(Target::Sidebar(SidebarEvent::Session(1)));
    rig.right_click(at);
    assert_eq!(
        rig.state(|state| state.session_menu.as_ref().map(|menu| menu.session)),
        Some(1)
    );
    rig.click_on(Target::Menu(MenuEvent::SessionMenuCopyBranch));
    assert!(rig.state(|state| state.session_menu.is_none()));
    assert!(rig.clipboard.text.borrow().is_some());
}

/// Renaming from a session's menu asks for the new name in a field that
/// takes the keyboard once pressed, and Enter there renames the session.
#[test]
fn renaming_from_the_session_menu() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = rig.center(Target::Sidebar(SidebarEvent::Session(1)));
    rig.right_click(at);
    rig.click_on(Target::Menu(MenuEvent::SessionMenuRename));
    assert!(rig.drawn(Target::Git(GitEvent::RenameField)));
    rig.click_on(Target::Git(GitEvent::RenameField));
    assert_eq!(rig.focused(), Some(Target::Git(GitEvent::RenameField)));
    rig.chord(
        rustle::Modifiers {
            ctrl: true,
            ..Default::default()
        },
        Key::Character("a".into()),
    );
    rig.type_text("Renamed");
    rig.key(Key::Enter);
    assert!(rig.state(|state| state.git.prompt.is_none()));
    assert_eq!(
        rig.state(|state| state.sessions[1].title.clone()),
        "Renamed"
    );
}

/// A press anywhere outside an open menu of the sidebar closes it without acting.
#[test]
fn a_press_outside_closes_a_list_menu() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = rig.center(Target::Sidebar(SidebarEvent::Project(0)));
    rig.right_click(at);
    assert!(rig.state(|state| state.project_menu.is_some()));
    rig.click_on(Target::Diff(DiffEvent::TabDiff));
    assert!(rig.state(|state| state.project_menu.is_none()));
}

/// A press on the chip that opened an element's popover closes it rather than opening it
/// again, though the press is outside the popover.
#[test]
fn a_popover_toggles_on_its_chip() {
    let mut rig = Rig::shot(Shot::Elements);
    let chip = Target::Composer(ComposerEvent::Element(0));
    rig.click_on(chip);
    assert!(rig.state(|state| state.picker.is_some()));
    rig.click_on(chip);
    assert!(rig.state(|state| state.picker.is_none()));
    rig.click_on(chip);
    assert!(rig.state(|state| state.picker.is_some()));
}

/// The secondary button over the conversation opens the message's menu, which selects all of
/// the conversation's text composed (the messages near what is shown, as a selection container
/// around a lazy list holds) and copies it.
#[test]
fn the_message_menu_selects_and_copies() {
    let mut rig = Rig::shot(Shot::Workspace);
    let shown = rig
        .area(pm_pages::Screen::conversation_area)
        .expect("the conversation is drawn");
    rig.move_to((shown.origin.x + 100.0, shown.origin.y + 100.0));
    rig.wheel(-10_000.0);
    let area = rig
        .region(Target::Chat(ChatEvent::ChatText))
        .expect("the conversation is drawn");
    let at = (area.origin.x + 40.0, area.origin.y + 10.0);
    rig.right_click(at);
    assert!(rig.state(|state| state.message_menu.is_some()));
    rig.click_on(Target::Menu(MenuEvent::MessageMenuSelectAll));
    assert!(rig.app.borrow().text_selection().is_some());
    rig.right_click(at);
    rig.click_on(Target::Menu(MenuEvent::MessageMenuCopy));
    let copied = rig.clipboard.text.borrow().clone().unwrap_or_default();
    assert!(copied.contains("Checkpoints are in."), "{copied}");
}
