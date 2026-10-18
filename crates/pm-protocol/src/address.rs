//! The addresses of projects on other machines.

/// What an address starts with when the folder is on a machine reached over ssh.
pub const SSH_SCHEME: &str = "ssh://";

/// What an address starts with when the folder is on a machine whose server is reached straight at its address, `tcp://host:port/folder`.
pub const DIRECT_SCHEME: &str = "tcp://";

/// Whether `destination` can safely be handed to ssh: it is not empty, does not start with a dash that ssh would read as an option, and holds no whitespace or control characters.
pub fn is_destination(destination: &str) -> bool {
    !destination.is_empty()
        && !destination.starts_with('-')
        && !destination
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

/// What an address starts with when the folder is in a Windows Subsystem for Linux distribution, `wsl://Distro/folder`.
pub const WSL_SCHEME: &str = "wsl://";

/// What an address starts with when the folder is in the dev container of a folder on this machine, `devcontainer://<folder>/workspace`, the folder written as [`escape_folder`] does.
pub const CONTAINER_SCHEME: &str = "devcontainer://";

/// What an address starts with when the server is reached through a tunnel that opens no port on its machine, `tunnel://hostname/folder`.
pub const TUNNEL_SCHEME: &str = "tunnel://";

/// How a client reaches the server of a machine, as the machine is named in a project's address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Route<'a> {
    /// The server on this machine.
    Local,
    /// Straight to `host:port`.
    Direct(&'a str),
    /// Through ssh to this destination.
    Ssh(&'a str),
    /// Inside this Windows Subsystem for Linux distribution.
    Wsl(&'a str),
    /// Inside the dev container of this folder of this machine.
    Container(String),
    /// Through the tunnel that reaches this hostname.
    Tunnel(&'a str),
}

/// The folder `folder` written so that it holds no slash and no whitespace, which is how a machine's name carries it.
pub fn escape_folder(folder: &str) -> String {
    let mut escaped = String::with_capacity(folder.len());
    for character in folder.chars() {
        match character {
            '%' => escaped.push_str("%25"),
            '/' => escaped.push_str("%2F"),
            character if character.is_whitespace() || character.is_control() => {
                let mut bytes = [0u8; 4];
                for byte in character.encode_utf8(&mut bytes).bytes() {
                    escaped.push_str(&format!("%{byte:02X}"));
                }
            }
            character => escaped.push(character),
        }
    }
    escaped
}

/// The folder that [`escape_folder`] wrote as `escaped`.
pub fn unescape_folder(escaped: &str) -> String {
    let bytes = escaped.as_bytes();
    let mut folder = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let decoded = (bytes[index] == b'%' && index + 2 < bytes.len())
            .then(|| escaped.get(index + 1..index + 3))
            .flatten()
            .and_then(|digits| u8::from_str_radix(digits, 16).ok());
        match decoded {
            Some(byte) => {
                folder.push(byte);
                index += 3;
            }
            None => {
                folder.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&folder).into_owned()
}

/// The machine and the folder on it that an address `ssh://destination/folder`, `tcp://host:port/folder`, `wsl://Distro/folder`, `devcontainer://<folder>/workspace` or `tunnel://hostname/folder` names, or nothing when it is no such address; a machine keeps its scheme unless it is reached over ssh, so that the ways of reaching it are told apart.
pub fn host_of(address: &str) -> Option<(&str, &str)> {
    for scheme in [DIRECT_SCHEME, WSL_SCHEME, CONTAINER_SCHEME, TUNNEL_SCHEME] {
        if let Some(rest) = address.strip_prefix(scheme) {
            let (host, folder) = rest.split_once('/').unwrap_or((rest, ""));
            let host = &address[..scheme.len() + host.len()];
            return (is_destination(host) && host.len() > scheme.len()).then_some((host, folder));
        }
    }
    let rest = address.strip_prefix(SSH_SCHEME)?;
    let (host, folder) = rest.split_once('/').unwrap_or((rest, ""));
    is_destination(host).then_some((host, folder))
}

/// How the server of the machine `host`, as [`host_of`] gives it, is reached; nothing for this machine is [`Route::Local`].
pub fn route_of(host: Option<&str>) -> Route<'_> {
    let Some(host) = host else {
        return Route::Local;
    };
    if let Some(address) = host.strip_prefix(DIRECT_SCHEME) {
        Route::Direct(address)
    } else if let Some(distro) = host.strip_prefix(WSL_SCHEME) {
        Route::Wsl(distro)
    } else if let Some(folder) = host.strip_prefix(CONTAINER_SCHEME) {
        Route::Container(unescape_folder(folder))
    } else if let Some(hostname) = host.strip_prefix(TUNNEL_SCHEME) {
        Route::Tunnel(hostname)
    } else {
        Route::Ssh(host)
    }
}
