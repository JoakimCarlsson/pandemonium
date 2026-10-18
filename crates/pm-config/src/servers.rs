//! The servers the window knows besides the one on this machine, and which one new projects are added on.

use serde::{Deserialize, Serialize};

/// How the window reaches a server.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    /// Straight to its address, over a network that is trusted, with the token it was started with.
    #[default]
    Direct,
    /// Through ssh to a machine where it already runs.
    Ssh,
    /// Inside a Windows Subsystem for Linux distribution, where it is started when it is not running.
    Wsl,
    /// Inside the dev container of a folder on this machine, which is built and started first.
    Container,
    /// Through a tunnel that the server's machine makes outward, so that it opens no port.
    Tunnel,
}

impl Reach {
    /// Whether a server reached this way is let in with a token the window keeps, rather than one it reads from the machine.
    pub fn keeps_token(self) -> bool {
        matches!(self, Self::Direct | Self::Tunnel)
    }
}

/// A server on another machine the reader told the window about.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KnownServer {
    /// What the reader calls it.
    pub name: String,
    /// How it is reached.
    pub reach: Reach,
    /// Where it is: `host:port` for a direct one, the ssh destination for one reached over ssh, the distribution's name for one in WSL, the folder on this machine for one in a dev container, the tunnel's hostname for one behind a tunnel.
    pub address: String,
    /// The token a direct one or one behind a tunnel wants; the others read it from the machine.
    pub token: String,
}

impl KnownServer {
    /// The machine a project on it is addressed by, as the part of a project's address before its folder.
    pub fn host(&self) -> String {
        match self.reach {
            Reach::Direct => format!("{}{}", pm_protocol::DIRECT_SCHEME, self.address),
            Reach::Ssh => self.address.clone(),
            Reach::Wsl => format!("{}{}", pm_protocol::WSL_SCHEME, self.address),
            Reach::Container => format!(
                "{}{}",
                pm_protocol::CONTAINER_SCHEME,
                pm_protocol::escape_folder(&self.address)
            ),
            Reach::Tunnel => format!("{}{}", pm_protocol::TUNNEL_SCHEME, self.address),
        }
    }

    /// The address of `folder` on it.
    pub fn project(&self, folder: &str) -> String {
        let folder = folder.trim_start_matches('/');
        let host = match self.reach {
            Reach::Ssh => format!("{}{}", pm_protocol::SSH_SCHEME, self.address),
            _ => self.host(),
        };
        format!("{host}/{folder}")
    }
}

/// The servers the window knows besides this machine's, and where new projects go.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerList {
    /// The servers, in the order they were added.
    pub known: Vec<KnownServer>,
    /// The server new projects are added on, by name, or this machine when nothing.
    pub default_server: Option<String>,
}

impl ServerList {
    /// The server new projects are added on, or nothing for this machine.
    pub fn default_target(&self) -> Option<&KnownServer> {
        let name = self.default_server.as_deref()?;
        self.known.iter().find(|server| server.name == name)
    }

    /// The token of the server reached straight at `address`.
    pub fn token_of(&self, address: &str) -> Option<&str> {
        self.known
            .iter()
            .find(|server| server.reach.keeps_token() && server.address == address)
            .map(|server| server.token.as_str())
    }
}
