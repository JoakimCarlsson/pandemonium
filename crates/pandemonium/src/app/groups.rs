//! Creating, naming and assigning project groups through the shared preferences.

use pm_core::ProjectId;
use pm_ui::{MenuItem, menu_entry, menu_separator};

use crate::app::App;
use crate::message::Message;
use crate::picker::Kind;
use crate::project_groups::{self, ProjectGroup};
use crate::workspace::MenuTarget;

impl App {
    /// Handles the commands that edit project groups.
    pub(super) fn group_command(&mut self, message: Message) -> bool {
        match message {
            Message::NewProjectGroup(project) => {
                self.open_picker_with(Kind::ProjectGroup(None, project), Vec::new(), String::new());
            }
            Message::RenameProjectGroup(index) => {
                if let Some(group) = self.project_groups.get(index) {
                    self.open_picker_with(
                        Kind::ProjectGroup(Some(index), None),
                        Vec::new(),
                        group.name.clone(),
                    );
                }
            }
            Message::RemoveProjectGroup(index) => {
                if index < self.project_groups.len() {
                    self.project_groups.remove(index);
                    self.store();
                }
            }
            Message::ToggleProjectGroup(index) => {
                if let Some(group) = self.project_groups.get_mut(index) {
                    group.collapsed = !group.collapsed;
                    self.store();
                }
            }
            Message::ProjectGroupMenu(index) => self.open_menu(MenuTarget::ProjectGroup(index)),
            Message::AssignProjectGroup(project, group) => {
                self.assign_project_group(project, group)
            }
            _ => return false,
        }
        true
    }

    /// Saves a nonempty, unique group name and any requested project membership.
    pub(super) fn save_project_group(
        &mut self,
        index: Option<usize>,
        project: Option<ProjectId>,
        typed: &str,
    ) {
        let name = typed.trim();
        if name.is_empty() {
            return;
        }
        if self
            .project_groups
            .iter()
            .enumerate()
            .any(|(held, group)| Some(held) != index && group.name.eq_ignore_ascii_case(name))
        {
            self.notices
                .trouble("A project group already has that name", None);
            return;
        }
        let target = match index {
            Some(index) => {
                let Some(group) = self.project_groups.get_mut(index) else {
                    return;
                };
                group.name = name.to_owned();
                index
            }
            None => {
                self.project_groups.push(ProjectGroup {
                    name: name.to_owned(),
                    ..ProjectGroup::default()
                });
                self.project_groups.len() - 1
            }
        };
        if let Some(project) = project {
            self.assign_project_group(project, Some(target));
        }
        self.store();
    }

    /// Moves a project into a group without closing or activating it.
    pub(super) fn assign_project_group(&mut self, project: ProjectId, group: Option<usize>) {
        if group.is_some_and(|index| index >= self.project_groups.len()) {
            return;
        }
        if let Some(project) = self.open.get(project) {
            project_groups::assign(&mut self.project_groups, &project.root().stored(), group);
            self.store();
        }
    }

    /// Menu entries for assigning a project to existing or newly named groups.
    pub(super) fn project_group_items(&self, project: ProjectId) -> Vec<MenuItem<Message>> {
        let current = self.open.get(project).and_then(|project| {
            project_groups::membership(&self.project_groups, &project.root().stored())
        });
        let mut items = vec![
            menu_separator(),
            menu_entry("New Group…", Some(Message::NewProjectGroup(Some(project)))),
        ];
        items.extend(
            self.project_groups
                .iter()
                .enumerate()
                .filter(|(index, _)| Some(*index) != current)
                .map(|(index, group)| {
                    menu_entry(
                        format!("Move to {}", group.name),
                        Some(Message::AssignProjectGroup(project, Some(index))),
                    )
                }),
        );
        if current.is_some() {
            items.push(menu_entry(
                "Remove from Group",
                Some(Message::AssignProjectGroup(project, None)),
            ));
        }
        items
    }
}
