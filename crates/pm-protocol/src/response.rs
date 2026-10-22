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
    /// The id of the login it runs as, or nothing for the agent's own.
    #[serde(default)]
    pub login: Option<String>,
    /// The submodule its agent works in, from the root of the project, or nothing for the project itself.
    #[serde(default)]
    pub submodule: Option<String>,
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
        /// The folders that hold no tracked file, including immediate subfolders of expanded ignored folders.
        #[serde(default)]
        folders: Vec<String>,
        /// Which of `paths` and `folders` git ignores, which are listed so the tree can show them dimmed.
        #[serde(default)]
        ignored: Vec<String>,
    },
    /// The branches of a repository.
    Branches {
        /// Their names, the current one first.
        names: Vec<String>,
    },
    /// The submodules of a repository.
    Submodules {
        /// Where each is, nested ones included, from the root of the repository.
        paths: Vec<String>,
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
        /// The installed language the file is written in, by folder name, when one is.
        #[serde(default)]
        language: Option<String>,
    },
    /// The bytes a file holds.
    Bytes {
        /// What it holds, cut short when it was longer than can be carried.
        #[serde(with = "crate::bytes")]
        bytes: Vec<u8>,
        /// Whether the bytes were cut short.
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
        /// The submodules its worktree could not check out, each as its path and why; the session started without them.
        #[serde(default)]
        skipped: Vec<(String, String)>,
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
    /// The customizations an agent is given in a project.
    Library {
        /// The product the agent runs, when it is one the server knows the files of.
        product: Option<crate::library::Product>,
        /// What was found, kind by kind.
        items: Vec<crate::library::Item>,
        /// The keys of what is turned off.
        withheld: Vec<String>,
    },
    /// The tools an agent is given.
    ToolSets {
        /// The product the agent runs, when it is one the server knows the files of.
        product: Option<crate::library::Product>,
        /// Its own tools first, then each tool server's.
        sets: Vec<crate::library::ToolSet>,
        /// The keys of what is turned off.
        withheld: Vec<String>,
    },
    /// What the reader set up for the work the server does on its machine.
    MachineSettings {
        /// The settings, whole.
        settings: crate::MachineSettings,
    },
    /// The newest release on a channel.
    LatestRelease {
        /// Its version, as `0.14.0` or, for the nightly build, the date it was made; nothing when none could be found.
        version: Option<String>,
        /// The page it is published on.
        url: String,
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
    /// What a folder can be built, tested and run with.
    Tasks {
        /// Each one, those the project writes first.
        tasks: Vec<crate::Task>,
        /// Why the project's own file could not be read, when it could not.
        #[serde(default)]
        problem: Option<String>,
    },
    /// The toolchains a project can start its tasks and tools with.
    Toolchains {
        /// Each one, grouped by language.
        toolchains: Vec<crate::ToolchainInfo>,
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
    /// The extensions there are to install and those installed.
    Extensions {
        /// Each one, by name; hidden ones included.
        extensions: Vec<crate::ExtensionInfo>,
    },
    /// What an extension package in a folder carries and asks for, not installed.
    ExtensionPreview {
        /// The extension; `provides` and `requests` say what it carries and what it asks.
        extension: crate::ExtensionInfo,
    },
    /// What the installed extensions bring that a window uses itself.
    ExtensionAssets(crate::ExtensionAssets),
    /// What a command an extension brings came to.
    ExtensionCommand {
        /// The text to put in the prompt box, when the command gives some.
        prompt: Option<String>,
        /// A line to tell the reader, when the command has one.
        message: Option<String>,
    },
    /// An installed language's grammar and queries.
    Grammar(crate::GrammarBundle),
    /// A file was opened for its language server.
    Opened {
        /// The characters after which the server offers completions without being asked; empty when the language has no server.
        triggers: Vec<String>,
        /// The characters after which the server tells what the call the caret is in looks like.
        #[serde(default)]
        signature: Vec<String>,
        /// The characters after which the server formats what was just typed.
        #[serde(default)]
        format: Vec<String>,
        /// Whether the server formats a stretch of a file.
        #[serde(default)]
        formats_ranges: bool,
        /// Whether the server says which blocks of a file can be folded.
        #[serde(default)]
        folds: bool,
        /// Whether the server says which stretches of a file are edited together.
        #[serde(default)]
        links: bool,
        /// Whether the server says what each name of a file is.
        #[serde(default)]
        semantic_tokens: bool,
        /// Whether the server says what to show inside a line.
        #[serde(default)]
        inlay_hints: bool,
        /// Whether the server says what to show above a line.
        #[serde(default)]
        code_lens: bool,
        /// Whether the server says which names to show the values of while a program is paused.
        #[serde(default)]
        inline_values: bool,
    },
    /// What could be written at a place.
    Completions {
        /// The best match for the word typed there first.
        items: Vec<pm_lsp::Completion>,
    },
    /// The edits that format a file, each a stretch of its text at the version asked for and what replaces it; none when its language server cannot format.
    Formatted {
        /// The edits, in any order; they do not overlap.
        edits: Vec<pm_lsp::TextChange>,
    },
    /// The blocks of lines of a file that can be folded.
    FoldingRanges {
        /// The blocks, as the server gives them.
        ranges: Vec<pm_lsp::FoldRange>,
    },
    /// What each name of a file is, one token to a stretch of a line; none when the language server does not say.
    SemanticTokens {
        /// The tokens, in the order of the file.
        tokens: Vec<pm_lsp::SemanticToken>,
    },
    /// What to show inside the lines of a file; none when the language server does not say.
    InlayHints {
        /// The hints, in the order of the file.
        hints: Vec<pm_lsp::InlayHint>,
    },
    /// Which stretches of a file to show the values of while a program is paused; none when the language server does not say.
    InlineValues {
        /// The stretches, with what each shows.
        values: Vec<pm_lsp::InlineValue>,
    },
    /// What to show above the lines of a file; none when the language server does not say.
    CodeLenses {
        /// The lenses, in the order the server gave them, which is the order they are pressed by.
        lenses: Vec<pm_lsp::CodeLens>,
    },
    /// What pressing a lens came to.
    LensPressed {
        /// The places it points to, to be shown, as the uses of a name are.
        places: Vec<pm_lsp::Location>,
        /// The changes it made to the open files, for the client to make.
        files: Vec<pm_lsp::FileEdit>,
    },
    /// The stretches of a file that are edited together with the one at a place; none when there are none.
    LinkedRanges {
        /// The stretches, the one at the place among them.
        spans: Vec<pm_buffer::Span>,
    },
    /// Where the thing at a place is defined.
    Definitions {
        /// Each place, the one the server names first first.
        locations: Vec<pm_lsp::Location>,
    },
    /// The commits that changed a file, newest first.
    FileHistory {
        /// The commits.
        commits: Vec<pm_git::FileCommit>,
    },
    /// The states a file was kept in, newest first.
    LocalHistory {
        /// The states.
        versions: Vec<crate::LocalVersion>,
    },
    /// The versions of a file left in conflict.
    Conflict {
        /// What each side holds, and what the working folder does.
        files: crate::ConflictFiles,
    },
    /// The files a commit or a branch changed.
    RevisionChanges {
        /// The files, with their diffs.
        changes: Vec<Change>,
    },
    /// A web address.
    Address {
        /// The address.
        url: String,
    },
    /// Who last changed a line of a file.
    Blamed {
        /// The commit and its author.
        blame: pm_git::Blame,
    },
    /// What the language servers say is wrong with the files of a folder.
    Problems {
        /// What is wrong, the worst first.
        problems: Vec<pm_lsp::Problem>,
    },
    /// The stretches of a file where the thing at a place is used.
    Highlights {
        /// The stretches.
        spans: Vec<pm_buffer::Span>,
    },
    /// The signature of the call a place is in.
    Signature {
        /// The signature, or nothing when the place is in no call.
        signature: Option<pm_lsp::Signature>,
    },
    /// The changes the language server offers for a stretch.
    CodeActions {
        /// Each change, in the order the server gave them.
        actions: Vec<pm_lsp::CodeAction>,
    },
    /// An edit across a project was made: files no client has open were changed on disk, and these are the changes to the open ones, for the client to make.
    Edited {
        /// The changes to each open file.
        files: Vec<pm_lsp::FileEdit>,
    },
    /// What the language server says about the thing at a place.
    Hovered {
        /// What it says, or nothing when it says nothing.
        hover: Option<pm_lsp::Hover>,
    },
    /// What a folder can be debugged as.
    DebugScenarios {
        /// Each one, in the order a client chooses by.
        scenarios: Vec<crate::DebugScenario>,
    },
    /// The programs a debugger can attach to, the newest first, and the adapters that can attach to them.
    DebugProcesses {
        /// The adapters installed on the machine.
        adapters: Vec<crate::DebugAdapter>,
        /// Each one.
        processes: Vec<crate::DebugProcess>,
    },
    /// A debugging session was started, or is being watched.
    Debug {
        /// The session.
        debug: u64,
        /// What it looks like now.
        view: crate::DebugView,
    },
    /// The debugging sessions the server holds.
    Debugs {
        /// Each one, oldest first.
        debugs: Vec<crate::DebugInfo>,
    },
    /// The folder a repository was cloned into.
    Directory {
        /// Its path on the server's machine.
        path: String,
    },
    /// A session was not closed because its worktree has changes that are not committed; closing it with `discard` removes them.
    Uncommitted,
}
