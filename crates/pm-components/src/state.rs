//! What the workspace remembers: the page on screen, the panels that are open and what was typed.

use crate::areas::SidebarEvent;
use crate::areas::{
    BrowserEvent, ChatEvent, ComposerEvent, DebugEvent, DialogEvent, DiffEvent, EditorEvent,
    GitEvent, SearchEvent, SettingsEvent, TerminalEvent, TreeEvent, WindowEvent,
};

use crate::events::{Event, FieldEvent};
use crate::target::Target;
use pm_editor::Edit;
use pm_harness::{Event as HarnessEvent, Setting, Slash};
use rustle::TextFieldState;

use crate::adder::{Adder, is_adder};
use crate::arrange::{ProjectDrag, ProjectMenu, SessionMenu, is_session_menu_item};
use crate::context_menu::ContextMenu;

use crate::folder_picker::{FolderPicker, is_picker};
use crate::git_controls::GitControls;

use crate::message_menu::MessageMenu;
use crate::project::Project;
use crate::sample::{Change, Comment, Message, Range, Session, Status, projects, sessions};
use crate::tree_actions::TreeTarget;

/// A prompt holding `text`, which may run to several lines.
pub fn prompt_field(text: &str) -> TextFieldState {
    TextFieldState::new(text)
}

/// A box for a commit message, which runs to several lines.
pub fn commit_field() -> TextFieldState {
    TextFieldState::new("")
}

/// A folder of a machine, which the problems of the dock are about: the machine the folder is on, when it is not this one, and the folder.
pub type ProblemsKey = (Option<String>, String);

/// How many values of the open menu can be chosen.
pub const MENU_PICK_ROOM: usize = 100_000;

/// What the rows of the goal's menu send to the agent, in the order of [`ComposerEvent::GoalRow`].
pub const GOAL_COMMANDS: [&str; 3] = ["/goal pause", "/goal resume", "/goal clear"];

/// Something the window has to do or tell an agent, taken off the state by whoever runs the agents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outgoing {
    /// Asks the server of the session at this position for the grammar of a language, by its folder name or another name.
    Grammar {
        /// The session, by position.
        session: usize,
        /// The language.
        language: String,
    },
    /// Opens a file of the session at this position for its language server.
    OpenDocument {
        /// The session, by position.
        session: usize,
        /// The file, from the root of the session's folder.
        path: String,
        /// The installed language it is written in.
        language: String,
        /// The version of its text.
        version: i32,
        /// Its text.
        text: String,
    },
    /// Tells the language server how a file of the session at this position was edited.
    ChangeDocument {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version the edits take the text to.
        version: i32,
        /// The stretches replaced, in order.
        changes: Vec<pm_protocol::TextChange>,
    },
    /// Tells the language server a file of the session at this position was closed.
    CloseDocument {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
    },
    /// Asks the language server of a file of the session at this position what it adds to one of the completions it listed.
    ResolveCompletion {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The item as its server wrote it.
        item: String,
    },
    /// Asks the language server what could be written at a place of a file of the session at this position.
    Completion {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_protocol::Position,
        /// The character typed that asked, when one did.
        trigger: Option<String>,
        /// The name of the scope of the language the place is in, when it is in one.
        scope: Option<String>,
    },
    /// Asks the language server for the edits that format a file of the session at this position.
    Format {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the edits are for.
        version: i32,
        /// Whether the file is written to the folder once it is formatted.
        save: bool,
        /// The stretch to format, when only a stretch is.
        range: Option<pm_protocol::Span>,
    },
    /// Asks the language server for the edits that format what was just typed in a file of the session at this position.
    FormatOnType {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the edits are for.
        version: i32,
        /// The place the caret is at after the character.
        position: pm_protocol::Position,
        /// The character typed.
        character: String,
    },
    /// Asks the language server which blocks of a file of the session at this position can be folded.
    FoldingRanges {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the blocks are about.
        version: i32,
    },
    /// Asks the language server which stretches of a file of the session at this position are edited together with the one at a place.
    LinkedRanges {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_protocol::Position,
    },
    /// Asks the language server where the thing at a place of a file of the session at this position is defined.
    Definition {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_protocol::Position,
        /// What is looked for.
        seek: pm_protocol::Seek,
    },
    /// Asks who last changed a line of a file of the session at this position.
    Blame {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The line, counted from 1.
        line: usize,
    },
    /// Asks what the language servers say is wrong with the files of the project of the session at this position.
    Problems {
        /// The session, by position.
        session: usize,
    },
    /// Asks for the lines of the files of the project of the session at this position that hold some text, as the search of the review panel does.
    SearchProject {
        /// The session, by position.
        session: usize,
        /// What is looked for.
        query: String,
        /// Whether upper and lower case must match.
        case: bool,
        /// Whether the query is a regular expression.
        regex: bool,
    },
    /// Asks for the lines of the files of the project of the session at this position that hold some text.
    SearchFiles {
        /// The session, by position.
        session: usize,
        /// What is looked for.
        query: String,
    },
    /// Asks the server of every machine for the sessions whose titles or conversations hold some text.
    SearchChats {
        /// What is looked for.
        query: String,
    },
    /// Asks the language server of a file of the session at this position for the symbols of the project matching a query.
    WorkspaceSymbols {
        /// The session, by position.
        session: usize,
        /// The open file whose language server is asked.
        path: String,
        /// What the names are matched against.
        query: String,
    },
    /// Asks the language server what each name of a file of the session at this position is.
    SemanticTokens {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the names are in.
        version: i32,
        /// The stretch the names are wanted for first; the whole file when nothing.
        range: Option<pm_protocol::Span>,
    },
    /// Asks the language server for the changes of one kind it offers for the whole of a file of the session at this position, takes the one it prefers, and goes on as the rest of the kinds and saving say.
    SourceActions {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the changes are for.
        version: i32,
        /// The whole of the text at that version.
        span: pm_protocol::Span,
        /// The kinds of change to take, one after another, the first asked for now.
        kinds: Vec<String>,
        /// Whether the file is formatted and written after the last of them.
        save: bool,
        /// Whether the reader is told when the server offers nothing.
        notify: bool,
    },
    /// Asks the language server how a colour can be written in place of a stretch of a file of the session at this position.
    ColorPresentations {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the stretch is in.
        version: i32,
        /// The stretch that spells the colour.
        span: pm_protocol::Span,
        /// The colour, as red, green, blue and opacity from 0 to 255.
        color: [u8; 4],
    },
    /// Asks the language server where a link of a file of the session at this position leads, when it gave no address.
    ResolveLink {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The link as its server wrote it.
        link: String,
    },
    /// Asks the language server which stretches of a file of the session at this position lead somewhere when pressed.
    DocumentLinks {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the links are about.
        version: i32,
    },
    /// Asks the language server which stretches of a file of the session at this position spell a colour.
    Colors {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the colours are about.
        version: i32,
    },
    /// Asks the language server what to show inside the lines of a file of the session at this position.
    InlayHints {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the hints are about.
        version: i32,
        /// The stretch the hints are wanted for.
        span: pm_protocol::Span,
    },
    /// Asks the language server which names of a file of the session at this position to show the values of while the program is paused.
    InlineValues {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the stretches are about.
        version: i32,
        /// The stretch the values are wanted for.
        span: pm_protocol::Span,
        /// The frame of the stack the debugger is looking at.
        frame: i64,
        /// Where the program is paused.
        stopped: pm_protocol::Span,
    },
    /// Asks the language server what to show above the lines of a file of the session at this position.
    CodeLenses {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the lenses are about.
        version: i32,
        /// Whether the newest versions of the dependencies the file lists are looked up.
        dependencies: bool,
    },
    /// Presses one of the lenses the language server last gave for a file of the session at this position.
    PressCodeLens {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The lens, by its place in the list given.
        index: usize,
    },
    /// Asks the language server where the thing at a place of a file of the session at this position is used in that file.
    Highlights {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_protocol::Position,
    },
    /// Asks the language server what the call a place of a file of the session at this position is in looks like.
    Signature {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_protocol::Position,
    },
    /// Asks the language server what can be renamed at a place of a file of the session at this position.
    PrepareRename {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_protocol::Position,
    },
    /// Asks the language server to rename the thing at a place of a file of the session at this position across its project.
    RenameSymbol {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_protocol::Position,
        /// The new name.
        name: String,
    },
    /// Asks the language server what changes it offers for a stretch of a file of the session at this position.
    CodeActions {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the stretch is in.
        version: i32,
        /// The stretch.
        span: pm_protocol::Span,
    },
    /// Takes one of the changes the language server last offered for a file of the session at this position.
    TakeCodeAction {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The change, by its place in the list offered.
        index: usize,
    },
    /// Asks the server of the session at this position for the diffs of changed files it listed without them.
    Diffs {
        /// The session, by position.
        session: usize,
        /// The files, as the changes name them.
        paths: Vec<String>,
    },
    /// Reads a file of the session at this position as the last commit holds it, to mark how the open file differs from it.
    ReadBase {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
    },
    /// Shows a file of the session at this position in the file manager of this machine.
    RevealFile {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
    },
    /// Copies or opens the web address of a file of the session at this position, or of a line of it, on the host of its repository.
    Permalink {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The line, counted from 1, or nothing for the whole file.
        line: Option<usize>,
        /// The last line of the range that starts at `line`, counted from 1, or nothing for the one line.
        end_line: Option<usize>,
        /// Whether the address is opened in the browser rather than copied.
        open: bool,
    },
    /// Asks for the commits that changed a file of the session at this position.
    FileHistory {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
    },
    /// Asks for the states a file of the session at this position was kept in each time it was saved.
    LocalHistory {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
    },
    /// Reads one of the states a file of the session at this position was kept in, to set the file against it.
    ReadVersion {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The state, by the number the history gave it.
        version: u64,
        /// How long ago it was saved, as it is said.
        when: String,
    },
    /// Asks for the versions of a conflicted file of the session at this position, to open the resolver on it.
    ReadConflict {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
    },
    /// Writes a conflicted file of the session at this position as it was resolved, and stages it.
    ResolveConflict {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// What the file holds once resolved.
        text: String,
    },
    /// Asks for the files a commit changed, or a branch changed since it left another, in the folder of the session at this position.
    RevisionChanges {
        /// The session, by position.
        session: usize,
        /// The branch or commit to count from; the commit's own parent when nothing.
        from: Option<String>,
        /// The commit or branch.
        to: String,
        /// What the view of the files is called.
        label: String,
    },
    /// Reads a file of the session at this position as a commit left it, to show it read only.
    ReadRevision {
        /// The session, by position.
        session: usize,
        /// The path the file had in the commit.
        path: String,
        /// The commit, by its full object name.
        revision: String,
        /// What the tab is called.
        label: String,
    },
    /// Asks the language server what it says about a place of a file of the session at this position.
    Hover {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the place is in.
        version: i32,
        /// The place.
        position: pm_protocol::Position,
    },
    /// Asks the server on this machine for the extensions there are to install.
    Extensions,
    /// Has the server on this machine install extensions.
    InstallExtensions {
        /// The extensions, by folder name.
        ids: Vec<String>,
    },
    /// Asks the server on this machine what the extension in a folder of this machine carries and asks to be allowed, before anything is installed.
    InspectExtension {
        /// The folder.
        path: String,
    },
    /// Has the server on this machine install the extension in a folder of this machine, with the capabilities the reader allowed it.
    InstallExtension {
        /// The folder.
        path: String,
        /// What the reader allowed, by the names of capabilities.
        grants: Vec<String>,
    },
    /// Has the server on this machine take an extension package off; the packages of other machines are theirs.
    RemovePackage {
        /// The extension, by folder name.
        id: String,
    },
    /// Allows an extension installed on this machine a capability, or takes it back.
    GrantExtension {
        /// The extension, by folder name.
        id: String,
        /// The capability, by name.
        capability: String,
        /// Whether it is allowed.
        granted: bool,
    },
    /// Asks the server of a machine for the themes, icon themes and commands its extensions bring.
    ExtensionAssets {
        /// The machine, as projects name it; nothing for this one.
        host: Option<String>,
    },
    /// Runs a command an extension of a machine brings, to put what it comes to in the prompt box.
    RunExtensionCommand {
        /// The machine, as projects name it; nothing for this one.
        host: Option<String>,
        /// The extension, by folder name.
        extension: String,
        /// The command.
        command: String,
        /// What was typed after its name.
        argument: String,
        /// The project folder it runs in, when it runs in one.
        directory: Option<String>,
        /// The session open when it was taken, by position, to tell what it came to in.
        session: Option<usize>,
    },
    /// Has the server on this machine take an extension off.
    RemoveExtension {
        /// The extension, by folder name.
        id: String,
    },
    /// Asks the server for the agents added by hand and the tool servers every agent is started with.
    LoadServers,
    /// Asks every connected server what its sessions used over the last `days` days, today included.
    UsageReport {
        /// How many days the report covers.
        days: u32,
    },
    /// Opens `address` in the reader's browser.
    Open {
        /// The page to open.
        address: String,
    },
    /// Asks the public registry of tool servers for what matches `query`.
    SearchTools {
        /// What to look for; the well known servers when blank.
        query: String,
    },
    /// Has the server keep these as the tool servers every agent is started with.
    SaveMcpServers(Vec<pm_protocol::McpServer>),
    /// Asks the server for the model servers declared on its machine.
    ModelServers,
    /// Has the harness make `change` to the model slots, when there is one, then tell the slots and presets: through the agent of the session at `session` when there is one, which also sees its project's file.
    ModelSlots {
        /// The session, by position, whose agent is asked.
        session: Option<usize>,
        /// The change to make first.
        change: Option<pm_harness::SlotChange>,
    },
    /// Has the server keep this model server, in place of the one with the same id.
    SaveModelServer(Box<pm_harness::ModelServer>),
    /// Has the server take the model server `id` out.
    RemoveModelServer {
        /// The id of the server.
        id: String,
    },
    /// Asks the server which models this model server offers, the answer coming back under `ticket`.
    FindModels {
        /// The number the answer is matched to the question by.
        ticket: u64,
        /// The server to ask.
        server: Box<pm_harness::ModelServer>,
    },
    /// Lists what the agent at `agent` is given in the project at `project`: its skills, instructions, agent definitions and hooks.
    Library {
        /// The agent, by position.
        agent: usize,
        /// The project, by position, or nothing when none is open.
        project: Option<usize>,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The project's folder, or empty when none is open.
        directory: String,
    },
    /// Makes one of crab's prompt files, empty, when it is not there yet, for the project at `project`, and opens it.
    CreatePromptFile {
        /// The project, by position, or nothing when none is open.
        project: Option<usize>,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The project's folder, or empty when none is open.
        directory: String,
        /// Which file.
        file: pm_protocol::library::PromptFile,
        /// Whether it goes in the project's folder or the reader's own.
        scope: pm_protocol::library::Scope,
    },
    /// Writes a new skill, instruction file or agent definition from a template for the project at `project`.
    CreateCustomization {
        /// The project, by position, or nothing when none is open.
        project: Option<usize>,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The project's folder, or empty when none is open.
        directory: String,
        /// What it is.
        kind: pm_protocol::library::Kind,
        /// Whether it goes in the project's folder or the reader's own.
        scope: pm_protocol::library::Scope,
        /// Its name.
        name: String,
        /// What it is for.
        description: String,
    },
    /// Lists the tools the agent at `agent` is given, on the machine of the project at `project`.
    ToolSets {
        /// The agent, by position.
        agent: usize,
        /// The project, by position, or nothing when none is open.
        project: Option<usize>,
        /// The machine, or nothing for this one.
        host: Option<String>,
    },
    /// Has the server of a machine keep `keys` as what it leaves out of what agents are given.
    SaveWithheld {
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// Every key turned off.
        keys: Vec<String>,
    },
    /// Opens the file at `path` of this machine in the program the system opens it with.
    OpenPath {
        /// The file.
        path: String,
    },
    /// Has every server the window reaches keep these as what the reader set up for the work it does on its machine.
    SaveMachineSettings(Box<pm_protocol::MachineSettings>),
    /// Tells the reader something happened while they may be looking elsewhere, in the way they chose.
    Notify {
        /// What happened, in a few words.
        title: String,
        /// More about it.
        body: String,
        /// How they are told.
        how: pm_config::Notification,
    },
    /// Asks the server on this machine to download the release `version` on `channel`, and installs it and restarts into it.
    DownloadUpdate {
        /// The channel the window follows.
        channel: pm_config::Channel,
        /// The version to install.
        version: String,
    },
    /// Stops the download of the update.
    CancelUpdate,
    /// Asks the server on this machine for the newest release on `channel`.
    LatestRelease {
        /// The channel the window follows.
        channel: pm_config::Channel,
    },
    /// Asks the server what the folder of the session at this position can be debugged as.
    DebugScenarios {
        /// The session, by position.
        session: usize,
        /// The file in front, from the root of the session's folder.
        file: Option<String>,
    },
    /// Asks the server on the machine of the session at this position for the programs a debugger can attach to.
    DebugProcesses {
        /// The session, by position.
        session: usize,
    },
    /// Starts debugging the folder of the session at this position as one of the things listed.
    DebugStart {
        /// The session, by position.
        session: usize,
        /// The file in front, as it was when the list was asked for.
        file: Option<String>,
        /// The scenario, by its place in the list.
        scenario: usize,
        /// The breakpoints to stop at, file by file.
        files: Vec<pm_protocol::DebugFile>,
    },
    /// Starts debugging a program that is running on the machine of the session at this position.
    DebugAttach {
        /// The session, by position.
        session: usize,
        /// The adapter, by the name the server gives it.
        adapter: String,
        /// The program's process number.
        pid: u32,
        /// The breakpoints to stop at, file by file.
        files: Vec<pm_protocol::DebugFile>,
    },
    /// Does something to the program debugged in the folder of the session at this position.
    DebugDo {
        /// The session, by position.
        session: usize,
        /// The debugging session, by the number the server gave it.
        debug: u64,
        /// What is done.
        action: pm_protocol::DebugAction,
    },
    /// Asks the server what the folder of the session at this position can be built, tested and run with.
    Tasks {
        /// The session, by position.
        session: usize,
    },
    /// Asks the machine of the session at this position which tasks can run a place of a file that its language marks as runnable.
    RunnableTasks {
        /// The session, by position.
        session: usize,
        /// The file, from the root of the session's folder.
        path: String,
        /// What its language tagged it with.
        tags: Vec<String>,
        /// What each of its captures matched, by the capture's name.
        variables: std::collections::BTreeMap<String, String>,
        /// Its line and its column, from zero.
        position: (usize, usize),
    },
    /// Runs a task in the shell of a tab, in the folder of the session at this position.
    RunTask {
        /// The session, by position.
        session: usize,
        /// The task.
        task: pm_protocol::Task,
        /// The tab, by the name this window knows it by.
        tab: u64,
        /// How many columns wide its window is.
        columns: u16,
        /// How many lines tall its window is.
        rows: u16,
    },
    /// Runs a place of a file that its language marks as runnable in the shell of a tab, in the folder of the session at this position.
    RunRunnable {
        /// The session, by position.
        session: usize,
        /// The file, from the root of the session's folder.
        path: String,
        /// What its language tagged it with.
        tags: Vec<String>,
        /// What each of its captures matched, by the capture's name.
        variables: std::collections::BTreeMap<String, String>,
        /// Its line and its column, from zero.
        position: (usize, usize),
        /// The name of the task tied to it that is run in place of the first, when one was chosen.
        task: Option<String>,
        /// The tab, by the name this window knows it by.
        tab: u64,
        /// How many columns wide its window is.
        columns: u16,
        /// How many lines tall its window is.
        rows: u16,
    },
    /// Asks the machine of the session at this position which toolchains it has.
    Toolchains {
        /// The session, by position.
        session: usize,
    },
    /// Has the machine of the session at this position run a language of the session's project with a toolchain.
    SelectToolchain {
        /// The session, by position.
        session: usize,
        /// The language, by family.
        language: String,
        /// The toolchain, by the id the server gave it, or nothing for the machine's own path.
        toolchain: Option<String>,
    },
    /// Starts a shell for the tab, in the folder of the session at this position.
    OpenTerminal {
        /// The session, by position.
        session: usize,
        /// The folder inside the session's folder the shell starts in, from its root; empty for the root itself.
        folder: String,
        /// The tab, by the name this window knows it by.
        tab: u64,
        /// How many columns wide its window is.
        columns: u16,
        /// How many lines tall its window is.
        rows: u16,
    },
    /// Types into the shell of a tab.
    TerminalInput {
        /// The tab.
        tab: u64,
        /// The bytes the shell reads.
        data: Vec<u8>,
    },
    /// Tells the shell of a tab how large its window is.
    ResizeTerminal {
        /// The tab.
        tab: u64,
        /// How many columns wide.
        columns: u16,
        /// How many lines tall.
        rows: u16,
    },
    /// Ends the shell of a tab.
    CloseTerminal {
        /// The tab.
        tab: u64,
    },
    /// Shows the folder of the session at this position in the file manager, when it is on this machine.
    OpenFolder {
        /// The session, by position.
        session: usize,
    },
    /// Gives the session at this position a new title.
    Rename {
        /// The session, by position.
        session: usize,
        /// What it is called now.
        title: String,
    },
    /// Closes the session at this position: its agent is stopped, its worktree is removed and it leaves the sidebar, the server keeping its log.
    Close {
        /// The session, by position.
        session: usize,
        /// Whether the worktree goes even though it has changes that are not committed.
        discard: bool,
    },
    /// Tells the window's own bookkeeping that the sessions moved: for each old position, the new one, or nothing for a session that is gone.
    Reindexed(Vec<Option<usize>>),
    /// Starts the agent of a new session, in a worktree off `base` when one is given, and hands it the words.
    Start {
        /// The session, by position.
        session: usize,
        /// What the reader wrote.
        text: String,
        /// The files attached, by path.
        files: Vec<String>,
        /// The settings the reader changed before starting, as the agent names them and their values.
        settings: Vec<(String, String)>,
        /// The branch to make the session's worktree from, when it works in one of its own.
        base: Option<String>,
        /// Whether its agent and terminals run in a dev container.
        container: bool,
    },
    /// Asks whether the folder of a project holds a `devcontainer.json`.
    DevContainer {
        /// The project, by position.
        project: usize,
        /// The machine it is on, when not this one.
        host: Option<String>,
        /// Its folder.
        directory: String,
    },
    /// Saves the named logins and the one in use for each agent, and points the agents at them.
    Accounts {
        /// The named logins.
        logins: Vec<(String, String, String)>,
        /// The login in use for each agent.
        in_use: Vec<(String, String)>,
    },
    /// Has a session run as a login, or as its agent's own when there is none.
    SessionAccount {
        /// The session, by position.
        session: usize,
        /// The login's id.
        login: Option<String>,
    },
    /// Lists the files of the folder `directory` on `host`, for the list behind an `@`.
    Files {
        /// What the answer is held under, to tell whether it is still the one wanted.
        key: String,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The folder.
        directory: String,
        /// Ignored folders currently unfolded in the tree.
        expanded: Vec<String>,
    },
    /// Lists the submodules of the repository `directory` on `host`, for the project at `project`.
    Submodules {
        /// The project, by position.
        project: usize,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The folder.
        directory: String,
    },
    /// Asks the server of the chat at this position to continue it as a session in a project: its agent writes a handoff summary, and the server starts the session with it.
    ContinueChat {
        /// The chat, by position.
        session: usize,
        /// The project the session starts in, by position.
        project: usize,
        /// The branch the session's worktree is made from.
        base: String,
    },
    /// Lists the branches of the repository `directory` on `host`, for the project at `project`.
    Branches {
        /// The project, by position.
        project: usize,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The folder.
        directory: String,
    },
    /// Asks how much of its allowance the account of the agent at `agent`, signed in under the login `login`, has used.
    Allowance {
        /// The agent, by position.
        agent: usize,
        /// The added account, by id, or nothing for the agent's own login.
        login: Option<String>,
    },
    /// Asks the agent at `agent` on `host` what it lets the reader change and offers behind a slash.
    Discover {
        /// The agent, by position.
        agent: usize,
        /// The machine, or nothing for this one.
        host: Option<String>,
    },
    /// Asks who the agent at `agent` is signed in as, under the login `naming` and naming it after the answer, or under the login in use when there is none.
    Identity {
        /// The agent, by position.
        agent: usize,
        /// The login the answer names, by id.
        naming: Option<String>,
    },
    /// Signs the agent, by its position, in under the login `login`, or the one now in use when there is none.
    SignIn {
        /// The agent, by position.
        agent: usize,
        /// The login's id.
        login: Option<String>,
    },
    /// Keeps `key` as the API key of the agent, by position, or forgets the saved one when `key` is nothing.
    SaveKey {
        /// The agent, by position.
        agent: usize,
        /// The key, or nothing to forget the saved one.
        key: Option<String>,
        /// The added account the key belongs to, by id, or nothing for the agent's own login.
        login: Option<String>,
    },
    /// Signs in again as the account of the open session.
    SignInSession {
        /// The session, by position.
        session: usize,
    },
    /// Hands the agent of a session the words as its next turn, or queues them while it is on one.
    Prompt {
        /// The session, by position.
        session: usize,
        /// What the reader wrote.
        text: String,
        /// The files attached, by path.
        files: Vec<String>,
        /// Whether to stop the turn the agent is on and hand it these words in its place.
        now: bool,
    },
    /// Stops the turn the agent of a session is on and hands it a queued prompt in its place.
    Hurry {
        /// The session, by position.
        session: usize,
        /// The queued prompt, by the number the server gave it.
        queued: u64,
    },
    /// Removes a waiting prompt and its attached files without sending it.
    RemoveQueued {
        /// The session.
        session: usize,
        /// The queued prompt, by the id its `Queued` event gave it.
        queued: u64,
    },
    /// Holds the queue of a session while a waiting prompt is edited, or lets it go on.
    HoldQueue {
        /// The session.
        session: usize,
        /// Whether to hold the queue.
        held: bool,
    },
    /// Replaces the text of a waiting prompt, which keeps its place and its files, and lets the queue go on.
    EditQueued {
        /// The session.
        session: usize,
        /// The queued prompt, by the id its `Queued` event gave it.
        queued: u64,
        /// What the prompt says from now on.
        text: String,
    },
    /// Answers the question the agent of a session asked.
    Choose {
        /// The session, by position.
        session: usize,
        /// The question, by the number the agent put it under.
        ticket: u64,
        /// The choice made.
        option: String,
    },
    /// Answers the form of questions the agent of a session asked, or abandons it.
    Answer {
        /// The session, by position.
        session: usize,
        /// The form, by the number the agent put it under.
        ticket: u64,
        /// What was answered to each question, or nothing to abandon the form.
        replies: Option<Vec<pm_harness::Reply>>,
    },
    /// Answers the request of the agent of a session to trust its project.
    Trust {
        /// The session, by position.
        session: usize,
        /// The request, by the number the agent put it under.
        ticket: u64,
        /// What the reader decided.
        verdict: pm_harness::Verdict,
    },
    /// Takes the conversation of a session back to before one of the reader's turns, dropping it and all after it.
    Rewind {
        /// The session, by position.
        session: usize,
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
    },
    /// Starts a session that branches off the conversation of a session after one of the reader's turns and the agent's reply to it.
    Fork {
        /// The session, by position.
        session: usize,
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
    },
    /// Puts the files of a session back as they were before one of the reader's turns.
    Restore {
        /// The session, by position.
        session: usize,
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
    },
    /// Asks for the files the agent of a session changed during one of the reader's turns.
    TurnChanges {
        /// The session, by position.
        session: usize,
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
    },
    /// Takes what one of the reader's turns changed back out of the files of a session.
    UndoTurn {
        /// The session, by position.
        session: usize,
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
        /// The files to take the changes out of, or every file the turn changed when there are none.
        paths: Vec<String>,
    },
    /// Stops the turn the agent of a session is on.
    Cancel {
        /// The session, by position.
        session: usize,
    },
    /// Stops one subagent of the agent of a session, while the turn goes on.
    StopSubagent {
        /// The session, by position.
        session: usize,
        /// The subagent, by the id it is stopped by.
        subagent: String,
    },
    /// Gives a setting of the agent of a session a value.
    Set {
        /// The session, by position.
        session: usize,
        /// What the agent calls the setting.
        setting: String,
        /// The value.
        value: String,
    },
    /// Stages or unstages files in the folder of a session.
    Stage {
        /// The session, by position.
        session: usize,
        /// The files, from the root of the repository.
        paths: Vec<String>,
        /// Whether to stage them, rather than unstage them.
        stage: bool,
    },
    /// Puts files back as they are in the last commit in the folder of a session.
    Discard {
        /// The session, by position.
        session: usize,
        /// The files, from the root of the repository.
        paths: Vec<String>,
    },
    /// Adds files to the `.gitignore` of the folder of a session.
    Ignore {
        /// The session, by position.
        session: usize,
        /// The files, from the root of the repository.
        paths: Vec<String>,
    },
    /// Checks a branch out in the folder of a session.
    Checkout {
        /// The session, by position.
        session: usize,
        /// The branch.
        branch: String,
        /// The submodule of the folder it is done in, from the folder, or nothing for the folder's own repository.
        submodule: Option<String>,
    },
    /// Makes a branch in the folder of a session and checks it out.
    CreateBranch {
        /// The session, by position.
        session: usize,
        /// What the branch is called.
        name: String,
        /// The submodule of the folder it is done in, from the folder, or nothing for the folder's own repository.
        submodule: Option<String>,
    },
    /// Lists the branches of the repository the folder of a session is in.
    SessionBranches {
        /// The session, by position.
        session: usize,
        /// The submodule of the folder it is done in, from the folder, or nothing for the folder's own repository.
        submodule: Option<String>,
    },
    /// Lists what each checked out submodule of the folder of a session has checked out.
    SubmoduleHeads {
        /// The session, by position.
        session: usize,
    },
    /// Reads a file of the folder of a session.
    ReadFile {
        /// The session, by position.
        session: usize,
        /// The file, from the root of the repository.
        path: String,
        /// Whether to read it from the last commit.
        head: bool,
    },
    /// Reads a picture file of the folder of a session as bytes, to be decoded.
    ReadPicture {
        /// The session, by position.
        session: usize,
        /// The file, from the root of the repository.
        path: String,
        /// Whether to read it from the last commit.
        head: bool,
    },
    /// Draws a diagram written in markdown into pixels.
    DrawDiagram {
        /// The key it is kept under.
        key: u64,
        /// What it is written as.
        source: String,
    },
    /// Sets a formula written in TeX in markdown into pixels.
    DrawFormula {
        /// The key it is kept under.
        key: u64,
        /// The TeX it is written in.
        source: String,
    },
    /// Reads a picture a markdown file of the folder of a session shows, to be decoded.
    ReadFigure {
        /// The key it is kept under.
        key: u64,
        /// The session, by position.
        session: usize,
        /// The picture, from the root of the repository.
        path: String,
    },
    /// Fetches a picture markdown shows from the web, to be decoded.
    FetchFigure {
        /// The key it is kept under.
        key: u64,
        /// Its web address.
        address: String,
    },
    /// Draws the text of a vector picture open in a session into pixels.
    DrawVector {
        /// The session, by position.
        session: usize,
        /// The file, from the root of the repository.
        path: String,
        /// Whether it was read from the last commit.
        head: bool,
        /// The text it holds now.
        text: String,
    },
    /// Commits what is staged in the folder of a session.
    Commit {
        /// The session, by position.
        session: usize,
        /// The commit message.
        message: String,
        /// Whether, with nothing staged, to take the changes of every tracked file.
        tracked: bool,
        /// Whether to push the branch afterwards.
        push: bool,
        /// Whether to rewrite the last commit in place of making another.
        amend: bool,
    },
    /// Takes the next step of the merge, rebase or cherry-pick the folder of a session is stopped in.
    Step {
        /// The session, by position.
        session: usize,
        /// `continue`, `skip` or `abort`.
        action: &'static str,
        /// The message to finish a merge or a cherry-pick with.
        message: Option<String>,
    },
    /// Saves the changes of the folder of a session in a stash, or applies, pops or drops one.
    Stash {
        /// The session, by position.
        session: usize,
        /// `push`, `apply`, `pop` or `drop`.
        action: &'static str,
        /// The stash to take, by its index in the list.
        index: Option<usize>,
        /// What to call the stash being made.
        message: Option<String>,
    },
    /// Lists the stashes of the repository the folder of a session is in.
    Stashes {
        /// The session, by position.
        session: usize,
    },
    /// Applies a commit to the branch the folder of a session has checked out.
    CherryPick {
        /// The session, by position.
        session: usize,
        /// The commit, by its abbreviated name.
        commit: String,
    },
    /// Pushes the branch of a session or acts on its pull request.
    PullRequestAction {
        /// The session, by position.
        session: usize,
        /// What to do, as `pull_request_action` names it.
        action: &'static str,
        /// The title a new pull request is given.
        title: String,
        /// The steps of the session's latest plan.
        plan: Vec<String>,
    },
    /// Fetches, pulls or pushes the repository the folder of a session is in, or syncs it.
    Sync {
        /// The session, by position.
        session: usize,
        /// `fetch`, `pull`, `push` or `sync`.
        operation: &'static str,
    },
    /// Asks what the controls over branches and remotes need to know about the repository the folder of a session is in, and about `branch` there.
    BranchInfo {
        /// The session, by position.
        session: usize,
        /// The local branch counted and looked for in other worktrees, or nothing for the remotes alone.
        branch: Option<String>,
    },
    /// Publishes, sets or unsets the upstream of, renames, deletes, merges or rebases a branch of the repository the folder of a session is in.
    BranchOperation {
        /// The session, by position.
        session: usize,
        /// `publish`, `set_upstream`, `unset_upstream`, `rename`, `delete`, `merge` or `rebase`.
        action: &'static str,
        /// The branch acted on.
        branch: String,
        /// The new name, the name on the remote, or the remote branch to follow.
        target: Option<String>,
        /// The remote to publish to.
        remote: Option<String>,
        /// Whether to act on the branch on the remote too.
        with_remote: bool,
    },
    /// Lists the commits of the repository the folder of a session is in.
    Log {
        /// The session, by position.
        session: usize,
        /// Whether to list what every reference reaches.
        all: bool,
    },
    /// Reads the changes of the folder of a session again.
    Refresh {
        /// The session, by position.
        session: usize,
    },
    /// Writes a file in the folder of a session.
    Write {
        /// The session, by position.
        session: usize,
        /// The file, from the root of the folder.
        path: String,
        /// What it holds afterwards.
        text: String,
    },
    /// Makes, renames, copies or removes a file or folder in the folder of a session.
    Entry {
        /// The session, by position.
        session: usize,
        /// What is done.
        change: crate::tree_actions::EntryChange,
    },
    /// Reads files and folders of this machine and writes them into the folder of a session.
    Import {
        /// The session, by position.
        session: usize,
        /// Each file or folder: where it is on this machine, and where it goes from the root of the session's folder.
        files: Vec<(String, String)>,
    },
    /// Lists the GitHub accounts and organizations on this machine, for the panel for adding a project.
    Owners,
    /// Lists the GitHub repositories of the account `login` on this machine, for the panel for adding a project.
    Repositories {
        /// The account's login.
        login: String,
        /// Whether it is an organization.
        organization: bool,
    },
    /// Lists the folders inside the folder `path` on the machine a project is being added on, for the panel for adding a project.
    Folders {
        /// The folder; `~` is the home folder.
        path: String,
        /// The machine whose folders they are, or this one when nothing.
        host: Option<String>,
    },
    /// Asks the servers which sessions they hold again, so that those of a project just added come back.
    Relist,
    /// Clones the repository at `url` into the folder `under` on the machine a project is being added on, for the panel for adding a project.
    Clone {
        /// Where to fetch it from.
        url: String,
        /// The folder to put it in.
        under: String,
        /// The machine to clone it on, or this one when nothing.
        host: Option<String>,
    },
    /// Puts text on the clipboard.
    Copy {
        /// What to copy.
        text: String,
    },
    /// Asks the server for a file of the session at this position as the folder holds it, to compare it with the file `active` as it is being edited.
    ReadToCompare {
        /// The session, by position.
        session: usize,
        /// The file being edited.
        active: String,
        /// The file to read, which is `active` itself to compare it with what was saved of it.
        path: String,
        /// Whether `path` is `active` as it was saved.
        saved: bool,
    },
    /// Asks the language server whether it offers changes at the caret of a file of the session at this position, only to mark the line.
    Lightbulb {
        /// The session, by position.
        session: usize,
        /// The file.
        path: String,
        /// The version of the text the stretch is in.
        version: i32,
        /// The line the caret is on.
        line: usize,
        /// The stretch.
        span: pm_protocol::Span,
    },
}

/// A prompt put aside while another session is worked in: what was typed, the files attached, the elements picked and the captures held.
pub type StashedPrompt = (
    TextFieldState,
    Vec<String>,
    Vec<crate::elements::Element>,
    Vec<crate::captures::Capture>,
);

/// What a menu over the prompt box is open for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Picker {
    /// A setting of the agent, by position among those on offer.
    Setting(usize),
    /// Where the work is done.
    Location,
    /// Which login of the agent is used.
    Account,
    /// What is done with the goal the agent works toward.
    Goal,
    /// Which project, and from which branch, the open chat is continued in.
    Continue,
    /// An element of a web page picked for the prompt, by position among those picked.
    Element(usize),
    /// A screenshot or the console of a web page held for the prompt, by position among those held.
    Capture(usize),
}

/// Where new work is done.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Location {
    /// In a worktree of its own, on a new branch.
    Worktree,
    /// In the project's own folder.
    #[default]
    Local,
}

/// A panel opened from the chips above the prompt box.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Activity {
    /// The agent's plan.
    Plan,
    /// The questions waiting for an answer.
    Approvals,
    /// The subagents the agent started.
    Subagents,
    /// The commands the agent left running.
    Shells,
}

/// What the main area shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Page {
    /// One session, by position.
    Session(usize),
    /// A prompt to start work in one project, by position.
    New(usize),
    /// A prompt to start a chat, which belongs to no project.
    #[default]
    Chat,
    /// The overview of one project, by position: its sessions and its repository.
    Project(usize),
    /// Every session.
    History,
    /// The extensions there are to install.
    Extensions,
}

/// Which view of a session is showing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tab {
    /// The conversation with the agent.
    #[default]
    Chat,
    /// The files the agent changed.
    Diff,
    /// A file being read.
    File,
    /// The lines around the hits of a search, from every file, edited in place.
    Excerpts,
    /// A page of the web.
    Browser,
    /// The notes of the newer release, and the button that installs it.
    Update,
}

/// What the review panel lists.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReviewTab {
    /// The files the agent changed, to stage and commit.
    #[default]
    Changes,
    /// Every file of the project.
    Files,
    /// The text of every file of the project, searched for.
    Search,
}

/// What a comment being started or written is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentTarget {
    /// The changed file, by position.
    pub change: usize,
    /// The lines, or the whole file when there are none.
    pub range: Option<Range>,
    /// The line the pointer went down on while lines are being dragged over.
    pub anchor: usize,
    /// Whether lines are still being dragged over, which is before the box for the comment shows.
    pub dragging: bool,
}

/// What one machine said about its agents: the settings and the commands of each.
pub type Discovered = (Vec<Vec<Setting>>, Vec<Vec<Slash>>);

/// Everything the workspace window shows and lets the reader change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceState {
    /// The projects, in the order the sidebar lists them.
    pub projects: Vec<Project>,
    /// Every session, across projects.
    pub sessions: Vec<Session>,
    /// What the main area shows.
    pub page: Page,
    /// Which view of the open session shows.
    pub tab: Tab,
    /// The pages and views visited, walked back and forward.
    pub trail: crate::trail::Trail,
    /// Whether the review panel is open.
    pub review: bool,
    /// What the reader picked and opened among the changes.
    pub git: GitControls,
    /// What the review panel lists.
    pub review_tab: ReviewTab,
    /// The search of the review panel and what it found.
    pub project_search: crate::project_search::ProjectSearch,
    /// The hits of a search shown as one document, while they are.
    pub excerpts: Option<crate::excerpts::Excerpts>,
    /// The page of the web beside the work, while one is open.
    pub browser: Option<crate::browser::Browser>,
    /// The scope of the session whose browser `browser` is, or `NO_SCOPE` on a page that is no session's.
    pub browser_scope: u64,
    /// The browsers of the sessions other than the one shown, by scope.
    pub browsers: std::collections::HashMap<u64, crate::browser::Browser>,
    /// The folders of the project's files that are unfolded, by path.
    pub open_tree: std::collections::HashSet<String>,
    /// The folders of the changed files that are folded away, by group and path.
    pub closed_folders: std::collections::HashSet<String>,
    /// The changed files folded away in the diff, by path.
    pub closed_diffs: std::collections::HashSet<String>,
    /// The lines or file the comment being written or the lines being dragged over are about.
    pub comment_target: Option<CommentTarget>,
    /// The comment being written.
    pub comment_text: TextFieldState,
    /// The diff's lazy list of the parts of each changed file, which holds how far down it is scrolled.
    pub diff_list: rustle::LazyListState,
    /// How far down the diff was scrolled when the state last moved it or looked at it,
    /// so a scroll by the reader, which leaves the two apart, can be told from its own.
    pub diff_seen: f32,
    /// Where the diff's scrolling area is drawn, while it is shown.
    pub diff_bounds: crate::bounds::Bounds,
    /// The file, by session and path, whose header the diff's list is asked to bring to its top, until the list has done so or the diff is scrolled by hand.
    pub(crate) diff_anchor: Option<(usize, String)>,
    /// The files, by session and path, whose diffs were asked of the server and have not come.
    pub(crate) asking_diffs: std::collections::HashSet<(usize, String)>,
    /// How far down the review panel's list of changes or search results is scrolled, in logical pixels.
    pub tree_scroll: rustle::ScrollState,
    /// The lazy list of the project's files in the review panel: how far down it is scrolled and how tall its rows turned out.
    pub files_list: rustle::LazyListState,
    /// The project whose folder the review panel was last asked to read while no session was open.
    pub homed: Option<usize>,
    /// Whether the plan the agent proposed is open in a panel in place of the review.
    pub plan_open: bool,
    /// How far down the plan panel is scrolled, in logical pixels.
    pub plan_scroll: rustle::ScrollState,
    /// How far down the subagent panel rows are scrolled, in logical pixels.
    pub subagents_scroll: rustle::ScrollState,
    /// How far down a project's overview is scrolled, in logical pixels.
    pub overview_scroll: rustle::ScrollState,
    /// Whether the terminal is open.
    pub terminal: bool,
    /// Whether the sidebar of projects and sessions is showing.
    pub sidebar_open: bool,
    /// Which tab of the dock is showing.
    pub bottom: crate::dock::BottomTab,
    /// What is wrong with the files of the folder `problems_of` names, as the server last said.
    pub problems: Vec<pm_protocol::Problem>,
    /// The folder, and the machine it is on, that `problems` is about.
    pub problems_of: Option<ProblemsKey>,
    /// The folder, and the machine it is on, the server was asked about and has not answered.
    pub problems_asked: Option<ProblemsKey>,
    /// How far down the list of problems is scrolled, in logical pixels.
    pub problems_scroll: rustle::ScrollState,
    /// The problem of the list the reader pressed, whose place is to be shown.
    pub problem_picked: Option<usize>,
    /// How many columns and lines the dock's window last held.
    pub terminal_size: (u16, u16),
    /// The name the next shell gets in this window.
    pub next_terminal: u64,
    /// The agent chosen for new work, by position.
    pub agent: usize,
    /// The changed file the reader picked, by position, if one was.
    pub file: Option<usize>,
    /// The prompt being written.
    pub prompt: TextFieldState,
    /// How far the lines of the prompt box are scrolled down.
    pub prompt_scroll: rustle::ScrollState,
    /// The files attached to the prompt, by path.
    pub attachments: Vec<String>,
    /// The elements of web pages picked for the prompt.
    pub elements: Vec<crate::elements::Element>,
    /// The screenshots and consoles of web pages held for the prompt.
    pub captures: Vec<crate::captures::Capture>,
    /// The machine, by destination, that the settings and commands held belong to; nothing for this one.
    pub discovery_host: Option<String>,
    /// What other machines said about the agents, by destination, while the settings of one machine are showing.
    pub discovered_elsewhere: std::collections::HashMap<Option<String>, Discovered>,
    /// What the attached pictures look like, by path, once they are decoded.
    pub previews: std::collections::HashMap<String, rustle::ImageData>,
    /// The diagrams and images of markdown drawn or fetched so far.
    pub figures: crate::figures::Figures,
    /// The figures asked for, by their key, so none is asked for twice.
    pub figures_asked: std::collections::HashSet<u64>,
    /// What the window still has to do for the exhibits agents show and the mockups of their questions.
    pub exhibits: crate::exhibit::Exhibits,
    /// The commit message being written.
    pub message: TextFieldState,
    /// What the history page is narrowed to.
    pub history_search: TextFieldState,
    /// How far the history session groups have scrolled, in logical pixels.
    pub history_scroll: rustle::ScrollState,
    /// How far the sidebar's list is scrolled, in logical pixels.
    pub sidebar_scroll: rustle::ScrollState,
    /// The submodule of the project the new session page starts work in, or nothing for the project itself.
    pub new_in: Option<String>,
    /// How big the panels around the main area are, as observable state.
    pub sizes: crate::panel_sizes::PanelSizes,
    /// The panel for adding a project, if it is open.
    pub adder: Option<Adder>,
    /// The window for choosing a folder, if it is open.
    pub folder_picker: Option<FolderPicker>,
    /// The section showing in the settings panel, if it is open.
    pub settings: Option<crate::settings::Section>,
    /// The lists and the form of the settings pages that list servers.
    pub settings_servers: crate::settings_servers::ServerSettings,
    /// The menu open over a field, and where, if one is.
    pub menu: Option<ContextMenu>,
    /// The menu open over a project, and where, if one is.
    pub project_menu: Option<ProjectMenu>,
    /// The menu open over a session, and where, if one is.
    pub session_menu: Option<SessionMenu>,
    /// The menu open over a message of the open session, and where, if one is.
    pub message_menu: Option<MessageMenu>,
    /// The menu open over the window of the shell showing, and where, if one is.
    pub terminal_menu: Option<crate::terminal_menu::TerminalMenu>,
    /// The menu open over the page of the browser, if any.
    pub browser_menu: Option<crate::browser_menu::BrowserMenu>,
    /// The project being dragged to another place, if one is.
    pub project_drag: Option<ProjectDrag>,
    /// What the agents have yet to be told.
    pub outbox: Vec<Outgoing>,
    /// How many presses asked for a shell to take the keyboard, such as a shell's tab or
    /// a new shell: the shell's view gives itself the keyboard each time this changes.
    pub shell_asked: u64,
    /// The session, by position, whose files are the ones open now.
    pub files_owner: Option<usize>,
    /// The files open for each session other than the one whose tabs are showing, by its position, kept so that switching away does not lose them or what was typed in them.
    pub file_stash:
        std::collections::HashMap<usize, (Vec<crate::open_files::Viewer>, Option<usize>)>,
    /// The view each session was left on, by its position, so that coming back to it shows the same view.
    pub tab_stash: std::collections::HashMap<usize, Tab>,
    /// The panes of the main area of the page whose files are open now.
    pub panes: crate::panes::Panes,
    /// The panes of each session other than the one whose files are open now, by its position.
    pub pane_stash: std::collections::HashMap<usize, crate::panes::Panes>,
    /// What was typed in the prompt, and the files attached to it, for every session but the one being worked in, by session, so that coming back to one finds its message as it was left.
    pub prompt_stash: std::collections::HashMap<usize, StashedPrompt>,
    /// The search of the review panel of every session but the one being worked in, by session.
    pub search_stash: std::collections::HashMap<usize, crate::project_search::ProjectSearch>,
    /// The tab being dragged to another pane, if one is.
    pub pane_drag: Option<crate::panes::PaneDrag>,
    /// Whether the menu of panes is open, hanging from its button.
    pub pane_menu: bool,
    /// The list of the conversation's messages, laid out from its end, so the offset of its
    /// scroll is how far the conversation is scrolled back from its end, in logical pixels.
    pub chat_list: rustle::LazyListState,
    /// Where the conversation's scrolling area is drawn, while it is shown.
    pub chat_bounds: crate::bounds::Bounds,
    /// Where the scrolling area of the file shown is drawn, while it is shown.
    pub code_bounds: crate::bounds::Bounds,
    /// Where the lines of code text were drawn in the latest frame.
    pub code_lines: crate::text_lines::TextLines,
    /// The selection of the conversation's text, held by its selection container.
    pub chat_selection: rustle::SelectionState,
    /// The selection of the text of the card about a place in the file shown, held by its
    /// selection container.
    pub hover_selection: rustle::SelectionState,
    /// Where the areas the app asks about by name were drawn in the latest frame.
    pub places: crate::bounds::Places,
    /// The menu open over the prompt box, if one is.
    pub picker: Option<Picker>,
    /// The reader's message of the open session, by position, whose offer to put the files back as they were before it is open, if one is.
    pub restoring: Option<usize>,
    /// What each agent lets the reader change, by the agent's position, once it has been asked.
    pub agent_settings: Vec<Vec<Setting>>,
    /// The settings the reader changed before starting work, by agent and name.
    pub touched: Vec<(usize, String)>,
    /// What the reader last chose for each setting of each agent: the agent, the setting and the value, as the agent names them.
    pub choices: Vec<(String, String, String)>,
    /// Where new work is done.
    pub location: Location,
    /// Whether new work runs in a dev container, which only a project with a `devcontainer.json` offers.
    pub container: bool,
    /// The branch new work is made from, when the reader picked one; otherwise the one checked out.
    pub base: Option<String>,
    /// The branches of the project being started in, the one checked out first; none when it is not a repository.
    pub branches: Vec<String>,
    /// The project `branches` was read for.
    pub branches_of: Option<usize>,
    /// Where the open chat is to be continued, as chosen so far.
    pub continuing: crate::continue_chat::Choice,
    /// The panel open over the conversation's chips, if one is.
    pub activity: Option<Activity>,
    /// The commands and skills each agent offers behind a slash, by the agent's position, once it has been asked.
    pub agent_commands: Vec<Vec<Slash>>,
    /// The files of the folder the prompt is for, which an `@` in the prompt offers.
    pub files: Vec<String>,
    /// The folders among `files` that hold no tracked file, by path.
    pub tree_folders: Vec<String>,
    /// The files and folders of the tree that git ignores, by path.
    pub tree_ignored: std::collections::HashSet<String>,
    /// The folder `files` was read for.
    pub files_of: Option<String>,
    /// Recent server listings shared across state copies, including replies received after navigation.
    pub(crate) file_listings:
        std::collections::VecDeque<std::sync::Arc<crate::choices::FileListing>>,
    /// Who each agent is signed in as, by the agent's position, once it has been looked up.
    pub identities: Vec<Option<Option<String>>>,
    /// How much of its allowance each account has used, by the agent's position and the added login's id, or nothing for the agent's own login, once the provider has said.
    pub allowances: std::collections::HashMap<(usize, Option<String>), pm_harness::Limits>,
    /// When each account's allowance was last asked after, by the agent's position and the login, in Unix milliseconds.
    pub allowance_asked: std::collections::HashMap<(usize, Option<String>), u64>,
    /// Who each agent is signed in as in its own default folder, by the agent's position, once it has been looked up.
    pub defaults: Vec<Option<Option<String>>>,
    /// The named logins the reader has added: the agent's id, the folder name and what it is called.
    pub logins: Vec<(String, String, String)>,
    /// The login in use for each agent, as the agent's id and the folder name; an agent not named uses its own.
    pub in_use: Vec<(String, String)>,
    /// How far the list of values in the open menu is scrolled, in logical pixels.
    pub menu_scroll: rustle::ScrollState,
    /// What the values of the open menu are narrowed to, when it has many.
    pub menu_filter: TextFieldState,
    /// The command highlighted in the list offered behind a slash, by position.
    pub slash: usize,
    /// The steps whose detail is open, by session, reply and position.
    pub opened: Vec<(usize, usize, usize)>,
    /// The fold-outs of replies that are open, by session, reply and place in the reply.
    pub unfolded: Vec<(usize, usize, usize)>,
    /// The languages the server on this machine has installed and can install, once it has said.
    pub extensions: Vec<pm_protocol::ExtensionInfo>,
    /// What the extensions page remembers.
    pub extensions_page: crate::extensions::ExtensionsPage,
    /// What the extensions of each machine bring that the window uses itself, by machine; nothing for this one.
    pub extension_assets: std::collections::BTreeMap<Option<String>, pm_protocol::ExtensionAssets>,
    /// The list hanging over the file shown, when one is open.
    pub listing: Option<crate::listing::Listing>,
    /// The commands last carried out from the quick input, the latest first.
    pub recent_commands: Vec<pm_editor::Action>,
    /// The files last opened, by path from the root of the project, the latest first.
    pub recent_files: Vec<String>,
    /// The file tabs closed last, the latest last, to bring back.
    pub closed_tabs: Vec<crate::tab_steps::ClosedTab>,
    /// The box giving the thing at the caret a new name, when open.
    pub renaming: Option<crate::navigate::Renaming>,
    /// What modal editing shares between files: its bindings, registers, macros and the last change and search.
    pub vim: pm_vim::Vim,
    /// The colour of the file shown being changed, when the box for it is open.
    pub color_picking: Option<crate::color_picker::ColorPicking>,
    /// The box for going to a line of the file shown by its number, when open.
    pub going_to: Option<crate::line_edits::GoingTo>,
    /// The waiting prompt whose text is being edited in place, when one is.
    pub editing_queued: Option<crate::queued::EditingQueued>,
    /// Where the line of the caret is while the signature of the call the caret is in shows,
    /// in logical pixels of the window: the caret's left edge and the line's top, which the
    /// signature hangs over.
    pub signature_at: (f32, f32),
    /// The bar for finding and replacing in the file shown, when open.
    pub finding: Option<crate::find::Finding>,
    /// How the editor looks and behaves.
    pub editor: pm_editor::EditorSettings,
    /// The action keys are being recorded for on the keymap page, when they are.
    pub recording: Option<pm_editor::Action>,
    /// The chords pressed so far while keys are recorded, as they are written.
    pub recorded: Vec<String>,
    /// Whether the editor's settings kept between runs were put in, so that what is shown is not mistaken for them before.
    pub editor_loaded: bool,
    /// The size the view of the file shown was last laid out at, told by the view, which soft wrap breaks lines by and which tells the lines in view.
    pub code_size: crate::code_view::CodeViewSize,
    /// Whether the reader asked to close the window and nothing is left to ask first.
    pub quitting: bool,
    /// How far down the notes in the window of an update are scrolled, in logical pixels.
    pub update_scroll: rustle::ScrollState,
    /// Whether the release notes of the newer release are open as a tab.
    pub update_open: bool,
    /// How far the update the reader asked for has got.
    pub update_run: crate::update_dialog::UpdateRun,
    /// The code a sign-in waits for the reader to enter, while one does and the reader has not hidden it.
    pub sign_in_code: Option<crate::sign_in_code::SignInCode>,
    /// The breakpoints and the program being debugged.
    pub debug: crate::debug::DebugState,
    /// What the project can be built, tested and run with.
    pub tasks: crate::tasks::TaskState,
    /// What the window keeps of its own.
    pub window: pm_config::WindowSettings,
    /// Whether the window's own settings kept between runs were put in, so that what is shown is not mistaken for them before.
    pub window_loaded: bool,
    /// What the server of each machine keeps, as the server on this machine sent it.
    pub machine: pm_protocol::MachineSettings,
    /// Whether the server on this machine sent what it keeps, so that what is shown is not sent to the servers before.
    pub machine_loaded: bool,
    /// The machine the chat being started goes to, by destination, or nothing for this one.
    pub chat_on: Option<String>,
    /// What the settings window keeps beside the settings.
    pub settings_page: crate::settings_state::SettingsPage,
    /// The menu on the tab of an open file, when open.
    pub tab_menu: Option<crate::tab_menu::TabMenu>,
    /// Who last changed the line of the caret of a file, once asked: the session, the file, the line counted from 1, and the answer.
    pub blame: Option<(usize, String, usize, pm_git::Blame)>,
}

impl WorkspaceState {
    /// A workspace of `projects`, with no sessions yet, open on the page for starting work.
    pub fn new(projects: &[Project]) -> Self {
        Self::with(projects.to_vec(), Vec::new())
    }

    /// A workspace of made-up projects and sessions, open on the first session.
    pub fn sample() -> Self {
        Self {
            page: Page::Session(0),
            ..Self::with(projects(), sessions())
        }
    }

    /// A workspace of `projects` and `sessions`, open on the page for starting work.
    fn with(projects: Vec<Project>, sessions: Vec<Session>) -> Self {
        let builtin = crate::agent_list::AgentEntry::shipped();
        let page = if projects.is_empty() {
            Page::Chat
        } else {
            Page::New(0)
        };
        let mut state = Self {
            projects,
            sessions,
            page,
            review: true,
            terminal_size: crate::dock::FIRST_SIZE,
            next_terminal: 1,
            sidebar_open: true,
            sizes: crate::panel_sizes::PanelSizes::opening(),
            settings_servers: crate::settings_servers::ServerSettings {
                agents: builtin.clone(),
                ..Default::default()
            },
            agent_settings: vec![Vec::new(); builtin.len()],
            agent_commands: vec![Vec::new(); builtin.len()],
            identities: vec![None; builtin.len()],
            defaults: vec![None; builtin.len()],
            ..Self::default()
        };
        state.sync_context();
        state
    }

    /// The workspace with what the reader chose for the agents' settings before.
    pub fn with_choices(mut self, choices: Vec<(String, String, String)>) -> Self {
        self.choices = choices;
        self
    }

    /// Adds `source` as a project unless it names no folder or is there already.
    pub fn with_project(mut self, source: &str) -> Self {
        if let Some(project) = Project::from_source(source)
            && !self.projects.iter().any(|held| held.path == project.path)
        {
            self.projects.push(project);
            self.ask_submodules(self.projects.len() - 1);
        }
        self.sync_context();
        self
    }

    /// The state with `source` added as a project, or found if it is there already, and open on the page for starting a session in it.
    pub fn with_project_opened(mut self, source: &str) -> Self {
        self = self.with_project(source);
        let path = Project::from_source(source).map(|project| project.path);
        if let Some(index) = self
            .projects
            .iter()
            .position(|held| Some(&held.path) == path.as_ref())
        {
            self.start_in(index, None);
            self.sync_context();
        }
        self
    }

    /// Whether a page for starting work is showing: the one of a project, or the one that starts a chat.
    pub fn starting(&self) -> bool {
        match self.page {
            Page::New(project) => project < self.projects.len(),
            Page::Chat => true,
            _ => false,
        }
    }

    /// How much room the sidebar takes from the left of the window, in logical pixels: its width while it shows and nothing while it is hidden.
    pub fn sidebar_room(&self) -> f32 {
        if self.sidebar_open {
            self.sizes.sidebar_width.get()
        } else {
            0.0
        }
    }

    /// The session on screen, if the page is one.
    pub fn open(&self) -> Option<&Session> {
        match self.page {
            Page::Session(index) => self.sessions.get(index),
            _ => None,
        }
    }

    /// The project the next new work goes to: the open session's, or the one being started in; the first one while a chat or no project is shown.
    pub fn current_project(&self) -> usize {
        match self.page {
            Page::New(project) | Page::Project(project) => project,
            _ => self.open().and_then(|session| session.project).unwrap_or(0),
        }
    }

    /// The state after `event`, the reducer of what the reader did: a press of a control,
    /// or typing in, editing or leaving a field.
    pub fn handle(self, event: Event) -> Self {
        match event {
            Event::Sidebar(event) => self.pressed(Target::Sidebar(event)),
            Event::Chat(event) => self.pressed(Target::Chat(event)),
            Event::Composer(event) => self.pressed(Target::Composer(event)),
            Event::Diff(event) => self.pressed(Target::Diff(event)),
            Event::Tree(event) => self.pressed(Target::Tree(event)),
            Event::Editor(event) => self.pressed(Target::Editor(event)),
            Event::Search(event) => self.pressed(Target::Search(event)),
            Event::Git(event) => self.pressed(Target::Git(event)),
            Event::Terminal(event) => self.pressed(Target::Terminal(event)),
            Event::Debug(event) => self.pressed(Target::Debug(event)),
            Event::Browser(event) => self.pressed(Target::Browser(event)),
            Event::Settings(event) => self.pressed(Target::Settings(event)),
            Event::Menu(event) => self.pressed(Target::Menu(event)),
            Event::Dialog(event) => self.pressed(Target::Dialog(event)),
            Event::Window(event) => self.pressed(Target::Window(event)),
            Event::Field(FieldEvent::Changed(field)) => self.field_typed(field),
            Event::Field(FieldEvent::Typed { field, text }) => self.typed(field, &text),
            Event::Field(FieldEvent::Left(field)) => self.field_left(field),
            Event::Field(FieldEvent::Edited { field, edit }) => self.edited(field, edit),
        }
    }

    /// The state after `action` was pressed: a press that shows a shell asks it to take the
    /// keyboard (see [`shell_asked`](Self::shell_asked)), and a press on the find bar or a
    /// row of the list over the file scrolls the file shown to its caret.
    pub(crate) fn pressed(self, action: Target) -> Self {
        let mut state = self.after_press(action);
        if Self::takes_keyboard(action) {
            state.shell_asked += 1;
        }
        if crate::find::is_find_part(action)
            || matches!(action, Target::Editor(EditorEvent::ListingRow(_)))
        {
            state = state.with_caret_in_view();
        }
        state
    }

    /// The state with the file shown scrolled to bring its caret into view, in the room
    /// its lines were last drawn in, when they were drawn.
    fn with_caret_in_view(self) -> Self {
        match self.code_bounds.get() {
            Some(area) => self.with_caret_shown(area.size.height + crate::open_files::STRIP),
            None => self,
        }
    }

    /// Opens the browser, or shows it when it is open, asking its address to take the keyboard.
    pub fn browser_summoned(&mut self) {
        self.open_browser(crate::browser::HOME);
        if let Some(browser) = self.browser.as_mut() {
            browser.asked += 1;
        }
    }

    /// The state after `action` was pressed, before the shell and the caret follow the press.
    fn after_press(mut self, action: Target) -> Self {
        if let Target::Editor(event) = action {
            match event {
                EditorEvent::CompletionRow(place) => return self.completion_taken(Some(place)),
                EditorEvent::CompletionBody
                | EditorEvent::HoverCard
                | EditorEvent::Lightbulb
                | EditorEvent::ListingFilter
                | EditorEvent::ListingBody
                | EditorEvent::RenameBox
                | EditorEvent::RenameSymbol
                | EditorEvent::ColorBox
                | EditorEvent::ColorHex => return self,
                EditorEvent::Breakpoint { file, line } => {
                    self.toggle_breakpoint(file, line);
                    return self;
                }
                EditorEvent::RunLens(index) => {
                    self.run_lens_pressed(index);
                    return self;
                }
                EditorEvent::Run { file, line } => {
                    self.runnable_run(file, line);
                    return self;
                }
                EditorEvent::CodeFold(line) => {
                    if let Some(document) = self.shown_text() {
                        document.unfold_line(line);
                    }
                    return self;
                }
                EditorEvent::ListingRow(row) => return self.listing_taken(Some(row)),
                EditorEvent::ListingPrompt => return self.listing_prompted(),
                EditorEvent::ListingTake => return self.listing_taken(None),
                EditorEvent::ColorHexSubmit => return self.color_format_taken(None),
                EditorEvent::ColorFormat(row) => return self.color_format_taken(Some(row)),
                EditorEvent::RenameSymbolSubmit => return self.rename_submitted(),
                _ => {}
            }
        }
        if let Some(command) =
            crate::tab_menu::TabCommand::of(action).filter(|_| self.tab_menu.is_some())
        {
            return self.tab_command(command);
        }
        self.tab_menu = None;
        if crate::find::is_find_part(action) {
            return self.find_pressed(action);
        }
        if crate::excerpts::is_excerpt_part(action) {
            return self.excerpts_pressed(action);
        }
        if crate::browser::is_browser_part(action) {
            return self.browser_pressed(action);
        }
        if self.press_sign_in_code(action) {
            return self;
        }
        if crate::project_search::is_search_part(action) {
            return self.project_search_pressed(action);
        }
        if action == Target::Editor(EditorEvent::GoToLineSubmit) {
            return self.go_to_line_submitted();
        }
        if action == Target::Editor(EditorEvent::GoToLine)
            || action == Target::Editor(EditorEvent::GoToLineBox)
        {
            return self;
        }
        if let Some(address) = self
            .shown_document()
            .and_then(|document| document.hover.as_ref())
            .and_then(|card| card.text.as_deref())
            .and_then(|said| crate::assist::hover_link(said, action))
        {
            if pm_harness::is_web_address(&address) {
                self.outbox.push(Outgoing::Open { address });
            }
            return self;
        }
        if self.pane_tab_of(action).is_some() {
            self.panes.parked = None;
        }
        self.settle_panes();
        self.pane_menu = false;
        if self.press_empty_pane(action) {
            return self;
        }
        if let Some(side) = crate::panes::split_row_of(action) {
            self.split_focused(side);
            return self;
        }
        if let Target::Window(WindowEvent::Pane(pane)) = action {
            self.focus_pane(pane);
            return self;
        }
        let restoring = self.restoring.take();
        match action {
            Target::Chat(ChatEvent::RestoreFiles) => self.restore(restoring),
            row if is_session_menu_item(row) => return self.session_menu_pressed(row),
            row if crate::message_menu::is_message_menu_item(row) => {
                return self.message_menu_pressed(row);
            }
            row if crate::arrange::is_project_menu_item(row) => {
                return self.project_menu_pressed(row);
            }
            Target::Sidebar(SidebarEvent::AddProject) => return self.with_adder(),
            Target::Sidebar(SidebarEvent::Settings) => return self.with_settings(),
            Target::Window(WindowEvent::Update) => self.open_release(),
            Target::Window(WindowEvent::LanguageUpdate) => return self.with_extensions_page(),
            update if self.press_update(update) => {}
            trust if self.trust_pressed(trust) => {}
            action if crate::settings::is_settings(action) => {
                return self.settings_pressed(action);
            }
            action
                if is_adder(action)
                    && !matches!(action, Target::Dialog(DialogEvent::AdderRow(_))) =>
            {
                return self.adder_pressed(action);
            }
            action if is_picker(action) => return self.folder_picker_pressed(action),
            Target::Sidebar(SidebarEvent::History) => self.page = Page::History,
            Target::Sidebar(SidebarEvent::NewChat) => self.start_chat(),
            Target::Settings(SettingsEvent::Extensions) => return self.with_extensions_page(),
            action if crate::extensions::is_extensions(action) => {
                return self.extensions_pressed(action);
            }
            Target::Chat(ChatEvent::TabChat) => self.tab = Tab::Chat,
            Target::Diff(DiffEvent::TabDiff) => self.tab = Tab::Diff,
            Target::Terminal(TerminalEvent::Terminal)
            | Target::Terminal(TerminalEvent::NewTerminal)
            | Target::Terminal(TerminalEvent::MaximizeTerminal) => {
                self.press_terminal(action);
            }
            Target::Diff(DiffEvent::Review) => self.review = !self.review,
            Target::Sidebar(SidebarEvent::SidebarToggle) => self.sidebar_open = !self.sidebar_open,
            Target::Diff(DiffEvent::ReviewChanges) => self.show_review_tab(ReviewTab::Changes),
            Target::Diff(DiffEvent::ReviewFiles) => self.show_review_tab(ReviewTab::Files),
            Target::Diff(DiffEvent::CommentAdd) => self.add_comment(),
            Target::Diff(DiffEvent::CommentCancel) => self.comment_target = None,
            Target::Diff(DiffEvent::SendComments) => self.send_comments(),
            Target::Chat(ChatEvent::OpenPlan) => self.plan_open = true,
            Target::Chat(ChatEvent::ClosePlan) => self.plan_open = false,
            Target::Chat(ChatEvent::Revise) => self.reject(),
            Target::Diff(DiffEvent::OpenDiff) => self.tab = Tab::Diff,
            Target::Chat(ChatEvent::SessionSignIn) => {
                if let Page::Session(session) = self.page {
                    self.outbox.push(Outgoing::SignInSession { session });
                }
            }
            Target::Chat(ChatEvent::QueuedCancel) => self.cancel_queued_edit(),
            Target::Composer(ComposerEvent::Send) => self.send(false),
            Target::Composer(ComposerEvent::SendNow) => self.send(true),
            Target::Composer(ComposerEvent::Stop) => self.stop(),
            Target::Git(GitEvent::StageAll) => self.stage_all(),
            Target::Git(GitEvent::Commit) => self.primary_pressed(),
            _ => {
                if !self.press_git(action) {
                    self.after_listed(action);
                }
            }
        }
        self.sync_context();
        self.refresh_files();
        self.ask_log_once();
        self.debug_revealed().problem_revealed()
    }

    /// Whether the field `id` saves what was written in it when the reader leaves it: the boxes of a category on the models page.
    pub fn saves_on_leaving(id: Target) -> bool {
        crate::settings_slots::saves_on_leaving(id)
    }

    /// The state after the reader left the field named `id`, with what was written in it saved when it is one that saves on leaving.
    pub(crate) fn field_left(mut self, id: Target) -> Self {
        self.slot_field_left(id);
        self
    }

    /// The state after `edit` is made to the code editor's field, which `id` names.
    pub(crate) fn edited(mut self, id: Target, edit: Edit) -> Self {
        if id == Target::Editor(EditorEvent::EditorField) {
            self.file_edited(edit);
            self.refresh_files();
        }
        self
    }

    /// The state after the reader changed the text of the field named `id`
    /// through its text field: what follows from what it now holds.
    pub(crate) fn field_typed(mut self, id: Target) -> Self {
        match id {
            Target::Composer(ComposerEvent::Prompt) => self.reset_slash(),
            Target::Editor(EditorEvent::RenameSymbol) => {
                if let Some(renaming) = self.renaming.as_mut() {
                    renaming.problem = None;
                }
            }
            Target::Editor(EditorEvent::ColorHex) => self.color_hex_typed(),
            Target::Editor(EditorEvent::FindField) => return self.find_typed(),
            Target::Search(SearchEvent::ProjectSearchField) => return self.project_search_typed(),
            Target::Editor(EditorEvent::ListingFilter) => return self.listing_filtered(),
            Target::Browser(BrowserEvent::BrowserFindField) => self.browser_find_typed(),
            Target::Dialog(DialogEvent::AdderField) => self.adder_typed(),
            Target::Dialog(DialogEvent::PickerField) => self.folder_picker_typed(),
            Target::Sidebar(SidebarEvent::HistorySearch) => self.history_scroll.jump_to(0.0),
            Target::Settings(SettingsEvent::ExtensionsSearch) => self.extensions_typed(),
            Target::Composer(ComposerEvent::MenuFilter) => self.menu_scroll.jump_to(0.0),
            id if crate::settings_drafts::is_draft_field(id) => self.draft_typed(),
            id if crate::settings_pages::is_setting_field(id) => self.setting_typed(id),
            id if crate::settings_slots::is_slot_field(id) => self.slot_typed(id),
            Target::Settings(SettingsEvent::SettingsFind) => {
                self.settings_servers.scroll.jump_to(0.0);
                self.settings_page.picker = None;
            }
            _ => {}
        }
        self.refresh_files();
        self
    }

    /// The state after the reader typed `text` into the text field `id` over its
    /// selection, as its text field takes typing, and what follows from that.
    pub(crate) fn typed(self, id: Target, text: &str) -> Self {
        let Some(field) = self.text_field(id) else {
            return self;
        };
        field.edit(|buffer| {
            let selection = buffer.selection();
            buffer.replace(selection.min(), selection.max(), text);
            buffer.place_cursor_before_char_at(selection.min() + text.chars().count());
        });
        self.field_typed(id)
    }

    /// The code editor's field, which `id` names, while a text file is shown.
    pub fn field(&self, id: Target) -> Option<&pm_editor::Field> {
        (id == Target::Editor(EditorEvent::EditorField))
            .then(|| self.shown_document().map(|document| &document.field))
            .flatten()
    }

    /// The state of the text field `id` names, if it names one other than the code editor's.
    pub fn text_field(&self, id: Target) -> Option<&TextFieldState> {
        match id {
            Target::Composer(ComposerEvent::Prompt) => Some(&self.prompt),
            Target::Git(GitEvent::CommitMessage) => Some(&self.message),
            Target::Diff(DiffEvent::CommentField) => Some(&self.comment_text),
            Target::Git(GitEvent::BranchFilter) => Some(&self.git.filter),
            Target::Git(GitEvent::StashField) => Some(&self.git.stash_message),
            Target::Git(GitEvent::BranchTaskField) => {
                self.git.branching.as_ref().map(|open| &open.field)
            }
            Target::Git(GitEvent::RenameField) => Some(&self.git.rename),
            Target::Tree(TreeEvent::EntryField) => self.git.naming.as_ref().map(|(_, field)| field),
            Target::Editor(EditorEvent::RenameSymbol) => {
                self.renaming.as_ref().map(|renaming| &renaming.field)
            }
            Target::Editor(EditorEvent::ColorHex) => {
                self.color_picking.as_ref().map(|picking| &picking.hex)
            }
            Target::Editor(EditorEvent::GoToLine) => {
                self.going_to.as_ref().map(|going| &going.field)
            }
            Target::Browser(BrowserEvent::BrowserAddress) => {
                self.browser.as_ref().map(|browser| &browser.address)
            }
            Target::Browser(BrowserEvent::BrowserFindField) => self
                .browser
                .as_ref()
                .and_then(|browser| browser.search.as_ref())
                .map(|search| &search.query),
            Target::Editor(EditorEvent::FindField) => {
                self.finding.as_ref().map(|finding| &finding.query)
            }
            Target::Search(SearchEvent::ProjectSearchField) => Some(&self.project_search.query),
            Target::Editor(EditorEvent::ListingFilter) => self.listing_filter(),
            Target::Debug(DebugEvent::DebugConsoleField) => Some(&self.debug.console),
            Target::Debug(DebugEvent::DebugWatchField) => Some(&self.debug.watch),
            Target::Editor(EditorEvent::ReplaceField) => {
                self.finding.as_ref().map(|finding| &finding.replacement)
            }
            Target::Sidebar(SidebarEvent::HistorySearch) => Some(&self.history_search),
            Target::Settings(SettingsEvent::ExtensionsSearch) => Some(&self.extensions_page.search),
            Target::Composer(ComposerEvent::MenuFilter) => Some(&self.menu_filter),
            Target::Dialog(DialogEvent::AdderField) => self.adder.as_ref().map(|open| &open.field),
            Target::Dialog(DialogEvent::PickerField) => {
                self.folder_picker.as_ref().map(|open| &open.field)
            }
            id if crate::settings_servers::is_form_field(id) => self.server_form_field(id),
            id if crate::settings_drafts::is_draft_field(id) => self.draft_field(id),
            id if crate::settings_agents::is_key_field(id) => self.key_field(id),
            id if crate::settings_pages::is_setting_field(id) => self.setting_field(id),
            id if crate::settings_machines::is_machine_field(id) => self.machine_field(id),
            id if crate::settings_models::is_model_field(id) => self.model_field(id),
            id if crate::settings_slots::is_slot_field(id) => self.slot_field(id),
            Target::Settings(SettingsEvent::SettingsFind) => Some(&self.settings_page.find),
            Target::Settings(SettingsEvent::SettingsKeymapFind) => {
                Some(&self.settings_page.keymap_find)
            }
            Target::Chat(ChatEvent::Other) => self.other_field(),
            _ => None,
        }
    }

    /// The state with a menu open over the field `field` at `at`.
    pub fn with_menu(mut self, field: Target, at: (f32, f32)) -> Self {
        self.menu = Some(ContextMenu { field, at });
        self
    }

    /// The state with no menu open.
    pub fn without_menu(mut self) -> Self {
        self.menu = None;
        self.project_menu = None;
        self.session_menu = None;
        self.message_menu = None;
        self.terminal_menu = None;
        self.browser_menu = None;
        self.git.menu = None;
        self.settings_servers.menu = None;
        self.tab_menu = None;
        self.pane_menu = false;
        self
    }

    /// The state with no popover of a chip above the prompt open: what pressing outside the
    /// popover, or Escape, leaves.
    pub fn without_open_picker(mut self) -> Self {
        self.picker = None;
        self
    }

    /// The state with the panel over the conversation's chips closed.
    pub fn without_activity(mut self) -> Self {
        self.activity = None;
        self
    }

    /// The state with the list of branches of the changes panel closed.
    pub fn without_branch_menu(mut self) -> Self {
        self.git.branch_menu = false;
        self
    }

    /// Takes what the agents have yet to be told off the state.
    pub fn take_outbox(&mut self) -> Vec<Outgoing> {
        self.ensure_terminal();
        self.ask_problems_again();
        std::mem::take(&mut self.outbox)
    }

    /// The state after the agent of the session at `session` did `event`.
    pub fn heard(mut self, session: usize, event: HarnessEvent) -> Self {
        self.hear_in_place(session, event);
        self
    }

    /// Takes in what the agent of the session at `session` did, as `event`, without building the state anew.
    pub fn hear_in_place(&mut self, session: usize, event: HarnessEvent) {
        if matches!(&event, HarnessEvent::Ask(ask) if ask.plan.is_some()) {
            self.plan_open = true;
            self.plan_scroll.jump_to(0.0);
        }
        let finishing = matches!(&event, HarnessEvent::Finished(_) | HarnessEvent::Failed(_));
        let pictured = matches!(&event, HarnessEvent::Show(_) | HarnessEvent::Inquire(_));
        if let Some(held) = self.sessions.get_mut(session) {
            held.hear(event);
            let gone = self.editing_queued.as_ref().is_some_and(|editing| {
                editing.scope == held.scope
                    && !held.queued.iter().any(|queued| queued.id == editing.id)
            });
            if gone {
                self.cancel_queued_edit();
            }
        }
        if finishing {
            self.ask_reply_figures(session);
        }
        if pictured {
            self.ask_exhibit_figures(session);
        }
    }

    /// The state after the agent of the session at `position` finished a turn while the window watched: the reader is told, or the session opens on what it changed, as the settings say.
    pub fn agent_finished(mut self, position: usize) -> Self {
        let Some(session) = self.sessions.get(position) else {
            return self;
        };
        let changed = session.changes.len();
        let title = session.title.clone();
        match self.window.agents.on_finish {
            pm_config::Finish::Nothing => {}
            pm_config::Finish::OpenReview if changed > 0 => {
                self = self.handle(Event::Sidebar(SidebarEvent::Session(position)));
                self.tab = Tab::Diff;
            }
            pm_config::Finish::Notify | pm_config::Finish::OpenReview => {
                let how = self.window.agents.notification;
                if how != pm_config::Notification::None {
                    let body = match changed {
                        0 => "The agent finished its turn.".to_owned(),
                        1 => "The agent finished its turn with 1 file changed.".to_owned(),
                        many => format!("The agent finished its turn with {many} files changed."),
                    };
                    self.outbox.push(Outgoing::Notify { title, body, how });
                }
            }
        }
        self
    }

    /// The plan the open session's agent proposed, while the panel for it is open.
    pub fn plan_shown(&self) -> Option<&str> {
        self.open()
            .and_then(|session| session.ask.as_ref())
            .and_then(|ask| ask.plan.as_deref())
            .filter(|_| self.plan_open)
    }

    /// Answers the question with its first refusal, if it offers one.
    fn reject(&mut self) {
        let refusal = self
            .open()
            .and_then(|session| session.ask.as_ref())
            .and_then(|ask| {
                ask.choices
                    .iter()
                    .position(|choice| choice.verdict == pm_harness::Verdict::Reject)
            });
        if let Some(index) = refusal {
            self.choose(index);
        }
    }

    /// Whether an agent is at work, so the window should keep looking for what it says.
    pub fn waiting(&self) -> bool {
        self.sessions
            .iter()
            .any(|session| session.status == Status::Working || session.background_running())
    }

    /// Shows the page that starts work in the project at `project`, in its `submodule` when one is given.
    pub(crate) fn start_in(&mut self, project: usize, submodule: Option<String>) {
        self.page = Page::New(project);
        self.new_in = submodule;
    }

    /// The state after one of the rows or chips of a list, or a button of the parts that handle their own, was pressed.
    fn after_listed(&mut self, id: Target) {
        if matches!(id, Target::Diff(DiffEvent::Gutter { .. }))
            || self.press_exhibit(id)
            || self.press_inquiry(id)
            || self.press_terminal(id)
            || self.press_debug(id)
            || self.press_continue(id)
            || self.press_subagent_stop(id)
            || self.press_choice(id)
        {
            return;
        }
        match id {
            Target::Dialog(DialogEvent::AdderRow(index))
                if index < self.adder.as_ref().map_or(0, |open| open.count()) =>
            {
                self.adder_chosen(index);
            }
            Target::Sidebar(SidebarEvent::Project(index)) if index < self.projects.len() => {
                self.page = Page::Project(index);
                self.overview_scroll.jump_to(0.0);
                self.rescope_browser();
            }
            Target::Sidebar(SidebarEvent::NewIn(index)) if index < self.projects.len() => {
                self.start_in(index, None);
            }
            Target::Sidebar(SidebarEvent::NewInSubmodule(row)) => {
                if let Some((project, submodule)) =
                    crate::sidebar::submodule_rows(self).into_iter().nth(row)
                {
                    self.start_in(project, Some(submodule));
                }
            }
            Target::Sidebar(SidebarEvent::Session(index)) if index < self.sessions.len() => {
                self.sessions[index].closed = false;
                if let Page::Session(before) = self.page
                    && before != index
                {
                    self.tab_stash.insert(before, self.tab);
                }
                self.page = Page::Session(index);
                self.tab = Tab::Chat;
                self.file = None;
                self.comment_target = None;
                self.fresh_git();
                self.chat_list
                    .scroll()
                    .jump_to(self.sessions[index].chat_back);
                self.rescope_browser();
            }
            Target::Sidebar(SidebarEvent::Agent(index)) if index < self.agent_slots() => {
                self.agent = index;
                self.ask_allowance(index, true);
            }
            Target::Chat(ChatEvent::Choice(index)) if index < self.choices() => self.choose(index),
            Target::Chat(ChatEvent::Hurry(index)) if index < self.queued_open() => {
                self.act_on_queued(index, false);
            }
            Target::Chat(ChatEvent::RemoveQueued(index)) if index < self.queued_open() => {
                self.act_on_queued(index, true);
            }
            Target::Chat(ChatEvent::EditQueued(index)) if index < self.queued_open() => {
                self.open_queued_edit(index);
            }
            Target::Chat(ChatEvent::Copy(index)) if index < self.messages_open() => {
                self.copy(index)
            }
            Target::Chat(ChatEvent::Rewind(index)) if index < self.messages_open() => {
                self.rewind(index);
            }
            Target::Chat(ChatEvent::Fork(index)) if index < self.messages_open() => {
                self.fork(index)
            }
            Target::Chat(ChatEvent::Restore(index)) if index < self.messages_open() => {
                self.restoring = Some(index);
            }
            Target::Chat(ChatEvent::TurnChanges(index)) if index < self.messages_open() => {
                self.turn_changes(index);
            }
            Target::Chat(ChatEvent::Turn(index)) if index < self.messages_open() => {
                self.toggle_turn(index);
            }
            Target::Tree(TreeEvent::Tree(index))
                if index < self.files.len() + self.tree_folders.len() =>
            {
                self.fold_tree_folder(index);
            }
            Target::Chat(ChatEvent::Link {
                message,
                piece,
                link,
            }) => self.open_link(message, piece, link),
            Target::Chat(ChatEvent::Details { message, place }) => {
                if let Some(open) = self.open_index() {
                    toggle_key(&mut self.unfolded, (open, message, place));
                }
            }
            Target::Chat(ChatEvent::Step { message, position }) => {
                if let Some(open) = self.open_index() {
                    toggle_key(&mut self.opened, (open, message, position));
                }
            }
            Target::Diff(DiffEvent::ChangedFile(index))
                if self
                    .review_session()
                    .and_then(|session| session.changes.get(index))
                    .is_some_and(|change| change.conflicted) =>
            {
                if self.open().is_some() {
                    self.open_change(index);
                } else {
                    self.request_resolver(index);
                }
            }
            Target::Diff(DiffEvent::ChangedFile(index)) if index < self.change_count() => {
                self.open_change(index);
            }
            Target::Diff(DiffEvent::Stage(index)) if index < self.change_count() => {
                self.stage_changes(&[index]);
            }
            Target::Diff(DiffEvent::ChangeFolder(index)) if index < self.change_folder_count() => {
                self.fold_change_folder(index);
            }
            Target::Diff(DiffEvent::ChangeFolderStage(index))
                if index < self.change_folder_count() =>
            {
                self.stage_change_folder(index);
            }
            Target::Diff(DiffEvent::OpenArrow(index)) if index < self.change_count() => {
                self.open_changed_file(index);
            }
            Target::Diff(DiffEvent::DiffFold(index)) if index < self.change_count() => {
                self.fold_diff(index);
            }
            Target::Diff(DiffEvent::CommentFile(index)) if index < self.change_count() => {
                self.start_comment(index, None, false);
            }
            Target::Diff(DiffEvent::CommentRemove(index)) if index < self.comment_count() => {
                self.remove_comment(index);
            }
            Target::Diff(DiffEvent::Viewed(index)) if index < self.change_count() => {
                self.change_at(index, |change| change.viewed = !change.viewed);
            }
            _ => {}
        }
    }

    /// The files sent with the open session's messages that no preview is known for, each once.
    pub fn unpreviewed_sent_files(&self) -> Vec<String> {
        let Some(session) = self.open_index().and_then(|index| self.sessions.get(index)) else {
            return Vec::new();
        };
        let mut files: Vec<String> = Vec::new();
        for path in session
            .messages
            .iter()
            .flat_map(|message| message.attachments.iter())
        {
            if !self.previews.contains_key(path) && !files.contains(path) {
                files.push(path.clone());
            }
        }
        files
    }

    /// The position of the open session, if the page is one.
    pub fn open_index(&self) -> Option<usize> {
        match self.page {
            Page::Session(index) => Some(index),
            _ => None,
        }
    }

    /// How many messages the open session has.
    fn messages_open(&self) -> usize {
        self.open().map_or(0, |session| session.messages.len())
    }

    /// Asks for the reply at `index` of the open session to be put on the clipboard.
    fn copy(&mut self, index: usize) {
        let Some(message) = self.open().and_then(|session| session.messages.get(index)) else {
            return;
        };
        let text = message.markdown_text();
        self.outbox.push(Outgoing::Copy { text });
    }

    /// Opens in the browser the web address of the link at `link` of the piece `piece` of the reply at `message` of the open session.
    fn open_link(&mut self, message: usize, piece: usize, link: usize) {
        let address = self
            .open()
            .and_then(|session| session.messages.get(message))
            .and_then(|message| {
                let (_, piece) = *message.chunks().get(piece)?;
                pm_markdown::links(piece).into_iter().nth(link)
            })
            .filter(|address| pm_harness::is_web_address(address));
        if let Some(address) = address {
            self.outbox.push(Outgoing::Open { address });
        }
    }

    /// Takes the open session's conversation back to before the reader's message at `index`, which goes back into the prompt box to be written again.
    fn rewind(&mut self, index: usize) {
        let Page::Session(session) = self.page else {
            return;
        };
        let held = &self.sessions[session];
        let Some(message) = held.messages.get(index).filter(|message| message.from_user) else {
            return;
        };
        if held.status == Status::Working {
            return;
        }
        let turn = held.turn_of(index);
        self.prompt = prompt_field(&message.text);
        self.outbox.push(Outgoing::Rewind { session, turn });
    }

    /// Starts a session that branches off the open session's conversation after the agent's reply at `index`.
    fn fork(&mut self, index: usize) {
        let Page::Session(session) = self.page else {
            return;
        };
        let held = &self.sessions[session];
        let writing = held.status == Status::Working && index + 1 == held.messages.len();
        let Some(turn) = held.turn_of(index).checked_sub(1) else {
            return;
        };
        if !writing
            && held
                .messages
                .get(index)
                .is_some_and(|reply| !reply.from_user)
        {
            self.outbox.push(Outgoing::Fork { session, turn });
        }
    }

    /// Puts the open session's files back as they were before the reader's message at `restoring`, when there is one.
    fn restore(&mut self, restoring: Option<usize>) {
        let (Page::Session(session), Some(index)) = (self.page, restoring) else {
            return;
        };
        let held = &self.sessions[session];
        if held.status == Status::Working
            || !held
                .messages
                .get(index)
                .is_some_and(|message| message.checkpoint)
        {
            return;
        }
        let turn = held.turn_of(index);
        self.outbox.push(Outgoing::Restore { session, turn });
    }

    /// Asks for what the agent changed during the turn the reader's message at `index` of the open session started, when the files were kept before it.
    fn turn_changes(&mut self, index: usize) {
        let Page::Session(session) = self.page else {
            return;
        };
        let held = &self.sessions[session];
        if held
            .messages
            .get(index)
            .is_some_and(|message| message.from_user && message.checkpoint)
        {
            let turn = held.turn_of(index);
            self.outbox.push(Outgoing::TurnChanges { session, turn });
        }
    }

    /// Shows or hides the steps of the reply at `index` of the open session.
    fn toggle_turn(&mut self, index: usize) {
        let Page::Session(open) = self.page else {
            return;
        };
        let Some(session) = self.sessions.get_mut(open) else {
            return;
        };
        let latest = session.latest_steps();
        if let Some(message) = session.messages.get_mut(index) {
            message.open = Some(!message.open.unwrap_or(latest == Some(index)));
        }
    }

    /// Stops the subagent whose stop button `id` is, one of the open session's that is still at work: asks its server to, and takes the button away until the server says how the subagent stands. Returns whether `id` is such a button.
    fn press_subagent_stop(&mut self, id: Target) -> bool {
        let Target::Chat(ChatEvent::SubagentStop {
            message,
            position: step,
        }) = id
        else {
            return false;
        };
        let Page::Session(open) = self.page else {
            return true;
        };
        let tool = self
            .sessions
            .get_mut(open)
            .and_then(|session| session.messages.get_mut(message))
            .and_then(|reply| reply.steps.get_mut(step));
        if let Some(crate::sample::Step::Tool(tool)) = tool
            && let Some(subagent) = tool.stoppable().map(str::to_owned)
        {
            if let Some(stop) = &mut tool.stop {
                stop.asked = true;
            }
            self.outbox.push(Outgoing::StopSubagent {
                session: open,
                subagent,
            });
        }
        true
    }

    /// The fold-outs open in the reply at `message` of the open session, by place.
    pub fn unfolded_in(&self, message: usize) -> Vec<usize> {
        let page = self.open_index();
        self.unfolded
            .iter()
            .filter(|(session, reply, _)| page == Some(*session) && *reply == message)
            .map(|(_, _, place)| *place)
            .collect()
    }

    /// How many answers the open session's agent is waiting on one of.
    fn choices(&self) -> usize {
        self.open()
            .and_then(|session| session.ask.as_ref())
            .map_or(0, |ask| ask.choices.len())
    }

    /// Answers the agent's question with its choice at `index`.
    fn choose(&mut self, index: usize) {
        let Page::Session(open) = self.page else {
            return;
        };
        let Some(ask) = self
            .sessions
            .get_mut(open)
            .and_then(|session| session.ask.take())
        else {
            return;
        };
        if let Some(choice) = ask.choices.get(index) {
            self.outbox.push(Outgoing::Choose {
                session: open,
                ticket: ask.ticket,
                option: choice.id.clone(),
            });
        }
    }

    /// How many prompts wait for the open session's agent.
    fn queued_open(&self) -> usize {
        self.open().map_or(0, |session| session.queued.len())
    }

    /// Removes the queued prompt at `index` when `remove` is set, otherwise hands it to the agent next, first giving up an edit of a waiting prompt left open so that the edit neither holds the queue nor outlives the prompts it was about.
    fn act_on_queued(&mut self, index: usize, remove: bool) {
        let Page::Session(session) = self.page else {
            return;
        };
        if let Some(queued) = self.sessions[session].queued.get(index) {
            let queued = queued.id;
            if !remove {
                self.give_up_edit_for_hurry(session);
            }
            self.outbox.push(if remove {
                Outgoing::RemoveQueued { session, queued }
            } else {
                Outgoing::Hurry { session, queued }
            });
        }
    }

    /// Asks the open session's agent to stop what it is doing.
    fn stop(&mut self) {
        if let Page::Session(session) = self.page {
            self.outbox.push(Outgoing::Cancel { session });
        }
    }

    /// How many changes the session the review panel is about has.
    pub(crate) fn change_count(&self) -> usize {
        self.review_session()
            .map_or(0, |session| session.changes.len())
    }

    /// How many folders the trees of the changed files the review panel lists have, folded or not: a file can sit under several, as under its repository and then its own folder.
    pub(crate) fn change_folder_count(&self) -> usize {
        self.review_session().map_or(0, |session| {
            crate::change_list::folder_table(&session.changes, self.git.repository.as_deref()).len()
        })
    }

    /// Applies `edit` to the change at `index` of the session the review panel is about.
    fn change_at(&mut self, index: usize, edit: impl FnOnce(&mut Change)) {
        if let Some(open) = self.review_index()
            && let Some(change) = self
                .sessions
                .get_mut(open)
                .and_then(|session| session.changes.get_mut(index))
        {
            edit(change);
        }
    }

    /// Hands the prompt to the agent: as the next message of the open
    /// session, or as the first of a new one. While the agent is on a turn
    /// the server queues it, or, `now`, stops the turn and hands it over in
    /// its place. A question asked on the side with `/btw` is handed over
    /// as a queued prompt is, shown as nothing until its answer comes. While
    /// a waiting prompt is being edited, it saves the edit instead, and the
    /// command that switches orchestrator mode is answered here and not sent.
    fn send(&mut self, now: bool) {
        if self.editing_queued.is_some() {
            self.save_queued_edit();
            return;
        }
        if self.orchestrate_typed() {
            self.prompt = prompt_field("");
            self.reset_slash();
            self.toggle_orchestrator();
            return;
        }
        let mut text = self.prompt.text().trim().to_owned();
        if !self.elements.is_empty() {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str(&crate::elements::message(&self.elements));
        }
        let captured = crate::captures::message(&self.captures);
        if !captured.is_empty() {
            if !text.is_empty() {
                text.truncate(text.trim_end().len());
                text.push_str("\n\n");
            }
            text.push_str(&captured);
        }
        let sent_files = self.prompt_files();
        if text.is_empty() && sent_files.is_empty() {
            return;
        }
        let starting = matches!(self.page, Page::New(_) | Page::Chat);
        let session = match self.page {
            Page::Session(index) => {
                let Some(session) = self.sessions.get_mut(index) else {
                    return;
                };
                if session.signed_out.is_some() {
                    return;
                }
                if session.status == Status::Working || pm_harness::aside_question(&text).is_some()
                {
                    self.queue(index, text, now);
                    return;
                }
                session.echoes += 1;
                session.closed = false;
                let mut message = Message::user(&text);
                message.attachments = sent_files;
                session.messages.push(message);
                index
            }
            Page::New(project) if project >= self.projects.len() => return,
            Page::New(_) | Page::Chat => {
                let title: String = text.chars().take(40).collect();
                let project = match self.page {
                    Page::New(project) => Some(project),
                    _ => None,
                };
                let mut session = match project {
                    Some(project) => {
                        let mut session = Session::started(&title, project, &text);
                        session.directory = self.projects[project]
                            .directory()
                            .to_string_lossy()
                            .into_owned();
                        session.base = self.base_for_new_work();
                        session.in_container = self.container_for_new_work();
                        session.submodule = self.new_in.take();
                        session.directory = session.folder_in(&session.directory);
                        session.branch = if session.base.is_some() {
                            String::new()
                        } else {
                            self.branches.first().cloned().unwrap_or_default()
                        };
                        session
                    }
                    None => Session::started_chat(&title, self.chat_on.clone(), &text),
                };
                session.agent = self.agent;
                session.login = self.login_for_new_work(self.agent).map(str::to_owned);
                session.messages[0].attachments = sent_files;
                self.sessions.push(session);
                let index = self.sessions.len() - 1;
                self.pane_stash.insert(index, self.panes.clone());
                match project {
                    Some(project) => self.hand_over_new_page(project, index),
                    None => self.hand_over_new_chat(index),
                }
                self.page = Page::Session(index);
                self.tab = Tab::Chat;
                index
            }
            Page::Project(_) | Page::History | Page::Extensions => return,
        };
        let files = self.take_prompt();
        let held = &mut self.sessions[session];
        held.status = Status::Working;
        held.updated_at = crate::age::now();
        held.turn_started = Some(std::time::Instant::now());
        let outgoing = if starting {
            Outgoing::Start {
                session,
                text,
                files,
                settings: self.settings_changed(),
                base: self.sessions[session].base.clone(),
                container: self.sessions[session].in_container,
            }
        } else {
            Outgoing::Prompt {
                session,
                text,
                files,
                now: false,
            }
        };
        self.outbox.push(outgoing);
    }

    /// Hands `text` to the server for the agent of `session`, which is on a
    /// turn: to wait behind it, or, `now`, in place of it. The server says
    /// when it waits and when it is handed over, so nothing is shown yet.
    fn queue(&mut self, session: usize, text: String, now: bool) {
        let files = self.take_prompt();
        self.outbox.push(Outgoing::Prompt {
            session,
            text,
            files,
            now,
        });
    }

    /// Empties the prompt box for the next message and returns the files that were attached.
    fn take_prompt(&mut self) -> Vec<String> {
        let files = self.prompt_files();
        self.prompt = prompt_field("");
        self.chat_list.scroll().jump_to(0.0);
        if let Some(index) = self.open_index() {
            self.sessions[index].chat_back = 0.0;
        }
        self.reset_slash();
        self.elements.clear();
        self.captures.clear();
        if matches!(self.picker, Some(Picker::Element(_) | Picker::Capture(_))) {
            self.picker = None;
        }
        self.attachments.clear();
        files
    }

    /// The files that go with the prompt: those attached and the screenshots of the captures held, each once.
    fn prompt_files(&self) -> Vec<String> {
        let mut files = self.attachments.clone();
        for path in self.capture_files() {
            if !files.contains(&path) {
                files.push(path);
            }
        }
        files
    }

    /// Stages every change of the session the review panel is about, or unstages them all when all are staged.
    fn stage_all(&mut self) {
        let all: Vec<usize> = (0..self.change_count()).collect();
        self.stage_changes(&all);
    }

    /// Stages the changes at `indices` of the session the review panel is about, or unstages them when all of them are staged already.
    pub(crate) fn stage_changes(&mut self, indices: &[usize]) {
        let stage = !self.review_session().is_some_and(|session| {
            indices
                .iter()
                .filter_map(|&index| session.changes.get(index))
                .all(|change| change.fully_staged())
        });
        self.set_staged(indices, stage);
    }

    /// Stages the changes at `indices` of the session the review panel is about, or, when `stage` is not set, unstages them.
    pub(crate) fn set_staged(&mut self, indices: &[usize], stage: bool) {
        self.git.trouble = None;
        let Some(open) = self.review_index() else {
            return;
        };
        let Some(session) = self.sessions.get_mut(open) else {
            return;
        };
        let mut paths = Vec::new();
        for &index in indices {
            if let Some(change) = session.changes.get_mut(index)
                && !change.conflicted
                && change.fully_staged() != stage
            {
                if stage {
                    change.index = Some(match change.worktree {
                        Some('?') => 'A',
                        Some(letter) => letter,
                        None => change.index.unwrap_or('M'),
                    });
                    change.worktree = None;
                    change.untracked = false;
                } else if change.index == Some('A') {
                    change.index = None;
                    change.worktree = Some('?');
                    change.untracked = true;
                } else {
                    change.worktree = change.index.take().or(Some('M'));
                }
                change.staged = stage;
                change.unstaged = !stage;
                paths.push(change.path.clone());
            }
        }
        if !paths.is_empty() {
            self.outbox.push(Outgoing::Stage {
                session: open,
                paths,
                stage,
            });
        }
    }

    /// How many comments the open session has waiting.
    fn comment_count(&self) -> usize {
        self.open().map_or(0, |session| session.comments.len())
    }

    /// Opens the box for a comment on the change at `change`, on `range` or on the whole file.
    fn start_comment(&mut self, change: usize, range: Option<Range>, dragging: bool) {
        self.comment_text = TextFieldState::default();
        self.comment_target = Some(CommentTarget {
            change,
            range,
            anchor: range.map_or(0, |range| range.start),
            dragging,
        });
    }

    /// The state after the pointer went down on the line number `id` of the diff, which starts a selection of lines.
    pub fn comment_drag_begun(mut self, id: Target) -> Self {
        if let Target::Diff(DiffEvent::Gutter { change, new, line }) = id {
            let range = Range {
                new,
                start: line,
                end: line,
            };
            self.start_comment(change, Some(range), true);
        }
        self
    }

    /// The state after the pointer, held down, moved over the line number `id`, which stretches the selection to it when it is on the same side of the same file.
    pub fn comment_drag_extended(mut self, id: Target) -> Self {
        let Target::Diff(DiffEvent::Gutter { change, new, line }) = id else {
            return self;
        };
        if let Some(target) = self.comment_target.as_mut()
            && target.dragging
            && target.change == change
            && let Some(range) = target.range.as_mut()
            && range.new == new
        {
            range.start = target.anchor.min(line);
            range.end = target.anchor.max(line);
        }
        self
    }

    /// The state after the pointer was let go, which opens the box for a comment on the lines selected.
    pub fn comment_drag_ended(mut self) -> Self {
        if let Some(target) = self.comment_target.as_mut() {
            target.dragging = false;
        }
        self
    }

    /// Whether lines of the diff are being dragged over.
    pub fn comment_dragging(&self) -> bool {
        self.comment_target.is_some_and(|target| target.dragging)
    }

    /// Whether the box for a comment is open.
    pub fn commenting(&self) -> bool {
        self.comment_target.is_some_and(|target| !target.dragging)
    }

    /// Keeps the comment that was written on what it is about.
    fn add_comment(&mut self) {
        let text = self.comment_text.text().trim().to_owned();
        let Some(target) = self.comment_target else {
            return;
        };
        let Page::Session(open) = self.page else {
            return;
        };
        let Some(session) = self.sessions.get_mut(open) else {
            return;
        };
        let Some(change) = session.changes.get(target.change) else {
            return;
        };
        if text.is_empty() {
            return;
        }
        session.comments.push(Comment {
            path: change.path.clone(),
            range: target.range,
            text,
        });
        self.comment_target = None;
        self.comment_text = TextFieldState::default();
    }

    /// Takes back the comment at `index` of the open session.
    fn remove_comment(&mut self, index: usize) {
        if let Page::Session(open) = self.page
            && let Some(session) = self.sessions.get_mut(open)
            && index < session.comments.len()
        {
            session.comments.remove(index);
        }
    }

    /// Hands the comments of the open session to its agent as the next message, and shows the conversation.
    fn send_comments(&mut self) {
        let Page::Session(open) = self.page else {
            return;
        };
        let Some(session) = self.sessions.get_mut(open) else {
            return;
        };
        if session.comments.is_empty() || self.editing_queued.is_some() {
            return;
        }
        let items: Vec<String> = session
            .comments
            .iter()
            .map(|comment| match comment.range {
                Some(range) => {
                    format!("- {}, {}: {}", comment.path, range.describe(), comment.text)
                }
                None => format!("- {}: {}", comment.path, comment.text),
            })
            .collect();
        let message = format!(
            "Please address these review comments:\n\n{}",
            items.join("\n")
        );
        session.comments.clear();
        let draft = std::mem::replace(&mut self.prompt, prompt_field(&message));
        let attachments = std::mem::take(&mut self.attachments);
        let captures = std::mem::take(&mut self.captures);
        self.send(false);
        self.prompt = draft;
        self.attachments = attachments;
        self.captures = captures;
        self.comment_target = None;
        self.tab = Tab::Chat;
    }

    /// Shows `tab` in the review panel from its top.
    pub(crate) fn show_review_tab(&mut self, tab: ReviewTab) {
        self.review_tab = tab;
        self.tree_scroll.jump_to(0.0);
        self.files_list.scroll().jump_to(0.0);
    }

    /// Opens the diff scrolled to the change at `index`, unfolded, and keeps it scrolled there while the files above it settle.
    pub(crate) fn open_change(&mut self, index: usize) {
        self.file = Some(index);
        self.select_change(index);
        self.request_resolver(index);
        self.tab = Tab::Diff;
        if let Some(path) = self.change_path(index) {
            self.closed_diffs.remove(&path);
        }
        self.diff_anchor = self.review_index().zip(self.change_path(index));
        self.scroll_diff_to(0.0);
        self.pin_diff();
    }

    /// Folds the folder at `position` of the open session's changes, or unfolds it.
    fn fold_change_folder(&mut self, position: usize) {
        if let Some(folder) = self.change_folder(position) {
            toggle(
                &mut self.closed_folders,
                crate::change_list::fold_key(folder.group, &folder.path),
            );
        }
    }

    /// Stages everything under the folder at `position` of the open session's changes, or unstages it when all of it is staged.
    fn stage_change_folder(&mut self, position: usize) {
        if let Some(folder) = self.change_folder(position) {
            self.stage_changes(&folder.files);
        }
    }

    /// The folder at `position` of the trees of the changed files the review panel lists.
    fn change_folder(&self, position: usize) -> Option<crate::change_list::GroupFolder> {
        self.review_session().and_then(|session| {
            crate::change_list::folder_table(&session.changes, self.git.repository.as_deref())
                .into_iter()
                .nth(position)
        })
    }

    /// The path of the change at `index` of the session the review panel is about.
    fn change_path(&self, index: usize) -> Option<String> {
        self.review_session()
            .and_then(|session| session.changes.get(index))
            .map(|change| change.path.clone())
    }

    /// Folds the file at `index` of the diff, or unfolds it.
    fn fold_diff(&mut self, index: usize) {
        if let Some(path) = self.change_path(index) {
            toggle(&mut self.closed_diffs, path);
        }
        self.pin_diff();
    }

    /// Folds the folder at `position` of the project's files, or unfolds it.
    fn fold_tree_folder(&mut self, position: usize) {
        let paths = crate::file_tree::folder_paths(&self.files, &self.tree_folders);
        if let Some(path) = paths.into_iter().nth(position) {
            self.select_tree(TreeTarget {
                path: path.clone(),
                folder: true,
            });
            let ignored = self.tree_ignored.contains(&path);
            toggle(&mut self.open_tree, path.clone());
            if ignored && self.open_tree.contains(&path) {
                self.reread_files();
            }
        }
    }

    /// Scrolls the diff `offset` logical pixels down from its top, as the state's
    /// own move rather than the reader's.
    pub fn scroll_diff_to(&mut self, offset: f32) {
        self.diff_list.scroll().jump_to(offset);
        self.diff_seen = self.diff_list.scroll().offset();
    }

    /// Whether the reader scrolled the diff or the conversation since the state
    /// last looked, so [`follow_scrolls`](Self::follow_scrolls) has something to catch up with.
    pub fn scrolls_moved(&self) -> bool {
        self.diff_list.scroll().offset() != self.diff_seen
            || self.open_index().is_some_and(|index| {
                self.sessions[index].chat_back != self.chat_list.scroll().offset()
            })
    }

    /// Whether the conversation is shown at its end, give or take a pixel.
    pub fn conversation_at_end(&self) -> bool {
        self.chat_list
            .scroll()
            .offset()
            .min(self.chat_list.scroll().max_offset())
            <= 1.0
    }

    /// Catches up with what the reader scrolled since the state last looked:
    /// a diff scrolled by hand is no longer kept at the file it was opened at,
    /// and the open session keeps how far its conversation is scrolled back
    /// for when it is shown again. Says whether the diff moved, so the diffs
    /// that came into view can be asked for.
    pub fn follow_scrolls(&mut self) -> bool {
        if let Some(index) = self.open_index() {
            self.sessions[index].chat_back = self.chat_list.scroll().offset();
        }
        let offset = self.diff_list.scroll().offset();
        if offset == self.diff_seen {
            return false;
        }
        self.diff_seen = offset;
        self.diff_anchor = None;
        true
    }

    /// Commits the staged changes of the review folder under the message typed, or, with none staged, the changes of its tracked files, and pushes afterwards when `push` is set.
    pub(crate) fn commit_with(&mut self, push: bool, amend: bool) {
        let message = self.message.text().trim().to_owned();
        let Some(index) = self.review_index() else {
            return;
        };
        let Some(session) = self.sessions.get_mut(index) else {
            return;
        };
        let staged = session.changes.iter().any(|change| change.staged);
        let tracked = !staged && !amend;
        if message.is_empty()
            || (!amend
                && !staged
                && !session
                    .changes
                    .iter()
                    .any(|change| !change.untracked && !change.conflicted))
        {
            return;
        }
        if amend {
            session.commits.pop();
        }
        session.changes.retain_mut(|change| {
            if amend {
                return !change.staged || change.unstaged;
            }
            if tracked {
                return change.untracked || change.conflicted;
            }
            if !change.staged {
                return true;
            }
            change.index = None;
            change.staged = false;
            change.unstaged
        });
        session.commits.push(message.clone());
        self.message = commit_field();
        self.file = None;
        self.git.marked.clear();
        self.git.busy = Some(if push { "Pushing…" } else { "Committing…" });
        self.git.trouble = None;
        self.outbox.push(Outgoing::Commit {
            session: index,
            message,
            tracked,
            push,
            amend,
        });
    }

    /// The state after the folder of the session at `session` is ready: it
    /// works in `directory`, on `branch` or on a detached `head`.
    pub fn worktree_ready(
        mut self,
        session: usize,
        directory: &str,
        branch: Option<&str>,
        head: Option<&str>,
    ) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            held.directory = held.folder_in(directory);
            if branch.is_some() || head.is_some() {
                held.branch = Session::checkout_label(branch, head, held.base.as_deref());
            }
        }
        self.refresh_files();
        self
    }

    /// The state after the branch of the folder of the session at `session` was seen to stand as `tracking` says against the one it follows, `branch` being the one it has checked out, if it is on one.
    pub fn tracked(
        mut self,
        session: usize,
        branch: Option<&str>,
        tracking: pm_git::Tracking,
        operation: Option<pm_git::Operation>,
    ) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            held.on_branch = branch.is_some();
            held.tracking = tracking;
        }
        if self.review_index() == Some(session) {
            self.git.operation = operation;
        }
        self
    }

    /// The state after the folder of the session at `session` was seen to hold repositories that stand as `standings` say, none when it is one repository.
    pub fn with_standings(mut self, session: usize, standings: Vec<pm_git::Standing>) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            held.standings = standings;
        }
        self
    }

    /// The state after the project at `project` was seen to hold the submodules at `paths`.
    pub fn with_submodules(mut self, project: usize, paths: Vec<String>) -> Self {
        if let Some(held) = self.projects.get_mut(project) {
            held.submodules = paths;
            if self.page == Page::New(project)
                && self
                    .new_in
                    .as_ref()
                    .is_some_and(|path| !held.submodules.contains(path))
            {
                self.new_in = None;
            }
        }
        self
    }

    /// The state after the project at `project` was seen to have, or lack, a `devcontainer.json`.
    pub fn with_devcontainer(mut self, project: usize, configured: bool) -> Self {
        if let Some(held) = self.projects.get_mut(project) {
            held.devcontainer = configured;
        }
        self
    }

    /// Asks the machine of the project at `project` for its submodules.
    pub(crate) fn ask_submodules(&mut self, project: usize) {
        if let Some(held) = self.projects.get(project) {
            let host = held.host();
            let directory = held.directory().to_string_lossy().into_owned();
            self.outbox.push(Outgoing::DevContainer {
                project,
                host: host.clone(),
                directory: directory.clone(),
            });
            self.outbox.push(Outgoing::Submodules {
                project,
                host,
                directory,
            });
        }
    }

    /// The state after the folder of the session at `session` was seen to have
    /// `branch` checked out, or a detached `head`; a folder that is no repository leaves the label as it was.
    pub fn checked_out(mut self, session: usize, branch: Option<&str>, head: Option<&str>) -> Self {
        if let Some(held) = self.sessions.get_mut(session)
            && (branch.is_some() || head.is_some())
        {
            held.branch = Session::checkout_label(branch, head, held.base.as_deref());
        }
        self
    }

    /// The state after the folder of the session at `session` was looked at
    /// again: these files differ from the last commit, and these commits are its own.
    pub fn refreshed(
        mut self,
        session: usize,
        changes: &[pm_git::Change],
        commits: Option<Vec<String>>,
    ) -> Self {
        let marked = self.marked_by_path();
        if let Some(held) = self.sessions.get_mut(session) {
            held.unknown = false;
            let before: std::collections::HashMap<&str, &Change> = held
                .changes
                .iter()
                .map(|change| (change.path.as_str(), change))
                .collect();
            let now: Vec<Change> = changes
                .iter()
                .map(|found| {
                    let mut change = if found.lazy {
                        Change::listed(&found.path, found.added, found.removed)
                    } else {
                        Change::new(&found.path, &found.diff)
                    };
                    change.words.clone_from(&found.words);
                    change.staged = found.index.is_some() || found.staged;
                    change.unstaged = found.worktree.is_some() || !found.staged;
                    change.untracked = found.untracked;
                    change.conflicted = found.conflicted;
                    change.index = found.index;
                    change.worktree = found.worktree;
                    change.from = found.from.clone();
                    change.submodule = found.submodule.clone();
                    change.repository = found.repository.clone();
                    change.moved = found.moved.clone();
                    let known = before.get(found.path.as_str());
                    change.viewed = known.is_some_and(|known| known.viewed);
                    if let Some(read) = known.filter(|known| found.lazy && known.loaded) {
                        change.diff.clone_from(&read.diff);
                        change.words.clone_from(&read.words);
                        change.loaded = true;
                        change.stale = true;
                    }
                    change
                })
                .collect();
            held.changes = now;
            if let Some(commits) = commits {
                held.commits = commits;
            }
        }
        if self.file.is_some_and(|file| file >= self.change_count()) {
            self.file = None;
        }
        self.remarked(&marked);
        self.pin_diff();
        self
    }

    /// The state after `text` is said in the conversation of the session at `session`, as the app and not the agent.
    pub fn noted(mut self, session: usize, text: &str) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            let mut message = Message::reply(text);
            message.duration = Some(String::new());
            held.messages.push(message);
            if held.status == Status::Working {
                held.status = Status::Idle;
                held.turn_started = None;
            }
        }
        self
    }
}

/// Takes `path` out of `set` when it is in it and puts it in when it is not.
fn toggle(set: &mut std::collections::HashSet<String>, path: String) {
    if !set.remove(&path) {
        set.insert(path);
    }
}

/// Takes `key` out of `list` when it is in it and puts it at the end when it is not.
fn toggle_key(list: &mut Vec<(usize, usize, usize)>, key: (usize, usize, usize)) {
    match list.iter().position(|held| *held == key) {
        Some(at) => {
            list.remove(at);
        }
        None => list.push(key),
    }
}

/// Regression coverage for source control in project folders and session worktrees.
#[cfg(test)]
mod tests {
    use super::*;

    /// The files sent with the open session's messages are wanted until their preview is known.
    #[test]
    fn sent_files_want_a_preview_until_one_is_known() {
        let mut state = WorkspaceState::sample();
        state.page = Page::Session(0);
        let mut message = Message::user("look");
        message.attachments = vec!["/a.png".to_owned(), "/b.png".to_owned()];
        state.sessions[0].messages.push(message.clone());
        state.sessions[0].messages.push(message);
        assert_eq!(state.unpreviewed_sent_files(), ["/a.png", "/b.png"]);
        let picture = rustle::ImageData::from_rgba(1, 1, vec![0, 0, 0, 255]).unwrap();
        let state = state.with_preview("/a.png", picture);
        assert_eq!(state.unpreviewed_sent_files(), ["/b.png"]);
    }
    use crate::areas::ChatEvent;
    use crate::areas::DiffEvent;
    use crate::areas::EditorEvent;
    use crate::areas::GitEvent;
    use crate::areas::SearchEvent;
    use crate::areas::TerminalEvent;
    use crate::git_controls::Pick;
    use crate::panes::{Item, Side};

    /// A press that shows a shell asks it to take the keyboard, and other presses do not.
    #[test]
    fn a_press_notes_what_it_asks_of_the_app() {
        let state = WorkspaceState::sample().handle(Event::Terminal(TerminalEvent::Terminal));
        assert_eq!(state.shell_asked, 1);
        let state = state.handle(Event::Editor(EditorEvent::FindBar));
        assert_eq!(state.shell_asked, 1);
        let state = state.handle(Event::Diff(DiffEvent::TabDiff));
        assert_eq!(state.shell_asked, 1);
    }

    /// Taking in an agent's events in place leaves the workspace as taking them in by value does.
    #[test]
    fn hearing_in_place_matches_hearing_by_value() {
        let events = || {
            vec![
                HarnessEvent::Said("Hello, ".into()),
                HarnessEvent::Thought("hm".into()),
                HarnessEvent::Said("world.".into()),
                HarnessEvent::Failed("it broke".into()),
            ]
        };
        let mut by_value = WorkspaceState::sample();
        let mut in_place = WorkspaceState::sample();
        for event in events() {
            by_value = by_value.heard(0, event);
        }
        for event in events() {
            in_place.hear_in_place(0, event);
        }
        assert_eq!(by_value.sessions[0].status, in_place.sessions[0].status);
        assert_eq!(
            by_value.sessions[0].messages.len(),
            in_place.sessions[0].messages.len()
        );
        assert_eq!(by_value.plan_open, in_place.plan_open);
    }

    /// Removing a selected submodule returns new work to the project root without changing existing sessions.
    #[test]
    fn removed_submodule_resets_new_work() {
        let mut state = WorkspaceState::sample();
        state.start_in(0, Some("vendor/inner".into()));
        let sessions = state.sessions.len();
        state = state.with_submodules(0, vec!["vendor/inner".into()]);
        assert_eq!(state.new_in.as_deref(), Some("vendor/inner"));
        state = state.with_submodules(1, Vec::new());
        assert_eq!(state.new_in.as_deref(), Some("vendor/inner"));
        state = state.with_submodules(0, vec!["vendor/other".into()]);
        assert_eq!(state.new_in, None);
        assert_eq!(state.sessions.len(), sessions);
        assert_eq!(state.projects[0].submodules, ["vendor/other"]);
    }

    /// An open dock on the shells never shows an empty window: a session with none gets a first shell once it is in view.
    #[test]
    fn open_dock_starts_a_shell_for_a_session_with_none() {
        let mut state = WorkspaceState::sample();
        state.terminal = true;
        state.bottom = crate::dock::BottomTab::Terminal;
        let index = state.review_index().unwrap();
        state.sessions[index].terminals.clear();
        let outbox = state.take_outbox();
        assert_eq!(state.sessions[index].terminals.len(), 1);
        assert!(
            outbox
                .iter()
                .any(|out| matches!(out, Outgoing::OpenTerminal { .. }))
        );
        state.take_outbox();
        assert_eq!(state.sessions[index].terminals.len(), 1);
    }

    /// Opening, folding, picking, staging and committing use the folder being reviewed on every page.
    #[test]
    fn review_actions_without_an_open_session() {
        for page in [Page::Project(0), Page::New(0), Page::Session(0)] {
            let mut state = WorkspaceState::sample();
            let changes = state.sessions[0].changes.clone();
            state.page = page;
            state.sync_context();
            let index = state.review_index().unwrap();
            state.sessions[index].changes = changes;
            state.sessions[index].changes[1].untracked = true;
            state = state
                .pane_dragged(Item::Diff, Some((0, Side::Right)), (0.0, 0.0))
                .pane_dropped()
                .handle(Event::Chat(ChatEvent::TabChat));
            for row in [0, 1] {
                let path = state.sessions[index].changes[row].path.clone();
                state.closed_diffs.insert(path.clone());
                state = state.handle(Event::Diff(DiffEvent::ChangedFile(row)));
                assert_eq!(state.file, Some(row));
                assert_eq!(state.current_item(), Item::Diff);
                let panes = state.panes();
                let group = panes.layout.groups()[panes.focused];
                assert_eq!(group.items[group.active], Item::Diff);
                assert!(!state.closed_diffs.contains(&path));
                state = state.handle(Event::Diff(DiffEvent::DiffFold(row)));
                assert!(state.closed_diffs.contains(&path));
            }
            state.tab = Tab::Chat;
            state = state.change_picked(0, Pick::Toggle);
            state = state.change_picked(1, Pick::Range);
            assert_eq!(state.tab, Tab::Chat);
            assert!(state.git.marked.contains(&0));
            assert!(state.git.marked.contains(&1));
            state.set_staged(&[0, 1], true);
            state.take_outbox();
            state.message = TextFieldState::new("Review folder changes");
            state = state.handle(Event::Git(GitEvent::Commit));
            assert!(state.take_outbox().iter().any(|outgoing| matches!(
                outgoing,
                Outgoing::Commit { session, message, .. }
                    if *session == index && message == "Review folder changes"
            )));
        }
    }

    /// Pressing a folder of the changed files folds it, and its menu acts on the files inside it, folded or not.
    #[test]
    fn changed_folders_fold_and_act_on_their_files() {
        let mut state = WorkspaceState::sample();
        state.page = Page::Session(0);
        state.sync_context();
        let index = state.review_index().unwrap();
        state.sessions[index].changes = ["lib", "limb", "plugin", "two_bone"]
            .iter()
            .map(|name| Change::new(&format!("crates/pyre_ik/src/{name}.rs"), ""))
            .collect();
        let key =
            crate::change_list::fold_key(crate::change_list::Group::Tracked, "crates/pyre_ik/src");
        state = state.handle(Event::Diff(DiffEvent::ChangeFolder(0)));
        assert!(state.closed_folders.contains(&key));
        state = state.with_folder_change_menu(0, (0.0, 0.0));
        assert_eq!(state.marked_changes().len(), 4);
        state = state.handle(Event::Diff(DiffEvent::ChangeFolder(0)));
        assert!(!state.closed_folders.contains(&key));
    }

    /// A lone file under a repository of a project that holds several has more folders above it than there are changes, and each of them still folds.
    #[test]
    fn every_folder_above_a_lone_change_folds() {
        let mut state = WorkspaceState::sample();
        state.page = Page::Session(0);
        state.sync_context();
        let index = state.review_index().unwrap();
        let mut change = Change::new("pandemonium/crates/pm-harness/src/steer.rs", "");
        change.untracked = true;
        change.repository = Some("pandemonium".to_owned());
        state.sessions[index].changes = vec![change];
        assert_eq!(state.change_folder_count(), 2);
        let key = crate::change_list::fold_key(
            crate::change_list::Group::Untracked,
            "pandemonium/crates/pm-harness/src",
        );
        state = state.handle(Event::Diff(DiffEvent::ChangeFolder(1)));
        assert!(state.closed_folders.contains(&key));
        assert_eq!(
            state.change_folder_row_of(Target::Diff(DiffEvent::ChangeFolder(1))),
            Some(1)
        );
        state = state.handle(Event::Diff(DiffEvent::ChangeFolder(1)));
        assert!(!state.closed_folders.contains(&key));
    }

    /// Pressing the stop button of a subagent at work asks its server to stop that one, takes the button away, and leaves the panel of subagents open.
    #[test]
    fn stopping_a_subagent_asks_the_server_and_keeps_the_panel() {
        let mut state = WorkspaceState::new(&[]).with_project("/work/app");
        state.sessions.push(Session::started("Test", 0, "Hello"));
        let open = state.sessions.len() - 1;
        state.page = Page::Session(open);
        let session = &mut state.sessions[open];
        session.hear(pm_harness::Event::Tool(pm_harness::Tool {
            id: "t1".to_owned(),
            title: Some("Look".to_owned()),
            kind: None,
            state: Some(pm_harness::ToolState::Running),
            target: None,
            diff: None,
            output: None,
            parent: None,
            agent: Some("scout".to_owned()),
            worktree: None,
        }));
        session.hear(pm_harness::Event::Subagent(pm_harness::Subagent {
            id: "t1".to_owned(),
            call: "t1".to_owned(),
            agent: "scout".to_owned(),
            label: "Look".to_owned(),
            model: None,
            category: None,
            background: false,
            state: pm_harness::SubagentState::Running,
            spent: None,
        }));
        let (reply, position, _) = state.sessions[open].subagents()[0];
        state.activity = Some(Activity::Subagents);
        state.take_outbox();

        let button = Target::Chat(ChatEvent::SubagentStop {
            message: reply,
            position,
        });
        let mut state = state.handle(Event::pressing(button));

        assert_eq!(state.activity, Some(Activity::Subagents));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::StopSubagent { session, subagent } if *session == open && subagent == "t1"
        )));
        let (_, _, tool) = state.sessions[open].subagents()[0];
        assert_eq!(tool.stoppable(), None);
        let mut state = state.handle(Event::pressing(button));
        assert!(
            !state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::StopSubagent { .. })),
            "a second press asks nothing"
        );
    }

    /// The button under a message whose files were kept asks for what its turn changed, and the view that shows it asks for one file's change, or all of them, to be taken back out, closing itself.
    #[test]
    fn a_turn_shows_what_it_changed_and_undoes_it() {
        let mut state = WorkspaceState::new(&[]).with_project("/work/app");
        state.sessions.push(Session::started("Test", 0, "Hello"));
        let open = state.sessions.len() - 1;
        state.page = Page::Session(open);
        state.take_outbox();
        let mut state = state.handle(Event::Chat(ChatEvent::TurnChanges(0)));
        assert!(
            !state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::TurnChanges { .. })),
            "no files were kept"
        );
        state.sessions[open].messages[0].checkpoint = true;
        let mut state = state.handle(Event::Chat(ChatEvent::TurnChanges(0)));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::TurnChanges { session, turn: 0 } if *session == open
        )));

        let change = |path: &str| pm_git::Change {
            path: path.to_owned(),
            diff: "@@ -1 +1 @@\n-a\n+b".to_owned(),
            words: Vec::new(),
            staged: false,
            untracked: false,
            index: Some('M'),
            worktree: None,
            conflicted: false,
            from: None,
            submodule: None,
            repository: None,
            moved: None,
            added: 1,
            removed: 1,
            lazy: false,
        };
        let shown = |state: WorkspaceState| {
            state.revision_opened(
                open,
                "Turn 1 changes".to_owned(),
                Some(0),
                vec![change("a.rs"), change("b.rs")],
            )
        };
        let mut state = shown(state).handle(Event::Search(SearchEvent::ExcerptUndo(1)));
        assert!(state.excerpts.is_none());
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::UndoTurn { session, turn: 0, paths } if *session == open && paths == &["b.rs".to_owned()]
        )));
        let mut state = shown(state).handle(Event::Search(SearchEvent::ExcerptsUndoAll));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::UndoTurn { turn: 0, paths, .. } if paths.is_empty()
        )));
    }
}
