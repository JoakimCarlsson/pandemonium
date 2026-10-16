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

/// The machine and the folder on it that an address `ssh://destination/folder` or `tcp://host:port/folder` names, or nothing when it is no such address; a machine reached straight at its address keeps its scheme, `tcp://host:port`, so it is told from an ssh destination.
pub fn host_of(address: &str) -> Option<(&str, &str)> {
    if let Some(rest) = address.strip_prefix(DIRECT_SCHEME) {
        let (host, folder) = rest.split_once('/').unwrap_or((rest, ""));
        let host = &address[..DIRECT_SCHEME.len() + host.len()];
        return is_destination(host).then_some((host, folder));
    }
    let rest = address.strip_prefix(SSH_SCHEME)?;
    let (host, folder) = rest.split_once('/').unwrap_or((rest, ""));
    is_destination(host).then_some((host, folder))
}

/// The `host:port` of a machine whose server is reached straight at its address, from the machine as [`host_of`] gives it.
pub fn direct_address(host: &str) -> Option<&str> {
    host.strip_prefix(DIRECT_SCHEME)
}
