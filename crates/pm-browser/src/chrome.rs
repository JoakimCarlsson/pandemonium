//! What a page reports about itself beyond painting, for the pane to draw around it, and what the reader does with it: dialogs, the menu of a right click, downloads, full screen, sound, the address under the pointer, tooltips, printing, sharing the screen and the zoom of a site.

use serde::{Deserialize, Serialize};

/// Which JavaScript dialog a page opened.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum DialogKind {
    /// `alert`: a message with OK.
    #[default]
    Alert,
    /// `confirm`: a question with OK and Cancel.
    Confirm,
    /// `prompt`: a question with a line to type in, OK and Cancel.
    Prompt,
}

/// A JavaScript dialog the page opened, which stops it until the reader answers.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dialog {
    /// The number of the dialog, for the answer.
    pub id: u64,
    /// Which dialog it is.
    pub kind: DialogKind,
    /// What it says.
    pub message: String,
    /// The text a prompt starts with.
    pub default_text: String,
    /// The host of the page that opened it, or nothing for a page without one.
    pub host: String,
}

/// What was under the pointer when the reader pressed the secondary button on a page.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContextMenu {
    /// Logical pixels from the left of the page.
    pub x: i32,
    /// Logical pixels from the top of the page.
    pub y: i32,
    /// The address of the link under the pointer, or nothing.
    pub link_url: String,
    /// The address of the picture or media under the pointer, or nothing.
    pub src_url: String,
    /// Whether what is under the pointer is a picture.
    pub is_image: bool,
    /// The text that is marked.
    pub selection_text: String,
    /// Whether the pointer is over something the reader can type in.
    pub editable: bool,
    /// Whether something can be cut.
    pub can_cut: bool,
    /// Whether something can be copied.
    pub can_copy: bool,
    /// Whether something can be pasted.
    pub can_paste: bool,
    /// The misspelt word under the pointer, or nothing.
    pub misspelled: String,
    /// The words the spelling checker suggests for it.
    pub suggestions: Vec<String>,
    /// Whether the page can go back.
    pub can_back: bool,
    /// Whether the page can go forward.
    pub can_forward: bool,
}

/// How a download stands.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum DownloadProgress {
    /// Bytes are arriving.
    #[default]
    Running,
    /// The file was saved.
    Finished,
    /// The file could not be saved.
    Failed,
    /// The reader cancelled it.
    Cancelled,
}

/// One download and how far it got.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadItem {
    /// The number of the download, for cancelling it or showing it.
    pub id: u64,
    /// The name it is saved under.
    pub name: String,
    /// How many bytes have arrived.
    pub received: u64,
    /// How many bytes there are in all, or 0 when the server did not say.
    pub total: u64,
    /// How it stands.
    pub progress: DownloadProgress,
}

/// A screen or window a page may share, for the reader to choose from.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenSource {
    /// The engine's name for it.
    pub id: String,
    /// What it is called.
    pub name: String,
}

/// A page asks to share a screen or a window.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenShare {
    /// The number of the question, for the answer.
    pub id: u64,
    /// The origin that asks.
    pub origin: String,
    /// What it may share.
    pub sources: Vec<ScreenSource>,
}

/// Something a page reports for the pane to show around it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PageReport {
    /// A dialog opened; the reader answers with [`PageAction::AnswerDialog`].
    Dialog(Box<Dialog>),
    /// The dialog `id` closed another way, as when an agent answered it or nobody did in time.
    DialogClosed {
        /// The dialog.
        id: u64,
    },
    /// The reader pressed the secondary button on the page.
    ContextMenu(Box<ContextMenu>),
    /// A download started or moved on.
    Download(Box<DownloadItem>),
    /// The page went into full screen, or left it.
    Fullscreen {
        /// Whether it is in full screen.
        on: bool,
    },
    /// The page started or stopped making sound, or was muted.
    Audio {
        /// Whether it makes sound.
        audible: bool,
        /// Whether its sound is muted.
        muted: bool,
    },
    /// The pointer moved onto a link, or off every link.
    HoveredLink {
        /// The address of the link, or nothing.
        url: String,
    },
    /// The pointer rests on something with a title, or left it.
    Tooltip {
        /// The title, or nothing.
        text: String,
        /// Where the pointer rests, in logical pixels from the left of the page.
        #[serde(default)]
        x: i32,
        /// Where the pointer rests, in logical pixels from the top of the page.
        #[serde(default)]
        y: i32,
    },
    /// The page was printed to a file.
    Printed {
        /// Where the file is; nothing when it failed.
        path: String,
        /// Why it failed; nothing when it worked.
        error: String,
    },
    /// The page asks to share a screen or a window; the reader answers with [`PageAction::ChooseScreen`].
    ScreenShare(Box<ScreenShare>),
    /// The question about sharing the screen `id` is no longer open.
    ScreenShareCancelled {
        /// The question.
        id: u64,
    },
}

/// Something the reader does with a page from the pane around it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PageAction {
    /// Answers the dialog `id`.
    AnswerDialog {
        /// The dialog.
        id: u64,
        /// Whether OK was pressed.
        accept: bool,
        /// What was typed into a prompt.
        text: String,
    },
    /// Puts `word` in place of the misspelt word under the caret.
    ReplaceMisspelling {
        /// The word.
        word: String,
    },
    /// Shows the element at `x`, `y` in the developer tools, which must be open.
    InspectAt {
        /// Logical pixels from the left of the page.
        x: i32,
        /// Logical pixels from the top of the page.
        y: i32,
    },
    /// Copies the picture at `x`, `y` to the clipboard.
    CopyImageAt {
        /// Logical pixels from the left of the page.
        x: i32,
        /// Logical pixels from the top of the page.
        y: i32,
    },
    /// Downloads `url` into the Downloads folder.
    Save {
        /// The address.
        url: String,
    },
    /// Stops the download `id`.
    CancelDownload {
        /// The download.
        id: u64,
    },
    /// Shows the file of the download `id` in the system's file manager.
    ShowDownload {
        /// The download.
        id: u64,
    },
    /// Opens the file of the download `id` with the program the system chooses.
    OpenDownload {
        /// The download.
        id: u64,
    },
    /// Mutes or unmutes the page.
    Mute {
        /// Whether it is muted.
        muted: bool,
    },
    /// Takes the page out of full screen.
    LeaveFullscreen,
    /// Prints the page to a PDF in the Downloads folder and opens it.
    Print,
    /// Answers the question `id` about sharing the screen: the source chosen, or nothing to refuse.
    ChooseScreen {
        /// The question.
        id: u64,
        /// The source's engine name, or nothing.
        source: Option<String>,
    },
    /// Sets the zoom of the page as a factor, 1.0 being 100%, as when a site's remembered zoom is put back.
    SetZoom {
        /// The factor.
        factor: f32,
    },
}
