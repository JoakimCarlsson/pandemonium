# SSH

Pandemonium uses SSH in two ways: the desktop editor can open a project on
another machine, and an SSH client on a phone can control a running desktop
editor. The two connections have different destinations and binaries.

## Open a project on an SSH host

Install the `pandemonium-server` binary on the development host. Each release
has a separate server archive for every target. Use the same release version
as the desktop editor:

```sh
curl -fsSL https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.sh | PANDEMONIUM_COMPONENT=server sh
```

Set `PANDEMONIUM_VERSION` to pin the server alongside a pinned editor release.
On Windows, set `PANDEMONIUM_COMPONENT=server` before running `install.ps1`.
The server must be on the remote host's `PATH` so SSH can run
`pandemonium-server --stdio`.

Configure an SSH host alias in your normal SSH config, then use the command
palette's **Project: Open Remote…** command. Enter the alias and an absolute
directory path in the form `host:/absolute/path`. SSH handles authentication;
Pandemonium starts the server over that connection. Use **Project: Reconnect**
if the connection drops.

Remote projects currently support the host operations exposed by the server.
Worktree sessions and ACP agents cannot yet be started on remote projects.

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
