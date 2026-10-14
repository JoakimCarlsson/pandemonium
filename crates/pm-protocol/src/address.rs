//! The addresses of projects on other machines.

/// What an address starts with when the folder is on a machine reached over ssh.
pub const SSH_SCHEME: &str = "ssh://";

/// Whether `destination` can safely be handed to ssh: it is not empty, does not start with a dash that ssh would read as an option, and holds no whitespace or control characters.
pub fn is_destination(destination: &str) -> bool {
    !destination.is_empty()
        && !destination.starts_with('-')
        && !destination
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

/// The machine and the folder on it that an address `ssh://destination/folder` names, or nothing when it is no such address.
pub fn host_of(address: &str) -> Option<(&str, &str)> {
    let rest = address.strip_prefix(SSH_SCHEME)?;
    let (host, folder) = rest.split_once('/').unwrap_or((rest, ""));
    is_destination(host).then_some((host, folder))
}
