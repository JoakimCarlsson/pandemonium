//! What the reader does to the controls of each area of the window, as the workspace's
//! reducer takes it: one event enum per area, each variant the press of one control with
//! the data it carries (a session's index, a row, a file and a line, ...). An [`Event`]
//! holds one of them; [`WorkspaceState::handle`](crate::state::WorkspaceState::handle) matches it per area. These enums are the
//! one list of the window's controls: a [`Target`] is one of them wrapped in its area.

use serde::{Deserialize, Serialize};

use crate::events::Event;
use crate::target::Target;

/// A press of a control of the sidebar: its pages, projects and sessions.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum SidebarEvent {
    /// Shows the page that starts a chat.
    NewChat,
    /// Adds a project.
    AddProject,
    /// Lists every session.
    History,
    /// The field on the history page that narrows the sessions it lists.
    HistorySearch,
    /// Opens the settings.
    Settings,
    /// The button in the title bar that shows and hides the sidebar of projects and sessions.
    SidebarToggle,
    /// An agent to start new work with, by its place.
    Agent(usize),
    /// A session of the sidebar, by its place among the sessions.
    Session(usize),
    /// The button starting a session in a project, by the project's place.
    NewIn(usize),
    /// The header of a project in the sidebar, by the project's place.
    Project(usize),
    /// A row starting a session in a submodule, by its place among the submodule rows.
    NewInSubmodule(usize),
    /// The button opening the menu of a project, by the project's place.
    ProjectMore(usize),
}

/// A press of a control of the conversation: its messages, steps, questions and queued prompts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ChatEvent {
    /// The chip above a chat's prompt box that opens the choice of where to continue it.
    Continue,
    /// The button of the choice that asks the server to continue the chat in the project chosen.
    ContinueStart,
    /// Abandons the form.
    Close,
    /// Folds the form down to its title, or opens it again.
    Fold,
    /// Moves on to the next question, or hands the answers to the agent after the last.
    Next,
    /// The field the reader types an answer of their own in.
    Other,
    /// Leaves the question unanswered and moves on.
    Skip,
    /// Starts typing an answer of the reader's own.
    TypeOwn,
    /// Gives up the edit of a waiting prompt and puts back what was in the prompt box before it.
    QueuedCancel,
    /// Opens the questions waiting for an answer above the prompt box.
    ApprovalsChip,
    /// The text of the conversation's messages, which the pointer selects as
    /// one: each message is a selectable box of it, in the order of the messages.
    ChatText,
    /// Closes the panel showing the plan.
    ClosePlan,
    /// Closes the offer to put the files back, leaving them as they are.
    KeepFiles,
    /// Opens the plan the agent proposed in the panel beside the conversation.
    OpenPlan,
    /// Opens the agent's plan above the prompt box.
    PlanChip,
    /// Puts the files back as they were before the message whose offer is open.
    RestoreFiles,
    /// Turns the proposed plan down and gives the prompt the keyboard, to say what to change.
    Revise,
    /// Signs in again as the open session's account.
    SessionSignIn,
    /// Opens the commands the agent left running above the prompt box.
    ShellsChip,
    /// Opens the subagents the agent started above the prompt box.
    SubagentsChip,
    /// Shows the conversation of the open session.
    TabChat,
    /// Trusts the project for good, for every session of its repository.
    TrustAlways,
    /// Records that the project is not trusted.
    TrustDecline,
    /// Trusts the project for the conversation of this session only.
    TrustSession,
    /// The button opening an exhibit of a message live, by the message's place and the exhibit's number among its exhibits.
    ExhibitOpen { message: usize, number: usize },
    /// The button copying an exhibit of a message, by the message's place and the exhibit's number among its exhibits.
    ExhibitCopy { message: usize, number: usize },
    /// The button installing or turning on the browser an exhibit of a message is shown in, by the message's place and the exhibit's number.
    PreviewOffer { message: usize, number: usize },
    /// The button installing or turning on the browser a mockup of a question is shown in, by the mockup's place.
    MockupOffer(usize),
    /// The button showing what the turn of a message changed, by the message's place.
    TurnChanges(usize),
    /// The button putting the files back as they were before a message, by the message's place.
    Restore(usize),
    /// The button rewinding the conversation to before a message, by the message's place.
    Rewind(usize),
    /// The button copying a reply, by the message's place.
    Copy(usize),
    /// The button branching a new session from a reply, by the message's place.
    Fork(usize),
    /// The heading folding the steps of a turn away or showing them, by the message's place.
    Turn(usize),
    /// A step of a reply, by the message's place and the step's place in it.
    Step { message: usize, position: usize },
    /// The button stopping the helper agent a step of a reply started, by the message's place and the step's place in it.
    SubagentStop { message: usize, position: usize },
    /// The summary of a fold-out of a reply, by the message's place and the fold-out's place among the reply's.
    Details { message: usize, place: usize },
    /// A link of a reply, by the message's place, the piece of the reply and the link's place among the piece's links.
    Link {
        message: usize,
        piece: usize,
        link: usize,
    },
    /// A choice of the question the agent asked, by its place.
    Choice(usize),
    /// The button sending a queued message now, by its place in the queue.
    Hurry(usize),
    /// The button taking a message out of the queue, by its place in the queue.
    RemoveQueued(usize),
    /// The button editing a queued message, by its place in the queue.
    EditQueued(usize),
    /// An answer of a question the agent drafted, by its place.
    InquiryPick(usize),
    /// The button opening a mockup of a question the agent drafted, by its place.
    MockupOpen(usize),
    /// A project the chat is continued in, by its place among the projects.
    ContinueProject(usize),
    /// The area of a message of the conversation, by its place, by which the app finds the message under a point.
    MessageArea(usize),
}

/// A press of a control of the prompt box and the chips and menus around it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ComposerEvent {
    /// The chip naming the account the agent is signed in as.
    Account,
    /// Switches the agent persona on or off.
    AgentToggle,
    /// The button that attaches files to the prompt.
    Attach,
    /// The chip showing the goal the agent works toward, which opens what can be done with it.
    GoalChip,
    /// Opens the menu of where new work is done.
    Location,
    /// The button that starts mentioning a file in the prompt.
    Mention,
    /// Puts a slash in the prompt, which lists the commands the agent offers.
    /// The body of an open menu, which is not a button but keeps a click on it from counting as outside.
    MenuBody,
    /// The box the values of the open menu are narrowed with, when it has many.
    MenuFilter,
    /// Chooses the first value the box of the open menu leaves, as Enter in the box does.
    MenuFilterPick,
    /// The row of the model menu that switches the conversation in and out of orchestrator mode, in which the agent leads and hands work to subagents by category.
    Orchestrate,
    /// The prompt field.
    Prompt,
    /// Hands the prompt to the agent.
    Send,
    /// Stops the turn the agent is on and hands it the prompt in its place.
    SendNow,
    /// Puts a slash in the prompt, which lists the commands the agent offers.
    SlashButton,
    /// Stops the turn the agent is on.
    Stop,
    /// The button taking a file attached to the prompt off it, by its place among the attachments.
    Attached(usize),
    /// The chip of an element of a web page attached to the prompt, by its place.
    Element(usize),
    /// The button taking an element of a web page off the prompt, by its place.
    ElementRemove(usize),
    /// The chip of a capture of a web page attached to the prompt, by its place.
    Capture(usize),
    /// The button taking a capture of a web page off the prompt, by its place.
    CaptureRemove(usize),
    /// A row of the menu of accounts: the agent's own login first, then each added one, then adding another.
    AccountRow(usize),
    /// A row of the goal's menu, in the order of the goal's commands.
    GoalRow(usize),
    /// The chip of a setting of the agent under the prompt, by its place among the chips shown.
    SettingChip(usize),
    /// A value of the open menu of a setting, by its place among the setting's values.
    MenuPick(usize),
    /// A value of a setting's menu chosen without its box, by its place among the setting's values.
    SettingPick(usize),
    /// A stop of the stepper of the agent's effort, by its place.
    Effort(usize),
    /// A place new work runs in: a worktree of its own first, then the main checkout.
    Place(usize),
    /// The switch running new work in the project's dev container.
    DevContainer,
    /// A branch new work's worktree can be made from, by its place among the branches.
    Branch(usize),
    /// A row of the list of commands or paths offered under the prompt, by its place.
    Slash(usize),
}

/// A press of a control of the review of the changes and the comments on them.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DiffEvent {
    /// Keeps the comment that was written.
    CommentAdd,
    /// Drops the comment that was being written.
    CommentCancel,
    /// The box the comment on a file or on lines is written in.
    CommentField,
    /// Opens the diff from the summary of changes.
    OpenDiff,
    /// Shows or hides the review panel.
    Review,
    /// Shows the changed files in the review panel.
    ReviewChanges,
    /// Shows every file of the project in the review panel.
    ReviewFiles,
    /// Hands the comments to the agent as the next message.
    SendComments,
    /// Shows the diff of the open session.
    TabDiff,
    /// The button starting a comment on a changed file, by the file's place among the changes.
    CommentFile(usize),
    /// The button taking back a comment, by its place among the comments.
    CommentRemove(usize),
    /// A folder of the changed files, by its place among the folders.
    ChangeFolder(usize),
    /// The button staging a folder of the changed files, by its place among the folders.
    ChangeFolderStage(usize),
    /// The heading folding a file's diff away or showing it, by the file's place among the changes.
    DiffFold(usize),
    /// A changed file, by its place among the changes.
    ChangedFile(usize),
    /// The button staging a changed file, by its place among the changes.
    Stage(usize),
    /// The box marking a changed file viewed, by its place among the changes.
    Viewed(usize),
    /// The arrow opening a changed file, by its place among the changes.
    OpenArrow(usize),
    /// The number of a line of a diff, by the change's place, the side (new when true) and the line.
    Gutter {
        change: usize,
        new: bool,
        line: usize,
    },
}

/// A press of a control of the project's files.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TreeEvent {
    /// The box the name of a new or renamed file or folder is typed in.
    EntryField,
    /// Folds every folder of the project's files.
    TreeCollapse,
    /// Makes the file or folder, or renames the entry, under the name typed in the tree.
    TreeNameSubmit,
    /// Asks for a name for a new file at the root of the project.
    TreeNewFile,
    /// Asks for a name for a new folder at the root of the project.
    TreeNewFolder,
    /// Reads the project's files and changes again.
    TreeRefresh,
    /// The empty space under the rows, which opens the menu of the tree itself.
    TreeSpace,
    /// A folder of the project's files, by its place among the files and folders.
    Tree(usize),
    /// A file of the project's files, by its place among the files.
    TreeFile(usize),
}

/// A press of a control of the file shown and what is laid over it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum EditorEvent {
    /// The list of completions itself, which keeps a press on it from counting as outside.
    CompletionBody,
    /// The card about a place, which keeps the pointer resting on it from counting as leaving the place.
    HoverCard,
    /// The box the colour is chosen in, which keeps a press on it from counting as outside.
    ColorBox,
    /// The field the colour is typed in, as hex.
    ColorHex,
    /// Hands in the colour typed: the first way to write it is taken.
    ColorHexSubmit,
    /// The bar itself, which keeps a press on it from counting as outside.
    FindBar,
    /// Switches matching case on and off.
    FindCase,
    /// Closes the bar.
    FindClose,
    /// The field the text looked for is typed in.
    FindField,
    /// Limits finding to the lines that were selected, and lifts the limit again.
    FindInSelection,
    /// Selects the next match.
    FindNext,
    /// Selects the match before.
    FindPrevious,
    /// Switches reading the text as a regular expression on and off.
    FindRegex,
    /// Shows and hides the row for replacing.
    FindToggleReplace,
    /// Switches matching whole words on and off.
    FindWord,
    /// Replaces every match.
    ReplaceAll,
    /// The field the replacement is typed in.
    ReplaceField,
    /// Replaces the selected match.
    ReplaceOne,
    /// Turns the offer down for the language it is for, for good.
    OfferDecline,
    /// Installs the language the offer is for.
    OfferInstall,
    /// The marker beside the line of the caret, which opens the list of changes when pressed.
    Lightbulb,
    /// The field the number of the line to go to is typed in.
    GoToLine,
    /// The box the number of the line to go to is typed in, which keeps a press on it from counting as outside.
    GoToLineBox,
    /// Hands in the number of the line to go to.
    GoToLineSubmit,
    /// The list itself, which keeps a press on it from counting as outside.
    ListingBody,
    /// The field the names of an outline are narrowed by.
    ListingFilter,
    /// Takes the row picked in the list.
    ListingTake,
    /// The name of the minimap, which presses and drags on it are about.
    Minimap,
    /// The box the new name is typed in, which keeps a press on it from counting as outside.
    RenameBox,
    /// The field the new name of the thing at the caret is typed in.
    RenameSymbol,
    /// Hands in the new name typed.
    RenameSymbolSubmit,
    /// The text of the file being edited.
    EditorField,
    /// Writes the file being edited to the folder.
    EditorSave,
    /// Shows the file shown rendered.
    PreviewShow,
    /// Shows the source of the file shown.
    SourceShow,
    /// Pressed with Control or Command and Enter in the quick input: starts a new session whose prompt is what is typed.
    ListingPrompt,
    /// A row of the list over the file, by its place among the rows shown.
    ListingRow(usize),
    /// A link of the card shown over the code, by the stretch of prose and the link's place among its links.
    HoverLink { prose: usize, link: usize },
    /// A link of the preview of a markdown file, by its place among the file's links.
    PreviewLink(usize),
    /// The tab of an open file, by its place among the open files.
    FileTab(usize),
    /// The button closing an open file, by its place among the open files.
    FileClose(usize),
    /// A sticky line over the code, by its place from the top.
    Sticky(usize),
    /// The mark pressing a lens of the code, by its place in the list the server gave.
    Lens(usize),
    /// The link at the end of a line running a place with one task of its language, by its place among the links of the file shown.
    RunLens(usize),
    /// The mark in the gutter running the place that starts on a line, by the open file's place and the line.
    Run { file: usize, line: usize },
    /// The mark in the gutter setting or clearing a breakpoint, by the open file's place and the line.
    Breakpoint { file: usize, line: usize },
    /// The mark unfolding a folded stretch of the code, by the line it starts on.
    CodeFold(usize),
    /// A way of writing the colour of the colour picker, by its place.
    ColorFormat(usize),
    /// The square of a colour of the file, by its place among the file's colours.
    ColorSwatch(usize),
    /// A row of the completions offered at the caret, by its place among those shown.
    CompletionRow(usize),
}

/// A press of a control of the project search, the excerpts and the conflict resolver.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum SearchEvent {
    /// Writes the result and stages the file.
    ConflictConclude,
    /// Takes everything of the file from our side.
    ConflictOursAll,
    /// Takes everything of the file from their side.
    ConflictTheirsAll,
    /// Closes the excerpts.
    ExcerptsClose,
    /// Folds every file of the excerpts away, or opens them all when they are all folded.
    ExcerptsFoldAll,
    /// Puts the file shown back as it was in the state it is set against.
    ExcerptsRestore,
    /// Takes everything the turn shown changed back out of the files.
    ExcerptsUndoAll,
    /// The tab of the excerpts in a pane's strip.
    TabExcerpts,
    /// Switches matching case on and off.
    ProjectSearchCase,
    /// Empties the box and the hits.
    ProjectSearchClear,
    /// Opens the hits as excerpts in the editor.
    ProjectSearchExcerpts,
    /// The field the text looked for in the project is typed in.
    ProjectSearchField,
    /// Searches again for what is typed.
    ProjectSearchGo,
    /// Switches reading the text as a regular expression on and off.
    ProjectSearchRegex,
    /// Shows the search in the review panel.
    ReviewSearch,
    /// A file of the excerpts, by its place among them.
    ExcerptsFile(usize),
    /// The box of lines of a file of the excerpts that is not the one being edited, by the open file's place.
    ExcerptOwner(usize),
    /// The button undoing a turn's edits of the excerpts, by the turn's place.
    ExcerptUndo(usize),
    /// A choice of a conflict of the resolver, by the conflict's place and the choice.
    ConflictChoice {
        place: usize,
        choice: crate::conflict::Choice,
    },
    /// A file of the project search's results, by its place among the files.
    SearchGroup(usize),
    /// A hit of the project search, by its place among the hits.
    SearchHit(usize),
}

/// A press of a control of source control: branches, commits, stashes, pull requests and the history.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum GitEvent {
    /// The box a branch is named in, or the list of branches is searched in.
    BranchTaskField,
    /// The check box that carries a rename or a deletion over to the branch on the remote.
    BranchTaskRemote,
    /// The panel listing the branches.
    BranchBody,
    /// The button that opens the list of branches.
    BranchButton,
    /// The row that makes a branch of the name typed.
    BranchCreate,
    /// The box branches are searched in and a new one is named in.
    BranchFilter,
    /// What Enter does in the box branches are searched in.
    BranchSubmit,
    /// Opens the menu over the panel's actions.
    ChangesMore,
    /// Reads the changes and the graph again.
    ChangesRefresh,
    /// Folds or unfolds the changes section.
    ChangesToggle,
    /// Opens the menu of what else the button under the commit message can do.
    CommitChevron,
    /// Fetches from every remote.
    GraphFetch,
    /// Pulls the checked out branch.
    GraphPull,
    /// Pushes the checked out branch.
    GraphPush,
    /// Switches the graph between the checked out branch and every reference.
    GraphRefs,
    /// The handle between the changes and the graph, which is dragged to change how tall the graph is.
    GraphSash,
    /// Scrolls the graph back to the commit the checked out branch is at.
    GraphTarget,
    /// Folds or unfolds the graph.
    GraphToggle,
    /// Stages or unstages every tracked file that is not staged.
    GroupTracked,
    /// Stages or unstages every file git does not track yet.
    GroupUntracked,
    /// Closes whatever of these is open.
    ReviewDismiss,
    /// Dismisses the banner saying what git refused to do.
    TroubleDismiss,
    /// Does not.
    PromptCancel,
    /// Does what the question asks.
    PromptConfirm,
    /// The panel of a question.
    PromptInside,
    /// The dimmed window behind a question.
    PromptOutside,
    /// The box a session is renamed in.
    RenameField,
    /// The box a stash is named in.
    StashField,
    /// Opens the page of the pull request's checks.
    PullChecks,
    /// Pushes the branch and opens a pull request of it.
    PullCreate,
    /// Pushes the branch and opens a draft pull request of it.
    PullCreateDraft,
    /// Deletes the branch from its remote.
    PullDeleteBranch,
    /// Squashes and merges the open pull request.
    PullMerge,
    /// Opens the pull request on its code host.
    PullOpen,
    /// Pushes the branch.
    PullPush,
    /// Marks the draft pull request ready for review.
    PullReady,
    /// Reopens the closed pull request.
    PullReopen,
    /// Pushes the branch and redrafts the body of its open pull request.
    PullUpdate,
    /// Commits the staged changes.
    Commit,
    /// The commit message field.
    CommitMessage,
    /// Stages every change, or none when all are staged.
    StageAll,
    /// A branch of the menu of branches, by its place among the branches shown.
    BranchPick(usize),
    /// A repository of the menu of branches, by its place.
    Repository(usize),
    /// The branch of a repository of the menu of branches, by the repository's place.
    RepositoryBranch(usize),
    /// A stash of the list of stashes, by its place.
    StashRow(usize),
    /// A commit of the history graph, by its place among the commits shown.
    HistoryRow(usize),
    /// A row of the list of tasks to make a branch from, by its place.
    BranchTaskRow(usize),
    /// A row of the open menu of the changes, by its place.
    GitMenuRow(usize),
}

/// A press of a control of the dock: its shells and problems.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TerminalEvent {
    /// The dock's tab listing what is wrong with the files of the project.
    BottomProblems,
    /// The count of errors and warnings above a file, which shows the dock's tab of problems.
    BottomProblemsCount,
    /// The dock's tab showing the shells.
    BottomTerminal,
    /// Makes the terminal dock taller, or gives it its height back.
    MaximizeTerminal,
    /// Starts another shell in the terminal dock.
    NewTerminal,
    /// Shows or hides the terminal.
    Terminal,
    /// The body of the terminal dock, which takes the keyboard while it has focus.
    TerminalBody,
    /// A row of the problems in the dock, by its place.
    ProblemRow(usize),
    /// The tab of a shell, by its place among the shells.
    TerminalTab(usize),
    /// The button closing a shell, by its place among the shells.
    TerminalClose(usize),
}

/// A press of a control of the debug tab.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DebugEvent {
    /// The field an expression is typed in to be evaluated in the console.
    DebugConsoleField,
    /// Evaluates the expression typed in the console field.
    DebugConsoleSubmit,
    /// Runs the paused program on.
    DebugContinue,
    /// Pauses the running program.
    DebugPause,
    /// Starts debugging, listing what the folder can be debugged as.
    DebugStart,
    /// Runs into the call on the line.
    DebugStepIn,
    /// Runs out of the frame.
    DebugStepOut,
    /// Runs to the next line.
    DebugStepOver,
    /// Ends the debugging session.
    DebugStop,
    /// Shows the debug tab of the dock.
    DebugTab,
    /// Keeps the expression typed in the watch field.
    DebugWatchAdd,
    /// The field an expression is typed in to be watched.
    DebugWatchField,
    /// A frame of the stack of the debug tab, by its place.
    DebugFrame(usize),
    /// A variable of the debug tab, by its row.
    DebugVariable(usize),
    /// The button taking a watch away, by its place among the watches.
    DebugWatchRemove(usize),
}

/// A press of a control of the browser pane.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum BrowserEvent {
    /// The line the address of the page is typed in.
    BrowserAddress,
    /// Goes back one page in the history.
    BrowserBack,
    /// The window of the page, which takes the pointer and the keyboard while it has focus.
    BrowserBody,
    /// Opens the menu of what can be added to the prompt from the page: pictures of it and its console.
    BrowserCapture,
    /// Leaves the cookies and site data as they are.
    BrowserClearCancel,
    /// Clears the cookies and site data.
    BrowserClearConfirm,
    /// The panel of the question about clearing cookies and site data.
    BrowserClearInside,
    /// The dimmed window behind the question about clearing cookies and site data.
    BrowserClearOutside,
    /// Closes the browser.
    BrowserClose,
    /// The bar for finding in the page.
    BrowserFindBar,
    /// Closes the bar for finding in the page.
    BrowserFindClose,
    /// The line the text to find in the page is typed in.
    BrowserFindField,
    /// Goes to the next match of the text found in the page.
    BrowserFindNext,
    /// Goes to the previous match of the text found in the page.
    BrowserFindPrevious,
    /// Goes forward one page in the history.
    BrowserForward,
    /// Loads the address that was typed.
    BrowserGo,
    /// Turns on or off picking an element of the page to send to the agent.
    BrowserPick,
    /// Loads the page again.
    BrowserReload,
    /// The handle between the page and its developer tools, dragged to change how wide the tools are.
    BrowserSash,
    /// Shows or hides the developer tools beside the page.
    BrowserTools,
    /// The window of the developer tools, which takes the pointer and the keyboard while it has focus.
    BrowserToolsBody,
    /// The zoom of the page, shown while it is not 100%; pressing it puts the zoom back.
    BrowserZoom,
    /// The tab of the browser in a pane's strip.
    TabBrowser,
    /// Takes the page back from a site whose certificate is not trusted.
    BrowserCertBack,
    /// Loads a site whose certificate is not trusted anyway; the first press asks again, the second does it.
    BrowserCertProceed,
    /// Takes away the note about a download.
    BrowserDismiss,
    /// The bar with the first question, which takes presses that land on it and are for neither button, so they never reach the page under it.
    BrowserQuestionBar,
    /// Locks the tab being shown against agents, or unlocks it.
    BrowserLock,
    /// Opens a new, empty tab.
    BrowserNewTab,
    /// Scrolls the strip of tabs on to the tabs after the last one shown.
    BrowserTabsNext,
    /// Scrolls the strip of tabs back to the tabs before the first one shown.
    BrowserTabsPrevious,
    /// The button allowing what a question of the web page asks, by the question's id.
    BrowserAllow(u64),
    /// The button refusing what a question of the web page asks, by the question's id.
    BrowserBlock(u64),
    /// The tab of a web page, by its place among the browser's tabs.
    BrowserTab(usize),
    /// The button closing a tab of the browser, by its place among the tabs.
    BrowserTabClose(usize),
    /// Stops the page loading; it stands where the button that loads again is while the page loads.
    BrowserStop,
    /// Loads again the address the page could not load, from the view that says why.
    BrowserRetry,
    /// Loads again a page whose content process went away or stopped answering, from the note over it.
    BrowserRevive,
    /// Takes away the note that the browser was started again.
    BrowserRestartDismiss,
    /// Accepts the dialog the page opened: OK.
    BrowserDialogOk,
    /// Dismisses the dialog the page opened: Cancel.
    BrowserDialogCancel,
    /// The line the answer to a page's `prompt` is typed in.
    BrowserDialogField,
    /// The sheet of the dialog the page opened, which takes presses that land on it and are for none of its controls.
    BrowserDialogSheet,
    /// Shows or hides the list of downloads.
    BrowserDownloads,
    /// The list of downloads, which takes presses that land on it and are for none of its buttons.
    BrowserDownloadsPanel,
    /// Takes the downloads that ended off the list.
    BrowserDownloadsClear,
    /// Stops a running download, by its number.
    BrowserDownloadCancel(u64),
    /// Shows the file of a download in the system's file manager, by its number.
    BrowserDownloadShow(u64),
    /// Opens the file of a download, by its number.
    BrowserDownloadOpen(u64),
    /// Mutes or unmutes the page of a tab, by its place among the tabs.
    BrowserTabMute(usize),
    /// Shares the screen or window a page asked for, by its place in the list offered.
    BrowserScreenSource(usize),
    /// Refuses a page's request to share the screen.
    BrowserScreenCancel,
    /// A suggestion of the address bar, by its place in the list.
    BrowserSuggestion(usize),
}

/// A press of a control of the settings window and the extensions page.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum SettingsEvent {
    /// Opens the extensions page.
    Extensions,
    /// The box the extensions are searched with.
    ExtensionsSearch,
    /// Asks for a folder and installs the extension in it on the server on this machine.
    InstallExtension,
    /// Drops the extension the page asks about.
    PreviewCancel,
    /// Installs the extension the page asks about, with what the reader left switched on.
    PreviewInstall,
    /// Opens the form for adding a server to the list showing.
    SettingsAdd,
    /// Puts the form away without keeping it.
    SettingsCancel,
    /// Closes the settings window.
    SettingsClose,
    /// Opens the page that explains tool servers.
    SettingsDocs,
    /// The box the form's environment is written in.
    SettingsEnv,
    /// The box every setting is searched with.
    SettingsFind,
    /// Empties the box every setting is searched with.
    SettingsFindClear,
    /// The box the keymap's actions are searched with.
    SettingsKeymapFind,
    /// Copies the configuration of the server the open menu is over.
    SettingsMenuCopy,
    /// Opens the form for the server the open menu is over.
    SettingsMenuEdit,
    /// Takes the server the open menu is over away.
    SettingsMenuRemove,
    /// Switches the server the open menu is over on or off.
    SettingsMenuToggle,
    /// Opens the page of the publisher of the server the open menu is over.
    SettingsMenuWebsite,
    /// The box the form's name is written in.
    SettingsName,
    /// The box the open list of choices is narrowed with.
    SettingsPickFilter,
    /// Picks the first row of the open list of choices, which is what Enter in the box narrowing it does.
    SettingsPickFirst,
    /// Opens the page of the newest release.
    SettingsRelease,
    /// Keeps what the form holds.
    SettingsSave,
    /// The box the list showing is searched with.
    SettingsSearch,
    /// Searches the registry for the words in the search box, which is what Enter in the box does.
    SettingsSearchGo,
    /// The box the form's command or address is written in.
    SettingsTarget,
    /// Folds the list of what the registry offers away, or shows it again.
    SettingsToggleAvailable,
    /// Folds the list of what is installed away, or shows it again.
    SettingsToggleInstalled,
    /// Puts the box for an API key away.
    KeyCancel,
    /// The box an API key is typed into.
    KeyField,
    /// Keeps the API key typed.
    KeySave,
    /// Puts the box for an account's name away.
    NameCancel,
    /// The box a new name for an added account is typed into.
    NameField,
    /// Keeps the name typed for an added account.
    NameSave,
    /// Closes the form without writing anything.
    DraftCancel,
    /// Writes the new customization.
    DraftCreate,
    /// The box what the new customization is for is written in.
    DraftDescription,
    /// The box the new customization's name is written in.
    DraftName,
    /// Opens the form for a new customization of the page showing.
    DraftNew,
    /// Opens crab's `APPEND_SYSTEM.md`, making it first when it is not there.
    PromptAppend,
    /// Opens crab's `SYSTEM.md`, making it first when it is not there.
    PromptReplace,
    /// Opens the form for adding a server.
    MachineAdd,
    /// The box the form's address is written in.
    MachineAddress,
    /// Puts the form away without keeping it.
    MachineCancel,
    /// The box the form's name is written in.
    MachineName,
    /// Keeps what the form holds.
    MachineSave,
    /// The box the form's token is written in.
    MachineToken,
    /// Opens the form for declaring a model server.
    ModelAdd,
    /// Puts the form away without keeping it.
    ModelCancel,
    /// Asks the server which models it offers.
    ModelFind,
    /// The box the server's API key is written in; it never shows a key already saved.
    ModelKey,
    /// The box naming the environment variables the server's key is read from.
    ModelKeyEnv,
    /// The box the models the server offers are listed in, separated by commas.
    ModelList,
    /// The box the server's name is written in.
    ModelName,
    /// Keeps what the form holds.
    ModelSave,
    /// The box the server's base URL is written in.
    ModelUrl,
    /// Has the server spoken to through the Chat Completions API.
    ModelWireChat,
    /// Has the server spoken to through the Responses API.
    ModelWireResponses,
    /// Adds the category named in [`Self::CategoryNewName`].
    CategoryAdd,
    /// The box a category of the reader's own is named in, on the orchestration page.
    CategoryNewName,
    /// The box a preset is named in before it is saved.
    PresetName,
    /// Saves the slots as they stand as the preset named in [`Self::PresetName`].
    PresetSave,
    /// Opens the list of providers whose assignments the page shows.
    SlotsProvider,
    /// Saves changes to the project's file.
    SlotsScopeProject,
    /// Saves changes to the user's file.
    SlotsScopeUser,
    /// Adds the slot named in [`Self::SlotNewName`].
    SlotAdd,
    /// The box a slot of the reader's own is named in.
    SlotNewName,
    /// A switch of the card asking about an extension, granting what it asks, by the request's place.
    PreviewGrant(usize),
    /// The button installing or removing an extension of the extensions page, by its row.
    ExtensionRow(usize),
    /// A switch granting an installed extension a capability, by the extension's row and the request's place.
    ExtensionGrant { row: usize, request: usize },
    /// A control of the settings window repeated once per row of a list, by the list and its place in it.
    SettingsRow(crate::settings_rows::SettingsList, usize),
}

/// A press of a control of the menus opened over fields, tabs, messages, sessions and projects.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum MenuEvent {
    /// The row of a project's menu that starts a session in it.
    ProjectMenuNew,
    /// The row of a project's menu that takes it out of the window.
    ProjectMenuRemove,
    /// The row of a session's menu that closes it.
    SessionMenuClose,
    /// The row of a session's menu that copies the branch it is on.
    SessionMenuCopyBranch,
    /// The row of a session's menu that copies the folder it works in.
    SessionMenuCopyPath,
    /// The row of a session's menu that shows its folder in the file manager.
    SessionMenuOpenFolder,
    /// The row that toggles whether the session has been read.
    SessionMenuRead,
    /// The row of a chat's menu that removes it for good: its scratch folder is deleted with what is in it.
    SessionMenuRemove,
    /// The row of a session's menu that asks for a new title.
    SessionMenuRename,
    /// Copies the selection.
    MenuCopy,
    /// Copies the selection and deletes it.
    MenuCut,
    /// Inserts what the clipboard holds.
    MenuPaste,
    /// Makes again the change that was taken back.
    MenuRedo,
    /// Selects the whole line.
    MenuSelectAll,
    /// Takes back the last change to the field the menu was opened on.
    MenuUndo,
    /// Copies the selected text of the conversation.
    MessageMenuCopy,
    /// Copies the message the menu was opened on, as the markdown it was written in.
    MessageMenuCopyMarkdown,
    /// Copies the message the menu was opened on, as it reads.
    MessageMenuCopyReply,
    /// Selects the text of the whole conversation.
    MessageMenuSelectAll,
    /// A row of the menu on a file's tab: what the row does.
    TabMenu(crate::tab_menu::TabCommand),
    /// A row of the menu over a shell: what the row does.
    TerminalMenu(crate::terminal_menu::TerminalCommand),
    /// A row of the menu over a web page: what the row does.
    BrowserMenu(crate::browser_menu::BrowserCommand),
    /// A row of the menu over the file shown: what the row does.
    EditorMenu(crate::context_menu::EditorCommand),
}

/// A press of a control of the dialogs laid over the window.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DialogEvent {
    /// The panel itself, which keeps a press on it from counting as outside.
    AdderBody,
    /// The row offering the desktop's folder chooser.
    AdderBrowse,
    /// The row offering a repository to clone.
    AdderClone,
    /// Highlights the row below.
    AdderDown,
    /// Goes back to the first choices, or closes the panel when they are showing.
    AdderEscape,
    /// The field the reader types a search or an address into.
    AdderField,
    /// Closes the panel, which is what pressing outside it does.
    AdderScrim,
    /// Takes the row that is highlighted.
    AdderSubmit,
    /// Highlights the row above.
    AdderUp,
    /// Goes back to the folder looked inside before this one.
    PickerBack,
    /// Closes the picker without choosing.
    PickerClose,
    /// Takes the folder being looked inside.
    PickerConfirm,
    /// Highlights the folder below.
    PickerDown,
    /// The box the folders are filtered with.
    PickerField,
    /// Goes forward again to the folder that was left by going back.
    PickerForward,
    /// Goes up to the folder that holds the one being looked inside.
    PickerParent,
    /// Looks inside the folder that is highlighted.
    PickerSubmit,
    /// Highlights the folder above.
    PickerUp,
    /// Puts the code on the clipboard.
    SignInCodeCopy,
    /// Hides the dialog, leaving the sign-in running.
    SignInCodeHide,
    /// The panel of the dialog.
    SignInCodeInside,
    /// Opens the page the code is entered on.
    SignInCodeOpen,
    /// The dimmed window behind the dialog.
    SignInCodeOutside,
    /// The tab of the release notes in a pane's strip.
    TabUpdate,
    /// Closes the tab of the release notes.
    UpdateClose,
    /// Closes the tab of the release notes, or cancels the download first.
    UpdateLater,
    /// Downloads the release and restarts into it.
    UpdateNow,
    /// Opens the page of the release.
    UpdatePage,
    /// A row of the dialog adding a project, by its place.
    AdderRow(usize),
    /// A part of the path over the folder picker, by its place along the path.
    PickerCrumb(usize),
    /// A place of the folder picker's side, by its place.
    PickerPlace(usize),
    /// A folder of the folder picker, by its row.
    PickerRow(usize),
}

/// A press of a control of the window's frame: its title bar, panes and the handles between panels.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum WindowEvent {
    /// The control the tests press.
    Control,
    /// The button in the strip of tabs that opens the menu of panes.
    PaneMenu,
    /// The row of the menu of panes that splits the pane worked in with its tab below.
    SplitDown,
    /// The row of the menu of panes that splits the pane worked in with its tab on the left.
    SplitLeft,
    /// The row of the menu of panes that splits the pane worked in with its tab on the right.
    SplitRight,
    /// The row of the menu of panes that splits the pane worked in with its tab above.
    SplitUp,
    /// The handle on the left edge of the plan panel.
    PlanSash,
    /// The handle on the left edge of the review panel.
    ReviewSash,
    /// The handle on the right edge of the sidebar.
    SidebarSash,
    /// The handle on the top edge of the terminal dock.
    TerminalSash,
    /// The command center, which lists every command.
    CommandCenter,
    /// Goes back to the place shown before.
    GoBack,
    /// Goes forward to the place gone back from.
    GoForward,
    /// Opens the extensions page, where a newer language or language server is installed.
    LanguageUpdate,
    /// The bar itself, which moves the window when it is dragged and maximizes it when it is pressed twice.
    TitleBar,
    /// Opens the page of the newer release that is out.
    Update,
    /// Closes the window.
    WindowClose,
    /// Maximizes the window, or restores it.
    WindowMaximize,
    /// Minimizes the window.
    WindowMinimize,
    /// The box of a pane, by its place among the panes.
    Pane(usize),
    /// The handle between two panes, by its place among the handles.
    SplitSash(usize),
    /// A button of an empty pane: the page (0), the diff (1), a file (2), closing it (3) or the browser (4), by the pane's place.
    PanePut { pane: usize, put: usize },
    /// The main window's frame inside the band that holds its shadow, whose corner the coordinates the app keeps start from.
    Frame,
}

impl Event {
    /// The press of the control `target`, as the area it belongs to takes it: what a
    /// control named only by its target, such as the one holding the keyboard, sends.
    pub fn pressing(target: Target) -> Self {
        match target {
            Target::Sidebar(event) => Self::Sidebar(event),
            Target::Chat(event) => Self::Chat(event),
            Target::Composer(event) => Self::Composer(event),
            Target::Diff(event) => Self::Diff(event),
            Target::Tree(event) => Self::Tree(event),
            Target::Editor(event) => Self::Editor(event),
            Target::Search(event) => Self::Search(event),
            Target::Git(event) => Self::Git(event),
            Target::Terminal(event) => Self::Terminal(event),
            Target::Debug(event) => Self::Debug(event),
            Target::Browser(event) => Self::Browser(event),
            Target::Settings(event) => Self::Settings(event),
            Target::Menu(event) => Self::Menu(event),
            Target::Dialog(event) => Self::Dialog(event),
            Target::Window(event) => Self::Window(event),
        }
    }
}
