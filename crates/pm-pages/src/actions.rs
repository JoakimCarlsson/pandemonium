//! What the pages ask the shell to do when the person acts.

use std::rc::Rc;

use pm_components::ServerAsk;
use pm_components::{TerminalCommand, TerminalSelection};
use pm_shell::{
    BrowserAction, ComposerAction, ConversationCommand, DebugCommand, DockTab, FileAction,
    GitAction, InquiryReply, ProjectAction, QuickAction, RightTab, SearchAction, TabAction,
    TreeAction, TrustVerdict,
};

/// The callbacks the pages call; the shell decides what each does.
#[derive(Clone)]
pub struct Actions {
    /// Opens the session at the given index path.
    pub open_session: Rc<dyn Fn(Vec<usize>)>,
    /// Starts a new session in the project at the given index path.
    pub new_session: Rc<dyn Fn(Vec<usize>)>,
    /// Sends the text as a prompt to the open session.
    pub send_prompt: Rc<dyn Fn(String)>,
    /// Does what the reader did in the composer besides sending.
    pub composer: Rc<dyn Fn(ComposerAction)>,
    /// Chooses the agent at the given position on the welcome page.
    pub choose_agent: Rc<dyn Fn(usize)>,
    /// Starts the work the welcome page describes with the text as its first prompt.
    pub start: Rc<dyn Fn(String)>,
    /// Chooses a tab of the right panel.
    pub right_tab: Rc<dyn Fn(RightTab)>,
    /// Chooses a tab of the dock.
    pub dock_tab: Rc<dyn Fn(DockTab)>,
    /// Types the bytes into the dock's shell.
    pub terminal_input: Rc<dyn Fn(Vec<u8>)>,
    /// Tells the dock's shell how many columns and rows it has.
    pub terminal_resize: Rc<dyn Fn(u16, u16)>,
    /// Starts another shell in the dock's folder.
    pub terminal_new: Rc<dyn Fn()>,
    /// Shows the shell whose tab is kept by the given key.
    pub terminal_pick: Rc<dyn Fn(u64)>,
    /// Ends the shell whose tab is kept by the given key.
    pub terminal_close: Rc<dyn Fn(u64)>,
    /// Scrolls the dock's shell by lines, back when positive.
    pub terminal_scroll: Rc<dyn Fn(i32)>,
    /// Marks text of the dock's shell.
    pub terminal_select: Rc<dyn Fn(TerminalSelection)>,
    /// Runs a command of the terminal's menu or shortcuts.
    pub terminal_command: Rc<dyn Fn(TerminalCommand)>,
    /// Runs the task at the given place of the Tasks tab.
    pub run_task: Rc<dyn Fn(usize)>,
    /// Asks again for the tasks and debug scenarios of the dock's folder.
    pub refresh_dock: Rc<dyn Fn()>,
    /// Starts debugging as the scenario at the given place.
    pub debug_start: Rc<dyn Fn(usize)>,
    /// Does something to the debugging session followed.
    pub debug_command: Rc<dyn Fn(DebugCommand)>,
    /// Sets or takes away the breakpoint on the given line, from zero, of the given file.
    pub toggle_breakpoint: Rc<dyn Fn(String, usize)>,
    /// Shows or hides the sidebar, the dock or the right panel.
    pub toggle: Rc<dyn Fn(Panel)>,
    /// Opens the command centre.
    pub command_centre: Rc<dyn Fn()>,
    /// Opens a page of the sidebar's navigation.
    pub navigate: Rc<dyn Fn(Destination)>,
    /// Asks the window to minimise, maximise or close.
    pub window: Rc<dyn Fn(WindowAction)>,
    /// Opens the overview of the project at the given address.
    pub open_overview: Rc<dyn Fn(String)>,
    /// Opens the session with the given number from the history, closed or not.
    pub reopen_session: Rc<dyn Fn(u64)>,
    /// Installs, updates or uninstalls the extension with the given id.
    pub press_extension: Rc<dyn Fn(String)>,
    /// Answers the open session's permission with the given ticket by the choice with the given id.
    pub choose: Rc<dyn Fn(u64, String)>,
    /// Answers the open session's form of questions with the given ticket, or abandons it when nothing.
    pub answer_inquiry: Rc<dyn Fn(u64, Option<Vec<InquiryReply>>)>,
    /// Answers the open session's trust request with the given ticket.
    pub trust: Rc<dyn Fn(u64, TrustVerdict)>,
    /// Pushes the open session's branch or acts on its pull request, by the action's name.
    pub pull_request: Rc<dyn Fn(String)>,
    /// Carries out a conversation command on the open session.
    pub conversation: Rc<dyn Fn(ConversationCommand)>,
    /// Continues the open chat as a session in the project at the given index path.
    pub continue_chat: Rc<dyn Fn(Vec<usize>)>,
    /// Opens a web address in the reader's browser.
    pub open_address: Rc<dyn Fn(String)>,
    /// Does what the browser pane asks.
    pub browser: Rc<dyn Fn(BrowserAction)>,
    /// Gives a panel a new size, its width or for the dock its height, in logical pixels.
    pub resize: Rc<dyn Fn(Panel, f32)>,
    /// Does what the newer-release button or the update dialog asks.
    pub update: Rc<dyn Fn(UpdateAction)>,
    /// Does what the sign-in code dialog asks.
    pub sign_in: Rc<dyn Fn(SignInAction)>,
    /// Does what the Git panel or the Diff tab asks.
    pub git: Rc<dyn Fn(GitAction)>,
    /// Opens the file at the given path, from the open session's folder or whole, in the preview tab.
    pub open_file: Rc<dyn Fn(String)>,
    /// Does what the session view's tab strip or its menu asks of the tabs.
    pub file_tab: Rc<dyn Fn(TabAction)>,
    /// Saves the open file at the given position.
    pub save_file: Rc<dyn Fn(usize)>,
    /// Asks the language server of the open file at the given position what its editor asks.
    pub code: Rc<dyn Fn(usize, ServerAsk)>,
    /// Does something with the open file at the given position besides editing it.
    pub file_action: Rc<dyn Fn(usize, FileAction)>,
    /// Closes the popups the window opened over the editor of the open file at the given position.
    pub dismiss_popups: Rc<dyn Fn(usize)>,
    /// Opens the file at the given path at the given line, counted from 1.
    pub open_location: Rc<dyn Fn(String, usize)>,
    /// Does something to the file tree.
    pub tree: Rc<dyn Fn(TreeAction)>,
    /// Does something in the project search.
    pub search: Rc<dyn Fn(SearchAction)>,
    /// Does something in the quick input.
    pub quick: Rc<dyn Fn(QuickAction)>,
    /// Takes a chord pressed anywhere in the window, written as the settings write it, such as `ctrl+shift+p`, and says whether the keymap took it.
    pub shortcut: Rc<dyn Fn(String) -> bool>,
    /// Arranges the sidebar's projects, or works the adder or the folder picker.
    pub projects: Rc<dyn Fn(ProjectAction)>,
}

/// What the newer-release button and the update dialog ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateAction {
    /// Show the dialog offering the release.
    Open,
    /// Hide it, stopping a download under way.
    Close,
    /// Download and install the release, then restart into it.
    Install,
    /// Open the page the release is published on.
    ViewPage,
}

/// What the sign-in code dialog asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignInAction {
    /// Hide the dialog.
    Hide,
    /// Put the code on the clipboard.
    Copy,
    /// Open the page the code is entered on.
    Open,
}

/// A panel that can be shown or hidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    /// The sidebar.
    Sidebar,
    /// The bottom dock.
    Dock,
    /// The right panel.
    Right,
}

/// A page the sidebar's navigation opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    /// A new chat.
    NewChat,
    /// Adding a workspace.
    NewWorkspace,
    /// Past chats.
    History,
    /// Extensions.
    Extensions,
    /// Settings.
    Settings,
}

/// Something the window's own controls ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowAction {
    /// Minimise.
    Minimise,
    /// Maximise or restore.
    Maximise,
    /// Close.
    Close,
    /// Go back.
    Back,
    /// Go forward.
    Forward,
    /// Start moving the window with the pointer.
    Drag,
}

impl Actions {
    /// Actions that do nothing, for snapshots.
    pub fn inert() -> Self {
        Self {
            open_session: Rc::new(|_| {}),
            new_session: Rc::new(|_| {}),
            send_prompt: Rc::new(|_| {}),
            composer: Rc::new(|_| {}),
            choose_agent: Rc::new(|_| {}),
            start: Rc::new(|_| {}),
            right_tab: Rc::new(|_| {}),
            dock_tab: Rc::new(|_| {}),
            terminal_input: Rc::new(|_| {}),
            terminal_resize: Rc::new(|_, _| {}),
            terminal_new: Rc::new(|| {}),
            terminal_pick: Rc::new(|_| {}),
            terminal_close: Rc::new(|_| {}),
            terminal_scroll: Rc::new(|_| {}),
            terminal_select: Rc::new(|_| {}),
            terminal_command: Rc::new(|_| {}),
            run_task: Rc::new(|_| {}),
            refresh_dock: Rc::new(|| {}),
            debug_start: Rc::new(|_| {}),
            debug_command: Rc::new(|_| {}),
            toggle_breakpoint: Rc::new(|_, _| {}),
            toggle: Rc::new(|_| {}),
            command_centre: Rc::new(|| {}),
            navigate: Rc::new(|_| {}),
            window: Rc::new(|_| {}),
            open_overview: Rc::new(|_| {}),
            reopen_session: Rc::new(|_| {}),
            press_extension: Rc::new(|_| {}),
            choose: Rc::new(|_, _| {}),
            answer_inquiry: Rc::new(|_, _| {}),
            trust: Rc::new(|_, _| {}),
            pull_request: Rc::new(|_| {}),
            conversation: Rc::new(|_| {}),
            continue_chat: Rc::new(|_| {}),
            open_address: Rc::new(|_| {}),
            browser: Rc::new(|_| {}),
            resize: Rc::new(|_, _| {}),
            update: Rc::new(|_| {}),
            sign_in: Rc::new(|_| {}),
            git: Rc::new(|_| {}),
            open_file: Rc::new(|_| {}),
            file_tab: Rc::new(|_| {}),
            save_file: Rc::new(|_| {}),
            code: Rc::new(|_, _| {}),
            file_action: Rc::new(|_, _| {}),
            dismiss_popups: Rc::new(|_| {}),
            open_location: Rc::new(|_, _| {}),
            tree: Rc::new(|_| {}),
            search: Rc::new(|_| {}),
            quick: Rc::new(|_| {}),
            shortcut: Rc::new(|_| false),
            projects: Rc::new(|_| {}),
        }
    }
}
