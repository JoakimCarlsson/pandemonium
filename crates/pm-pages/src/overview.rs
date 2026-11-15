//! A project's overview: its sessions grouped by where they stand, and the
//! latest commits of its repository.

use rustle::prelude::*;

use pm_components::{
    Avatar, Button, ButtonVariant, Label, LabelVariant, Leading, ListRow, PageBody, PageHeader,
    Section, Size, project_colour,
};
use pm_shell::{Project, Session, SessionStatus, Workspace};

use crate::actions::Actions;

/// How wide the column naming a session's branch is.
const BRANCH_WIDTH: f32 = 140.0;

/// How wide the column saying how long ago a session changed is.
const AGE_WIDTH: f32 = 72.0;

/// How wide the column naming a commit is.
const COMMIT_WIDTH: f32 = 64.0;

/// The groups the sessions fall in, in the order the overview shows them,
/// each with its heading.
const GROUPS: [(SessionStatus, &str); 5] = [
    (SessionStatus::NeedsInput, "Needs input"),
    (SessionStatus::Error, "Error"),
    (SessionStatus::Unread, "Unread"),
    (SessionStatus::Working, "Working"),
    (SessionStatus::Idle, "Idle"),
];

/// Emits the overview of the project at address `path` in `workspace`.
pub fn overview_page(workspace: &Workspace, path: &str, actions: &Actions) {
    let Some((project, at)) = find_project(&workspace.projects, path, Vec::new()) else {
        PageHeader::new(path).show(|| {});
        return;
    };
    let name = project.name.clone();
    let tint = project_colour(&project.path);
    let start = actions.new_session.clone();
    let base = at.clone();
    PageHeader::new(project.name.clone())
        .subtitle(project.path.clone())
        .leading(move || Avatar::new(&name, tint).size(Size::Sm).show())
        .show(|| {
            Button::new("New session")
                .icon(phosphor::PLUS)
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(move || start(at.clone()))
                .show();
        });
    let mut sessions = Vec::new();
    collect_sessions(project, Vec::new(), &mut sessions);
    PageBody::new().show(|| {
        if sessions.is_empty() {
            Label::new("No sessions yet.")
                .variant(LabelVariant::Subtle)
                .show();
        }
        for (status, title) in GROUPS {
            key(title, || {
                let members: Vec<&(Vec<usize>, &Session)> = sessions
                    .iter()
                    .filter(|(_, session)| session.status == status)
                    .collect();
                Section::new(title).show(members.len(), |index| {
                    let (inner, session) = members[index];
                    let mut full = base.clone();
                    full.extend(inner.iter().copied());
                    session_row(session, full, actions);
                });
            });
        }
        Section::new("Recent commits").show(workspace.commits.len(), |index| {
            let commit = &workspace.commits[index];
            ListRow::new(commit.summary.clone())
                .leading(Leading::Icon(phosphor::GIT_COMMIT))
                .code_column(commit.id.clone(), COMMIT_WIDTH)
                .size(Size::Sm)
                .show();
        });
    });
}

/// Emits the row of `session`, found at index path `at`: where it stands,
/// its title, its branch and when it last changed.
fn session_row(session: &Session, at: Vec<usize>, actions: &Actions) {
    let open = actions.open_session.clone();
    ListRow::new(session.title.clone())
        .leading(match session.status {
            SessionStatus::Idle => Leading::Dot { filled: false },
            SessionStatus::Unread => Leading::Dot { filled: true },
            SessionStatus::Working => Leading::Spinner,
            SessionStatus::NeedsInput => Leading::Attention,
            SessionStatus::Error => Leading::Failed,
        })
        .code_column(session.branch.clone(), BRANCH_WIDTH)
        .end_column(session.age.clone(), AGE_WIDTH)
        .size(Size::Sm)
        .on_click(move || open(at.clone()))
        .show();
}

/// The project at address `path` among `projects`, with its index path under `prefix`.
fn find_project<'a>(
    projects: &'a [Project],
    path: &str,
    prefix: Vec<usize>,
) -> Option<(&'a Project, Vec<usize>)> {
    projects.iter().enumerate().find_map(|(index, project)| {
        let mut here = prefix.clone();
        here.push(index);
        if project.path == path {
            Some((project, here))
        } else {
            find_project(&project.children, path, here)
        }
    })
}

/// Adds the sessions of `project` and of the projects inside it to `found`,
/// each with its index path from `project`, under `prefix`.
fn collect_sessions<'a>(
    project: &'a Project,
    prefix: Vec<usize>,
    found: &mut Vec<(Vec<usize>, &'a Session)>,
) {
    for (index, session) in project.sessions.iter().enumerate() {
        let mut here = prefix.clone();
        here.push(index);
        found.push((here, session));
    }
    for (index, child) in project.children.iter().enumerate() {
        let mut here = prefix.clone();
        here.push(index);
        collect_sessions(child, here, found);
    }
}
