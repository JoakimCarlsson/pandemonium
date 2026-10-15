//! What the server answers.

use pm_acp::{Discovery, Slash};
use pm_git::{Change, Owner, Repository};
use serde::{Deserialize, Serialize};

/// What the server knows about one session, enough to list it and open it again.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// The session's number.
    pub session: u64,
    /// The agent that runs it, by its id.
    pub agent: String,
    /// The folder of the project it belongs to.
    pub repository: String,
    /// The folder its agent works in.
    pub directory: String,
    /// The branch its worktree has, when it was given one of its own.
    pub branch: Option<String>,
    /// The abbreviated commit its worktree has checked out on a detached head.
    #[serde(default)]
    pub head: Option<String>,
    /// The branch its worktree was made from, when it has one of its own.
    pub base: Option<String>,
    /// What it is called.
    pub title: String,
    /// When it last changed, in milliseconds since the epoch.
    pub updated: u64,
    /// The number of its latest event.
    pub last_seq: u64,
    /// Whether the reader closed it: its agent is stopped and it is left out of the lists of what is going on, but it can be opened again.
    #[serde(default)]
    pub closed: bool,
    /// Whether its agent is running now.
    pub running: bool,
}

/// What the server knows about one shell.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TerminalInfo {
    /// The shell's number.
    pub terminal: u64,
    /// The folder it was started in.
    pub directory: String,
    /// How many characters wide its window is.
    pub columns: u16,
    /// How many lines tall its window is.
    pub rows: u16,
}

/// The answer to one [`crate::Request`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum Response {
    /// The request was carried out and has nothing to report.
    Done,
    /// The request could not be carried out, for this reason.
    Failed {
        /// What went wrong.
        message: String,
    },
    /// What an agent said about itself.
    Discovery(Discovery),
    /// The skills an agent has in a project.
    Skills {
        /// The skills the agent can be asked for by name.
        skills: Vec<Slash>,
        /// The names of the skills, for telling them from commands.
        names: Vec<String>,
    },
    /// The files of a folder.
    Files {
        /// Their paths, from the root of the repository.
        paths: Vec<String>,
        /// The folders that hold no tracked file, which have no file to be told of by.
        #[serde(default)]
        folders: Vec<String>,
    },
    /// The branches of a repository.
    Branches {
        /// Their names, the current one first.
        names: Vec<String>,
    },
    /// The history of a repository.
    Log {
        /// The commits, newest first.
        commits: Vec<pm_git::Commit>,
    },
    /// The stashes of a repository.
    Stashes {
        /// The stashes, newest first.
        stashes: Vec<pm_git::Stash>,
    },
    /// What a file holds.
    File {
        /// Its text, cut short when it was longer than could be shown; nothing when it is not text.
        text: Option<String>,
        /// Whether the text was cut short.
        truncated: bool,
    },
    /// Who an agent is signed in as.
    Identity {
        /// Who it is signed in as in its own folder.
        default: Option<String>,
        /// Who it is signed in as under the login in use.
        selected: Option<String>,
    },
    /// A session was started.
    Started {
        /// The session.
        session: u64,
        /// The folder its agent works in.
        directory: String,
        /// The branch its worktree has, when it was given one of its own.
        branch: Option<String>,
        /// The abbreviated commit its worktree has checked out on a detached head.
        #[serde(default)]
        head: Option<String>,
    },
    /// The projects the reader has added.
    Projects {
        /// Their addresses, oldest first.
        paths: Vec<String>,
        /// Whether they were ever saved; when not, the app has not been set up yet.
        saved: bool,
    },
    /// What the reader chose for each agent's settings.
    Choices {
        /// Each choice as agent, setting and value.
        choices: Vec<(String, String, String)>,
    },
    /// The agents the server can start.
    AgentServers {
        /// Every agent, in the order they are kept.
        servers: Vec<crate::AgentServer>,
    },
    /// The agents the public registry offers.
    RegistryAgents {
        /// The well known ones first.
        agents: Vec<crate::RegistryAgent>,
    },
    /// The tool servers the public registry offers for a search.
    ToolListings {
        /// The best match first.
        listings: Vec<crate::ToolListing>,
    },
    /// The tool servers every agent is started with.
    McpServers {
        /// Every server, in the order they are kept.
        servers: Vec<crate::McpServer>,
    },
    /// The named logins and the one in use for each agent.
    Accounts {
        /// The logins there are.
        logins: Vec<crate::Login>,
        /// The login in use for each agent, as agent and login id.
        in_use: Vec<(String, String)>,
    },
    /// The sessions the server holds.
    Sessions {
        /// Each one, oldest first.
        sessions: Vec<SessionInfo>,
    },
    /// What a folder holds that differs from the last commit.
    Inspected {
        /// The changed files.
        changes: Vec<Change>,
        /// The commits of the session's own branch, when it has one.
        commits: Option<Vec<String>>,
        /// The branch the folder has checked out, when it is a repository and on one.
        #[serde(default)]
        branch: Option<String>,
        /// The commit the folder is at, abbreviated, when it is a repository with one.
        #[serde(default)]
        head: Option<String>,
        /// What the checked out branch follows and how far apart they are.
        #[serde(default)]
        tracking: pm_git::Tracking,
        /// The merge, rebase or cherry-pick the folder is stopped in, if it is in one.
        #[serde(default)]
        operation: Option<pm_git::Operation>,
    },
    /// A shell was started.
    Terminal {
        /// The shell.
        terminal: u64,
    },
    /// The shells the server holds.
    Terminals {
        /// Each one, oldest first.
        terminals: Vec<TerminalInfo>,
    },
    /// The GitHub accounts and organizations whose repositories can be listed.
    Owners {
        /// The signed-in account first, then its organizations.
        owners: Vec<Owner>,
    },
    /// The GitHub repositories of one account or organization.
    Repositories {
        /// Each one, by name.
        repositories: Vec<Repository>,
    },
    /// The folders inside a folder.
    Folders {
        /// The folder, as an absolute path.
        path: String,
        /// The names of the folders inside it, in order, hidden ones left out.
        names: Vec<String>,
    },
    /// The folder a repository was cloned into.
    Directory {
        /// Its path on the server's machine.
        path: String,
    },
}
