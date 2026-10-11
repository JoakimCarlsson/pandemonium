# Dev containers

Pandemonium reads the standard `devcontainer.json` of the
[Development Containers Specification](https://containers.dev/implementors/spec/)
and runs a project, or one agent worktree, inside a container of its own. The
`pm-devcontainer` crate implements the supported subset directly against a
local Docker engine. It needs no reference Dev Containers CLI, no Node runtime
and no SSH daemon in the container.

Requirements: Docker (or a program that speaks its command line, set with
`devcontainer_engine` in `settings.yaml`) running Linux containers, and a
container with a POSIX shell. The editor starts a matching `pandemonium-server`
inside it with `docker exec` and talks to it over standard input and output,
the same endpoint and transport that SSH projects use. The server is uploaded
from the binary next to the editor when the container has the same operating
system and architecture, or installed from the pinned release otherwise. A
container whose system the server cannot run on is reported by name: the error
says which operating system or architecture the container reported. A binary
built on a newer C library than the image has fails the same way, so build on a
system no newer than your images, or use a release.

## Using it

Nothing runs when a project is opened. Setup commands run only after an
explicit action, from the command palette:

| Command | What it does |
| --- | --- |
| Dev Container: Reopen in Container | Creates, starts and attaches the focused project's environment, running the setup that is due, then opens its workspace as a project. |
| Dev Container: Rebuild | Replaces only the container and volumes this environment owns, then sets it up again. |
| Dev Container: Reconnect | Attaches to the existing container without creating it or running creation commands. Also reachable as **Project: Reconnect**. |
| Dev Container: Stop | Releases processes and port listeners, then stops the container. Files stay. |
| Dev Container: Show Log | Opens the log of the latest operation. It grows while a build or setup runs. |

The native project stays open beside the container project, so unsaved buffers
and their identities survive reopening, stopping, reconnecting and rebuilding.
The container project is listed as `name (container)`. When the container is
stopped or gone, its tools report `Disconnected from <container>`; nothing falls
back to running on the desktop.

The same operations are available to `pandemonium control`:

```sh
pandemonium control project container open|rebuild|reconnect|stop N
pandemonium control session new-container PROJECT NAME
pandemonium control session container open|rebuild|reconnect|stop N
```

Declarations are read from `.devcontainer/devcontainer.json`, then
`.devcontainer.json`, in the folder being opened.

## Agent worktrees

The shortest path: start a session as usual. When the project is on this
machine and has a `.devcontainer/devcontainer.json`, the new-session flow asks
**In a dev container** or **On this machine**. The worktree is cut on the
desktop with the desktop's git, so worktree metadata is never written with
container paths. Choosing the container gives that worktree an environment of
its own: a separate container, a bind mount of only that worktree and its own
named volumes, built from the worktree's own declaration. The session is not
taken up, and no agent starts, until that setup has completed. A failed setup
removes the new worktree and reports the failure; retry by starting the session
again. Finishing a session removes its container, volumes, built image and
record, and nothing else. A project that was itself reopened in its container
gives every new session a container without asking.

Stopping or rebuilding one session's environment leaves every other
environment, and every other worktree's files, alone. Dependency trees that
other sessions symlink in from the project (`node_modules`, `.venv`) are not
linked into a containerized session, because a link would name a desktop path
the container cannot see; install them in the container with
`postCreateCommand`.

A session found again after the editor restarts shows as disconnected until it
is reconnected. Use the commands above with the session focused.

## The environment contract

The contract lives in `pm_devcontainer::environment`, below any window
wiring, and composes `pm-host` with a workspace identity:

- **Identity.** `Identity` is a project name plus a checkout or worktree folder.
  Every container, volume, image and record is labelled with the digits derived
  from it, so two worktrees of one project never share a container or writable
  volume.
- **Locations.** `Environment::source()` is the folder on the desktop and
  `Environment::workspace()` the same files as tools see them. Its `host()` runs
  every tool: files, git, terminals, language servers, tasks, debug adapters and
  agents all resolve their machine from the location they were given, so there
  is one resolution seam and no second implementation in any tool caller.
- **Context.** `ExecutionContext` is the user and variables tools run with.
- **State.** `Absent`, `Stopped`, `Working(phase)`, `Ready` or `Failed`.
- **Events.** `Event` carries state changes, log lines, notices and forwards
  from whichever thread did the work.
- **Capabilities.** Filesystem, processes, terminals, port forwarding,
  lifecycle and isolated worktrees.
- **Forwards.** The desktop listeners the environment currently owns.

`Environment::native` is the desktop itself and needs no engine and no
declaration; a native project never touches the provider. A future provider
(for example a Nix or devenv one) implements `Provider`: `declaration`, `open`,
`reconnect`, `stop` and `state`, and hands back an `Environment` whose
workspace location carries a `pm_host::Host` created with `Host::launched`.
`Host::launched` takes a `pm_host::Endpoint`, which only says how to start a
shell on the machine; the handshake, installation, file, process, terminal and
byte operations are `pm-host`'s own. The dependency direction is
`pm-devcontainer` to `pm-host`; the binary wires explicit actions and consumes
the generic events, and never builds an engine command.

Stored locations of an environment are `devcontainer://<container>/<path>`.
A restored one resolves through the provider's records and stays disconnected
until reconnected, so reopening the editor never runs setup.

## Lifecycle

Open runs, in this order:

1. `initializeCommand`, on the desktop in the checkout, on every explicit open
   or rebuild (not on reconnect), before anything is built or removed.
2. Image: pull when absent, or build the Dockerfile with `--progress=plain`.
3. Create the container with its labels, mounts, variables and user, then start it.
4. For a newly created container: `onCreateCommand`, `updateContentCommand`,
   `postCreateCommand`, then `postStartCommand`.
5. For an existing stopped container: start it, then `postStartCommand`.
6. Install and connect the endpoint, open port listeners.
7. `postAttachCommand` after an explicit open.
8. The environment becomes `Ready`.

Each command is a shell string (run with `sh -c`), an argument array (run
directly) or an object of named commands that run together; all must succeed.
They run in the container as the tool user, in the workspace folder, with the
tool variables. The environment is not ready, and no agent starts, until the
required commands finish. Reconnecting does not rerun creation commands: whether
they completed is recorded, so an interrupted creation resumes where it must
and a finished one is never repeated. Editing a failing command and opening
again retries it with the edited declaration.

Stopping releases the listeners and the transport before `docker stop`, so
nothing keeps the container busy. Rebuilding removes only containers and
volumes carrying this environment's owner and identity labels, and never a
shared image, repository metadata or a worktree folder. A container with the
expected name but without the owner label is refused rather than touched.

Build and setup failures keep their log (`~/.pandemonium/devcontainers/<id>.log`)
until the next attempt and leave `devcontainer.json` as the one thing to edit.

## Supported declaration fields

| Field | Support |
| --- | --- |
| `image`, `build.dockerfile`, `build.context`, `build.args`, `build.target`, `build.cacheFrom`, `build.options`, legacy `dockerFile` and `context` | Supported. Paths resolve against the declaration. |
| `workspaceFolder` | Supported; defaults to `/workspaces/<folder>`. |
| `workspaceMount`, `mounts` | Bind, named volume and tmpfs mounts, in string or object form. Named volume sources are prefixed with the environment's id and created labelled, so worktrees never share one. Other mount types and options are refused. A custom workspace mount source other than the checkout is not mapped back to the desktop. |
| `containerEnv`, `containerUser` | Applied to the container, so to everything in it. |
| `remoteEnv`, `remoteUser` | Applied to the editor's tools and lifecycle commands only. A `null` value is refused. |
| `forwardPorts` | Container ports as numbers. `service:port` strings are refused. |
| `runArgs`, `init`, `privileged`, `capAdd`, `securityOpt`, `overrideCommand` | Passed to the engine. |
| `initializeCommand`, `onCreateCommand`, `updateContentCommand`, `postCreateCommand`, `postStartCommand`, `postAttachCommand` | Supported in all three forms. `waitFor` is accepted; every command is waited for. |
| `customizations.pandemonium.extensions` | See below. Other tools' namespaces are not read. |
| `name`, `$schema`, `hostRequirements`, `portsAttributes`, `otherPortsAttributes`, `shutdownAction`, `updateRemoteUserUID` | Accepted and ignored. The container user's id is not remapped to the desktop user. |

Refused before anything is built, created or run, all at once in one report:
Docker Compose (`dockerComposeFile`, `service`, `runServices`), Dev Container
Features (a non-empty `features`, `overrideFeatureInstallOrder`), `appPort`,
`userEnvProbe`, `gpuRequirements`, every other unknown property, unsupported
mount types, options or variables, and a malformed field. Image metadata (the
`devcontainer.metadata` label) is not merged; its presence is reported as a
notice. Nothing is silently dropped, so an environment is either the one
declared or an error.

### Variables

`${localEnv:NAME}` (and its alias `${env:NAME}`) and `${localEnv:NAME:default}`
read the desktop; `${localWorkspaceFolder}` and
`${localWorkspaceFolderBasename}` name the checkout on the desktop;
`${containerWorkspaceFolder}` and its `Basename` form name the folder in the
container; `${devcontainerId}` is the identity digits; `${containerEnv:NAME}`
and `${containerEnv:NAME:default}` read the environment the container was
created with. Substitution is by place: `workspaceMount` and `workspaceFolder`
may not use the container workspace variables, and `${containerEnv:...}` is
defined only for `remoteEnv`, `remoteUser` and lifecycle commands. A desktop
value is never substituted for a container variable.

## Ports

Each `forwardPorts` entry opens a desktop listener on `127.0.0.1`, owned by the
environment. Connections are carried through the endpoint to the container's
loopback, so a service bound to `127.0.0.1` inside the container is reachable.
When the requested desktop port is busy, the listener binds a free port
instead and a notice names it. Stopping, rebuilding or dropping the environment
releases every listener.

## Extensions

`customizations.pandemonium.extensions` is a list of extension identifiers.
After the environment is ready, the window resolves each one against the
installed extensions and then the catalogue, installing what it finds through
the existing validated installer. Identifiers that are in neither are reported
by name; the environment stays usable. The container provider never installs
or reads extensions.

## Preferences

The engine is the only preference. `settings.yaml` holds
`devcontainer_engine: docker` (the default, omitted when unchanged), read when
the editor starts. Declarations
stay in the repository; records and logs live in
`~/.pandemonium/devcontainers`, one `<id>.json` and `<id>.log` per environment.

## Support limits

- A local Docker engine only. Remote engines, Docker Compose, Dev Container
  Features, image metadata merging, Windows containers and a new SSH feature
  are outside this implementation.
- The project folder must be on this machine. A session of a native project
  cannot be moved into a container; open the project in its container first.
- Agent CLIs and language servers must be available in the container, and the
  editor's own agent tool services are not forwarded into it, so an agent in a
  container starts without them.
- Symlinked dependency trees are not shared into containerized sessions.
- A worktree's git metadata is mounted at the paths its `.git` file records.
  Because sibling worktrees are not mounted, git inside the container lists
  them as prunable. Tools run with `gc.worktreePruneExpire=never` in their
  environment (not in the repository's configuration) so nothing prunes them;
  do not run `git worktree prune` inside a container.
- File ownership follows bind-mount semantics: use a container user whose id
  matches the desktop user.

## Fixtures and verification

Two runnable repositories live in `crates/pm-devcontainer/fixtures`: `image`
(a published Python development image) and `dockerfile` (built from
`.devcontainer/Dockerfile`). Each declares `PANDEMONIUM_FIXTURE`, a remote
variable, port 8080, every lifecycle command (each appends to
`.fixture/hooks.log` in the workspace), a task file, a debug configuration and
a file with a lint finding.

```sh
make lint
cargo build -p pandemonium -p pm-server
docker version
scripts/devcontainer-fixtures.sh        # build/devcontainer-fixtures/{image,dockerfile}
make run
```

1. **Native is untouched.** With Docker stopped, open any project without a
   declaration, edit and save, review git changes, open a terminal. No container
   or setup command is attempted.
2. **Open each fixture.** Add `build/devcontainer-fixtures/image` as a project,
   run **Dev Container: Reopen in Container**, and open **Dev Container: Show
   Log** to watch the pull or build and the setup stream.
3. **Context.** In the container project's terminal run
   `id; pwd; printenv PANDEMONIUM_FIXTURE; git status --short`, and
   `cat .fixture/hooks.log`: the order is `onCreateCommand`,
   `updateContentCommand`, `postCreateCommand`, `postStartCommand`,
   `postAttachCommand`. Stop and open again: only `postStartCommand` and
   `postAttachCommand` are added. Reconnect adds `postStartCommand` after a stop
   and nothing otherwise.
4. **Tools run inside.** Edit and save `app.py`, stage the change in the review
   pane, run the `whoami` and `serve` tasks, open `lint.py` for diagnostics from
   a language server running in the container, debug `app.py`, and start an
   agent installed in the container. `docker top pm-<name>-<id>` lists the
   processes.
5. **Isolation.** On the native project, start two sessions and choose **In a
   dev container** for each. Write a different file in each. Each has its own container; `git status`
   in both terminals is valid. Stop or rebuild one with **Dev Container: Stop**
   or **Rebuild** while it is focused; the other keeps working.
6. **Ports.** Run `serve` in a container terminal, then
   `curl --fail http://127.0.0.1:8080/` on the desktop (or the port the notice
   reports). Stop the environment and repeat: the listener is gone. Occupy 8080
   first and the notice names the alternate port.
7. **Recovery.** Leave an unsaved buffer open, then stop, reconnect and rebuild.
   Quit and reopen the editor: the container project and its sessions return
   disconnected, **Dev Container: Reconnect** reattaches without setup, and
   `docker ps -a` shows no unrelated container touched.
8. **Failures.** Set `"onCreateCommand": "exit 1"` or add `RUN false` to the
   Dockerfile: the log and notice show the failure; fix it and open again.
   Add `"features": {"ghcr.io/devcontainers/features/node:1": {}}`: it is
   refused before anything runs. Add
   `"customizations": {"pandemonium": {"extensions": ["missing"]}}`; the
   identifier is reported, and an installed one is accepted.
9. **No extra runtimes.** Repeat step 2 with `node` and `devcontainer` absent
   from `PATH`; nothing changes.
