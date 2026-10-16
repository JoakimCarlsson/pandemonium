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

/// An agent the server can start: how to start it, how to fetch it when it is missing and how it keeps logins apart. The ones the app ships are in the list from the start and can be changed or removed like any other.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentServer {
    /// What the app keeps it under.
    pub id: String,
    /// What a row calls it.
    pub name: String,
    /// The command that starts it in protocol mode, program and arguments.
    pub command: String,
    /// The environment it is started with, over the one it inherits.
    pub env: Vec<(String, String)>,
    /// The package the runner fetches when the program is not installed.
    #[serde(default)]
    pub package: Option<String>,
    /// The shell pipeline that installs the program when it is missing.
    #[serde(default)]
    pub installer: Option<String>,
    /// The variable that moves its config and login to another folder, for an agent that can keep more than one login.
    #[serde(default)]
    pub home: Option<String>,
}

/// A tool server every agent is started with.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct McpServer {
    /// What a row calls it.
    pub name: String,
    /// The command that starts it, or the address it listens at when that begins with `http://` or `https://`.
    pub target: String,
    /// The environment it is started with, or the headers it is sent when it is an address.
    pub env: Vec<(String, String)>,
    /// What it is for, where its publisher says.
    #[serde(default)]
    pub description: String,
    /// Where its publisher describes it, when they say.
    #[serde(default)]
    pub website: String,
    /// Whether agents are given it; a server switched off stays listed.
    #[serde(default = "enabled")]
    pub enabled: bool,
}

/// Servers are given to agents unless they were switched off.
fn enabled() -> bool {
    true
}

/// An agent the public registry offers, as the command that runs it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RegistryAgent {
    /// What the registry knows it by.
    pub id: String,
    /// What a row calls it.
    pub name: String,
    /// The version the registry lists.
    pub version: String,
    /// What it is, as its publisher says.
    pub description: String,
    /// Where its publisher describes it.
    pub website: String,
    /// The command that runs it, program and arguments.
    pub command: String,
    /// The environment it is started in, over the one it inherits.
    pub env: Vec<(String, String)>,
}

/// A tool server the public registry offers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolListing {
    /// What the registry knows it by.
    pub id: String,
    /// What a row calls it.
    pub title: String,
    /// What it does, as its publisher says.
    pub description: String,
    /// What it is called once installed.
    pub name: String,
    /// The command that starts it, or the address it listens at.
    pub target: String,
    /// The environment it is started with, or the headers it is sent, still to be filled in.
    pub env: Vec<(String, String)>,
    /// What it lists as taking, whether or not it runs without.
    pub inputs: Vec<String>,
    /// Whether installing it has to ask for something first.
    pub asks: bool,
    /// Where its publisher describes it, when they say.
    #[serde(default)]
    pub website: String,
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
    /// Closes a session: stops its agent and marks it finished. Its log is kept; a worktree it made is removed, and without `discard` that is refused while the worktree has changes that are not committed.
    CloseSession {
        /// The session.
        session: u64,
        /// Whether to remove the worktree even though it has changes that are not committed.
        #[serde(default)]
        discard: bool,
    },
    /// Gives a session a new title, which every client watching it is told of as a `Titled` event.
    RenameSession {
        /// The session.
        session: u64,
        /// What it is called now.
        title: String,
    },
    /// Hands a session's agent its next turn, or queues it while the agent is on one.
    Prompt {
        /// The session.
        session: u64,
        /// What the reader wrote.
        text: String,
        /// The files attached.
        attachments: Vec<Attachment>,
        /// Whether to stop the turn the agent is on and hand it this one next, ahead of the queue.
        #[serde(default)]
        now: bool,
    },
    /// Stops the turn a session's agent is on and hands it a queued prompt next.
    Hurry {
        /// The session.
        session: u64,
        /// The queued prompt, by the id its `Queued` event gave it.
        queued: u64,
    },
    /// Takes a session's conversation back to before the reader's turn `turn`: the turn and all after it are dropped, and the agent starts the next one afresh with what came before it.
    Rewind {
        /// The session.
        session: u64,
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
    },
    /// Puts the files of a session's folder back as they were before the reader's turn `turn`.
    Restore {
        /// The session.
        session: u64,
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
    },
    /// Starts a session that branches off a session's conversation after the reader's turn `turn` and its reply. It works in the same folder, and its agent is told the conversation so far with the first prompt it is handed.
    ForkSession {
        /// The session branched off.
        session: u64,
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
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
    /// Answers the form of questions a session's agent asked, or abandons it.
    Answer {
        /// The session.
        session: u64,
        /// The form, by the number the agent put it under.
        ticket: u64,
        /// What was answered to each question, in order, or nothing to abandon the form.
        replies: Option<Vec<pm_acp::Reply>>,
    },
    /// Stops the turn a session's agent is on and drops the prompts queued behind it.
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
    /// Puts files back as they are in the last commit, deleting those that are new.
    Discard {
        /// The folder.
        directory: String,
        /// The paths, from the root of the repository.
        paths: Vec<String>,
    },
    /// Adds files to the `.gitignore` at the root of the repository.
    Ignore {
        /// The folder.
        directory: String,
        /// The paths, from the root of the repository.
        paths: Vec<String>,
    },
    /// Checks out a branch of the repository a folder is in.
    Checkout {
        /// The folder.
        directory: String,
        /// The branch, local or a remote's.
        branch: String,
    },
    /// Makes a branch at the commit a folder is at and checks it out.
    CreateBranch {
        /// The folder.
        directory: String,
        /// What the branch is called.
        name: String,
    },
    /// Makes an empty file or a folder, and the folders above it, inside a folder.
    CreateEntry {
        /// The folder.
        directory: String,
        /// The path of the new entry, from the root of the folder.
        path: String,
        /// Whether to make a folder rather than a file.
        folder: bool,
    },
    /// Renames or moves a file or folder inside a folder, refusing a name that is taken.
    RenameEntry {
        /// The folder.
        directory: String,
        /// Where the entry is, from the root of the folder.
        from: String,
        /// Where it goes, from the root of the folder.
        to: String,
    },
    /// Copies a file or folder into another folder inside a folder, under a free name when its own is taken.
    CopyEntry {
        /// The folder.
        directory: String,
        /// Where the entry is, from the root of the folder.
        from: String,
        /// The folder the copy goes into, from the root of the folder; empty for the root itself.
        into: String,
    },
    /// Takes a file or folder, and everything in it, off the disk.
    RemoveEntry {
        /// The folder.
        directory: String,
        /// Where the entry is, from the root of the folder.
        path: String,
    },
    /// Reads a file of a folder, as it is in the working folder or in the last commit.
    ReadFile {
        /// The folder.
        directory: String,
        /// The path, from the root of the repository.
        path: String,
        /// Whether to read it from the last commit rather than from the working folder.
        head: bool,
        /// The commit to read it from, by object name, in place of the working folder or the last commit.
        #[serde(default)]
        revision: Option<String>,
    },
    /// The commits that changed a file of a folder, newest first.
    FileHistory {
        /// The folder.
        directory: String,
        /// The path, from the root of the folder.
        path: String,
    },
    /// The web address of a line of a file of a folder on the host of its repository, at the commit checked out.
    Permalink {
        /// The folder.
        directory: String,
        /// The path, from the root of the folder.
        path: String,
        /// The line, counted from 1.
        line: usize,
    },
    /// Replaces the contents of a file of a folder with text, making the file when it is not there.
    WriteFile {
        /// The folder.
        directory: String,
        /// The path, from the root of the folder.
        path: String,
        /// What the file holds afterwards.
        text: String,
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
    /// The agents the server can start.
    AgentServers,
    /// Replaces the agents the server can start.
    SaveAgentServers {
        /// Every agent, in the order they are kept.
        servers: Vec<AgentServer>,
    },
    /// The agents the public registry offers that are run from a package.
    RegistryAgents,
    /// The tool servers of the public registry that match `query`, or the well known ones when it is blank.
    SearchTools {
        /// What to look for.
        query: String,
    },
    /// The tool servers every agent is started with.
    McpServers,
    /// Replaces the tool servers every agent is started with.
    SaveMcpServers {
        /// Every server, in the order they are kept.
        servers: Vec<McpServer>,
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
        /// Whether, with nothing staged, to take the changes of every tracked file.
        #[serde(default)]
        tracked: bool,
        /// Whether to rewrite the last commit in place of making another.
        #[serde(default)]
        amend: bool,
    },
    /// The most recent commits of the repository a folder is in, with the lanes of the graph they are drawn in.
    Log {
        /// The folder.
        directory: String,
        /// The most commits to return.
        limit: usize,
        /// Whether to list what every reference reaches, rather than what the checked out branch reaches.
        all: bool,
    },
    /// Fetches, pulls or pushes the repository a folder is in, or syncs it by pulling and then pushing.
    Sync {
        /// The folder.
        directory: String,
        /// `fetch`, `pull`, `pull_rebase`, `push`, `force_push` or `sync`.
        operation: String,
    },
    /// Applies a commit to the branch a folder has checked out.
    CherryPick {
        /// The folder.
        directory: String,
        /// The commit, by its abbreviated or full name.
        commit: String,
    },
    /// The stashes of the repository a folder is in.
    Stashes {
        /// The folder.
        directory: String,
    },
    /// Saves a folder's changes in a stash, or applies, pops or drops one.
    Stash {
        /// The folder.
        directory: String,
        /// `push`, `apply`, `pop` or `drop`.
        action: String,
        /// The stash for `apply`, `pop` and `drop`, by its index in the list.
        index: Option<usize>,
        /// What to call the stash `push` makes.
        message: Option<String>,
    },
    /// Takes the next step of the merge, rebase or cherry-pick a folder is stopped in.
    Step {
        /// The folder.
        directory: String,
        /// `continue`, `skip` or `abort`.
        action: String,
        /// The message to finish a merge or a cherry-pick with, in place of the one git prepared.
        #[serde(default)]
        message: Option<String>,
    },
    /// Starts a shell in a folder, which keeps running until it exits or is closed, whoever is connected.
    OpenTerminal {
        /// The folder the shell starts in.
        directory: String,
        /// How many characters wide the window of the shell is.
        columns: u16,
        /// How many lines tall the window of the shell is.
        rows: u16,
    },
    /// Types into a shell.
    TerminalInput {
        /// The shell.
        terminal: u64,
        /// The bytes the shell reads, as the keys the reader pressed come out.
        #[serde(with = "crate::bytes")]
        data: Vec<u8>,
    },
    /// Tells a shell how large its window now is.
    ResizeTerminal {
        /// The shell.
        terminal: u64,
        /// How many characters wide.
        columns: u16,
        /// How many lines tall.
        rows: u16,
    },
    /// Ends a shell and forgets it.
    CloseTerminal {
        /// The shell.
        terminal: u64,
    },
    /// Subscribes the connection to what a shell prints and replays what it printed after the byte `after`.
    AttachTerminal {
        /// The shell.
        terminal: u64,
        /// How many bytes of its output the client has.
        after: u64,
    },
    /// Every shell the server holds.
    Terminals,
    /// The GitHub accounts and organizations whose repositories the account `gh` is signed in to on the server's machine can list.
    Owners,
    /// The GitHub repositories of an account or organization.
    Repositories {
        /// The account's login.
        owner: String,
        /// Whether it is an organization.
        organization: bool,
    },
    /// The folders inside a folder of the server's machine, for choosing one.
    Folders {
        /// The folder; a leading `~` is the home folder, and nothing is the home folder itself.
        path: String,
    },
    /// The extensions there are to install and those installed on the server's machine.
    Extensions,
    /// Installs extensions, with the extensions they need and, for a language, its language server, on the server's machine; every client is told how it goes with `extensions_changed`.
    InstallExtensions {
        /// The extensions, by folder name.
        ids: Vec<String>,
    },
    /// Installs the extension in a folder of the server's machine: copies the folder into the server's own folder of extensions, replacing one of the same name, and installs it; every client is told how it goes with `extensions_changed`.
    InstallExtension {
        /// The folder.
        path: String,
    },
    /// Takes an extension off the server's machine.
    RemoveExtension {
        /// The extension, by folder name.
        id: String,
    },
    /// The grammar and queries of an installed language, found by its folder name or by a name a code fence calls it.
    Grammar {
        /// The language.
        language: String,
    },
    /// Opens a file of a folder for its language server, starting the server for the folder when it is not running; the connection is told what the server finds wrong with the file.
    OpenDocument {
        /// The folder, which the server treats as the project.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The installed language it is written in, by folder name.
        language: String,
        /// The version of the text, counted by the client.
        version: i32,
        /// What the file holds in the client.
        text: String,
    },
    /// Edits an open file the way the client did, taking it to `version`, one more than the version before; edits that arrive out of order are put back in order by their versions.
    ChangeDocument {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version the edits take the text to.
        version: i32,
        /// The stretches replaced, in order.
        changes: Vec<pm_lsp::TextChange>,
    },
    /// Closes an open file, stopping its language server when no other open file of the folder uses it.
    CloseDocument {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
    },
    /// What could be written at a place of an open file, once the file reaches `version`.
    Completion {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_buffer::Position,
        /// The character typed that asked for completions, when one did.
        #[serde(default)]
        trigger: Option<String>,
    },
    /// The edits that format an open file as its language says, once the file reaches `version`.
    Format {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text the edits are made to.
        version: i32,
    },
    /// The places looked for from a place of an open file, once the file reaches `version`: where the thing there is defined, unless `seek` names something else.
    Definition {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_buffer::Position,
        /// What is looked for.
        #[serde(default)]
        seek: pm_lsp::Seek,
    },
    /// Who last changed a line of a file of a folder, and in which commit.
    Blame {
        /// The folder.
        directory: String,
        /// The path, from the root of the folder.
        path: String,
        /// The line, counted from 1.
        line: usize,
    },
    /// What the language servers last said is wrong with the files of a folder, open or not.
    Problems {
        /// The folder.
        directory: String,
    },
    /// The lines of the files of a folder's repository that hold some text, tracked or not but not ignored.
    SearchFiles {
        /// The folder.
        directory: String,
        /// What is looked for.
        query: String,
        /// Whether the query is a regular expression.
        #[serde(default)]
        regex: bool,
        /// Whether upper and lower case must match.
        #[serde(default)]
        case: bool,
    },
    /// The symbols of a folder whose names match a query, as the language server of an open file finds them.
    WorkspaceSymbols {
        /// The folder.
        directory: String,
        /// The open file whose language server is asked, from the root of the folder.
        path: String,
        /// What the names are matched against.
        query: String,
    },
    /// The stretches of an open file where the thing at a place is used, once the file reaches `version`.
    Highlights {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_buffer::Position,
    },
    /// The signature of the call a place of an open file is in, once the file reaches `version`.
    Signature {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_buffer::Position,
    },
    /// Renames the thing at a place of an open file across its project, once the file reaches `version`.
    Rename {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_buffer::Position,
        /// The new name.
        name: String,
    },
    /// The changes the language server offers for a stretch of an open file, once the file reaches `version`.
    CodeActions {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text the stretch is in.
        version: i32,
        /// The stretch.
        span: pm_buffer::Span,
    },
    /// Takes one of the changes last offered for an open file.
    TakeCodeAction {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The change, by its place in the list offered.
        index: usize,
    },
    /// What the language server says about the thing at a place of an open file, once the file reaches `version`.
    Hover {
        /// The folder.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_buffer::Position,
    },
    /// Fetches a repository into a folder of its own, named after it.
    Clone {
        /// Where to fetch it from.
        url: String,
        /// The folder to put it in, made when it is not there; the usual folder for clones when nothing.
        #[serde(default)]
        under: Option<String>,
    },
}
