//! What a client asks of the server.

use serde::{Deserialize, Serialize};

/// A file the reader attached to a prompt, carried whole so the server needs no access to the client's disk.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// The file's name, without a folder.
    pub name: String,
    /// What the file holds.
    #[serde(with = "crate::bytes")]
    pub data: Vec<u8>,
}

/// A named login of an agent: which agent, its id and what the reader calls it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Login {
    /// The agent, by its id.
    pub agent: String,
    /// The login's id, which names its folder.
    pub id: String,
    /// What the reader calls it.
    pub name: String,
}

/// One thing a client asks, answered by one [`crate::Response`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum Request {
    /// What the agent lets the reader change and offers behind a slash.
    Discover {
        /// The agent, by its id.
        agent: String,
    },
    /// The skills the agent has in the project folder, and their names.
    Skills {
        /// The agent, by its id.
        agent: String,
        /// The project's folder on the server's machine.
        directory: String,
    },
    /// The files of a folder that are tracked or not ignored.
    Files {
        /// The folder.
        directory: String,
        /// The most paths to return.
        limit: usize,
    },
    /// The local branches of the repository a folder is in.
    Branches {
        /// The folder.
        directory: String,
    },
    /// Who the agent is signed in as, by default and under the login in use.
    Identity {
        /// The agent, by its id.
        agent: String,
    },
    /// Saves the logins and points each agent at the folder of the login in use for it.
    SetAccounts {
        /// The logins there are.
        logins: Vec<Login>,
        /// The login in use for each agent, as agent and login id.
        in_use: Vec<(String, String)>,
    },
    /// Signs the agent in under the login now in use.
    SignIn {
        /// The agent, by its id.
        agent: String,
    },
    /// Starts a session: makes its worktree when asked, starts its agent and hands it the first prompt.
    StartSession {
        /// The agent, by its id.
        agent: String,
        /// The folder of the project.
        repository: String,
        /// The folder the agent works in when no worktree is made.
        directory: String,
        /// What the session is called, which names its worktree folder.
        title: String,
        /// The branch or commit to check out in a worktree of its own, detached, when the session works in one.
        base: Option<String>,
        /// The settings the reader changed before starting, as the agent names them and their values.
        settings: Vec<(String, String)>,
        /// What the reader wrote.
        text: String,
        /// The files attached.
        attachments: Vec<Attachment>,
    },
    /// Hands a session's agent its next turn.
    Prompt {
        /// The session.
        session: u64,
        /// What the reader wrote.
        text: String,
        /// The files attached.
        attachments: Vec<Attachment>,
    },
    /// Answers the question a session's agent asked.
    Choose {
        /// The session.
        session: u64,
        /// The question, by the number the agent put it under.
        ticket: u64,
        /// The choice made.
        option: String,
    },
    /// Stops the turn a session's agent is on.
    Cancel {
        /// The session.
        session: u64,
    },
    /// Gives a setting of a session's agent a value.
    Set {
        /// The session.
        session: u64,
        /// The setting.
        setting: String,
        /// The value.
        value: String,
    },
    /// Subscribes the connection to a session's events and replays those after `after`.
    Attach {
        /// The session.
        session: u64,
        /// The sequence number of the last event the client has.
        after: u64,
    },
    /// What a session's folder holds that differs from the last commit, and its own commits.
    Inspect {
        /// The folder.
        directory: String,
        /// The branch the session's worktree was made from, when it has one.
        base: Option<String>,
    },
    /// Stages or unstages files in a folder.
    Stage {
        /// The folder.
        directory: String,
        /// The paths, from the root of the repository.
        paths: Vec<String>,
        /// Whether to stage them, or unstage them.
        stage: bool,
    },
    /// The projects the reader has added on the server's machine.
    Projects,
    /// Replaces the projects the reader has added.
    SaveProjects {
        /// Their addresses, oldest first.
        paths: Vec<String>,
    },
    /// What the reader chose for each agent's settings.
    Choices,
    /// Replaces what the reader chose for each agent's settings.
    SaveChoices {
        /// Each choice as agent, setting and value.
        choices: Vec<(String, String, String)>,
    },
    /// The named logins and the one in use for each agent.
    Accounts,
    /// Every session the server holds, running or not.
    Sessions,
    /// Commits what is staged in a folder.
    Commit {
        /// The folder.
        directory: String,
        /// The commit message.
        message: String,
    },
}
