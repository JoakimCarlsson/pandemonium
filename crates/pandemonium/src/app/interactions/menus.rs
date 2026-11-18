//! Context menus: which one the secondary button opens over what, what
//! their rows do, and the press or key that closes them.

use pm_components::arrange::{SESSION_MENU_COPY_BRANCH, SESSION_MENU_RENAME};
use pm_components::context_menu::{MENU_PASTE, MENU_SELECT_ALL};
use pm_components::git_prompts::RENAME_FIELD;
use pm_components::kit::{FocusId, Key};
use pm_components::message_menu::{MESSAGE_MENU_COPY, MESSAGE_MENU_SELECT_ALL};
use pm_components::state::{CHAT_TEXT, PROJECT_BASE, PROMPT, SESSION_BASE, TAB_DIFF};

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// The text of the prompt.
fn prompt(rig: &Rig) -> String {
    rig.read(|screen| screen.field(PROMPT).map(|field| field.text().to_owned()))
        .unwrap_or_default()
}

/// The secondary button over a field opens its menu; Escape closes it.
#[test]
fn a_field_menu_opens_and_escape_closes_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = rig.center(PROMPT);
    rig.right_click(at);
    assert_eq!(
        rig.state(|state| state.menu.as_ref().map(|menu| menu.field)),
        Some(PROMPT)
    );
    assert!(rig.drawn(MENU_PASTE));
    rig.key(Key::Escape);
    assert!(rig.state(|state| state.menu.is_none()));
}

/// The rows of a field's menu act on that field: select everything, then paste over it.
#[test]
fn a_field_menu_selects_and_pastes() {
    let mut rig = Rig::shot(Shot::Workspace);
    *rig.clipboard.text.borrow_mut() = Some("pasted".to_owned());
    rig.click_on(PROMPT);
    rig.type_text("draft");
    let field = rig
        .report()
        .field(PROMPT)
        .cloned()
        .expect("the prompt is drawn");
    let inside = field.caret_at(2);
    let at = (inside.0, inside.1 + inside.2 / 2.0);
    rig.right_click(at);
    rig.click_on(MENU_SELECT_ALL);
    assert!(rig.state(|state| state.menu.is_none()));
    assert_eq!(rig.focused(), Some(PROMPT));
    rig.right_click(at);
    rig.click_on(MENU_PASTE);
    assert_eq!(prompt(&rig), "pasted");
}

/// The secondary button over a session's row opens its menu, whose rows act on that session.
#[test]
fn a_session_menu_opens_on_its_row() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = rig.center(FocusId(SESSION_BASE + 1));
    rig.right_click(at);
    assert_eq!(
        rig.state(|state| state.session_menu.as_ref().map(|menu| menu.session)),
        Some(1)
    );
    rig.click_on(SESSION_MENU_COPY_BRANCH);
    assert!(rig.state(|state| state.session_menu.is_none()));
    assert!(rig.clipboard.text.borrow().is_some());
}

/// Renaming from a session's menu asks for the new name in a field that
/// takes the keyboard once pressed, and Enter there renames the session.
#[test]
fn renaming_from_the_session_menu() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = rig.center(FocusId(SESSION_BASE + 1));
    rig.right_click(at);
    rig.click_on(SESSION_MENU_RENAME);
    assert!(rig.drawn(RENAME_FIELD));
    rig.click_on(RENAME_FIELD);
    assert_eq!(rig.focused(), Some(RENAME_FIELD));
    rig.chord(
        pm_components::kit::Modifiers {
            control: true,
            ..Default::default()
        },
        Key::Character('a'),
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
    let at = rig.center(FocusId(PROJECT_BASE));
    rig.right_click(at);
    assert!(rig.state(|state| state.project_menu.is_some()));
    rig.click_on(TAB_DIFF);
    assert!(rig.state(|state| state.project_menu.is_none()));
}

/// The secondary button over the conversation opens the message's menu,
/// which selects all of its text and copies it.
#[test]
fn the_message_menu_selects_and_copies() {
    let mut rig = Rig::shot(Shot::Workspace);
    let (_, area) = rig
        .report()
        .selectable_areas
        .iter()
        .copied()
        .find(|(id, _)| *id == CHAT_TEXT)
        .expect("the conversation is drawn");
    let at = (area.origin.x + 40.0, area.origin.y + 10.0);
    rig.right_click(at);
    assert!(rig.state(|state| state.message_menu.is_some()));
    rig.click_on(MESSAGE_MENU_SELECT_ALL);
    assert!(
        rig.app
            .borrow_mut()
            .text_selection
            .is_some_and(|held| !held.is_empty())
    );
    rig.right_click(at);
    rig.click_on(MESSAGE_MENU_COPY);
    let copied = rig.clipboard.text.borrow().clone().unwrap_or_default();
    assert!(copied.contains("Checkpoints are in."), "{copied}");
}
