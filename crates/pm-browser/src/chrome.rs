//! What a page reports about itself beyond painting, for the pane to draw around it, and what the reader does with it: dialogs, the menu of a right click, downloads, full screen, sound, the address under the pointer, tooltips, printing, sharing the screen, the zoom of a site, the logins the helper keeps, the ads and trackers it blocks, saving the page, reader view and files dropped on it.

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

/// A login the reader just sent from a page, which the helper offers to keep in the system's keychain; the password stays in the helper and is never sent to the window.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PasswordOffer {
    /// The number of the offer, for the answer.
    pub id: u64,
    /// The origin the login is for, such as `https://example.com`.
    pub origin: String,
    /// The user name sent with the password, or nothing when the form had none.
    pub username: String,
    /// Whether a password is already kept for that user name, so saving replaces it.
    pub update: bool,
}

/// The helper's question for the primary password that seals saved logins where the system's keychain cannot: to set one the first time a login is saved, or to unlock the saved logins when one is to be saved or filled in.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrimaryPasswordAsk {
    /// Whether no primary password is set yet, so the reader chooses one.
    pub create: bool,
    /// The origin of the login that needs the saved logins open, such as `https://example.com`.
    pub origin: String,
    /// Why the last answer was refused, such as a wrong or too short password, or nothing.
    pub problem: String,
}

/// A field of a login form that holds the keyboard, on a page whose origin has logins kept, for the pane to list them under it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoginField {
    /// Logical pixels from the left of the page to the field.
    pub x: i32,
    /// Logical pixels from the top of the page to the field.
    pub y: i32,
    /// The field's width in logical pixels.
    pub width: i32,
    /// The field's height in logical pixels.
    pub height: i32,
    /// The user names kept for the origin; the passwords stay in the helper.
    pub usernames: Vec<String>,
}

/// The article of a page, taken out of it for reader view.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Article {
    /// The address of the page it was taken from.
    pub url: String,
    /// Its title.
    pub title: String,
    /// Who wrote it, as the page says, or nothing.
    pub byline: String,
    /// The name of the site, as the page says, or nothing.
    pub site: String,
    /// The language of the page, such as `en`, or nothing.
    pub lang: String,
    /// The direction of its text, `ltr` or `rtl`, or nothing.
    pub dir: String,
    /// Its content as HTML, cleaned of scripts, styles and what is not part of it; nothing when there is no article.
    pub html: String,
    /// Why there is no article; nothing when there is one.
    pub error: String,
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
    /// How the blocker of ads and trackers stands on the page: whether it works on the page's site and how many requests it stopped since the page last loaded.
    Blocking {
        /// Whether it blocks on the page's site: on in the settings, the site not let through by the reader and not a local address.
        on: bool,
        /// How many requests of the page it stopped.
        blocked: u32,
    },
    /// The reader sent a login the helper can keep; the reader answers with [`PageAction::AnswerPassword`].
    OfferPassword(Box<PasswordOffer>),
    /// The password offer `id` is no longer open, as when it was answered in another way or its page went away.
    PasswordOfferClosed {
        /// The offer.
        id: u64,
    },
    /// A field of a login form holds the keyboard on a page whose origin has logins kept; the reader picks one with [`PageAction::FillLogin`].
    LoginField(Box<LoginField>),
    /// No field of a login form holds the keyboard any more.
    LoginFieldLeft,
    /// The saved logins are sealed by a primary password and must be opened, or one set, before a login is saved or filled in; the reader answers with [`PageAction::PrimaryPassword`].
    PrimaryPassword(Box<PrimaryPasswordAsk>),
    /// The question for the primary password is no longer open, as when it was answered rightly or asked on another page.
    PrimaryPasswordClosed,
    /// The page finished loading and looks like an article reader view can show, or it started loading another document and no longer does.
    Readable {
        /// Whether it looks like an article.
        readable: bool,
    },
    /// The article of the page, as [`PageAction::ReadArticle`] asked for it, or why there is none.
    Article(Box<Article>),
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
    /// Answers the password offer `id`.
    AnswerPassword {
        /// The offer.
        id: u64,
        /// Whether to keep the login in the system's keychain.
        save: bool,
        /// Whether never to offer again for the offer's origin.
        never: bool,
    },
    /// Fills the login form whose field holds the keyboard with the login kept for `username` on the page's origin.
    FillLogin {
        /// The user name.
        username: String,
    },
    /// Answers the question for the primary password: the password the reader typed, or nothing when they turned it down.
    PrimaryPassword {
        /// The password, or nothing.
        password: Option<String>,
    },
    /// Turns the blocker of ads and trackers on or off for the site of the page, and loads it again.
    SiteBlocking {
        /// Whether it blocks on the site.
        on: bool,
    },
    /// Saves the page as a complete web page, its document with the pictures, styles and scripts it uses in a folder beside it, into the Downloads folder; it is reported as a [`PageReport::Download`].
    SavePage,
    /// Takes the article out of the page, without its menus, sidebars and advertising; the answer is a [`PageReport::Article`].
    ReadArticle,
    /// Drops files of the reader's machine on the page at `x`, `y`, as dragging them from a file manager and letting go there does, for drop zones that upload what is dropped on them.
    DropFiles {
        /// Logical pixels from the left of the page.
        x: i32,
        /// Logical pixels from the top of the page.
        y: i32,
        /// The absolute paths of the files.
        paths: Vec<String>,
    },
}
