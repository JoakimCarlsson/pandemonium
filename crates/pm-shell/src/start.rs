//! Starting new work: the agents it can be started with, the welcome page
//! for a project or a chat, and the title a session takes from its first
//! prompt.

use crate::model::{AgentChoice, Project, Start, Workspace};

/// How many characters of the first prompt a new session's title keeps.
const TITLE_LENGTH: usize = 40;

/// The agents the app offers, one for each provider of the harness, in the harness's order.
pub fn offered_agents() -> Vec<AgentChoice> {
    pm_harness::agents()
        .iter()
        .map(|agent| AgentChoice {
            id: agent.id.to_owned(),
            name: agent.name.to_owned(),
            provider: agent.provider.to_owned(),
            server: agent.server,
        })
        .collect()
}

/// The welcome page for a new session in the project or submodule at index
/// path `at` among `projects`, or nothing when no project is there.
pub fn start_in(projects: &[Project], at: &[usize]) -> Option<Start> {
    let (first, nested) = at.split_first()?;
    let mut project = projects.get(*first)?;
    for index in nested {
        project = project.children.get(*index)?;
    }
    Some(Start {
        at: Some(at.to_vec()),
        heading: format!("What should we build in {}?", project.name),
        agent: 0,
        ..Start::default()
    })
}

/// The welcome page for a new chat, which belongs to no project.
pub fn start_chat() -> Start {
    Start {
        at: None,
        heading: "What would you like to talk about?".to_owned(),
        agent: 0,
        ..Start::default()
    }
}

/// Shows the welcome page `start` in `workspace` in place of the open session.
pub fn show_start(workspace: &mut Workspace, start: Start) {
    workspace.starting = Some(start);
    workspace.open = None;
}

/// The title a session started with `prompt` takes: its first characters.
pub fn title_of(prompt: &str) -> String {
    prompt.trim().chars().take(TITLE_LENGTH).collect()
}

#[cfg(test)]
/// Tests for starting new work.
mod tests {
    use super::*;
    use crate::sample::sample_workspace;

    /// The welcome page names the project or submodule the session starts in.
    #[test]
    fn heading_names_the_project() {
        let projects = sample_workspace().projects;
        let start = start_in(&projects, &[2, 0]).expect("a submodule is there");
        assert_eq!(
            start.heading,
            format!("What should we build in {}?", projects[2].children[0].name)
        );
        assert!(start_in(&projects, &[99]).is_none());
        assert_eq!(start_chat().at, None);
    }

    /// A title keeps the first characters of the prompt.
    #[test]
    fn titles_keep_the_start_of_the_prompt() {
        assert_eq!(title_of("  fix it  "), "fix it");
        assert_eq!(title_of(&"a".repeat(60)).chars().count(), TITLE_LENGTH);
    }
}
