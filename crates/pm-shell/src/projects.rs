//! Arranging the sidebar's projects and adding new ones: the project menu,
//! moving and removing projects, the adder with its clone form, and the
//! folder picker that chooses a folder on this machine or another one.

use pm_config::KnownServer;

/// Something the reader does to the sidebar's projects or to the adder and
/// the folder picker.
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectAction {
    /// Opens the menu of the project at this address, or closes it when nothing.
    Menu(Option<String>),
    /// Takes the project at this address out of the sidebar.
    Remove(String),
    /// Moves the project at this address by this many places, up when negative.
    Move(String, i32),
    /// Moves the project at the first place to the second, as a drag does.
    Reorder(usize, usize),
    /// Folds or unfolds the project at this address.
    Fold(String),
    /// Opens the adder at this step, or closes it when nothing.
    Adding(Option<AddStep>),
    /// Changes the address the clone form fetches from.
    CloneUrl(String),
    /// Clones the address the clone form holds and adds the clone.
    Clone,
    /// Changes the folder the clone form clones into; the usual folder for clones when empty.
    CloneUnder(String),
    /// Chooses the machine the clone form clones on, by host, or this one when nothing.
    CloneOn(Option<String>),
    /// Lists in the clone form the GitHub repositories of the account or organization with this login.
    CloneOwner(String),
    /// Does something in the folder picker.
    Picker(PickerAction),
}

/// What the adder shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AddStep {
    /// The choice between a folder and a clone.
    #[default]
    Choose,
    /// The form that clones a repository.
    Clone,
}

/// The adder: the popover that adds a project from a folder or a clone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Adder {
    /// What it shows.
    pub step: AddStep,
    /// The address a clone is fetched from.
    pub url: String,
    /// Whether a clone is under way.
    pub busy: bool,
    /// Why the last clone failed, when it did.
    pub error: Option<String>,
    /// The folder the clone goes in, or empty for the usual folder for clones.
    pub under: String,
    /// The machine the clone is made on, by host, or nothing for this one.
    pub machine: Option<String>,
    /// The machines a clone can be made on, this one first.
    pub machines: Vec<Machine>,
    /// The GitHub accounts the chosen machine is signed in to, the account first and then its organizations, as login and whether it is an organization; nothing until it answered.
    pub owners: Option<Vec<(String, bool)>>,
    /// The login of the GitHub account or organization whose repositories are listed.
    pub owner: Option<String>,
    /// The repositories of that owner, once listed.
    pub repositories: Option<Vec<GithubRepository>>,
    /// Why the GitHub accounts or repositories could not be listed, such as `gh` not being signed in.
    pub github_error: Option<String>,
}

/// A GitHub repository the clone form offers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GithubRepository {
    /// The owner and the name, as `owner/name`.
    pub name: String,
    /// What its owner wrote about it.
    pub description: String,
    /// Whether it is private.
    pub private: bool,
    /// The address to clone it from.
    pub url: String,
}

impl Adder {
    /// The repositories listed whose name holds what the address field holds, all of them while it holds an address of its own.
    pub fn matching_repositories(&self) -> Vec<&GithubRepository> {
        let typed = self.url.trim().to_lowercase();
        let filtering = !typed.contains("://") && !typed.starts_with("git@");
        self.repositories
            .iter()
            .flatten()
            .filter(|repository| !filtering || repository.name.to_lowercase().contains(&typed))
            .collect()
    }
}

/// Something the reader does in the folder picker.
#[derive(Clone, Debug, PartialEq)]
pub enum PickerAction {
    /// Opens the picker on the machine with this host, or this one when nothing.
    Open(Option<String>),
    /// Opens the picker on this machine to choose a folder to install an extension from.
    OpenForExtension,
    /// Looks inside the folder at this absolute path.
    Go(String),
    /// Looks inside the folder of this name within the one shown.
    Into(String),
    /// Goes up to the folder holding the one shown.
    Up,
    /// Goes back to the folder shown before.
    Back,
    /// Goes forward again to the folder left by going back.
    Forward,
    /// Filters the folders by this text.
    Filter(String),
    /// Takes the folder shown and adds it as a project.
    Choose,
    /// Closes the picker without choosing.
    Close,
}

/// How the folder picker stands with the machine it looks at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Reach {
    /// The machine's server is being reached or asked.
    #[default]
    Waiting,
    /// The machine's server answered.
    Reached,
    /// The machine's server cannot be reached or refused, for this reason.
    Failed(String),
}

/// A machine the folder picker can look at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Machine {
    /// What it is called.
    pub name: String,
    /// The part of a project's address before its folder, or nothing for this machine.
    pub host: Option<String>,
}

/// The folder picker: a window of its own that chooses a folder on the machine a server runs on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FolderPicker {
    /// The machine looked at, by host, or nothing for this one.
    pub host: Option<String>,
    /// The machines it can look at, this one first.
    pub machines: Vec<Machine>,
    /// The folder shown, as an absolute path once the server answered; `~` before.
    pub path: String,
    /// The names of the folders inside it, in order.
    pub names: Vec<String>,
    /// The text the folders are filtered by.
    pub filter: String,
    /// The folders shown before, newest last.
    pub back: Vec<String>,
    /// The folders left by going back, newest last.
    pub forward: Vec<String>,
    /// How it stands with the machine.
    pub reach: Reach,
    /// Whether the folder chosen is an extension to install rather than a project to add.
    pub for_extension: bool,
}

impl FolderPicker {
    /// A picker on the machine `host`, or this one, offering `known` besides this machine.
    pub fn open(host: Option<String>, known: &[KnownServer]) -> Self {
        let mut machines = vec![Machine {
            name: "This machine".to_owned(),
            host: None,
        }];
        machines.extend(known.iter().map(|server| Machine {
            name: server.name.clone(),
            host: Some(server.host()),
        }));
        Self {
            host,
            machines,
            path: "~".to_owned(),
            ..Self::default()
        }
    }

    /// The names of the folders the filter lets through.
    pub fn shown(&self) -> Vec<&str> {
        let filter = self.filter.to_lowercase();
        self.names
            .iter()
            .map(String::as_str)
            .filter(|name| name.to_lowercase().contains(&filter))
            .collect()
    }

    /// The path the picker moves to for `action`, remembering where it was, or nothing when it does not move.
    pub fn step(&mut self, action: &PickerAction) -> Option<String> {
        let target = match action {
            PickerAction::Go(path) => path.clone(),
            PickerAction::Into(name) => child_of(&self.path, name),
            PickerAction::Up => parent_folder(&self.path)?,
            PickerAction::Back => {
                let target = self.back.pop()?;
                self.forward.push(self.path.clone());
                return Some(self.moved_to(target));
            }
            PickerAction::Forward => {
                let target = self.forward.pop()?;
                self.back.push(self.path.clone());
                return Some(self.moved_to(target));
            }
            _ => return None,
        };
        if target == self.path {
            return None;
        }
        self.back.push(self.path.clone());
        self.forward.clear();
        Some(self.moved_to(target))
    }

    /// Shows `path` as waiting for its folders and gives it back.
    fn moved_to(&mut self, path: String) -> String {
        self.path = path.clone();
        self.names.clear();
        self.filter.clear();
        self.reach = Reach::Waiting;
        path
    }

    /// Takes the server's answer that `path` holds `names`.
    pub fn listed(&mut self, path: String, names: Vec<String>) {
        if self.path == "~" || self.reach == Reach::Waiting {
            self.path = path;
        }
        self.names = names;
        self.reach = Reach::Reached;
    }

    /// The address of the folder shown as a project: the folder on this machine, or `host/folder` on another.
    pub fn address(&self) -> String {
        match &self.host {
            None => self.path.clone(),
            Some(host) => join_host(host, &self.path),
        }
    }

    /// The steps of the path shown, each as its name and the path it leads to.
    pub fn trail(&self) -> Vec<(String, String)> {
        let mut steps = vec![("/".to_owned(), "/".to_owned())];
        let mut path = String::new();
        for part in self.path.split('/').filter(|part| !part.is_empty()) {
            path = format!("{path}/{part}");
            steps.push((part.to_owned(), path.clone()));
        }
        steps
    }
}

/// The address of `folder` on the machine `host`: `ssh://` added before a bare ssh destination.
pub fn join_host(host: &str, folder: &str) -> String {
    let folder = folder.trim_start_matches('/');
    if host.contains("://") {
        format!("{host}/{folder}")
    } else {
        format!("{}{host}/{folder}", pm_protocol::SSH_SCHEME)
    }
}

/// The path of the folder `name` inside `path`.
fn child_of(path: &str, name: &str) -> String {
    format!("{}/{name}", path.trim_end_matches('/'))
}

/// The folder holding `path`, or nothing at the root.
fn parent_folder(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let (parent, _) = trimmed.rsplit_once('/')?;
    Some(if parent.is_empty() { "/" } else { parent }.to_owned())
}

/// `paths` with the one at `from` moved to `to`, both kept within the list.
pub fn reordered(mut paths: Vec<String>, from: usize, to: usize) -> Vec<String> {
    if from >= paths.len() {
        return paths;
    }
    let to = to.min(paths.len() - 1);
    let moved = paths.remove(from);
    paths.insert(to, moved);
    paths
}

/// `paths` with the one equal to `path` moved `by` places, kept within the list.
pub fn moved(paths: Vec<String>, path: &str, by: i32) -> Vec<String> {
    let Some(from) = paths.iter().position(|kept| kept == path) else {
        return paths;
    };
    let to = (from as i64 + by as i64).clamp(0, paths.len() as i64 - 1) as usize;
    reordered(paths, from, to)
}

/// The folder a clone of `url` is named after: its last part without `.git`.
pub fn clone_name(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(url)
        .trim_end_matches(".git")
        .to_owned()
}

/// `url` as an address git can fetch: `owner/name` becomes a GitHub address.
pub fn clone_address(url: &str) -> String {
    let url = url.trim();
    let shorthand =
        !url.contains(':') && url.split('/').filter(|part| !part.is_empty()).count() == 2;
    if shorthand {
        format!("https://github.com/{}", url.trim_matches('/'))
    } else {
        url.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paths made of `names`.
    fn paths(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    /// Moving and reordering keep every project and stay within the list.
    #[test]
    fn moves_projects_within_the_list() {
        let list = paths(&["a", "b", "c"]);
        assert_eq!(moved(list.clone(), "a", 1), paths(&["b", "a", "c"]));
        assert_eq!(moved(list.clone(), "a", -1), list);
        assert_eq!(moved(list.clone(), "c", 5), list);
        assert_eq!(reordered(list.clone(), 2, 0), paths(&["c", "a", "b"]));
        assert_eq!(reordered(list.clone(), 9, 0), list);
    }

    /// The picker goes into folders, up, back and forward, remembering where it was.
    #[test]
    fn picker_walks_folders() {
        let mut picker = FolderPicker::open(None, &[]);
        picker.listed("/home/me".to_owned(), paths(&["code", "docs"]));
        assert_eq!(
            picker.step(&PickerAction::Into("code".to_owned())),
            Some("/home/me/code".to_owned())
        );
        assert_eq!(picker.step(&PickerAction::Up), Some("/home/me".to_owned()));
        assert_eq!(
            picker.step(&PickerAction::Back),
            Some("/home/me/code".to_owned())
        );
        assert_eq!(
            picker.step(&PickerAction::Forward),
            Some("/home/me".to_owned())
        );
        assert_eq!(parent_folder("/home"), Some("/".to_owned()));
        assert_eq!(parent_folder("/"), None);
    }

    /// The filter keeps the folders whose names hold its text, ignoring case.
    #[test]
    fn picker_filters_names() {
        let mut picker = FolderPicker::open(None, &[]);
        picker.listed("/".to_owned(), paths(&["Code", "docs", "decode"]));
        picker.filter = "code".to_owned();
        assert_eq!(picker.shown(), vec!["Code", "decode"]);
    }

    /// A folder on another machine is addressed through its host.
    #[test]
    fn addresses_remote_folders() {
        let mut picker = FolderPicker::open(Some("devbox".to_owned()), &[]);
        picker.listed("/srv/app".to_owned(), Vec::new());
        assert_eq!(picker.address(), "ssh://devbox/srv/app");
        assert_eq!(join_host("wsl://Ubuntu", "/x"), "wsl://Ubuntu/x");
    }

    /// Clone addresses accept GitHub's `owner/name` and name the folder after the repository.
    #[test]
    fn reads_clone_addresses() {
        assert_eq!(
            clone_address("rust-lang/rust"),
            "https://github.com/rust-lang/rust"
        );
        assert_eq!(
            clone_address("git@github.com:a/b.git"),
            "git@github.com:a/b.git"
        );
        assert_eq!(clone_name("https://github.com/a/b.git"), "b");
        assert_eq!(clone_name("git@github.com:a/b"), "b");
    }
}
