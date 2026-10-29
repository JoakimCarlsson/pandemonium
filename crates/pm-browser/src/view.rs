//! One web page, painted by the helper and shown here as pictures.

use std::sync::Arc;

use crate::engine::Engine;
use crate::error::Error;
use crate::frame::Frame;
use crate::input::{Button, Cursor, Key, Modifiers};
use crate::link::{Link, Notice, PageState};
use crate::wire::{
    Capture, CaptureKind, EditAction, PickedElement, Request, ToolResult, ZoomAction,
};

/// A page open in the helper, painted into [`Frame`]s instead of a window.
pub struct View {
    /// The number the helper knows the page by.
    page: u32,
    /// The connection to the helper.
    link: Arc<Link>,
    /// What the helper reported about the page.
    state: Arc<PageState>,
}

impl View {
    /// Opens `url` in a page of `width` by `height` logical pixels shown at display `scale`.
    pub fn open(
        engine: &Engine,
        url: &str,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<Self, Error> {
        if let Some(failure) = engine.failure() {
            return Err(Error::Helper(failure));
        }
        let link = engine.link();
        let (page, state) = link.register();
        link.send(&Request::Open {
            page,
            url: url.to_owned(),
            width,
            height,
            scale,
        });
        Ok(Self { page, link, state })
    }

    /// Opens the developer tools of this page as a page of their own, `width` by `height` logical pixels at `scale`.
    pub fn open_devtools(&self, width: u32, height: u32, scale: f32) -> Self {
        let (page, state) = self.link.register();
        self.link.send(&Request::OpenTools {
            page,
            of: self.page,
            width,
            height,
            scale,
        });
        Self {
            page,
            link: self.link.clone(),
            state,
        }
    }

    /// Navigates the page to `url`.
    pub fn load(&self, url: &str) {
        self.link.send(&Request::Load {
            page: self.page,
            url: url.to_owned(),
        });
    }

    /// Goes back one page in the history.
    pub fn back(&self) {
        self.link.send(&Request::Back { page: self.page });
    }

    /// Goes forward one page in the history.
    pub fn forward(&self) {
        self.link.send(&Request::Forward { page: self.page });
    }

    /// Loads the page again.
    pub fn reload(&self) {
        self.link.send(&Request::Reload { page: self.page });
    }

    /// Resizes the page to `width` by `height` logical pixels at display `scale`.
    pub fn resize(&self, width: u32, height: u32, scale: f32) {
        self.link.send(&Request::Resize {
            page: self.page,
            width,
            height,
            scale,
        });
    }

    /// Tells the page whether it has the keyboard.
    pub fn focus(&self, focused: bool) {
        self.link.send(&Request::Focus {
            page: self.page,
            focused,
        });
    }

    /// Moves the pointer to `x`, `y` logical pixels from the page's corner, or out of it when `left`.
    pub fn pointer_moved(&self, x: i32, y: i32, modifiers: Modifiers, left: bool) {
        self.link.send(&Request::PointerMoved {
            page: self.page,
            x,
            y,
            modifiers: modifiers.0,
            left,
        });
    }

    /// Presses or releases `button` at `x`, `y`; `clicks` counts quick repeats.
    pub fn pointer_button(
        &self,
        x: i32,
        y: i32,
        button: Button,
        pressed: bool,
        clicks: i32,
        modifiers: Modifiers,
    ) {
        self.link.send(&Request::PointerButton {
            page: self.page,
            x,
            y,
            button,
            pressed,
            clicks,
            modifiers: modifiers.0,
        });
    }

    /// Scrolls by `dx`, `dy` logical pixels with the pointer at `x`, `y`; positive `dy` scrolls up.
    pub fn scrolled(&self, x: i32, y: i32, dx: i32, dy: i32, modifiers: Modifiers) {
        self.link.send(&Request::Scrolled {
            page: self.page,
            x,
            y,
            dx,
            dy,
            modifiers: modifiers.0,
        });
    }

    /// Sends a key press or release, and the text it typed.
    pub fn key(&self, key: &Key) {
        self.link.send(&Request::Key {
            page: self.page,
            key: key.clone(),
        });
    }

    /// Types `text` as finished input, as an input method does.
    pub fn commit_text(&self, text: &str) {
        self.link.send(&Request::CommitText {
            page: self.page,
            text: text.to_owned(),
        });
    }

    /// Turns on or off the mode in which a click on the page picks the element under the pointer.
    pub fn pick(&self, on: bool) {
        self.link.send(&Request::Pick {
            page: self.page,
            on,
        });
    }

    /// The pick the helper reported since the last call: the element, or nothing inside when the mode ended without one.
    pub fn take_pick(&self) -> Option<Option<PickedElement>> {
        self.state.take_pick()
    }

    /// Asks for a picture of the page, or its console, to add to the prompt; the answer comes from [`View::take_captures`].
    pub fn capture(&self, kind: CaptureKind) {
        self.link.send(&Request::Capture {
            page: self.page,
            kind,
        });
    }

    /// Turns on or off the mode in which the reader drags a rectangle over the page to take a picture of it.
    pub fn pick_area(&self, on: bool) {
        self.link.send(&Request::PickArea {
            page: self.page,
            on,
        });
    }

    /// What the helper reported for the reader's additions to the prompt since the last call.
    pub fn take_captures(&self) -> Vec<Capture> {
        self.state.take_captures()
    }

    /// Runs the tool `tool` of an agent against the page, given `arguments` as JSON text; the answer comes from [`View::take_tool_results`].
    pub fn tool(&self, call: u64, tool: &str, arguments: &str) {
        self.link.send(&Request::Tool {
            page: self.page,
            call,
            tool: tool.to_owned(),
            arguments: arguments.to_owned(),
        });
    }

    /// The tool calls that ended since the last call.
    pub fn take_tool_results(&self) -> Vec<ToolResult> {
        self.state.take_tools()
    }

    /// Gives the helper the reader's answer to the question `id` of the page; the answer holds for that origin until the helper ends.
    pub fn answer_permission(&self, id: u64, allow: bool) {
        self.link.send(&Request::PermissionAnswer {
            page: self.page,
            id,
            allow,
        });
    }

    /// Tells the helper what the reader chose on a site whose certificate is not trusted: to load it anyway, for that origin until the helper ends, or to go back.
    pub fn certificate_choice(&self, proceed: bool) {
        self.link.send(&Request::CertificateChoice {
            page: self.page,
            proceed,
        });
    }

    /// What the reader is to see or answer, reported since the last call.
    pub fn take_notices(&self) -> Vec<Notice> {
        self.state.take_notices()
    }

    /// Cuts, copies or pastes in the page, or marks everything in it.
    pub fn edit(&self, action: EditAction) {
        self.link.send(&Request::Edit {
            page: self.page,
            action,
        });
    }

    /// Looks for `text` in the page; `again` steps to the next match of the text already searched for, forward or, when `forward` is false, back.
    pub fn find(&self, text: &str, forward: bool, again: bool) {
        self.link.send(&Request::Find {
            page: self.page,
            text: text.to_owned(),
            forward,
            again,
        });
    }

    /// Ends the search and clears the marks.
    pub fn stop_find(&self) {
        self.link.send(&Request::StopFind { page: self.page });
    }

    /// The count the helper reported since the last call: the search's current match, counting from 1 and 0 for none, and how many matches there are.
    pub fn take_found(&self) -> Option<(u32, u32)> {
        self.state.found.lock().ok()?.take()
    }

    /// Changes the page's zoom.
    pub fn zoom(&self, action: ZoomAction) {
        self.link.send(&Request::Zoom {
            page: self.page,
            action,
        });
    }

    /// The page's zoom as a factor, 1.0 being 100%, when the helper reported it since the last call.
    pub fn take_zoom(&self) -> Option<f32> {
        self.state.zoom.lock().ok()?.take()
    }

    /// Loads the page again without using the cache.
    pub fn hard_reload(&self) {
        self.link.send(&Request::HardReload { page: self.page });
    }

    /// Throws away the cookies, site storage and cache of the helper's session, then loads the page again.
    pub fn clear_data(&self) {
        self.link.send(&Request::ClearData { page: self.page });
    }

    /// The newest picture of the page, once it has painted.
    pub fn frame(&self) -> Option<Frame> {
        self.state.frame()
    }

    /// The page's title, address and pointer shape.
    fn about(&self) -> (String, String, Cursor) {
        self.state
            .about
            .lock()
            .map(|about| about.clone())
            .unwrap_or_default()
    }

    /// The page's title.
    pub fn title(&self) -> String {
        self.about().0
    }

    /// The page's address.
    pub fn address(&self) -> String {
        self.about().1
    }

    /// How the pointer should look over the page.
    pub fn cursor(&self) -> Cursor {
        self.about().2
    }
}

impl Drop for View {
    /// Closes the page.
    fn drop(&mut self) {
        self.link.send(&Request::Close { page: self.page });
        self.link.forget(self.page);
    }
}
