//! What the workspace remembers: the page on screen, the panels that are open and what was typed.

use pm_acp::{Event, Setting, Slash};
use pm_input::FocusId;
use pm_text::{Edit, Field};

use crate::adder::{ADDER_FIELD, ADDER_ROW_BASE, ADDER_SUBMIT, Adder, is_adder};
use crate::arrange::{
    PROJECT_MENU_NEW, PROJECT_MENU_REMOVE, ProjectDrag, ProjectMenu, SessionMenu,
    is_session_menu_item,
};
use crate::context_menu::ContextMenu;
use crate::draft::{NEXT, OTHER};
use crate::folder_picker::{FolderPicker, PICKER_FIELD, PICKER_SUBMIT, is_picker};
use crate::git_controls::{BRANCH_FILTER, BRANCH_SUBMIT, GitControls, OPEN_ARROW_BASE};
use crate::git_prompts::{PROMPT_CONFIRM, RENAME_FIELD, STASH_FIELD};
use crate::message_menu::{
    MESSAGE_MENU_COPY, MESSAGE_MENU_COPY_MARKDOWN, MESSAGE_MENU_COPY_REPLY,
    MESSAGE_MENU_SELECT_ALL, MessageMenu,
};
use crate::open_files::EDITOR_FIELD;
use crate::project::Project;
use crate::sample::{Change, Comment, Message, Range, Session, Status, projects, sessions};
use crate::tree_actions::{ENTRY_FIELD, TreeTarget};

/// A prompt holding `text`, which may run to several lines.
pub fn prompt_field(text: &str) -> Field {
    Field::new(text).multiline(true)
}

/// A box for a commit message, which runs to several lines.
pub fn commit_field() -> Field {
    Field::new("").multiline(true)
}

/// Lists every session.
pub const HISTORY: FocusId = FocusId(301);

/// Adds a project.
pub const ADD_PROJECT: FocusId = FocusId(303);

/// Opens the settings.
pub const SETTINGS: FocusId = FocusId(304);

/// Shows the conversation of the open session.
pub const TAB_CHAT: FocusId = FocusId(310);

/// Shows the diff of the open session.
pub const TAB_DIFF: FocusId = FocusId(311);

/// Shows or hides the terminal.
pub const TERMINAL: FocusId = FocusId(312);

/// Shows or hides the review panel.
pub const REVIEW: FocusId = FocusId(313);

/// Opens the plan the agent proposed in the panel beside the conversation.
pub const OPEN_PLAN: FocusId = FocusId(328);

/// Closes the panel showing the plan.
pub const CLOSE_PLAN: FocusId = FocusId(329);

/// Turns the proposed plan down and gives the prompt the keyboard, to say what to change.
pub const REVISE: FocusId = FocusId(330);

/// The scrolling body of the plan panel.
pub const PLAN_SCROLL: FocusId = FocusId(352);

/// The independently scrolling rows above the composer.
pub const SUBAGENTS_SCROLL: FocusId = FocusId(353);

/// Shows the changed files in the review panel.
pub const REVIEW_CHANGES: FocusId = FocusId(7001);

/// Shows every file of the project in the review panel.
pub const REVIEW_FILES: FocusId = FocusId(7002);

/// The box the comment on a file or on lines is written in.
pub const COMMENT_FIELD: FocusId = FocusId(7003);

/// Keeps the comment that was written.
pub const COMMENT_ADD: FocusId = FocusId(7006);

/// Drops the comment that was being written.
pub const COMMENT_CANCEL: FocusId = FocusId(7007);

/// Hands the comments to the agent as the next message.
pub const SEND_COMMENTS: FocusId = FocusId(7008);

/// The first of the buttons that start a comment on a whole file; the rest follow it.
pub const COMMENT_FILE_BASE: u64 = 7900;

/// The first of the buttons that take a comment back; the rest follow it.
pub const COMMENT_REMOVE_BASE: u64 = 8100;

/// The scrolling body of the diff.
pub const DIFF_SCROLL: FocusId = FocusId(7004);

/// The scrolling list of the review panel.
pub const TREE_SCROLL: FocusId = FocusId(7005);

/// The first of the rows folding a folder of the changed files; the rest follow it.
pub const FOLDER_BASE: u64 = 150_000;

/// The first of the check boxes staging a folder of the changed files; the rest follow it.
pub const FOLDER_STAGE_BASE: u64 = 170_000;

/// The first of the headers folding a file of the diff; the rest follow it.
pub const DIFF_FOLD_BASE: u64 = 7500;

/// The first of the rows folding a folder of the project's files; the rest follow it.
pub const TREE_BASE: u64 = 2_000_000;

/// Opens the diff from the summary of changes.
pub const OPEN_DIFF: FocusId = FocusId(314);

/// Hands the prompt to the agent.
pub const SEND: FocusId = FocusId(315);

/// Stages every change, or none when all are staged.
pub const STAGE_ALL: FocusId = FocusId(316);

/// Commits the staged changes.
pub const COMMIT: FocusId = FocusId(317);

/// Stops the turn the agent is on.
pub const STOP: FocusId = FocusId(322);

/// Stops the turn the agent is on and hands it the prompt in its place.
pub const SEND_NOW: FocusId = FocusId(327);

/// Signs in again as the open session's account.
pub const SESSION_SIGN_IN: FocusId = FocusId(331);

/// The first of the buttons handing a queued prompt to the agent at once; the rest follow it.
pub const HURRY_BASE: u64 = 4900;

/// The first of the buttons removing a waiting prompt; the rest follow it.
pub const REMOVE_QUEUED_BASE: u64 = 4_900_000_000_000;

/// The conversation's scrolling area.
pub const CHAT_SCROLL: FocusId = FocusId(350);

/// The text of the conversation's messages, which the pointer selects as
/// one: each message is a selectable box of it, in the order of the messages.
pub const CHAT_TEXT: FocusId = FocusId(353);

/// The prompt field.
pub const PROMPT: FocusId = FocusId(320);

/// The commit message field.
pub const COMMIT_MESSAGE: FocusId = FocusId(321);

/// The field on the history page that narrows the sessions it lists.
pub const HISTORY_SEARCH: FocusId = FocusId(326);

/// The first of the chips choosing an agent; the rest follow it.
pub const AGENT_BASE: u64 = 4000;

/// The first of the rows folding a project; the rest follow it.
pub const PROJECT_BASE: u64 = 1000;

/// The first of the rows starting work in a project; the rest follow it.
pub const NEW_IN_BASE: u64 = 1100;

/// The first of the rows starting work in a submodule of a project, numbered across the projects as [`crate::sidebar::submodule_rows`] lists them; the rest follow it.
pub const NEW_IN_SUBMODULE_BASE: u64 = 1300;

/// The first of the rows opening a session; the rest follow it.
pub const SESSION_BASE: u64 = 2000;

/// The first of the rows picking a changed file; the rest follow it.
pub const FILE_BASE: u64 = 3000;

/// The first of the check boxes staging a file; the rest follow it.
pub const STAGE_BASE: u64 = 3200;

/// The first of the check boxes marking a file viewed; the rest follow it.
pub const VIEWED_BASE: u64 = 3400;

/// The first of the buttons opening a setting's menu; the rest follow it.
pub const SETTING_BASE: u64 = 4200;

/// The first of the rows of the open setting's menu; the rest follow it.
pub const PICK_BASE: u64 = 4300;

/// The first of the rows of the commands offered behind a slash; the rest follow it.
pub const SLASH_BASE: u64 = 4400;

/// The first of the buttons choosing the effort; the rest follow it.
pub const EFFORT_BASE: u64 = 4500;

/// Opens the menu of where new work is done.
pub const LOCATION: FocusId = FocusId(4250);

/// The first of the rows choosing where new work is done; the rest follow it.
pub const PLACE_BASE: u64 = 4260;

/// The first of the rows choosing the branch new work is made from; the rest follow it.
pub const BRANCH_BASE: u64 = 4600;

/// Opens the agent's plan above the prompt box.
pub const PLAN_CHIP: FocusId = FocusId(4270);

/// Opens the questions waiting for an answer above the prompt box.
pub const APPROVALS_CHIP: FocusId = FocusId(4271);

/// Opens the subagents the agent started above the prompt box.
pub const SUBAGENTS_CHIP: FocusId = FocusId(4272);

/// Opens the commands the agent left running above the prompt box.
pub const SHELLS_CHIP: FocusId = FocusId(4273);

/// The body of the terminal dock, which takes the keyboard while it has focus.
pub const TERMINAL_BODY: FocusId = FocusId(323);

/// Starts another shell in the terminal dock.
pub const NEW_TERMINAL: FocusId = FocusId(324);

/// Makes the terminal dock taller, or gives it its height back.
pub const MAXIMIZE_TERMINAL: FocusId = FocusId(325);

/// The dock's tab listing what is wrong with the files of the project.
pub const BOTTOM_PROBLEMS: FocusId = FocusId(20_200_000);

/// The count of errors and warnings above a file, which shows the dock's tab of problems.
pub const BOTTOM_PROBLEMS_COUNT: FocusId = FocusId(20_200_003);

/// The button in the title bar that shows and hides the sidebar of projects and sessions.
pub const SIDEBAR_TOGGLE: FocusId = FocusId(20_200_004);

/// The dock's tab showing the shells.
pub const BOTTOM_TERMINAL: FocusId = FocusId(20_200_001);

/// A folder of a machine, which the problems of the dock are about: the machine the folder is on, when it is not this one, and the folder.
pub type ProblemsKey = (Option<String>, String);

/// The list of problems in the dock, which scrolls.
pub const PROBLEMS_SCROLL: FocusId = FocusId(20_200_002);

/// The first of the rows of the dock's list of problems; the rest follow it.
pub const PROBLEM_ROW_BASE: u64 = 20_201_000;

/// The first of the tabs showing one of the dock's shells; the rest follow it.
pub const TERMINAL_TAB_BASE: u64 = 5000;

/// The first of the buttons ending one of the dock's shells; the rest follow it.
pub const TERMINAL_CLOSE_BASE: u64 = 5100;

/// The first of the buttons copying a reply; the rest follow it.
pub const COPY_BASE: u64 = 300_000;

/// The first of the buttons taking the conversation back to before one of the reader's messages to write it again; the rest follow it.
pub const REWIND_BASE: u64 = 1_000_000_000_000;

/// The first of the buttons offering to put the files back as they were before one of the reader's messages; the rest follow it.
pub const RESTORE_BASE: u64 = 2_000_000_000_000;

/// The first of the buttons starting a session that branches off the conversation after one of the agent's replies; the rest follow it.
pub const FORK_BASE: u64 = 3_000_000_000_000;

/// The first of the links in the agent's replies; each reply has [`LINKS_PER_REPLY`] after its own base and each piece of it [`LINKS_PER_PIECE`].
pub const LINK_BASE: u64 = 4_000_000_000_000;

/// How many links one reply has room for, its pieces following one another.
pub const LINKS_PER_REPLY: u64 = 100_000;

/// How many links one piece of a reply has room for.
pub const LINKS_PER_PIECE: u64 = 1000;

/// Puts the files back as they were before the message whose offer is open.
pub const RESTORE_FILES: FocusId = FocusId(4273);

/// Closes the offer to put the files back, leaving them as they are.
pub const KEEP_FILES: FocusId = FocusId(4274);

/// The scrolling list of values in the open menu.
pub const MENU_SCROLL: FocusId = FocusId(351);

/// Switches the agent persona on or off.
pub const AGENT_TOGGLE: FocusId = FocusId(4290);

/// Puts a dollar sign in the prompt, which lists the skills the agent offers.
/// The button that attaches files to the prompt.
pub const ATTACH: FocusId = FocusId(318);

/// The first of the buttons that remove an attached file; the rest follow it.
pub const ATTACHED_BASE: u64 = 4800;

/// The button that starts mentioning a file in the prompt.
pub const MENTION: FocusId = FocusId(4281);

pub const SKILL_BUTTON: FocusId = FocusId(4282);

/// Puts a slash in the prompt, which lists the commands the agent offers.
/// The body of an open menu, which is not a button but keeps a click on it from counting as outside.
pub const MENU_BODY: FocusId = FocusId(4299);

/// The chip naming the account the agent is signed in as.
pub const ACCOUNT: FocusId = FocusId(4291);

/// The first of the rows choosing the account; the rest follow it.
pub const ACCOUNT_BASE: u64 = 4700;

pub const SLASH_BUTTON: FocusId = FocusId(4280);

/// The first of the lines folding the steps of a reply; the rest follow it.
pub const TURN_BASE: u64 = 100_000;

/// The first of the lines opening a step's detail; the rest follow it, a thousand to a reply.
pub const STEP_BASE: u64 = 200_000;

/// The first of the summaries folding a part of a reply; the rest follow it, a thousand to a reply.
pub const DETAILS_BASE: u64 = 400_000;

/// The first of the buttons answering the agent's question; the rest follow it.
pub const CHOICE_BASE: u64 = 3600;

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
    /// Has the server keep these as the agents that can be started.
    SaveAgentServers(Vec<pm_protocol::AgentServer>),
    /// Has the server keep these as the tool servers every agent is started with.
    SaveMcpServers(Vec<pm_protocol::McpServer>),
    /// Lists what the agent at `agent` is given in the project at `project`: its skills, instructions, agent definitions and hooks.
    Library {
        /// The agent, by position.
        agent: usize,
        /// The project, by position.
        project: usize,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The project's folder.
        directory: String,
    },
    /// Lists the tools the agent at `agent` is given, on the machine of the project at `project`.
    ToolSets {
        /// The agent, by position.
        agent: usize,
        /// The project, by position.
        project: usize,
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
    SaveMachineSettings(pm_protocol::MachineSettings),
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
    /// Lists the branches of the repository `directory` on `host`, for the project at `project`.
    Branches {
        /// The project, by position.
        project: usize,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The folder.
        directory: String,
    },
    /// Lists the skills of the agent at `agent` in the project at `project`.
    Skills {
        /// The agent, by position.
        agent: usize,
        /// The project, by position.
        project: usize,
        /// The machine, or nothing for this one.
        host: Option<String>,
        /// The project's folder.
        directory: String,
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
        replies: Option<Vec<pm_acp::Reply>>,
    },
    /// Trusts hooks the Codex of a session holds back, as they were shown.
    TrustHooks {
        /// The session, by position.
        session: usize,
        /// The hooks to trust.
        hooks: Vec<pm_acp::Hook>,
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
    /// Stops the turn the agent of a session is on.
    Cancel {
        /// The session, by position.
        session: usize,
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
    /// Fetches, pulls or pushes the repository the folder of a session is in, or syncs it.
    Sync {
        /// The session, by position.
        session: usize,
        /// `fetch`, `pull`, `push` or `sync`.
        operation: &'static str,
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
    Field,
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
    /// An element of a web page picked for the prompt, by position among those picked.
    Element(usize),
    /// A screenshot or the console of a web page held for the prompt, by position among those held.
    Capture(usize),
}

/// Where new work is done.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    /// In a worktree of its own, on a new branch.
    Worktree,
    /// In the project's own folder.
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Page {
    /// One session, by position.
    Session(usize),
    /// A prompt to start work in one project, by position.
    New(usize),
    /// The overview of one project, by position: its sessions and its repository.
    Project(usize),
    /// Every session.
    History,
    /// The extensions there are to install.
    Extensions,
}

/// Which view of a session is showing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
    /// The conversation with the agent.
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

/// About how wide the diff is, in logical pixels, when scrolling it to a file, which only the height of the notes on the files above depends on.
pub(crate) const DIFF_WIDTH: f32 = 900.0;

/// What the review panel lists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewTab {
    /// The files the agent changed, to stage and commit.
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
#[derive(Clone, Debug, PartialEq)]
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
    /// The folders of the project's files that are unfolded, by path.
    pub open_tree: std::collections::HashSet<String>,
    /// The folders of the changed files that are folded away, by group and path.
    pub closed_folders: std::collections::HashSet<String>,
    /// The changed files folded away in the diff, by path.
    pub closed_diffs: std::collections::HashSet<String>,
    /// The lines or file the comment being written or the lines being dragged over are about.
    pub comment_target: Option<CommentTarget>,
    /// The comment being written.
    pub comment_text: Field,
    /// How far down the diff is scrolled, in logical pixels.
    pub diff_scroll: f32,
    /// The files, by session and path, whose diffs were asked of the server and have not come.
    pub(crate) asking_diffs: std::collections::HashSet<(usize, String)>,
    /// How far down the review panel's list is scrolled, in logical pixels.
    pub tree_scroll: f32,
    /// The project whose folder the review panel was last asked to read while no session was open.
    pub homed: Option<usize>,
    /// Whether the plan the agent proposed is open in a panel in place of the review.
    pub plan_open: bool,
    /// How far down the plan panel is scrolled, in logical pixels.
    pub plan_scroll: f32,
    /// How far down the subagent panel rows are scrolled, in logical pixels.
    pub subagents_scroll: f32,
    /// How far down a project's overview is scrolled, in logical pixels.
    pub overview_scroll: f32,
    /// Whether the terminal is open.
    pub terminal: bool,
    /// Whether the terminal is taller than usual.
    pub terminal_maximized: bool,
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
    pub problems_scroll: f32,
    /// The problem of the list the reader pressed, whose place is to be shown.
    pub problem_picked: Option<usize>,
    /// How tall the terminal dock is while it is not maximized, in logical pixels.
    pub terminal_height: f32,
    /// How many columns and lines the dock's window last held.
    pub terminal_size: (u16, u16),
    /// The name the next shell gets in this window.
    pub next_terminal: u64,
    /// The agent chosen for new work, by position.
    pub agent: usize,
    /// The changed file the reader picked, by position, if one was.
    pub file: Option<usize>,
    /// The prompt being written.
    pub prompt: Field,
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
    pub previews: std::collections::HashMap<String, pm_ui::Picture>,
    /// The diagrams and images of markdown drawn or fetched so far.
    pub figures: crate::figures::Figures,
    /// The figures asked for, by their key, so none is asked for twice.
    pub figures_asked: std::collections::HashSet<u64>,
    /// The commit message being written.
    pub message: Field,
    /// What the history page is narrowed to.
    pub history_search: Field,
    /// How far the history session groups have scrolled, in logical pixels.
    pub history_scroll: f32,
    /// How far the sidebar's list is scrolled, in logical pixels.
    pub sidebar_scroll: f32,
    /// The submodule of the project the new session page starts work in, or nothing for the project itself.
    pub new_in: Option<String>,
    /// How wide the sidebar is, in logical pixels.
    pub sidebar_width: f32,
    /// How wide the review panel is, in logical pixels.
    pub review_width: f32,
    /// How wide the plan panel was dragged to be, in logical pixels, or zero while it takes the width it would by itself.
    pub plan_width: f32,
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
    /// Where the menu of panes opened, while it is open.
    pub pane_menu: Option<(f32, f32)>,
    /// How far the conversation is scrolled back from its end, in logical pixels.
    pub chat_back: f32,
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
    /// The panel open over the conversation's chips, if one is.
    pub activity: Option<Activity>,
    /// The commands each agent offers behind a slash, by the agent's position, once it has been asked.
    pub agent_commands: Vec<Vec<Slash>>,
    /// The skills the chosen agent offers in the project being started in.
    pub skills: Vec<Slash>,
    /// The agent and project `skills` was read for.
    pub skills_of: Option<(usize, usize)>,
    /// The names of the skills installed for the chosen agent, which tell a skill from a command in the list behind a slash.
    pub skill_names: std::collections::HashSet<String>,
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
    /// Who each agent is signed in as in its own default folder, by the agent's position, once it has been looked up.
    pub defaults: Vec<Option<Option<String>>>,
    /// The named logins the reader has added: the agent's id, the folder name and what it is called.
    pub logins: Vec<(String, String, String)>,
    /// The login in use for each agent, as the agent's id and the folder name; an agent not named uses its own.
    pub in_use: Vec<(String, String)>,
    /// How far the list of values in the open menu is scrolled, in logical pixels.
    pub menu_scroll: f32,
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
    /// Where the signature of the call the caret is in hangs, in logical pixels of the window.
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
    /// How wide the view of the file shown was last drawn and how wide a character of code is in it, in logical pixels, which soft wrap breaks lines by; nothing until it was drawn.
    pub code_view: Option<(f32, f32)>,
    /// How tall the view of the file shown was last drawn, in logical pixels, which tells the lines in view; nothing until it was drawn.
    pub code_height: Option<f32>,
    /// Whether the reader asked to close the window and nothing is left to ask first.
    pub quitting: bool,
    /// How far down the notes in the window of an update are scrolled, in logical pixels.
    pub update_scroll: f32,
    /// Whether the release notes of the newer release are open as a tab.
    pub update_open: bool,
    /// How far the update the reader asked for has got.
    pub update_run: crate::update_dialog::UpdateRun,
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
        let builtin = pm_protocol::builtin_agents();
        let mut state = Self {
            projects,
            sessions,
            page: Page::New(0),
            tab: Tab::Chat,
            review: true,
            git: GitControls::default(),
            review_tab: ReviewTab::Changes,
            project_search: crate::project_search::ProjectSearch::default(),
            excerpts: None,
            browser: None,
            open_tree: std::collections::HashSet::new(),
            closed_folders: std::collections::HashSet::new(),
            closed_diffs: std::collections::HashSet::new(),
            comment_target: None,
            comment_text: Field::default(),
            diff_scroll: 0.0,
            asking_diffs: std::collections::HashSet::new(),
            tree_scroll: 0.0,
            homed: None,
            plan_open: false,
            plan_scroll: 0.0,
            subagents_scroll: 0.0,
            overview_scroll: 0.0,
            terminal: false,
            terminal_maximized: false,
            sidebar_open: true,
            bottom: crate::dock::BottomTab::default(),
            problems: Vec::new(),
            problems_of: None,
            problems_asked: None,
            problems_scroll: 0.0,
            problem_picked: None,
            terminal_height: crate::terminal::HEIGHT,
            terminal_size: crate::dock::FIRST_SIZE,
            next_terminal: 1,
            agent: 0,
            file: None,
            prompt: prompt_field(""),
            message: commit_field(),
            history_search: Field::default(),
            history_scroll: 0.0,
            sidebar_scroll: 0.0,
            new_in: None,
            sidebar_width: crate::sidebar::WIDTH,
            review_width: crate::review::WIDTH,
            plan_width: 0.0,
            adder: None,
            folder_picker: None,
            settings: None,
            settings_servers: crate::settings_servers::ServerSettings {
                agents: builtin.clone(),
                ..Default::default()
            },
            menu: None,
            restoring: None,
            project_menu: None,
            session_menu: None,
            message_menu: None,
            terminal_menu: None,
            browser_menu: None,
            project_drag: None,
            outbox: Vec::new(),
            files_owner: None,
            file_stash: std::collections::HashMap::new(),
            tab_stash: std::collections::HashMap::new(),
            panes: crate::panes::Panes::default(),
            pane_stash: std::collections::HashMap::new(),
            search_stash: std::collections::HashMap::new(),
            prompt_stash: std::collections::HashMap::new(),
            pane_drag: None,
            pane_menu: None,
            chat_back: 0.0,
            picker: None,
            agent_settings: vec![Vec::new(); builtin.len()],
            touched: Vec::new(),
            choices: Vec::new(),
            location: Location::Local,
            container: false,
            base: None,
            branches: Vec::new(),
            branches_of: None,
            activity: None,
            agent_commands: vec![Vec::new(); builtin.len()],
            skills: Vec::new(),
            skills_of: None,
            skill_names: std::collections::HashSet::new(),
            attachments: Vec::new(),
            elements: Vec::new(),
            captures: Vec::new(),
            previews: std::collections::HashMap::new(),
            figures: crate::figures::Figures::new(),
            figures_asked: std::collections::HashSet::new(),
            discovery_host: None,
            discovered_elsewhere: std::collections::HashMap::new(),
            files: Vec::new(),
            tree_folders: Vec::new(),
            tree_ignored: std::collections::HashSet::new(),
            files_of: None,
            file_listings: Default::default(),
            identities: vec![None; builtin.len()],
            defaults: vec![None; builtin.len()],
            logins: Vec::new(),
            in_use: Vec::new(),
            menu_scroll: 0.0,
            slash: 0,
            opened: Vec::new(),
            unfolded: Vec::new(),
            extensions: Vec::new(),
            extensions_page: crate::extensions::ExtensionsPage::default(),
            extension_assets: std::collections::BTreeMap::new(),
            listing: None,
            recent_commands: Vec::new(),
            recent_files: Vec::new(),
            closed_tabs: Vec::new(),
            renaming: None,
            color_picking: None,
            vim: pm_vim::Vim::default(),
            going_to: None,
            signature_at: (0.0, 0.0),
            finding: None,
            blame: None,
            tab_menu: None,
            editor: pm_editor::EditorSettings::default(),
            editor_loaded: false,
            code_view: None,
            code_height: None,
            quitting: false,
            update_scroll: 0.0,
            update_open: false,
            update_run: crate::update_dialog::UpdateRun::Idle,
            debug: crate::debug::DebugState::default(),
            tasks: crate::tasks::TaskState::default(),
            window: pm_config::WindowSettings::default(),
            window_loaded: false,
            machine: pm_protocol::MachineSettings::default(),
            machine_loaded: false,
            settings_page: crate::settings_state::SettingsPage::default(),
            recording: None,
            recorded: Vec::new(),
            trail: crate::trail::Trail::default(),
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

    /// The state after the sidebar's list is scrolled by `delta` logical
    /// pixels, in a window `height` logical pixels tall, kept within the list.
    pub fn scrolled(mut self, delta: f32, height: f32) -> Self {
        let room = (self.list_height() - crate::sidebar::list_viewport(height)).max(0.0);
        self.sidebar_scroll = (self.sidebar_scroll + delta).clamp(0.0, room);
        self
    }

    /// How tall the sidebar's list of projects is, in logical pixels.
    fn list_height(&self) -> f32 {
        (0..self.projects.len())
            .map(|index| crate::sidebar::project_height(self, index))
            .sum()
    }

    /// Whether the page for starting work is showing, with a project to start it in.
    pub fn starting(&self) -> bool {
        matches!(self.page, Page::New(_)) && !self.projects.is_empty()
    }

    /// How much room the sidebar takes from the left of the window, in logical pixels: its width while it shows and nothing while it is hidden.
    pub fn sidebar_room(&self) -> f32 {
        if self.sidebar_open {
            self.sidebar_width
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

    /// The project the next new work goes to: the open session's, or the one being started in.
    pub fn current_project(&self) -> usize {
        match self.page {
            Page::New(project) | Page::Project(project) => project,
            _ => self.open().map_or(0, |session| session.project),
        }
    }

    /// The state after `action` was pressed.
    pub fn after(mut self, action: FocusId) -> Self {
        if !Self::keeps_picker(action) {
            self.picker = None;
        }
        if !Self::keeps_activity(action) {
            self.activity = None;
        }
        if !Self::keeps_branches(action) {
            self.git.branch_menu = false;
        }
        if !self.is_menu_row(action) {
            self.git.menu = None;
        }
        if !self.settings_menu_row(action) {
            self.settings_servers.menu = None;
        }
        if let Some(place) = action
            .0
            .checked_sub(crate::assist::COMPLETION_ROW_BASE)
            .filter(|_| action.0 < crate::assist::COMPLETION_ROW_END)
        {
            return self.completion_taken(Some(place as usize));
        }
        if action == crate::assist::COMPLETION_BODY || action == crate::assist::HOVER_CARD {
            return self;
        }
        if let Some((file, line)) = crate::debug::breakpoint_of(action) {
            self.toggle_breakpoint(file, line);
            return self;
        }
        if let Some(index) = crate::tasks::run_lens_of(action) {
            self.run_lens_pressed(index);
            return self;
        }
        if let Some((file, line)) = crate::tasks::run_of(action) {
            self.runnable_run(file, line);
            return self;
        }
        if action == crate::lightbulb::LIGHTBULB {
            return self;
        }
        if let Some(line) = crate::viewer::fold_of(action) {
            if let Some(document) = self.shown_text() {
                document.unfold_line(line);
            }
            return self;
        }
        if let Some(row) = crate::listing::listing_row_of(action) {
            return self.listing_taken(Some(row));
        }
        if action == crate::listing::LISTING_TAKE {
            return self.listing_taken(None);
        }
        if action == crate::listing::LISTING_FILTER
            || action == crate::listing::LISTING_BODY
            || action == crate::navigate::RENAME_BOX
            || action == crate::navigate::RENAME_SYMBOL
            || action == crate::color_picker::COLOR_BOX
            || action == crate::color_picker::COLOR_HEX
        {
            return self;
        }
        if action == crate::color_picker::COLOR_HEX_SUBMIT {
            return self.color_format_taken(None);
        }
        if let Some(row) = crate::color_picker::color_format_of(action) {
            return self.color_format_taken(Some(row));
        }
        if action == crate::navigate::RENAME_SYMBOL_SUBMIT {
            return self.rename_submitted();
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
        if crate::project_search::is_search_part(action) {
            return self.project_search_pressed(action);
        }
        if action == crate::line_edits::GO_TO_LINE_SUBMIT {
            return self.go_to_line_submitted();
        }
        if action == crate::line_edits::GO_TO_LINE || action == crate::line_edits::GO_TO_LINE_BOX {
            return self;
        }
        if let Some(address) = self
            .shown_document()
            .and_then(|document| document.hover.as_ref())
            .and_then(|card| card.text.as_deref())
            .and_then(|said| crate::assist::hover_link(said, action))
        {
            if pm_acp::is_web_address(&address) {
                self.outbox.push(Outgoing::Open { address });
            }
            return self;
        }
        if self.pane_tab_of(action).is_some() {
            self.panes.parked = None;
        }
        self.settle_panes();
        self.pane_menu = None;
        if self.press_empty_pane(action) {
            return self;
        }
        if let Some(side) = crate::panes::split_row_of(action) {
            self.split_focused(side);
            return self;
        }
        if let Some(pane) = crate::panes::pane_of(action) {
            self.focus_pane(pane);
            return self;
        }
        let restoring = self.restoring.take();
        match action {
            RESTORE_FILES => self.restore(restoring),
            row if is_session_menu_item(row) => return self.session_menu_pressed(row),
            MESSAGE_MENU_COPY
            | MESSAGE_MENU_COPY_REPLY
            | MESSAGE_MENU_COPY_MARKDOWN
            | MESSAGE_MENU_SELECT_ALL => return self.message_menu_pressed(action),
            PROJECT_MENU_NEW | PROJECT_MENU_REMOVE => return self.project_menu_pressed(action),
            ADD_PROJECT => return self.with_adder(),
            SETTINGS => return self.with_settings(),
            crate::title_bar::UPDATE => self.open_release(),
            crate::title_bar::LANGUAGE_UPDATE => return self.with_extensions_page(),
            update if self.press_update(update) => {}
            action if crate::settings::is_settings(action) => {
                return self.settings_pressed(action);
            }
            action if is_adder(action) && action.0 < ADDER_ROW_BASE => {
                return self.adder_pressed(action);
            }
            action if is_picker(action) => return self.folder_picker_pressed(action),
            HISTORY => self.page = Page::History,
            crate::extensions::EXTENSIONS => return self.with_extensions_page(),
            action if crate::extensions::is_extensions(action) => {
                return self.extensions_pressed(action);
            }
            TAB_CHAT => self.tab = Tab::Chat,
            TAB_DIFF => self.tab = Tab::Diff,
            TERMINAL | NEW_TERMINAL | MAXIMIZE_TERMINAL => {
                self.press_terminal(action);
            }
            REVIEW => self.review = !self.review,
            SIDEBAR_TOGGLE => self.sidebar_open = !self.sidebar_open,
            REVIEW_CHANGES => self.show_review_tab(ReviewTab::Changes),
            REVIEW_FILES => self.show_review_tab(ReviewTab::Files),
            COMMENT_ADD => self.add_comment(),
            COMMENT_CANCEL => self.comment_target = None,
            SEND_COMMENTS => self.send_comments(),
            OPEN_PLAN => self.plan_open = true,
            CLOSE_PLAN => self.plan_open = false,
            REVISE => self.reject(),
            OPEN_DIFF => self.tab = Tab::Diff,
            SESSION_SIGN_IN => {
                if let Page::Session(session) = self.page {
                    self.outbox.push(Outgoing::SignInSession { session });
                }
            }
            SEND => self.send(false),
            SEND_NOW => self.send(true),
            STOP => self.stop(),
            STAGE_ALL => self.stage_all(),
            COMMIT => self.primary_pressed(),
            FocusId(id) => {
                if !self.press_git(action) {
                    self.after_numbered(id);
                }
            }
        }
        self.sync_context();
        self.refresh_files();
        self.ask_log_once();
        self.debug_revealed().problem_revealed()
    }

    /// The state after `edit` is made to the field named `id`.
    pub fn edited(mut self, id: FocusId, edit: Edit) -> Self {
        match id {
            PROMPT => {
                let before = self.prompt.revision();
                self.prompt.apply(edit);
                if self.prompt.revision() != before {
                    self.menu_scroll = 0.0;
                }
            }
            COMMIT_MESSAGE => self.message.apply(edit),
            COMMENT_FIELD => self.comment_text.apply(edit),
            BRANCH_FILTER => self.git.filter.apply(edit),
            STASH_FIELD => self.git.stash_message.apply(edit),
            RENAME_FIELD => self.git.rename.apply(edit),
            EDITOR_FIELD => self.file_edited(edit),
            crate::navigate::RENAME_SYMBOL => {
                if let Some(renaming) = self.renaming.as_mut() {
                    renaming.field.apply(edit);
                    renaming.problem = None;
                }
            }
            crate::color_picker::COLOR_HEX => self.color_hex_edited(edit),
            crate::line_edits::GO_TO_LINE => {
                if let Some(going) = self.going_to.as_mut() {
                    going.field.apply(edit);
                }
            }
            crate::find::FIND_FIELD => return self.find_typed(edit),
            crate::project_search::PROJECT_SEARCH_FIELD => {
                return self.project_search_typed(edit);
            }
            crate::listing::LISTING_FILTER => return self.listing_filtered(edit),
            crate::debug::DEBUG_CONSOLE_FIELD | crate::debug::DEBUG_WATCH_FIELD => {
                self.debug_field_edited(id, edit)
            }
            crate::browser::BROWSER_ADDRESS => {
                if let Some(browser) = self.browser.as_mut() {
                    browser.address.apply(edit);
                }
            }
            crate::browser::BROWSER_FIND_FIELD => self.browser_find_edited(edit),
            crate::find::REPLACE_FIELD => return self.replacement_typed(edit),
            ENTRY_FIELD => {
                if let Some((_, field)) = self.git.naming.as_mut() {
                    field.apply(edit);
                }
            }
            ADDER_FIELD => self.adder_edited(edit),
            PICKER_FIELD => self.folder_picker_edited(edit),
            HISTORY_SEARCH => {
                let before = self.history_search.text().to_owned();
                self.history_search.apply(edit);
                if self.history_search.text() != before {
                    self.history_scroll = 0.0;
                }
            }
            crate::extensions::EXTENSIONS_SEARCH => self.extensions_searched(edit),
            id if crate::settings_servers::is_form_field(id) => self.server_form_edited(id, edit),
            id if crate::settings_pages::is_setting_field(id) => self.setting_edited(id, edit),
            id if crate::settings_machines::is_machine_field(id) => self.machine_edited(id, edit),
            crate::settings::SETTINGS_FIND => {
                self.settings_page.find.apply(edit);
                self.settings_servers.scroll = 0.0;
                self.settings_page.picker = None;
            }
            crate::settings::SETTINGS_KEYMAP_FIND => self.settings_page.keymap_find.apply(edit),
            OTHER => {
                if let Page::Session(open) = self.page
                    && let Some(draft) =
                        self.sessions.get_mut(open).and_then(|s| s.inquiry.as_mut())
                {
                    draft.others[draft.step].apply(edit);
                }
            }
            _ => {}
        }
        self.refresh_files();
        self
    }

    /// The text field `id` names, if it names one.
    pub fn field(&self, id: FocusId) -> Option<&Field> {
        match id {
            PROMPT => Some(&self.prompt),
            COMMIT_MESSAGE => Some(&self.message),
            COMMENT_FIELD => Some(&self.comment_text),
            BRANCH_FILTER => Some(&self.git.filter),
            STASH_FIELD => Some(&self.git.stash_message),
            RENAME_FIELD => Some(&self.git.rename),
            ENTRY_FIELD => self.git.naming.as_ref().map(|(_, field)| field),
            EDITOR_FIELD => self.shown_document().map(|document| &document.field),
            crate::navigate::RENAME_SYMBOL => {
                self.renaming.as_ref().map(|renaming| &renaming.field)
            }
            crate::color_picker::COLOR_HEX => {
                self.color_picking.as_ref().map(|picking| &picking.hex)
            }
            crate::line_edits::GO_TO_LINE => self.going_to.as_ref().map(|going| &going.field),
            crate::browser::BROWSER_ADDRESS => {
                self.browser.as_ref().map(|browser| &browser.address)
            }
            crate::browser::BROWSER_FIND_FIELD => self
                .browser
                .as_ref()
                .and_then(|browser| browser.search.as_ref())
                .map(|search| &search.query),
            crate::find::FIND_FIELD => self.finding.as_ref().map(|finding| &finding.query),
            crate::project_search::PROJECT_SEARCH_FIELD => Some(&self.project_search.query),
            crate::listing::LISTING_FILTER => self.listing_filter(),
            crate::debug::DEBUG_CONSOLE_FIELD => Some(&self.debug.console),
            crate::debug::DEBUG_WATCH_FIELD => Some(&self.debug.watch),
            crate::find::REPLACE_FIELD => self.finding.as_ref().map(|finding| &finding.replacement),
            HISTORY_SEARCH => Some(&self.history_search),
            crate::extensions::EXTENSIONS_SEARCH => Some(&self.extensions_page.search),
            ADDER_FIELD => self.adder.as_ref().map(|open| &open.field),
            PICKER_FIELD => self.folder_picker.as_ref().map(|open| &open.field),
            id if crate::settings_servers::is_form_field(id) => self.server_form_field(id),
            id if crate::settings_pages::is_setting_field(id) => self.setting_field(id),
            id if crate::settings_machines::is_machine_field(id) => self.machine_field(id),
            crate::settings::SETTINGS_FIND => Some(&self.settings_page.find),
            crate::settings::SETTINGS_KEYMAP_FIND => Some(&self.settings_page.keymap_find),
            OTHER => self.other_field(),
            _ => None,
        }
    }

    /// Whether `id` names a text field.
    pub fn is_field(id: FocusId) -> bool {
        id == PROMPT
            || id == COMMIT_MESSAGE
            || id == COMMENT_FIELD
            || id == BRANCH_FILTER
            || id == STASH_FIELD
            || id == RENAME_FIELD
            || id == ENTRY_FIELD
            || id == EDITOR_FIELD
            || id == crate::navigate::RENAME_SYMBOL
            || id == crate::color_picker::COLOR_HEX
            || id == crate::line_edits::GO_TO_LINE
            || id == crate::browser::BROWSER_ADDRESS
            || id == crate::browser::BROWSER_FIND_FIELD
            || id == crate::find::FIND_FIELD
            || id == crate::project_search::PROJECT_SEARCH_FIELD
            || id == crate::listing::LISTING_FILTER
            || id == crate::debug::DEBUG_CONSOLE_FIELD
            || id == crate::debug::DEBUG_WATCH_FIELD
            || id == crate::find::REPLACE_FIELD
            || id == HISTORY_SEARCH
            || id == crate::extensions::EXTENSIONS_SEARCH
            || id == OTHER
            || id == ADDER_FIELD
            || id == PICKER_FIELD
            || crate::settings_servers::is_form_field(id)
            || crate::settings_pages::is_setting_field(id)
            || crate::settings_machines::is_machine_field(id)
            || id == crate::settings::SETTINGS_FIND
            || id == crate::settings::SETTINGS_KEYMAP_FIND
    }

    /// What pressing the field `id` hands in when Enter is pressed in it.
    pub fn submit_of(id: FocusId) -> FocusId {
        match id {
            COMMIT_MESSAGE => COMMIT,
            COMMENT_FIELD => COMMENT_ADD,
            BRANCH_FILTER => BRANCH_SUBMIT,
            STASH_FIELD | RENAME_FIELD => PROMPT_CONFIRM,
            ENTRY_FIELD => crate::tree_actions::TREE_NAME_SUBMIT,
            EDITOR_FIELD => crate::open_files::EDITOR_SAVE,
            crate::navigate::RENAME_SYMBOL => crate::navigate::RENAME_SYMBOL_SUBMIT,
            crate::color_picker::COLOR_HEX => crate::color_picker::COLOR_HEX_SUBMIT,
            crate::line_edits::GO_TO_LINE => crate::line_edits::GO_TO_LINE_SUBMIT,
            crate::browser::BROWSER_ADDRESS => crate::browser::BROWSER_GO,
            crate::browser::BROWSER_FIND_FIELD => crate::browser::BROWSER_FIND_NEXT,
            crate::find::FIND_FIELD => crate::find::FIND_NEXT,
            crate::project_search::PROJECT_SEARCH_FIELD => crate::project_search::PROJECT_SEARCH_GO,
            crate::listing::LISTING_FILTER => crate::listing::LISTING_TAKE,
            crate::debug::DEBUG_CONSOLE_FIELD => crate::debug::DEBUG_CONSOLE_SUBMIT,
            crate::debug::DEBUG_WATCH_FIELD => crate::debug::DEBUG_WATCH_ADD,
            crate::find::REPLACE_FIELD => crate::find::REPLACE_ONE,
            HISTORY_SEARCH => HISTORY,
            ADDER_FIELD => ADDER_SUBMIT,
            PICKER_FIELD => PICKER_SUBMIT,
            crate::settings::SETTINGS_SEARCH => crate::settings::SETTINGS_SEARCH_GO,
            id if crate::settings_servers::is_form_field(id) => crate::settings::SETTINGS_SAVE,
            id if crate::settings_machines::is_machine_field(id) => {
                crate::settings_machines::MACHINE_SAVE
            }
            crate::settings_catalog::LIST_FIELD => crate::settings_catalog::LIST_ADD,
            id if crate::settings_pages::is_setting_field(id)
                || id == crate::settings::SETTINGS_FIND
                || id == crate::settings::SETTINGS_KEYMAP_FIND =>
            {
                id
            }
            OTHER => NEXT,
            _ => SEND,
        }
    }

    /// What pressing Control or Command with Enter in the field `id` hands in, when it hands in something of its own.
    pub fn hurried_submit_of(id: FocusId) -> Option<FocusId> {
        match id {
            PROMPT => Some(SEND_NOW),
            COMMIT_MESSAGE => Some(COMMIT),
            _ => None,
        }
    }

    /// The state with a menu open over the field `field` at `at`.
    pub fn with_menu(mut self, field: FocusId, at: (f32, f32)) -> Self {
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
        self.pane_menu = None;
        self
    }

    /// The state after the list of values in the open menu is scrolled by
    /// `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_menu(mut self, delta: f32, room: f32) -> Self {
        self.menu_scroll = (self.menu_scroll + delta).clamp(0.0, room);
        self
    }

    /// The state after the conversation is scrolled by `delta` logical
    /// pixels, downwards when positive, with `room` pixels to scroll back.
    pub fn scrolled_chat(mut self, delta: f32, room: f32) -> Self {
        self.chat_back = (self.chat_back - delta).clamp(0.0, room);
        if let Some(index) = self.open_index() {
            self.sessions[index].chat_back = self.chat_back;
        }
        self
    }

    /// Takes what the agents have yet to be told off the state.
    pub fn take_outbox(&mut self) -> Vec<Outgoing> {
        self.ensure_terminal();
        self.ask_problems_again();
        std::mem::take(&mut self.outbox)
    }

    /// The state after the agent of the session at `session` did `event`.
    pub fn heard(mut self, session: usize, event: Event) -> Self {
        if matches!(&event, Event::Ask(ask) if ask.plan.is_some()) {
            self.plan_open = true;
            self.plan_scroll = 0.0;
        }
        let finishing = matches!(&event, Event::Finished(_) | Event::Failed(_));
        if let Some(held) = self.sessions.get_mut(session) {
            held.hear(event);
        }
        if finishing {
            self.ask_reply_figures(session);
        }
        self
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
                self = self.after(FocusId(SESSION_BASE + position as u64));
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

    /// The state after the plan panel is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_plan(mut self, delta: f32, room: f32) -> Self {
        self.plan_scroll = (self.plan_scroll + delta).clamp(0.0, room);
        self
    }

    /// Scrolls the subagent rows by `delta` logical pixels within `room`.
    pub fn scrolled_subagents(mut self, delta: f32, room: f32) -> Self {
        self.subagents_scroll = (self.subagents_scroll + delta).clamp(0.0, room);
        self
    }

    /// Answers the question with its first refusal, if it offers one.
    fn reject(&mut self) {
        let refusal = self
            .open()
            .and_then(|session| session.ask.as_ref())
            .and_then(|ask| {
                ask.choices
                    .iter()
                    .position(|choice| choice.verdict == pm_acp::Verdict::Reject)
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

    /// The state after one of the rows or chips numbered from a base was pressed.
    fn after_numbered(&mut self, id: u64) {
        if crate::diff::gutter_of(FocusId(id)).is_some() {
            return;
        }
        if self.press_inquiry(FocusId(id)) {
            return;
        }
        if self.press_hooks(FocusId(id)) {
            return;
        }
        if self.press_terminal(FocusId(id)) {
            return;
        }
        if self.press_debug(FocusId(id)) {
            return;
        }
        if self.press_choice(id) {
            return;
        }
        let position = |base: u64, count: usize| {
            id.checked_sub(base)
                .map(|offset| offset as usize)
                .filter(|offset| *offset < count)
        };
        let rows = self.adder.as_ref().map_or(0, |open| open.count());
        if let Some(index) = position(ADDER_ROW_BASE, rows) {
            self.adder_chosen(index);
        } else if let Some(index) = position(PROJECT_BASE, self.projects.len()) {
            self.page = Page::Project(index);
            self.overview_scroll = 0.0;
        } else if let Some(index) = position(NEW_IN_BASE, self.projects.len()) {
            self.start_in(index, None);
        } else if let Some((project, submodule)) = (NEW_IN_SUBMODULE_BASE..SESSION_BASE)
            .contains(&id)
            .then(|| crate::sidebar::submodule_rows(self))
            .and_then(|rows| rows.into_iter().nth((id - NEW_IN_SUBMODULE_BASE) as usize))
        {
            self.start_in(project, Some(submodule));
        } else if let Some(index) = position(SESSION_BASE, self.sessions.len()) {
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
            self.chat_back = self.sessions[index].chat_back;
        } else if let Some(index) = position(AGENT_BASE, self.agent_slots()) {
            self.agent = index;
        } else if let Some(index) = position(CHOICE_BASE, self.choices()) {
            self.choose(index);
        } else if let Some(index) = position(HURRY_BASE, self.queued_open()) {
            self.act_on_queued(index, false);
        } else if let Some(index) = position(REMOVE_QUEUED_BASE, self.queued_open()) {
            self.act_on_queued(index, true);
        } else if let Some(index) = id
            .checked_sub(COPY_BASE)
            .map(|offset| offset as usize)
            .filter(|offset| *offset < self.messages_open())
        {
            self.copy(index);
        } else if let Some(index) = position(REWIND_BASE, self.messages_open()) {
            self.rewind(index);
        } else if let Some(index) = position(FORK_BASE, self.messages_open()) {
            self.fork(index);
        } else if let Some(index) = position(RESTORE_BASE, self.messages_open()) {
            self.restoring = Some(index);
        } else if let Some(index) = position(TURN_BASE, self.messages_open()) {
            self.toggle_turn(index);
        } else if let Some(index) = position(TREE_BASE, self.files.len() + self.tree_folders.len())
        {
            self.fold_tree_folder(index);
        } else if id >= LINK_BASE {
            self.open_link(id - LINK_BASE);
        } else if id >= DETAILS_BASE {
            let offset = (id - DETAILS_BASE) as usize;
            self.toggle_details(offset / 1000, offset % 1000);
        } else if id >= STEP_BASE {
            let offset = (id - STEP_BASE) as usize;
            self.toggle_step(offset / 1000, offset % 1000);
        } else if let Some(index) = position(
            FILE_BASE,
            self.review_session()
                .map_or(0, |session| session.changes.len()),
        )
        .filter(|index| {
            self.review_session()
                .is_some_and(|held| held.changes[*index].conflicted)
        }) {
            if self.open().is_some() {
                self.open_change(index);
            } else {
                self.request_resolver(index);
            }
        } else if let Some(index) = position(FILE_BASE, self.change_count()) {
            self.open_change(index);
        } else if let Some(index) = position(STAGE_BASE, self.change_count()) {
            self.stage_changes(&[index]);
        } else if let Some(index) = position(FOLDER_BASE, self.change_count()) {
            self.fold_change_folder(index);
        } else if let Some(index) = position(FOLDER_STAGE_BASE, self.change_count()) {
            self.stage_change_folder(index);
        } else if let Some(index) = position(OPEN_ARROW_BASE, self.change_count()) {
            self.open_changed_file(index);
        } else if let Some(index) = position(DIFF_FOLD_BASE, self.change_count()) {
            self.fold_diff(index);
        } else if let Some(index) = position(COMMENT_FILE_BASE, self.change_count()) {
            self.start_comment(index, None, false);
        } else if let Some(index) = position(COMMENT_REMOVE_BASE, self.comment_count()) {
            self.remove_comment(index);
        } else if let Some(index) = position(VIEWED_BASE, self.change_count()) {
            self.change_at(index, |change| change.viewed = !change.viewed);
        }
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

    /// Opens in the browser the web address of the link that sits `offset` past [`LINK_BASE`] in the open session's replies.
    fn open_link(&mut self, offset: u64) {
        let (message, rest) = (offset / LINKS_PER_REPLY, offset % LINKS_PER_REPLY);
        let (piece, link) = (rest / LINKS_PER_PIECE, rest % LINKS_PER_PIECE);
        let address = self
            .open()
            .and_then(|session| session.messages.get(message as usize))
            .and_then(|message| {
                let (_, piece) = *message.chunks().get(piece as usize)?;
                pm_markdown::links(piece).into_iter().nth(link as usize)
            })
            .filter(|address| pm_acp::is_web_address(address));
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

    /// Opens or closes the detail of step `step` of the reply at `message` of the open session.
    fn toggle_step(&mut self, message: usize, step: usize) {
        let Page::Session(open) = self.page else {
            return;
        };
        let key = (open, message, step);
        match self.opened.iter().position(|held| *held == key) {
            Some(at) => {
                self.opened.remove(at);
            }
            None => self.opened.push(key),
        }
    }

    /// Opens or closes fold-out `place` of the reply at `message` of the open session.
    fn toggle_details(&mut self, message: usize, place: usize) {
        let Page::Session(open) = self.page else {
            return;
        };
        let key = (open, message, place);
        match self.unfolded.iter().position(|held| *held == key) {
            Some(at) => {
                self.unfolded.remove(at);
            }
            None => self.unfolded.push(key),
        }
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

    /// Removes the queued prompt at `index` when `remove` is set, otherwise hands it to the agent next.
    fn act_on_queued(&mut self, index: usize, remove: bool) {
        let Page::Session(session) = self.page else {
            return;
        };
        if let Some(queued) = self.sessions[session].queued.get(index) {
            let queued = queued.id;
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
    /// its place.
    fn send(&mut self, now: bool) {
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
        let starting = matches!(self.page, Page::New(_));
        let session = match self.page {
            Page::Session(index) => {
                let Some(session) = self.sessions.get_mut(index) else {
                    return;
                };
                if session.signed_out.is_some() {
                    return;
                }
                if session.status == Status::Working {
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
            Page::New(project) => {
                let title: String = text.chars().take(40).collect();
                let mut session = Session::started(&title, project, &text);
                session.agent = self.agent;
                session.login = self.login_for_new_work(self.agent).map(str::to_owned);
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
                session.skills = self.skills.clone();
                session.messages[0].attachments = sent_files;
                self.sessions.push(session);
                self.pane_stash
                    .insert(self.sessions.len() - 1, self.panes.clone());
                self.page = Page::Session(self.sessions.len() - 1);
                self.tab = Tab::Chat;
                self.sessions.len() - 1
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
        self.chat_back = 0.0;
        if let Some(index) = self.open_index() {
            self.sessions[index].chat_back = 0.0;
        }
        self.slash = 0;
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
        self.comment_text = Field::default();
        self.comment_target = Some(CommentTarget {
            change,
            range,
            anchor: range.map_or(0, |range| range.start),
            dragging,
        });
    }

    /// The state after the pointer went down on the line number `id` of the diff, which starts a selection of lines.
    pub fn comment_drag_begun(mut self, id: FocusId) -> Self {
        if let Some((change, new, line)) = crate::diff::gutter_of(id) {
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
    pub fn comment_drag_extended(mut self, id: FocusId) -> Self {
        let Some((change, new, line)) = crate::diff::gutter_of(id) else {
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
        self.comment_text = Field::default();
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
        if session.comments.is_empty() {
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
        self.tree_scroll = 0.0;
    }

    /// Opens the diff scrolled to the change at `index`, unfolded.
    pub(crate) fn open_change(&mut self, index: usize) {
        self.file = Some(index);
        self.select_change(index);
        self.request_resolver(index);
        self.tab = Tab::Diff;
        if let Some(path) = self
            .review_session()
            .and_then(|session| session.changes.get(index))
            .map(|change| change.path.clone())
        {
            self.closed_diffs.remove(&path);
        }
        self.diff_scroll = self.review_session().map_or(0.0, |session| {
            crate::diff::offset_of(self, session, index, DIFF_WIDTH)
        });
    }

    /// Folds the folder at `position` of the open session's changes, or unfolds it.
    fn fold_change_folder(&mut self, position: usize) {
        let key = self.review_session().and_then(|session| {
            crate::change_list::folder_table(&session.changes, self.git.repository.as_deref())
                .get(position)
                .map(|folder| crate::change_list::fold_key(folder.group, &folder.path))
        });
        if let Some(key) = key {
            toggle(&mut self.closed_folders, key);
        }
    }

    /// Stages everything under the folder at `position` of the open session's changes, or unstages it when all of it is staged.
    fn stage_change_folder(&mut self, position: usize) {
        let files = self.review_session().and_then(|session| {
            crate::change_list::folder_table(&session.changes, self.git.repository.as_deref())
                .into_iter()
                .nth(position)
                .map(|folder| folder.files)
        });
        if let Some(files) = files {
            self.stage_changes(&files);
        }
    }

    /// Folds the file at `index` of the diff, or unfolds it.
    fn fold_diff(&mut self, index: usize) {
        if let Some(path) = self
            .review_session()
            .and_then(|session| session.changes.get(index))
            .map(|change| change.path.clone())
        {
            toggle(&mut self.closed_diffs, path);
        }
    }

    /// Folds the folder at `position` of the project's files, or unfolds it.
    fn fold_tree_folder(&mut self, position: usize) {
        let paths = crate::file_tree::folder_paths(&self.files, &self.tree_folders);
        if let Some(path) = paths.into_iter().nth(position) {
            self.select_tree(TreeTarget {
                path: path.clone(),
                folder: true,
            });
            toggle(&mut self.open_tree, path);
        }
    }

    /// The state after the diff is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_diff(mut self, delta: f32, room: f32) -> Self {
        self.diff_scroll = (self.diff_scroll + delta).clamp(0.0, room);
        self
    }

    /// The state after the review panel's list is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_tree(mut self, delta: f32, room: f32) -> Self {
        self.tree_scroll = (self.tree_scroll + delta).clamp(0.0, room);
        self
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

/// Regression coverage for source control in project folders and session worktrees.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_controls::Pick;
    use crate::panes::{Item, Side};

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
                .after(TAB_CHAT);
            for row in [0, 1] {
                let path = state.sessions[index].changes[row].path.clone();
                state.closed_diffs.insert(path.clone());
                state = state.after(FocusId(FILE_BASE + row as u64));
                assert_eq!(state.file, Some(row));
                assert_eq!(state.current_item(), Item::Diff);
                let panes = state.panes();
                let group = panes.layout.groups()[panes.focused];
                assert_eq!(group.items[group.active], Item::Diff);
                assert!(!state.closed_diffs.contains(&path));
                state = state.after(FocusId(DIFF_FOLD_BASE + row as u64));
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
            state.message = Field::new("Review folder changes");
            state = state.after(COMMIT);
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
        state = state.after(FocusId(FOLDER_BASE));
        assert!(state.closed_folders.contains(&key));
        state = state.with_folder_change_menu(0, (0.0, 0.0));
        assert_eq!(state.marked_changes().len(), 4);
        state = state.after(FocusId(FOLDER_BASE));
        assert!(!state.closed_folders.contains(&key));
    }
}
