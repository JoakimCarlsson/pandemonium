//! Popups placed and dismissed by rustle: a menu hangs from the control that opened it,
//! and a menu, a popover or a list closes itself on Escape or a press outside it, while a
//! press on the control that opens it toggles it.

use pm_components::areas::{ComposerEvent, GitEvent, MenuEvent, SidebarEvent};
use pm_components::git_menus::MenuPlace;
use pm_components::target::Target;

use rustle::Key;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// The menu over the changes panel's actions hangs from the button that opened it, its right
/// edge against the button's, and Escape closes it.
#[test]
fn a_source_menu_hangs_from_its_button_and_escape_closes_it() {
    let mut rig = Rig::shot(Shot::Source);
    let button = rig
        .region(Target::Git(GitEvent::ChangesMore))
        .expect("the button over the changes is drawn");
    rig.click_on(Target::Git(GitEvent::ChangesMore));
    let place = rig.state(|state| state.git.menu.map(|open| open.place));
    assert!(
        matches!(place, Some(MenuPlace::Under(control)) if control == button),
        "{place:?}"
    );
    let row = rig
        .region(Target::Git(GitEvent::GitMenuRow(0)))
        .expect("the menu is drawn");
    assert!(row.origin.x + row.size.width <= button.origin.x + button.size.width);
    rig.key(Key::Escape);
    assert!(rig.state(|state| state.git.menu.is_none()));
    assert!(!rig.drawn(Target::Git(GitEvent::GitMenuRow(0))));
}

/// A press outside the menu under the commit button closes it, and the press does not reach
/// the field it landed on.
#[test]
fn a_press_outside_closes_the_commit_menu() {
    let mut rig = Rig::shot(Shot::Source);
    rig.click_on(Target::Git(GitEvent::CommitChevron));
    assert!(rig.state(|state| state.git.menu.is_some()));
    rig.click_on(Target::Git(GitEvent::CommitMessage));
    assert!(rig.state(|state| state.git.menu.is_none()));
}

/// The list of branches toggles on the button that opens it, though that button is outside
/// it, and closes on Escape and on a press elsewhere.
#[test]
fn the_branch_list_toggles_on_its_button_and_closes_itself() {
    let mut rig = Rig::shot(Shot::Source);
    let button = Target::Git(GitEvent::BranchButton);
    let open = |rig: &Rig| rig.state(|state| state.git.branch_menu);
    rig.click_on(button);
    assert!(open(&rig));
    rig.click_on(button);
    assert!(!open(&rig));
    rig.click_on(button);
    assert!(open(&rig));
    rig.key(Key::Escape);
    assert!(!open(&rig));
    rig.click_on(button);
    assert!(open(&rig));
    rig.click_on(Target::Git(GitEvent::CommitMessage));
    assert!(!open(&rig));
}

/// Escape closes a session's menu, which rustle hands the key to before anything else.
#[test]
fn escape_closes_a_list_menu() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = rig.center(Target::Sidebar(SidebarEvent::Session(1)));
    rig.right_click(at);
    assert!(rig.drawn(Target::Menu(MenuEvent::SessionMenuCopyPath)));
    rig.key(Key::Escape);
    assert!(rig.state(|state| state.session_menu.is_none()));
    assert!(!rig.drawn(Target::Menu(MenuEvent::SessionMenuCopyPath)));
}

/// A popover of an element closes itself on Escape and on a press outside it, below the
/// chip that opened it.
#[test]
fn a_popover_closes_on_escape_and_on_a_press_outside() {
    let mut rig = Rig::shot(Shot::Elements);
    let chip = Target::Composer(ComposerEvent::Element(0));
    rig.click_on(chip);
    assert!(rig.state(|state| state.picker.is_some()));
    rig.key(Key::Escape);
    assert!(rig.state(|state| state.picker.is_none()));
    rig.click_on(chip);
    assert!(rig.state(|state| state.picker.is_some()));
    let below = rig.region(chip).expect("the chip is drawn");
    rig.click((
        below.origin.x + below.size.width / 2.0,
        below.origin.y + below.size.height + 40.0,
    ));
    assert!(rig.state(|state| state.picker.is_none()));
}
