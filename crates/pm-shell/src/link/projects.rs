//! The link's part in arranging the sidebar's projects and adding new ones:
//! saves the projects' order and removals on the server, clones
//! repositories, and lists folders for the folder picker on this machine or
//! on the machine the picker looks at.

use std::collections::HashMap;

use pm_protocol::{Request, Response};

use super::remote::Standing;
use super::{Ask, Link};
use crate::projects::{
    Adder, FolderPicker, PickerAction, ProjectAction, Reach, clone_address, moved, reordered,
};
use crate::sample::sample_projects;

impl Link {
    /// Does what the reader asked of the sidebar's projects, the adder or the folder picker.
    pub fn project_action(&mut self, action: ProjectAction) {
        match action {
            ProjectAction::Menu(path) => self.workspace.project_menu = path,
            ProjectAction::Remove(path) => {
                self.workspace.project_menu = None;
                let paths = self
                    .paths
                    .iter()
                    .filter(|kept| **kept != path)
                    .cloned()
                    .collect();
                self.save_projects(paths);
            }
            ProjectAction::Move(path, by) => {
                self.workspace.project_menu = None;
                self.save_projects(moved(self.paths.clone(), &path, by));
            }
            ProjectAction::Reorder(from, to) => {
                self.save_projects(reordered(self.paths.clone(), from, to))
            }
            ProjectAction::Fold(path) => {
                if !self.folded.remove(&path) {
                    self.folded.insert(path);
                }
                self.rebuild();
            }
            ProjectAction::Adding(step) => {
                self.workspace.adder = step.map(|step| Adder {
                    step,
                    ..self.workspace.adder.clone().unwrap_or_default()
                });
            }
            ProjectAction::CloneUrl(url) => {
                let adder = self.workspace.adder.get_or_insert_with(Adder::default);
                adder.url = url;
                adder.error = None;
            }
            ProjectAction::Clone => self.clone_repository(),
            ProjectAction::Picker(action) => self.picker_action(action),
        }
    }

    /// Has the server keep `paths` as the projects, showing them at once.
    fn save_projects(&mut self, paths: Vec<String>) {
        self.send(Request::SaveProjects {
            paths: paths.clone(),
        });
        self.set_projects(paths);
    }

    /// Adds the project at `address`, unless it is already in the sidebar.
    fn add_project(&mut self, address: String) {
        if self.paths.contains(&address) {
            return;
        }
        let mut paths = self.paths.clone();
        paths.push(address);
        self.save_projects(paths);
    }

    /// Clones the address the adder holds into the usual folder for clones, adding the clone once it is made.
    fn clone_repository(&mut self) {
        let Some(adder) = &mut self.workspace.adder else {
            return;
        };
        let url = clone_address(&adder.url);
        if url.is_empty() || adder.busy {
            return;
        }
        adder.busy = true;
        adder.error = None;
        self.ask(Request::Clone { url, under: None }, Ask::Cloned);
    }

    /// Takes the server's answer to a clone.
    pub(super) fn answer_clone(&mut self, response: Response) {
        match response {
            Response::Directory { path } => {
                self.workspace.adder = None;
                self.add_project(path);
            }
            Response::Failed { message } => {
                if let Some(adder) = &mut self.workspace.adder {
                    adder.busy = false;
                    adder.error = Some(message);
                }
            }
            _ => {}
        }
    }

    /// Does what the reader asked of the folder picker.
    fn picker_action(&mut self, action: PickerAction) {
        match action {
            PickerAction::Open(host) => {
                self.workspace.adder = None;
                let known = self.settings.window.servers.known.clone();
                self.workspace.picker = Some(FolderPicker::open(host, &known));
                self.list_folders("~".to_owned());
            }
            PickerAction::Close => self.workspace.picker = None,
            PickerAction::Filter(text) => {
                if let Some(picker) = &mut self.workspace.picker {
                    picker.filter = text;
                }
            }
            PickerAction::Choose => {
                if let Some(picker) = self.workspace.picker.take() {
                    self.add_project(picker.address());
                }
            }
            action => {
                let target = self
                    .workspace
                    .picker
                    .as_mut()
                    .and_then(|picker| picker.step(&action));
                if let Some(path) = target {
                    self.list_folders(path);
                }
            }
        }
    }

    /// Asks the machine the picker looks at for the folders inside `path`.
    fn list_folders(&mut self, path: String) {
        let Some(picker) = &self.workspace.picker else {
            return;
        };
        let request = Request::Folders { path };
        match picker.host.clone() {
            None => self.ask(request, Ask::Folders(None)),
            Some(host) => self.ask_remote(&host, request, Ask::Folders(Some(host.clone()))),
        }
    }

    /// The token of each server reached straight or through a tunnel, by its address.
    pub(super) fn tokens(&self) -> HashMap<String, String> {
        self.settings
            .window
            .servers
            .known
            .iter()
            .filter(|server| server.reach.keeps_token())
            .map(|server| (server.address.clone(), server.token.clone()))
            .collect()
    }

    /// Takes the answer of the machine `host`, or this one, to listing folders for the picker.
    pub(super) fn answer_folders(&mut self, host: Option<String>, response: Response) {
        let Some(picker) = &mut self.workspace.picker else {
            return;
        };
        if picker.host != host {
            return;
        }
        match response {
            Response::Folders { path, names } => picker.listed(path, names),
            Response::Failed { message } => picker.reach = Reach::Failed(message),
            _ => {}
        }
    }

    /// Takes what arrived from the servers of other machines, giving back whether anything changed.
    pub(super) fn pump_remotes(&mut self) -> bool {
        let pumped = self.remotes.pump();
        let pushed = !pumped.pushed.is_empty();
        let answered = self.take_remote(pumped.answers);
        for (host, message) in pumped.pushed {
            self.take_pushed(&host, message);
        }
        let changed = answered || pumped.changed || pushed;
        if changed {
            self.rebuild();
        }
        if let Some(picker) = &mut self.workspace.picker
            && let Some(host) = &picker.host
            && let Some(Standing::Lost(reason)) = self.remotes.standing(host)
        {
            picker.reach = Reach::Failed(reason);
        }
        changed
    }

    /// Keeps whether the reader ever saved projects, so the made-up ones stand in while none were.
    pub(super) fn note_saved(&mut self, saved: bool) {
        self.saved = saved;
    }

    /// Shows the made-up projects in the sidebar while the reader has added none and never saved any.
    pub(super) fn show_sample(&mut self) {
        self.workspace.sample = !self.saved && self.paths.is_empty();
        if self.workspace.sample {
            self.workspace.projects = sample_projects();
        }
    }
}
