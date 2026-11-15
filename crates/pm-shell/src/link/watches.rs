//! The folders the window has the servers watch: the projects, the folder
//! the Files tab lists and the Git panel's folder, each sent to the server
//! of the machine it is on, and the file tree listed again when one of them
//! changes outside the app.

use std::collections::BTreeMap;

use pm_protocol::Request;

use super::{Ask, Link, expand_home};
use crate::model::RightTab;

impl Link {
    /// The folders the window wants watched, by the host of the machine they
    /// are on (`None` for this one), each as that machine's server names it.
    fn wanted_watches(&self) -> BTreeMap<Option<String>, Vec<String>> {
        let mut wanted: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
        let addresses = self
            .paths
            .iter()
            .map(|path| expand_home(path))
            .chain(std::iter::once(self.workspace.tree.root.clone()))
            .chain(std::iter::once(self.workspace.git.folder.clone()))
            .filter(|address| !address.is_empty());
        for address in addresses {
            let (host, folder) = match pm_protocol::host_of(&address) {
                Some((host, folder)) => (
                    Some(host.to_owned()),
                    format!("/{}", folder.trim_start_matches('/')),
                ),
                None => (None, address),
            };
            let folders = wanted.entry(host).or_default();
            if !folders.contains(&folder) {
                folders.push(folder);
            }
        }
        wanted
    }

    /// Tells every server whose share of the watched folders changed which
    /// folders to watch now, and the ones no longer concerned to watch none.
    pub(super) fn sync_watches(&mut self) {
        let wanted = self.wanted_watches();
        let gone: Vec<Option<String>> = self
            .folder_watches
            .keys()
            .filter(|host| !wanted.contains_key(*host))
            .cloned()
            .collect();
        for host in gone {
            self.folder_watches.remove(&host);
            self.send_watches(host.as_deref(), Vec::new());
        }
        for (host, folders) in wanted {
            if self.folder_watches.get(&host) == Some(&folders) {
                continue;
            }
            self.folder_watches.insert(host.clone(), folders.clone());
            self.send_watches(host.as_deref(), folders);
        }
    }

    /// Has the server of `host`, or this machine's for `None`, watch `directories`.
    fn send_watches(&mut self, host: Option<&str>, directories: Vec<String>) {
        let request = Request::WatchFolders { directories };
        match host {
            Some(host) => self.ask_remote(host, request, Ask::Nothing),
            None => {
                if let Some(client) = &self.client {
                    let id = client.request(request);
                    self.asks.insert(id, Ask::Nothing);
                }
            }
        }
    }

    /// Forgets what the server of `host`, or this machine's for `None`,
    /// watches, after it let the window in again, so it is told once more.
    pub(super) fn forget_watches(&mut self, host: Option<String>) {
        self.folder_watches.remove(&host);
    }

    /// Lists the Files tab's folder again when `directory`, which changed,
    /// holds it or lies inside it; a tree not shown is listed again once it shows.
    pub(super) fn tree_folder_changed(&mut self, directory: &str) {
        let root = self.workspace.tree.root.clone();
        if root.is_empty() || !overlaps(&root, directory) {
            return;
        }
        if self.workspace.right_tab == RightTab::Files {
            self.load_files();
        } else {
            self.workspace.tree.loaded = false;
        }
    }
}

/// Whether one of the folders `a` and `b` is the other or lies inside it.
fn overlaps(a: &str, b: &str) -> bool {
    let a = a.trim_end_matches('/');
    let b = b.trim_end_matches('/');
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

#[cfg(test)]
mod tests {
    use super::overlaps;

    /// Folders overlap when equal or nested either way, not when they only share a prefix.
    #[test]
    fn folders_overlap_when_nested() {
        assert!(overlaps("/a/b", "/a/b/"));
        assert!(overlaps("/a/b/c", "/a/b"));
        assert!(overlaps("/a", "/a/b"));
        assert!(!overlaps("/a/bc", "/a/b"));
    }
}
