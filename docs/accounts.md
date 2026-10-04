# Agent accounts and sign-in

Every agent offers an account icon and **Sign in or switch default account…**.
This explicitly requests the login methods advertised by that agent's ACP
handshake. The editor runs its terminal or browser authentication flow before
opening the new conversation. Multiple methods are offered for selection.
Default sign-in uses that provider's shared account storage. Agents that offer
no supported login method report that in the conversation.

Claude Code, Codex and Grok additionally support separate named profiles.
New chats from the palette, keybinding or chat pane start immediately with the
default account. Add or remove profiles under **Settings → Agents → Agent
Servers → Accounts…**; use a chat's account icon to choose another profile. Creating a profile asks only for a name, such as Work,
Personal or your organisation. In a worktree, creation immediately opens a new
conversation using empty, separate provider storage. Login is requested
immediately after the provider handshake, before opening a conversation,
even if the agent would otherwise accept the new session. When the agent offers one
login method, the editor starts it automatically; when it offers several, the
conversation lets you choose. Complete the provider's sign-in flow and choose
your account and organisation there. No organisation UUID is required.

Claude profiles use the adapter's Claude subscription login, which runs
`--cli auth login --claudeai` (or interactive login for remote environments),
as offered by the [Claude ACP adapter](https://github.com/agentclientprotocol/claude-agent-acp/blob/main/src/acp-agent.ts).
Each profile has its own `CLAUDE_CONFIG_DIR`, using Claude's documented
[multiple-account setup](https://code.claude.com/docs/en/authentication#log-in-with-multiple-accounts).
New profiles select `forceLoginMethod: claudeai` without pinning an organisation.
Codex profiles request file credential storage in their own `config.toml`;
Grok profiles start with empty provider storage. Provider policies still apply.
Existing profiles and their provider settings are retained, including any
organisation restrictions created previously.

Two conversations can use different profiles of the same agent simultaneously.
The selected label appears on hover over the account icon, in the chat list and
in the agent menu, and survives login, reconnect, history loading and restart.
Click the account icon in a conversation header to choose another profile in
the same project and worktree. Selecting it opens a new conversation; the
original keeps its identity and history.

Settings uses the active project's worktree for account creation and sign-in.
Open a project before creating a profile; without one, the chooser still lists
profiles and offers removal, and explains why creation is unavailable.
Use **Sign in to account profile…** to authenticate an existing profile again,
including a profile created before automatic sign-in was implemented.

Config stores metadata in `settings.yaml` under the editor's home
(`PANDEMONIUM_HOME`, otherwise `~/.pandemonium`). Agent-owned storage lives in
`accounts/<agent>/<profile-id>/`; names never become paths. Removing a profile
removes its listing, retains its login storage and leaves existing conversations
running. A new profile with the same label gets fresh storage. Credentials are
never copied, read by profile management or included in launch arguments.

## Availability

Claude Code, Codex and Grok offer account profiles through **Settings → Agents
→ Agent Servers → Accounts…**, beside each supported agent. New chats start
immediately with the default account; use a conversation's account icon to
choose another configured profile. Listing and removal work without a project;
creation and sign-in use the active worktree. No manual editor-settings flag is
required.

| Agent | Profile environment | Availability |
| --- | --- | --- |
| Claude Code | `CLAUDE_CONFIG_DIR` | Claude subscription accounts and organisation selection |
| Codex | `CODEX_HOME` | File credential storage; ChatGPT workspace selection |
| Grok | `GROK_HOME` | Separate local state; team selection |
| Cursor | — | Unavailable while credential isolation is unconfirmed |
| Copilot | — | Unavailable while credential isolation is unconfirmed |
| Gemini and custom agents | — | Unavailable |

Claude Console sign-in without an API key stores credentials outside the
configuration directory and is not an isolated profile login. New Claude
profiles select Claude subscription login. Cursor profiles stay unavailable
on every platform until its token storage is established, including macOS's
machine-wide Keychain login.

Codex limits read session records under the selected profile's home. Grok
limits come from the selected agent's ACP connection. Claude limits come only
from that connection's rate-limit updates for profiled sessions; the fallback
usage service is disabled because it would read profile credentials. Cursor
profiles likewise do not read stored credentials for dashboard usage. Missing
profile-specific limits stay absent instead of showing another account's usage.

Run `make lint` and `make run`, create two profiles for each supported agent,
log in to different identities and confirm both in simultaneous sessions.
Confirm the labels and identities after login, history loading, reconnect and
editor restart. Start each other agent and confirm its own advertised authentication methods
are offered by the default-account sign-in action.
Inspect process arguments to confirm profile paths and credentials are not
arguments. For Claude organisation profiles, confirm that creation asks only
for a name,
starts the subscription sign-in flow and lets you choose an organisation in
that flow. Confirm that restart retains the chosen identity. Provider login
and token refresh remain the agent's responsibility.
