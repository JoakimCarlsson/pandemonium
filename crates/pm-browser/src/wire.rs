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
    /// A selector that finds it, from the nearest id or a few levels up.
    pub selector: String,
    /// Its tag name.
    pub tag: String,
    /// Its id, when it has one.
    pub id: String,
    /// Its classes.
    pub classes: Vec<String>,
    /// Its text, cut short.
    pub text: String,
    /// Its HTML, cut short.
    pub html: String,
    /// Where its box is in the page, in CSS pixels.
    pub x: f64,
    /// Where its box is in the page, in CSS pixels.
    pub y: f64,
    /// How wide its box is, in CSS pixels.
    pub width: f64,
    /// How tall its box is, in CSS pixels.
    pub height: f64,
    /// How far the page was scrolled across when it was picked, in CSS pixels.
    pub scroll_x: f64,
    /// How far the page was scrolled down when it was picked, in CSS pixels.
    pub scroll_y: f64,
    /// Where a picture of the element was saved, when one could be taken.
    pub screenshot: Option<String>,
    /// A few of its computed styles, name and value.
    pub styles: Vec<(String, String)>,
    /// The address of the page.
    pub url: String,
    /// The title of the page.
    pub title: String,
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
    /// Closes the page.
    Close {
        /// The page.
        page: u32,
    },
    /// Closes every page and ends the helper.
    Shutdown,
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
}

/// `message` written as one line of JSON, ending in a newline.
pub fn line<T: Serialize>(message: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(message).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}
