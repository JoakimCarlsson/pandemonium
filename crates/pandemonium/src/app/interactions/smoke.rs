//! The rig itself: the app comes up headless and draws its first frame.

use crate::app::headless::Rig;
use crate::snapshot::Shot;
use pm_components::areas::ComposerEvent;
use pm_components::target::Target;

/// The workspace comes up and draws its prompt.
#[test]
fn the_workspace_draws_its_prompt() {
    let rig = Rig::shot(Shot::Workspace);
    assert!(
        rig.drawn(Target::Composer(ComposerEvent::Prompt)),
        "{:?}",
        rig.tagged().len()
    );
}
