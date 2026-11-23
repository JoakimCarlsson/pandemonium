//! Tests of which profile a tab's page lives in.

use crate::areas::{BrowserEvent, SidebarEvent};
use crate::events::Event;
use crate::state::WorkspaceState;
use crate::target::Target;

use super::{PRIVATE_PARTITION, project_partition};

/// A workspace showing its first session, with the browser open on one tab.
fn opened() -> WorkspaceState {
    let mut state = WorkspaceState::sample().handle(Event::Sidebar(SidebarEvent::Session(0)));
    state.open_browser("http://localhost:5173");
    state
}

/// Tabs share one profile until each project has its own; a private tab always lives in the private profile.
#[test]
fn a_tab_lives_in_the_profile_of_its_kind() {
    let mut state = opened().browser_pressed(Target::Browser(BrowserEvent::BrowserNewPrivateTab));
    let browser = state.browser.as_ref().unwrap();
    let (shared, private) = (browser.tabs[0].id, browser.tabs[1].id);
    assert_eq!(state.browser_partition(shared), None);
    assert_eq!(
        state.browser_partition(private).as_deref(),
        Some(PRIVATE_PARTITION)
    );
    state.window.general.project_browser_profiles = true;
    let project = state.sessions[0].project.map(|at| &state.projects[at]);
    assert_eq!(
        state.browser_partition(shared),
        project.map(project_partition)
    );
    assert_eq!(
        state.browser_partition(private).as_deref(),
        Some(PRIVATE_PARTITION)
    );
}

/// A project's profile is named by its address alone, so it is the same after a restart and differs between projects.
#[test]
fn a_project_profile_is_named_by_its_address() {
    let one = crate::project::Project::from_source("/work/one").unwrap();
    let two = crate::project::Project::from_source("/work/two").unwrap();
    assert_eq!(project_partition(&one), project_partition(&one.clone()));
    assert_ne!(project_partition(&one), project_partition(&two));
    assert!(project_partition(&one).starts_with("persist:project-"));
}
