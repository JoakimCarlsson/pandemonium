//! SSH authentication, remote folder browsing and connection recovery.

use crate::app::App;
use crate::app::Wake;
use crate::panel::PanelView;
use crate::picker::{Choice, Kind, Row};
use pm_core::Scope;
use pm_host::Host;
use pm_host::Location;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A local SSH login terminal whose success permits the endpoint handshake.
pub(super) struct Authentication {
    /// The local terminal showing SSH's own prompts.
    pub shell: crate::terminal::Shell,
    /// The shared remote identity to dial after authentication.
    host: Host,
    /// A directory to open, or none for a reconnect.
    path: Option<PathBuf>,
    /// Whether a terminal pane was visible before authentication.
    terminal_visible: bool,
    /// Whether authentication should open the remote folder browser.
    browsing: bool,
}

/// A remote handshake completed away from the window thread.
pub(super) enum RemoteBack {
    /// A remote directory ready to open.
    Open(Location),
    /// A machine explicitly reconnected.
    Reconnected(Host),
    /// A refused connection or directory.
    Failed(String),
    /// A directory listing ready for the remote folder browser.
    Browse(Location, Vec<PathBuf>),
}

impl App {
    /// Shows file errors without changing buffers or their unsaved state.
    pub(super) fn report_file_errors(&mut self) {
        for error in self.editor.take_troubles() {
            self.notices.trouble(error, None);
        }
    }
    /// Opens an absolute directory on a named SSH host through the project seam.
    pub(super) fn open_remote_project(&mut self, typed: &str) {
        let result = (|| {
            let (host, path) = typed
                .trim()
                .split_once(':')
                .map_or((typed.trim(), None), |(host, path)| (host, Some(path)));
            self.remote_browse = None;
            let Some(path) = path else {
                let host = self
                    .hosts
                    .prepare(host)
                    .map_err(|error| error.to_string())?;
                self.remote_browse = Some(Location::new(host.clone(), PathBuf::new()));
                return self.begin_authentication(host, None, true);
            };
            if !Path::new(path).is_absolute() {
                return Err("The remote project path must be absolute".to_owned());
            }
            let host = self
                .hosts
                .prepare(host)
                .map_err(|error| error.to_string())?;
            self.begin_authentication(host, Some(PathBuf::from(path)), false)
        })();
        if let Err(error) = result {
            self.notices.trouble(error, None);
        }
    }

    /// Starts an SSH login terminal before redialing the active machine.
    pub(super) fn reconnect_project(&mut self) {
        let Some(host) = self
            .open
            .active()
            .map(|project| project.root().host.clone())
        else {
            return;
        };
        if host.is_local() {
            return;
        }
        if let Err(error) = self.begin_authentication(host, None, false) {
            self.notices.trouble(error, None);
        }
    }

    /// Shows system SSH prompts in a local terminal while authentication runs.
    fn begin_authentication(
        &mut self,
        host: Host,
        path: Option<PathBuf>,
        browsing: bool,
    ) -> Result<(), String> {
        if self.authentication.is_some() || host.connecting() {
            return Err("An SSH connection is already authenticating".to_owned());
        }
        if host.connected() && (path.is_some() || browsing) {
            self.dial_remote(host, path, browsing);
            return Ok(());
        }
        host.set_notify(self.waker(Wake::Remote));
        let command = host.authentication().map_err(|error| error.to_string())?;
        let cwd = Location::local(std::env::home_dir().unwrap_or_else(|| PathBuf::from(".")));
        let shell = pm_vt::Terminal::spawn(command, cwd, 100, 8, self.waker(Wake::Terminal))
            .map_err(|error| error.to_string())?;
        self.authentication = Some(Authentication {
            shell: Rc::new(RefCell::new(shell)),
            host,
            path,
            terminal_visible: self.showing_terminals(),
            browsing,
        });
        self.show_panel(PanelView::Terminal);
        self.terminal_focused = true;
        self.editor_focused = false;
        Ok(())
    }

    /// Pumps the login terminal, hides it after exit and starts the handshake.
    pub(super) fn follow_authentication(&mut self) -> bool {
        let Some(auth) = &self.authentication else {
            return false;
        };
        let mut shell = auth.shell.borrow_mut();
        let pumped = shell.pump();
        if shell.is_running() {
            return pumped;
        }
        let code = shell.exit_code();
        let said = shell.text();
        drop(shell);
        let auth = self.authentication.take().unwrap();
        if !auth.terminal_visible
            && let Some(pane) = self.tool_pane(crate::panes::Tool::Terminal)
        {
            self.close_item(pane, self.tool_item(crate::panes::Tool::Terminal));
        }
        self.terminal_focused = false;
        match code {
            Some(0) => self.dial_remote(auth.host, auth.path, auth.browsing),
            _ => {
                auth.host.cancel_authentication();
                self.notices
                    .trouble(format!("SSH authentication failed: {said}"), None);
            }
        }
        true
    }

    /// Performs connection and directory checks away from the window thread.
    fn dial_remote(&mut self, host: Host, path: Option<PathBuf>, browsing: bool) {
        host.set_notify(self.waker(Wake::Remote));
        let back = self.remote_back.clone();
        let wake = self.waker(Wake::Remote);
        std::thread::spawn(move || {
            let result = (|| {
                if !host.connected() || (path.is_none() && !browsing) {
                    host.reconnect().map_err(|error| error.to_string())?;
                }
                if browsing {
                    let path = host
                        .home()
                        .ok_or("The host did not report a home directory")?;
                    let root = Location::new(host, path);
                    return remote_directories(root).map_err(|error| error.to_string());
                }
                match path {
                    Some(path) => {
                        let metadata = host
                            .fs()
                            .metadata(&path)
                            .map_err(|error| error.to_string())?;
                        if !metadata.is_dir() {
                            return Err(format!("{} is not a remote directory", path.display()));
                        }
                        Ok(RemoteBack::Open(Location::new(host, path)))
                    }
                    None => Ok(RemoteBack::Reconnected(host)),
                }
            })();
            back.lock()
                .unwrap()
                .push(result.unwrap_or_else(RemoteBack::Failed));
            wake();
        });
    }

    /// Takes completed handshakes and refreshes every project on that machine.
    pub(super) fn take_remote(&mut self) {
        let finished = std::mem::take(&mut *self.remote_back.lock().unwrap());
        for back in finished {
            match back {
                RemoteBack::Open(location) => {
                    self.remember_host(&location.host);
                    let stored = location.stored();
                    self.preferences
                        .remote_projects
                        .retain(|path| path != &stored);
                    self.preferences.remote_projects.insert(0, stored);
                    self.preferences.remote_projects.truncate(32);
                    match self.open.find_or_open(location) {
                        Ok(project) => {
                            self.point_at(Scope::checkout(project));
                            self.watch_worktrees();
                            self.store();
                        }
                        Err(error) => self.notices.trouble(error.to_string(), None),
                    }
                }
                RemoteBack::Reconnected(host) => {
                    self.editor.reconnect_host(&host);
                    let affected = self
                        .worktrees()
                        .into_iter()
                        .filter(|(_, root)| root.host == host)
                        .map(|(scope, _)| scope)
                        .collect::<Vec<_>>();
                    for scope in affected {
                        self.watchers.remove(&scope);
                        if let Some(tree) = self.files.get_mut(&scope) {
                            tree.reload();
                        }
                    }
                    self.watch_worktrees();
                    self.reread_changes();
                }
                RemoteBack::Failed(error) => self.notices.trouble(error, None),
                RemoteBack::Browse(root, directories) => {
                    self.remember_host(&root.host);
                    let Some(wanted) = self.remote_browse.as_ref() else {
                        continue;
                    };
                    if wanted.host != root.host
                        || (!wanted.path.as_os_str().is_empty() && wanted != &root)
                    {
                        continue;
                    }
                    self.remote_browse = Some(root.clone());
                    let host = root.host.name().unwrap_or_default().to_owned();
                    let mut rows = vec![Row {
                        section: None,
                        label: "Open this folder".to_owned(),
                        detail: format!("{host}:{}", root.display()),
                        choice: Choice::RemoteOpen(host.clone(), root.path.clone()),
                        enabled: true,
                    }];
                    if let Some(parent) = root.parent() {
                        rows.push(Row {
                            section: None,
                            label: "..".to_owned(),
                            detail: parent.display().to_string(),
                            choice: Choice::RemoteDirectory(host.clone(), parent.to_path_buf()),
                            enabled: true,
                        });
                    }
                    rows.extend(directories.into_iter().map(|path| {
                        Row {
                            section: None,
                            label: path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                            detail: path.display().to_string(),
                            choice: Choice::RemoteDirectory(host.clone(), path),
                            enabled: true,
                        }
                    }));
                    self.open_picker_with(Kind::RemoteFolders, rows, String::new());
                }
            }
        }
    }

    /// Retains successfully authenticated aliases through the preferences seam.
    fn remember_host(&mut self, host: &Host) {
        if let Some(name) = host.name()
            && !self
                .preferences
                .remote_hosts
                .iter()
                .any(|saved| saved == name)
        {
            self.preferences.remote_hosts.push(name.to_owned());
            self.store();
        }
    }

    /// Lists saved hosts and projects without reaching a remote machine.
    pub(super) fn remote_host_rows(&self) -> Vec<Row> {
        let mut rows = vec![Row {
            section: None,
            label: "Connect to another host…".to_owned(),
            detail: "SSH alias or host:/absolute/path".to_owned(),
            choice: Choice::RemoteAddress,
            enabled: true,
        }];
        let mut hosts = self
            .preferences
            .remote_hosts
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        for stored in &self.preferences.remote_projects {
            let text = stored.to_string_lossy();
            let Some((host, path)) = text
                .strip_prefix("ssh://")
                .and_then(|text| text.split_once('/'))
            else {
                continue;
            };
            let path = PathBuf::from(format!("/{path}"));
            hosts.insert(host.to_owned());
            rows.push(Row {
                section: Some("Recent projects"),
                label: format!("{host}:{}", path.display()),
                detail: String::new(),
                choice: Choice::RemoteOpen(host.to_owned(), path),
                enabled: true,
            });
        }
        rows.extend(hosts.into_iter().map(|host| Row {
            section: Some("Hosts"),
            label: host.clone(),
            detail: "Browse folders".to_owned(),
            choice: Choice::RemoteHost(host),
            enabled: true,
        }));
        rows
    }

    /// Reads a directory away from the window and discards stale browser replies.
    pub(super) fn browse_remote(&mut self, root: Location) {
        self.remote_browse = Some(root.clone());
        self.open_picker_with(Kind::RemoteFolders, Vec::new(), String::new());
        let back = self.remote_back.clone();
        let wake = self.waker(Wake::Remote);
        std::thread::spawn(move || {
            let result = remote_directories(root)
                .unwrap_or_else(|error| RemoteBack::Failed(error.to_string()));
            back.lock().unwrap().push(result);
            wake();
        });
    }
}

/// Collects child directories through the filesystem belonging to the selected host.
fn remote_directories(root: Location) -> std::io::Result<RemoteBack> {
    let mut directories = root
        .host
        .fs()
        .read_dir(&root)?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_dir())
                .map(|_| entry.path())
        })
        .collect::<Vec<_>>();
    directories.sort();
    Ok(RemoteBack::Browse(root, directories))
}
