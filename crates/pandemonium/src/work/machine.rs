//! What the reader set up for the work the servers do: read from the server on this machine, sent to every server the window reaches whenever it changes and to each server as it is reached, and the newest release looked up.

use pm_components::update::Update;
use pm_protocol::{MachineSettings, Request, Response};
use pm_state::State;

use super::{Host, Pending, Work};
use pm_pages::Screen;

impl Work {
    /// Asks the server on this machine what it keeps for the work the servers do.
    pub fn ask_machine_settings(&mut self) {
        self.ask(&None, Request::MachineSettings, Pending::MachineSettings);
    }

    /// Keeps `settings` as what every server is to work by, and sends them to each server the window reaches.
    pub(super) fn save_machine_settings(&mut self, settings: MachineSettings) {
        self.machine = Some(settings.clone());
        let hosts: Vec<Host> = self.links.keys().cloned().collect();
        for host in hosts {
            self.ask(
                &host,
                Request::SaveMachineSettings {
                    settings: settings.clone(),
                },
                Pending::Nothing,
            );
        }
    }

    /// The request that hands a server on another machine, `host`, what the servers are to work by, once the window knows it; the server on this machine is where it is read from, so it is handed nothing.
    pub(super) fn settings_for(&self, host: &Host) -> Option<Request> {
        host.as_ref()?;
        self.machine
            .clone()
            .map(|settings| Request::SaveMachineSettings { settings })
    }

    /// Asks the server on this machine for the newest release on `channel`.
    pub fn ask_release(&mut self, channel: pm_config::Channel) {
        let nightly = channel == pm_config::Channel::Nightly;
        let channel = if nightly { "nightly" } else { "stable" };
        self.ask(
            &None,
            Request::LatestRelease {
                channel: channel.to_owned(),
                running: Some(pm_components::update::RUNNING.to_owned()),
            },
            Pending::LatestRelease(nightly),
        );
    }

    /// Hands the workspace what looking for a newer release last found, when anything was.
    pub fn show_release(&self, model: &State<Screen>) {
        if let Some(update) = self.release.clone() {
            Self::in_workspace(model, |state| state.with_release(update));
        }
    }

    /// Learns the tokens of the servers reached straight at their address or through a tunnel, from the servers the window knows.
    pub fn set_servers(&mut self, servers: &pm_config::ServerList) {
        self.tokens = servers
            .known
            .iter()
            .filter(|server| server.reach.keeps_token())
            .map(|server| (server.address.clone(), server.token.clone()))
            .collect();
    }

    /// Takes in the answer to asking for what the servers work by, or for the newest release.
    pub(super) fn answered_machine(
        &mut self,
        model: &State<Screen>,
        pending: Pending,
        response: Response,
    ) {
        match (pending, response) {
            (Pending::MachineSettings, Response::MachineSettings { settings }) => {
                self.machine = Some(settings.clone());
                let hosts: Vec<Host> = self
                    .links
                    .keys()
                    .filter(|host| host.is_some())
                    .cloned()
                    .collect();
                for host in hosts {
                    if let Some(request) = self.settings_for(&host) {
                        self.ask(&host, request, Pending::Nothing);
                    }
                }
                Self::in_workspace(model, |state| state.with_machine_settings(settings));
            }
            (
                Pending::LatestRelease(nightly),
                Response::LatestRelease {
                    version,
                    url,
                    commit,
                },
            ) => {
                self.release = Some(Update::judged(nightly, version, commit, url));
                self.show_release(model);
            }
            (Pending::LatestRelease(_), _) => {
                self.release = Some(Update::Unknown);
                self.show_release(model);
            }
            _ => {}
        }
    }
}
