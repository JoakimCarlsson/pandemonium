//! What the browser pane asks of the link: the buttons, the address bar, the tabs, the find bar, the menu over the page, the safety prompts and the pointer and keys handed to the page.

use pm_browser::{CaptureKind, PageInput};

use crate::browser::ZoomStep;

/// A row of the menu over a page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageCommand {
    /// Goes back one page in the history.
    Back,
    /// Goes forward one page in the history.
    Forward,
    /// Loads the page again.
    Reload,
    /// Loads the page again without using the cache.
    HardReload,
    /// Cuts what is marked in the page.
    Cut,
    /// Copies what is marked in the page.
    Copy,
    /// Pastes the clipboard into the page.
    Paste,
    /// Marks everything in the page.
    SelectAll,
    /// Shows the bar for finding text in the page.
    Find,
    /// Zooms the page in.
    ZoomIn,
    /// Zooms the page out.
    ZoomOut,
    /// Puts the zoom of the page back to 100%.
    ZoomReset,
    /// Opens the page in the system's browser.
    OpenExternal,
    /// Asks whether to clear the cookies and site data of the browser.
    ClearData,
    /// Shows the developer tools beside the page.
    Inspect,
    /// Picks an element of the page for the prompt.
    PickElement,
    /// Attaches a screenshot of what the page shows to the prompt.
    Screenshot,
    /// Attaches a screenshot of the whole page to the prompt.
    FullPage,
    /// Lets the reader drag a rectangle over the page and attaches its picture to the prompt.
    Area,
    /// Attaches the lines the page wrote to its console to the prompt.
    Console,
}

impl PageCommand {
    /// Every row, in order, each with whether a separator follows it.
    pub const ROWS: [(Self, bool); 20] = [
        (Self::Back, false),
        (Self::Forward, false),
        (Self::Reload, false),
        (Self::HardReload, true),
        (Self::Cut, false),
        (Self::Copy, false),
        (Self::Paste, false),
        (Self::SelectAll, true),
        (Self::Find, true),
        (Self::ZoomIn, false),
        (Self::ZoomOut, false),
        (Self::ZoomReset, true),
        (Self::OpenExternal, false),
        (Self::ClearData, true),
        (Self::Inspect, true),
        (Self::PickElement, false),
        (Self::Screenshot, false),
        (Self::FullPage, false),
        (Self::Area, false),
        (Self::Console, false),
    ];

    /// What its row says.
    pub fn label(self) -> &'static str {
        match self {
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::Reload => "Reload",
            Self::HardReload => "Hard Reload",
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
            Self::Find => "Find in Page",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ZoomReset => "Actual Size",
            Self::OpenExternal => "Open in Default Browser",
            Self::ClearData => "Clear Cookies and Site Data...",
            Self::Inspect => "Inspect",
            Self::PickElement => "Pick Element",
            Self::Screenshot => "Add Screenshot to Chat",
            Self::FullPage => "Add Full Page Screenshot",
            Self::Area => "Add Area Screenshot to Chat",
            Self::Console => "Add Console Logs to Chat",
        }
    }

    /// The keys that do the same while the page has the keyboard, written beside its row.
    pub fn keys(self) -> Option<&'static str> {
        match self {
            Self::Reload => Some("Ctrl+R"),
            Self::HardReload => Some("Ctrl+Shift+R"),
            Self::Find => Some("Ctrl+F"),
            Self::ZoomIn => Some("Ctrl+="),
            Self::ZoomOut => Some("Ctrl+-"),
            Self::ZoomReset => Some("Ctrl+0"),
            _ => None,
        }
    }
}

/// Which page of the pane something is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Surface {
    /// The page of the tab shown.
    Page,
    /// The developer tools beside it.
    Tools,
}

/// What the browser pane asks of the link.
#[derive(Clone, Debug, PartialEq)]
pub enum BrowserAction {
    /// Opens the browser, on the page a dev server is usually at, or shows it when it is open.
    Open,
    /// Closes the browser and every tab of it.
    Close,
    /// The reader typed this in the address bar.
    Address(String),
    /// Loads what was typed in the address bar.
    Go,
    /// Goes back one page.
    Back,
    /// Goes forward one page.
    Forward,
    /// Loads the page again.
    Reload,
    /// Loads the page again without the cache.
    HardReload,
    /// Opens a new, empty tab and shows it.
    NewTab,
    /// Shows the tab at this index.
    SelectTab(usize),
    /// Closes the tab at this index.
    CloseTab(usize),
    /// Closes the tab shown.
    CloseShownTab,
    /// Locks the tab shown against agents, or unlocks it.
    Lock,
    /// Shows or hides the developer tools.
    ToggleTools,
    /// Makes the developer tools this wide, in logical pixels.
    ToolsWidth(f32),
    /// Shows the bar for finding in the page.
    Find,
    /// The reader typed this in the find bar.
    FindTyped(String),
    /// Goes to the next match, or the previous one when `back`.
    FindStep {
        /// Whether to go back.
        back: bool,
    },
    /// Hides the find bar.
    FindClose,
    /// Changes the zoom of the page.
    Zoom(ZoomStep),
    /// Opens the menu over the page at this point of it, or closes it.
    Menu(Option<(f32, f32)>),
    /// Runs a row of the menu over the page.
    Command(PageCommand),
    /// Clears the cookies and site data, as the reader agreed to.
    ClearConfirm,
    /// Leaves the cookies and site data as they are.
    ClearCancel,
    /// Answers the page's question `id`, allowing it when `allow`.
    Permission {
        /// The question.
        id: u64,
        /// Whether to allow it.
        allow: bool,
    },
    /// Takes away the note about the last download.
    DismissDownload,
    /// Goes back from a site whose certificate is not trusted.
    CertificateBack,
    /// Loads a site whose certificate is not trusted; the first press asks again.
    CertificateProceed,
    /// Hands a page the pointer or the keyboard.
    Input(Surface, PageInput),
    /// A page's window is this many logical pixels wide and tall, at this display scale.
    Resized(Surface, u32, u32, f32),
    /// A page's window took or lost the keyboard.
    Focused(Surface, bool),
    /// Starts or stops picking an element of the page for the prompt.
    Pick,
    /// Attaches a screenshot of the page, or its console, to the prompt.
    Capture(CaptureKind),
    /// Opens the menu of the captures below the camera button.
    CaptureMenu,
    /// Ends dragging a rectangle over the page without a picture.
    CancelArea,
}
