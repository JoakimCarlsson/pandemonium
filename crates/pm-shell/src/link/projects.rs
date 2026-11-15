//! The link's part in arranging the sidebar's projects and adding new ones:
//! saves the projects' order and removals on the server, clones
//! repositories, and lists folders for the folder picker on this machine or
//! on the machine the picker looks at.

use std::collections::HashMap;

use pm_protocol::{Request, Response};

use super::remote::Standing;
use super::{Ask, Link};
use crate::projects::{
    AddStep, Adder, FolderPicker, GithubRepository, PickerAction, ProjectAction, Reach,
    clone_address, moved, reordered,
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
                let servers = &self.settings.window.servers;
                let machines = FolderPicker::open(None, &servers.known).machines;
                let first = Adder {
                    machine: servers.default_target().map(pm_config::KnownServer::host),
                    ..Adder::default()
                };
                let cloning = step == Some(AddStep::Clone);
                self.workspace.adder = step.map(|step| Adder {
                    step,
                    machines,
                    ..self.workspace.adder.clone().unwrap_or(first)
                });
                if cloning {
                    self.ask_owners();
                }
            }
            ProjectAction::CloneUnder(under) => {
                let adder = self.workspace.adder.get_or_insert_with(Adder::default);
                adder.under = under;
                adder.error = None;
            }
            ProjectAction::CloneOn(machine) => {
                let adder = self.workspace.adder.get_or_insert_with(Adder::default);
                adder.machine = machine;
                adder.error = None;
                adder.owners = None;
                adder.owner = None;
                adder.repositories = None;
                self.ask_owners();
            }
            ProjectAction::CloneOwner(owner) => self.ask_repositories(owner),
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

    /// Clones the address the adder holds into the folder it names, or the usual folder for clones, on the machine it names, adding the clone once it is made.
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
        let under = Some(adder.under.trim().to_owned()).filter(|under| !under.is_empty());
        let request = Request::Clone { url, under };
        match adder.machine.clone() {
            Some(host) => self.ask_remote(&host, request, Ask::Cloned),
            None => self.ask(request, Ask::Cloned),
        }
    }

    /// Asks the machine the clone form clones on for the GitHub accounts it is signed in to.
    fn ask_owners(&mut self) {
        let Some(adder) = &mut self.workspace.adder else {
            return;
        };
        adder.github_error = None;
        let machine = adder.machine.clone();
        self.ask_on(machine.clone(), Request::Owners, Ask::Owners(machine));
    }

    /// Lists in the clone form the GitHub repositories of the account or organization `owner`.
    fn ask_repositories(&mut self, owner: String) {
        let Some(adder) = &mut self.workspace.adder else {
            return;
        };
        let organization = adder
            .owners
            .iter()
            .flatten()
            .any(|(login, organization)| *login == owner && *organization);
        adder.owner = Some(owner.clone());
        adder.repositories = None;
        adder.github_error = None;
        let machine = adder.machine.clone();
        self.ask_on(
            machine.clone(),
            Request::Repositories {
                owner: owner.clone(),
                organization,
            },
            Ask::Repositories(machine, owner),
        );
    }

    /// Sends `request` for `ask` to the server of the machine with host `machine`, or of this one when nothing.
    fn ask_on(&mut self, machine: Option<String>, request: Request, ask: Ask) {
        match machine {
            Some(host) => self.ask_remote(&host, request, ask),
            None => self.ask(request, ask),
        }
    }

    /// Takes the answer of the machine with host `machine` about the GitHub accounts it is signed in to, and lists the repositories of the first.
    pub(super) fn answer_owners(&mut self, machine: Option<String>, response: Response) {
        let Some(adder) = self
            .workspace
            .adder
            .as_mut()
            .filter(|adder| adder.machine == machine)
        else {
            return;
        };
        match response {
            Response::Owners { owners } => {
                let first = owners.first().map(|owner| owner.login.clone());
                adder.owners = Some(
                    owners
                        .into_iter()
                        .map(|owner| (owner.login, owner.organization))
                        .collect(),
                );
                if let Some(first) = first {
                    self.ask_repositories(first);
                }
            }
            Response::Failed { message } => {
                adder.owners = Some(Vec::new());
                adder.github_error = Some(message);
            }
            _ => {}
        }
    }

    /// Takes the answer of the machine with host `machine` about the repositories of `owner`.
    pub(super) fn answer_repositories(
        &mut self,
        machine: Option<String>,
        owner: String,
        response: Response,
    ) {
        let Some(adder) = self
            .workspace
            .adder
            .as_mut()
            .filter(|adder| adder.machine == machine && adder.owner.as_ref() == Some(&owner))
        else {
            return;
        };
        match response {
            Response::Repositories { repositories } => {
                adder.repositories = Some(
                    repositories
                        .into_iter()
                        .map(|repository| GithubRepository {
                            name: repository.name,
                            description: repository.description,
                            private: repository.private,
                            url: repository.url,
                        })
                        .collect(),
                );
            }
            Response::Failed { message } => {
                adder.repositories = Some(Vec::new());
                adder.github_error = Some(message);
            }
            _ => {}
        }
    }

    /// Takes the server's answer to a clone, adding the clone as a project on the machine it was made on.
    pub(super) fn answer_clone(&mut self, response: Response) {
        match response {
            Response::Directory { path } => {
                let machine = self.workspace.adder.take().and_then(|adder| adder.machine);
                let address = match machine {
                    Some(host) => super::routes::address_on(&host, &path),
                    None => path,
                };
                self.add_project(address);
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
            PickerAction::OpenForExtension => {
                self.workspace.adder = None;
                let mut picker = FolderPicker::open(None, &[]);
                picker.for_extension = true;
                self.workspace.picker = Some(picker);
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
                    match picker.for_extension {
                        true => self.inspect_extension(picker.path),
                        false => self.add_project(picker.address()),
                    }
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
