//! Opening remote projects, explicit reconnects and milestone restrictions.

use crate::app::App;
use crate::app::Wake;
use crate::panel::PanelView;
use crate::picker::{Choice, Row};
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
    /// Whether the bottom panel was showing before authentication.
    panel_open: bool,
    /// The previous bottom panel view.
    panel_view: PanelView,
}

/// A remote handshake completed away from the window thread.
pub(super) enum RemoteBack {
    /// A remote directory ready to open.
    Open(Location),
    /// A machine explicitly reconnected.
    Reconnected(Host),
    /// A refused connection or directory.
    Failed(String),
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
                .ok_or_else(|| "Use host:/absolute/path".to_owned())?;
            if !Path::new(path).is_absolute() {
                return Err("The remote project path must be absolute".to_owned());
            }
            let host = self
                .hosts
                .prepare(host)
                .map_err(|error| error.to_string())?;
            self.begin_authentication(host, Some(PathBuf::from(path)))
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
        if let Err(error) = self.begin_authentication(host, None) {
            self.notices.trouble(error, None);
        }
    }

    /// Shows system SSH prompts in a local terminal while authentication runs.
    fn begin_authentication(&mut self, host: Host, path: Option<PathBuf>) -> Result<(), String> {
        if self.authentication.is_some() {
            return Err("An SSH connection is already authenticating".to_owned());
        }
        if host.connected() && path.is_some() {
            self.dial_remote(host, path);
            return Ok(());
        }
        let command = host.authentication().map_err(|error| error.to_string())?;
        let cwd = Location::local(std::env::home_dir().unwrap_or_else(|| PathBuf::from(".")));
        let shell = pm_vt::Terminal::spawn(command, cwd, 100, 8, self.waker(Wake::Terminal))
            .map_err(|error| error.to_string())?;
        self.authentication = Some(Authentication {
            shell: Rc::new(RefCell::new(shell)),
            host,
            path,
            panel_open: self.bottom_panel_open,
            panel_view: self.panel_view,
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
        self.bottom_panel_open = auth.panel_open;
        self.panel_view = auth.panel_view;
        self.terminal_focused = false;
        match code {
            Some(0) => self.dial_remote(auth.host, auth.path),
            _ => self
                .notices
                .trouble(format!("SSH authentication failed: {said}"), None),
        }
        true
    }

    /// Performs connection and directory checks away from the window thread.
    fn dial_remote(&mut self, host: Host, path: Option<PathBuf>) {
        let back = self.remote_back.clone();
        let wake = self.waker(Wake::Remote);
        std::thread::spawn(move || {
            let result = (|| {
                if !host.connected() || path.is_none() {
                    host.reconnect().map_err(|error| error.to_string())?;
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
                RemoteBack::Open(location) => match self.open.find_or_open(location) {
                    Ok(project) => {
                        self.point_at(Scope::checkout(project));
                        self.watch_worktrees();
                        self.store();
                    }
                    Err(error) => self.notices.trouble(error.to_string(), None),
                },
                RemoteBack::Reconnected(host) => {
                    let affected = self
                        .open
                        .iter()
                        .filter(|project| project.root().host == host)
                        .map(|project| Scope::checkout(project.id()))
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
            }
        }
    }

    /// Whether a scope belongs to a remote project.
    pub(super) fn is_remote(&self, scope: Scope) -> bool {
        self.open
            .get(scope.project())
            .is_some_and(|project| !project.root().host.is_local())
    }

    /// Refuses a feature that has not yet been enabled for remote projects.
    pub(super) fn refuse_remote(&mut self, scope: Scope, feature: &str) -> bool {
        if !self.is_remote(scope) {
            return false;
        }
        self.notices.trouble(
            format!("{feature} on remote projects are not supported yet"),
            None,
        );
        true
    }

    /// A disabled picker row explaining a remote milestone restriction.
    pub(super) fn unsupported_row(&self, feature: &str) -> Vec<Row> {
        vec![Row {
            section: None,
            label: format!("{feature} on remote projects are not supported yet"),
            detail: String::new(),
            choice: Choice::Act(crate::keymap::Action::Cancel),
            enabled: false,
        }]
    }
}
