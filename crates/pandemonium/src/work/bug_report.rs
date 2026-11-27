//! Gathering a bug report: this window's facts and crash log, and the files every server it reaches gives, saved as one zip on this machine.

use std::collections::HashSet;

use pm_components::bug_report::BugReport;
use pm_protocol::{DiagnosticFile, Request};
use pm_server::diagnostics;

use crate::model::Model;

use super::{Host, Pending, Work};

/// A bug report being gathered: the servers still to answer and what those that did gave.
pub(super) struct Gathering {
    /// The machines whose servers have not answered yet.
    waiting: HashSet<Host>,
    /// What each machine that answered gave, or why it gave nothing, in the order they answered.
    machines: Vec<(Host, Result<Vec<DiagnosticFile>, String>)>,
}

impl Work {
    /// Starts gathering a bug report from the server of this machine and of every other the window reaches, unless one is being gathered already.
    pub(super) fn report_bug(&mut self, model: &Model) {
        if self.gathering.is_some() {
            return;
        }
        let hosts = self.usage_hosts();
        self.gathering = Some(Gathering {
            waiting: hosts.iter().cloned().collect(),
            machines: Vec::new(),
        });
        for host in hosts {
            self.ask(
                &host,
                Request::Diagnostics,
                Pending::Diagnostics(host.clone()),
            );
        }
        if self.offline.is_some() {
            self.diagnosed(model, &None, Err("the window reaches no server".to_owned()));
        }
    }

    /// Takes `answer`, what the server of `host` gave for the bug report being gathered, and saves the report once every server has answered, showing where it went or why it could not be saved.
    pub(super) fn diagnosed(
        &mut self,
        model: &Model,
        host: &Host,
        answer: Result<Vec<DiagnosticFile>, String>,
    ) {
        let Some(gathering) = self.gathering.as_mut() else {
            return;
        };
        if !gathering.waiting.remove(host) {
            return;
        }
        gathering.machines.push((host.clone(), answer));
        if !gathering.waiting.is_empty() && self.offline.is_none() {
            return;
        }
        let Some(gathering) = self.gathering.take() else {
            return;
        };
        let shown = match save(gathering) {
            Ok(path) => BugReport::Saved {
                path: path.display().to_string(),
            },
            Err(reason) => BugReport::Failed { reason },
        };
        model.change(|state| state.with_bug_report(shown));
    }
}

/// Writes the report `gathering` holds, with this window's own files first, into the downloads folder of this machine, and returns the zip's path.
///
/// # Errors
///
/// Fails when the machine has no folder to save it in or the zip cannot be written.
fn save(gathering: Gathering) -> Result<std::path::PathBuf, String> {
    let folder = pm_store::downloads_folder()
        .ok_or_else(|| "this machine has no downloads or home folder".to_owned())?;
    let mut machines = vec![(
        "window".to_owned(),
        Ok(diagnostics::files(diagnostics::WINDOW)),
    )];
    machines.extend(gathering.machines.into_iter().map(|(host, answer)| {
        let name = host.map_or_else(
            || "server on this machine".to_owned(),
            |host| format!("server on {host}"),
        );
        (name, answer)
    }));
    diagnostics::write_report(diagnostics::WINDOW, machines, &folder)
        .map_err(|error| format!("it could not be written to {}: {error}", folder.display()))
}

#[cfg(test)]
/// Tests for gathering a bug report.
mod tests {
    use super::*;
    use crate::work::Link;
    use pm_pages::{Screen, WorkspaceState};

    /// Every server reached is asked, the report waits for all of them, and a server that could not answer is noted in the saved zip.
    #[test]
    fn waits_for_every_server_then_saves() {
        let local = None;
        let remote = Some("builder".to_owned());
        let model = Model::new(Screen::Workspace(WorkspaceState::new(&[])));
        let mut work = Work::new(std::sync::Arc::new(|| {}));
        for host in [&local, &remote] {
            work.links.insert(host.clone(), Link::default());
        }
        work.report_bug(&model);
        for host in [&local, &remote] {
            assert!(
                work.links[host]
                    .backlog
                    .iter()
                    .any(|(request, pending)| matches!(
                        (request, pending),
                        (Request::Diagnostics, Pending::Diagnostics(asked)) if asked == host
                    ))
            );
        }
        let shown = |model: &Model| model.read(|screen| screen.bug_report.clone());
        work.diagnosed(&model, &local, Ok(diagnostics::files(diagnostics::SERVER)));
        assert_eq!(shown(&model), None);
        work.diagnosed(&model, &remote, Err("out of reach".to_owned()));
        let Some(BugReport::Saved { path }) = shown(&model) else {
            panic!("the report was not saved: {:?}", shown(&model));
        };
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("window/facts.json"));
        assert!(text.contains("server on this machine/facts.json"));
        assert!(text.contains("server on builder/trouble.txt"));
        assert!(work.gathering.is_none());
    }
}
