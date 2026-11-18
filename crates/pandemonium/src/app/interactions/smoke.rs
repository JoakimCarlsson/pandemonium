//! The rig itself: the app comes up headless and draws its first frame.

use pm_pages::PROMPT;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// The workspace comes up and draws its prompt.
#[test]
fn the_workspace_draws_its_prompt() {
    let rig = Rig::shot(Shot::Workspace);
    assert!(rig.drawn(PROMPT), "{:?}", rig.report().regions.len());
}
