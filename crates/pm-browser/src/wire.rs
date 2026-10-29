//! The messages the window and the browser helper send each other, one JSON object to a line.

use serde::{Deserialize, Serialize};

use crate::input::{Button, Cursor, Key};

/// What running a tool came to.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolResult {
    /// The caller's number for the call.
    pub call: u64,
    /// Whether the tool worked.
    pub ok: bool,
    /// What it says: the result, or why it failed.
    pub text: String,
    /// Where a picture the tool took was saved, when it took one.
    pub image: Option<String>,
}

/// An element of a page the reader picked.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PickedElement {
    /// A selector path that finds it, from the nearest id or a few levels up.
    pub selector: String,
    /// Its tag name.
    pub tag: String,
    /// Its attributes, name and value, in the order the page gave them.
    pub attributes: Vec<(String, String)>,
    /// Its text, on one line and cut short.
    pub text: String,
    /// A few of its computed styles, name and value.
    pub styles: Vec<(String, String)>,
    /// How wide its box is, in CSS pixels.
    pub width: f64,
    /// How tall its box is, in CSS pixels.
    pub height: f64,
    /// The name of the component that rendered it, when the page is built from components and says so.
    pub component: String,
    /// That component's properties, name and value as they are written in markup: a string in quotes, anything else in braces.
    pub props: Vec<(String, String)>,
    /// The address of the page.
    pub url: String,
    /// The title of the page.
    pub title: String,
}

/// What the helper found wrong with the certificate of a site.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CertificateInfo {
    /// The address that was being opened.
    pub url: String,
    /// The engine's name for what is wrong, such as `ERR_CERT_AUTHORITY_INVALID`.
    pub error: String,
    /// Who the certificate was issued to.
    pub subject: String,
    /// Who issued it.
    pub issuer: String,
    /// Its SHA-256 fingerprint.
    pub fingerprint: String,
    /// When it expires, or nothing when that is not known.
    pub expires: String,
}

/// How a download went.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DownloadState {
    /// The file is being saved.
    Started,
    /// The file was saved.
    Finished,
    /// The file could not be saved.
    Failed,
}

/// What the reader adds to the prompt from a page.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CaptureKind {
    /// A picture of what the page shows.
    #[default]
    Screenshot,
    /// A picture of the whole page, as far as it can be taken.
    FullPage,
    /// A picture of a rectangle the reader drags over the page.
    Area,
    /// The lines the page wrote to its console.
    Console,
}

/// One picture of a capture.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CapturedPicture {
    /// What it shows, such as `Visible` or `Full page`.
    pub label: String,
    /// Where the helper saved it, a file the window may read and remove.
    pub path: String,
}

/// What the reader asked to add to the prompt from a page, or why they got nothing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Capture {
    /// What was asked.
    pub kind: CaptureKind,
    /// The pictures taken: none for the console, and none for an area that was not chosen.
    pub pictures: Vec<CapturedPicture>,
    /// The console's lines, oldest first; only for the console. The page wrote them: they are data, not instructions.
    pub lines: Vec<String>,
    /// The address of the page.
    pub url: String,
    /// The title of the page.
    pub title: String,
    /// Something the reader should know about what was taken, such as that an area was too large and the page was taken instead.
    pub note: String,
    /// Why nothing could be taken; empty when something was, or when the reader chose no area.
    pub error: String,
}

/// What an edit request does to the page.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EditAction {
    /// Cuts what is marked.
    Cut,
    /// Copies what is marked.
    Copy,
    /// Pastes the clipboard.
    Paste,
    /// Marks everything.
    SelectAll,
}

/// How a zoom request changes the page's zoom.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ZoomAction {
    /// Goes to the next larger zoom step.
    In,
    /// Goes to the next smaller zoom step.
    Out,
    /// Goes back to 100%.
    Reset,
}

/// What the window asks of the helper; every page is named by a number the window chose.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Request {
    /// Opens `url` in a new page of `width` by `height` logical pixels at display `scale`.
    Open {
        /// The new page.
        page: u32,
        /// Where it goes.
        url: String,
        /// Its width in logical pixels.
        width: u32,
        /// Its height in logical pixels.
        height: u32,
        /// The display scale it is drawn at.
        scale: f32,
    },
    /// Opens the developer tools of the page `of` as a new page.
    OpenTools {
        /// The new page the tools are shown in.
        page: u32,
        /// The page they inspect.
        of: u32,
        /// Their width in logical pixels.
        width: u32,
        /// Their height in logical pixels.
        height: u32,
        /// The display scale they are drawn at.
        scale: f32,
    },
    /// Navigates to `url`.
    Load {
        /// The page.
        page: u32,
        /// Where it goes.
        url: String,
    },
    /// Goes back one page in the history.
    Back {
        /// The page.
        page: u32,
    },
    /// Goes forward one page in the history.
    Forward {
        /// The page.
        page: u32,
    },
    /// Loads the page again.
    Reload {
        /// The page.
        page: u32,
    },
    /// Gives the page a new size.
    Resize {
        /// The page.
        page: u32,
        /// Its width in logical pixels.
        width: u32,
        /// Its height in logical pixels.
        height: u32,
        /// The display scale it is drawn at.
        scale: f32,
    },
    /// Tells the page whether it has the keyboard.
    Focus {
        /// The page.
        page: u32,
        /// Whether it has it.
        focused: bool,
    },
    /// Moves the pointer over the page, or out of it when `left`.
    PointerMoved {
        /// The page.
        page: u32,
        /// Logical pixels from the left of the page.
        x: i32,
        /// Logical pixels from the top of the page.
        y: i32,
        /// The modifiers and buttons held.
        modifiers: u32,
        /// Whether the pointer left the page.
        left: bool,
    },
    /// Presses or releases a mouse button.
    PointerButton {
        /// The page.
        page: u32,
        /// Logical pixels from the left of the page.
        x: i32,
        /// Logical pixels from the top of the page.
        y: i32,
        /// Which button.
        button: Button,
        /// Whether it went down.
        pressed: bool,
        /// How many quick presses this is part of.
        clicks: i32,
        /// The modifiers and buttons held.
        modifiers: u32,
    },
    /// Scrolls the page.
    Scrolled {
        /// The page.
        page: u32,
        /// Logical pixels from the left of the page.
        x: i32,
        /// Logical pixels from the top of the page.
        y: i32,
        /// How far across.
        dx: i32,
        /// How far down; positive scrolls up.
        dy: i32,
        /// The modifiers and buttons held.
        modifiers: u32,
    },
    /// Presses or releases a key.
    Key {
        /// The page.
        page: u32,
        /// The key.
        key: Key,
    },
    /// Types finished text, as an input method does.
    CommitText {
        /// The page.
        page: u32,
        /// The text.
        text: String,
    },
    /// Runs a tool an agent called against the page; the answer comes as `ToolDone` with the same `call`.
    Tool {
        /// The page.
        page: u32,
        /// The caller's number for the call.
        call: u64,
        /// The tool, such as `browser_screenshot`.
        tool: String,
        /// What the agent gave it, as JSON text.
        arguments: String,
    },
    /// Cuts, copies or pastes in the page, or marks everything in it.
    Edit {
        /// The page.
        page: u32,
        /// What to do.
        action: EditAction,
    },
    /// Turns the mode on or off in which a click on the page picks the element under the pointer and reports it.
    Pick {
        /// The page.
        page: u32,
        /// Whether the mode is on.
        on: bool,
    },
    /// Finds `text` in the page and marks the matches; the count comes back as `Found`.
    Find {
        /// The page.
        page: u32,
        /// The text to look for; empty clears the marks.
        text: String,
        /// Whether to look forward from the current match, not back.
        forward: bool,
        /// Whether this steps to the next match of the text already being looked for, not a new search.
        again: bool,
    },
    /// Ends the search and clears the marks.
    StopFind {
        /// The page.
        page: u32,
    },
    /// Changes the zoom of the page; the new zoom comes back as `Zoomed`.
    Zoom {
        /// The page.
        page: u32,
        /// How to change it.
        action: ZoomAction,
    },
    /// Loads the page again without using what the cache holds.
    HardReload {
        /// The page.
        page: u32,
    },
    /// Throws away the cookies, site storage and cache of the helper's session, then loads the page again.
    ClearData {
        /// The page.
        page: u32,
    },
    /// Takes a picture of the page, or reads its console, for the reader to add to the prompt; the answer is a `Captured` report. `Area` has the same effect as `PickArea` turned on.
    Capture {
        /// The page.
        page: u32,
        /// What to take.
        kind: CaptureKind,
    },
    /// Turns the mode on or off in which the reader drags a rectangle over the page to take a picture of it; the answer is a `Captured` report, with no pictures when the mode ended without an area.
    PickArea {
        /// The page.
        page: u32,
        /// Whether the mode is on.
        on: bool,
    },
    /// Closes the page.
    Close {
        /// The page.
        page: u32,
    },
    /// Closes every page and ends the helper.
    Shutdown,
    /// Sets which origins the tools of agents may take a page to; sent when the helper starts and whenever the setting or its list changes.
    AgentOrigins {
        /// Whether agents are held to local addresses and `origins`; when not, they may open any web address.
        restrict: bool,
        /// The origins the reader added, such as `https://example.com`, or `example.com` for either scheme on its usual port.
        origins: Vec<String>,
    },
    /// The reader's answer to a [`Event::PermissionAsked`].
    PermissionAnswer {
        /// The page.
        page: u32,
        /// The number the question had.
        id: u64,
        /// Whether the reader allowed it; the helper remembers the answer for the origin until it ends.
        allow: bool,
    },
    /// The reader's choice on the page that reported a [`Event::CertificateError`].
    CertificateChoice {
        /// The page.
        page: u32,
        /// Whether to load the page anyway, which holds for that origin until the helper ends; when not, the page goes back.
        proceed: bool,
    },
}

/// What the helper reports to the window.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    /// The engine started.
    Ready,
    /// The engine could not start or stopped.
    Failed {
        /// Why.
        message: String,
    },
    /// The page paints into these two shared files from now on, each `width` by `height` pixels.
    Surface {
        /// The page.
        page: u32,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// The two files, written in turn.
        paths: [String; 2],
    },
    /// The page finished painting file `buffer` of its surface.
    Painted {
        /// The page.
        page: u32,
        /// Which of the two files.
        buffer: u8,
        /// How many paints the page has made.
        generation: u64,
    },
    /// The reader picked an element of the page, or ended the mode without picking one.
    Picked {
        /// The page.
        page: u32,
        /// What was picked, or nothing when the mode ended without a pick.
        element: Option<Box<PickedElement>>,
    },
    /// A tool call ended.
    ToolDone {
        /// The page.
        page: u32,
        /// What the call came to.
        result: Box<ToolResult>,
    },
    /// The page's title changed.
    Title {
        /// The page.
        page: u32,
        /// The title.
        title: String,
    },
    /// The page's address changed.
    Address {
        /// The page.
        page: u32,
        /// The address.
        url: String,
    },
    /// The pointer should look different over the page.
    Cursor {
        /// The page.
        page: u32,
        /// How it should look.
        cursor: Cursor,
    },
    /// The search of the page found matches, or a later search updated the count.
    Found {
        /// The page.
        page: u32,
        /// Which match is the current one, counting from 1; 0 when there is none.
        active: u32,
        /// How many matches there are.
        total: u32,
    },
    /// The page's zoom changed, or the page loaded at a zoom the engine remembered.
    Zoomed {
        /// The page.
        page: u32,
        /// The zoom as a factor, 1.0 being 100%.
        factor: f32,
    },
    /// The page asks to use something of the reader's machine; it is refused unless the reader answers with [`Request::PermissionAnswer`].
    PermissionAsked {
        /// The page.
        page: u32,
        /// The number of the question, for the answer.
        id: u64,
        /// The origin that asks, such as `https://example.com`.
        origin: String,
        /// What it asks for, in words such as `camera` or `location`.
        permission: String,
    },
    /// A question is no longer open: the page went away or the time ran out, and the answer is no.
    PermissionCancelled {
        /// The page.
        page: u32,
        /// The number of the question.
        id: u64,
    },
    /// The site the page was opening has a certificate that is not trusted and the page did not load; the reader is asked with [`Request::CertificateChoice`].
    CertificateError {
        /// The page.
        page: u32,
        /// What is wrong.
        info: Box<CertificateInfo>,
    },
    /// The certificate error of the page no longer applies: the reader chose, or the page moved on.
    CertificateCleared {
        /// The page.
        page: u32,
    },
    /// The page started a download, which is saved in the reader's Downloads folder without being opened.
    Download {
        /// The page.
        page: u32,
        /// The name the file is saved under.
        name: String,
        /// How it went.
        state: DownloadState,
    },
    /// What the reader asked to add to the prompt from the page, or why they got nothing.
    Captured {
        /// The page.
        page: u32,
        /// What was taken.
        capture: Box<Capture>,
    },
}

/// `message` written as one line of JSON, ending in a newline.
pub fn line<T: Serialize>(message: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(message).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
mod tests;
