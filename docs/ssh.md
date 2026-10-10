# SSH

Pandemonium uses SSH in two ways: the desktop editor can open a project on
another machine, and an SSH client on a phone can control a running desktop
editor. The two connections have different destinations and binaries.

## Open a project on an SSH host

Configure an SSH host alias in your normal SSH config, then use the command
palette's **Project: Open Remote…** command. Choose a saved project or host,
or enter an alias to browse its folders. An address such as
`host:/absolute/path` opens that directory directly. SSH handles authentication;
Pandemonium remembers successful connections in its preferences.

Pandemonium starts a matching `pandemonium-server` over SSH. If it is missing
or incompatible, setup uploads the adjacent server binary when the host has
the same operating system and architecture, or installs a checksum-verified
release pinned to the editor version. Automatic setup supports Linux and
macOS hosts and requires an internet connection when downloading a release.
When running from source, build both binaries with `cargo build --workspace`.
The endpoint lives under `~/.cache/pandemonium/server` on the host; projects and
tools remain in their normal locations.

Remote projects support files, git, worktree sessions, terminals, tasks, ACP
agents, language servers and debug adapters. Install the development tools and
agent adapters on the SSH host: executable discovery and processes run there.
Agent accounts are signed in on that host; desktop credentials are not copied.
Dropped desktop attachments are copied to temporary storage on the host and
removed when their conversation closes.
Language servers are scoped to both host and worktree, including when two
hosts use the same directory name. Debugger sockets travel through the server;
editor-owned agent services use temporary SSH forwarding.

Use **Project: Reconnect** after a connection drops. Open buffers retain their
edits; reconnect refreshes watchers, file trees and language servers for every
project and session on that host. A disconnected save reports failure, and the
server replaces files only after receiving a complete upload. Restart a stopped
terminal, task, agent or debugger through its usual command after reconnecting.

The control interface uses the same connection flow:

```sh
pandemonium control project remote my-host:/absolute/path
pandemonium control project remote my-host
pandemonium control project reconnect 0
```

Manual installation is also available:

```sh
curl -fsSL https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.sh | PANDEMONIUM_COMPONENT=server sh
```

Set `PANDEMONIUM_VERSION` to the editor's release version. The endpoint checks
both the package version and transport protocol before serving a project.

## Control the desktop editor from a phone

Leave the desktop editor running. From an SSH app on the phone, connect to the
machine running the editor as the same user. Run `pandemonium control` for an
interactive prompt, or send one command at a time:

```sh
ssh -t my-desktop 'pandemonium control'
ssh my-desktop 'pandemonium control status'
ssh my-desktop 'pandemonium control agent show 0'
ssh my-desktop 'pandemonium control agent send 0 "Review the current diff"'
```

Run `help` in the control prompt to see commands for projects, worktree
sessions, files, and ACP conversations, including permission requests. Use
`status` to find the numbered items; their numbers can change as the window
changes.

The editor accepts control commands through a Unix socket in its private
`~/.pandemonium/control` directory. It does not open a network port. Phone
control uses the regular `pandemonium` binary on the desktop machine, while
`pandemonium-server` runs on a separate development host for remote projects.

A native phone client can use the [control protocol](control-protocol.md)
through `pandemonium control --stdio` over SSH.
