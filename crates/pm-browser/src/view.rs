//! One web page, painted by the helper and shown here as pictures.

use std::sync::Arc;

use crate::chrome::{PageAction, PageReport};
use crate::engine::Engine;
use crate::error::Error;
use crate::frame::Frame;
use crate::input::{Button, Cursor, Key, Modifiers};
use crate::life::{Lifecycle, Popup};
use crate::link::{Copied, Link, Notice, PageState};
use crate::wire::{
    Caller, Capture, CaptureKind, EditAction, PickedElement, Request, ToolResult, ZoomAction,
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
        Self::open_in(engine, url, (width, height, scale), None)
    }

    /// Opens `url` in a page of `width` by `height` logical pixels shown at display `scale`, in the engine's profile `partition`, or the shared one when there is none.
    ///
    /// # Errors
    ///
    /// [`Error::Helper`] when the helper failed, or when a profile is named and the helper cannot open pages in one, as helpers released before profiles cannot: such a page would land in the shared profile.
    pub fn open_in(
        engine: &Engine,
        url: &str,
        (width, height, scale): (u32, u32, f32),
        partition: Option<&str>,
    ) -> Result<Self, Error> {
        if let Some(failure) = engine.failure() {
            return Err(Error::Helper(failure));
        }
        if partition.is_some() && !engine.partitions() {
            return Err(Error::Helper(
                "This version of the browser cannot keep a tab's data apart. Update the embedded browser in Settings to use private tabs and project profiles.".to_owned(),
            ));
        }
        let link = engine.link();
        let (page, state) = link.register();
        link.send(&Request::Open {
            page,
            url: url.to_owned(),
            width,
            height,
            scale,
            partition: partition.map(str::to_owned),
        });
        Ok(Self { page, link, state })
    }

    /// Opens the document `html` in a page of `width` by `height` logical pixels shown at display `scale`, sealed from the network: its scripts run, it reaches nothing outside itself and the tools of agents do not work on it. It is called `title` while the document names nothing.
    pub fn open_html(
        engine: &Engine,
        html: &str,
        title: &str,
        (width, height, scale): (u32, u32, f32),
    ) -> Result<Self, Error> {
        if let Some(failure) = engine.failure() {
            return Err(Error::Helper(failure));
        }
        let link = engine.link();
        let (page, state) = link.register();
        link.send(&Request::OpenHtml {
            page,
            html: html.to_owned(),
            title: title.to_owned(),
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

    /// Scrolls by `dx`, `dy` whole logical pixels with the pointer at `x`, `y`, as the notches of a wheel do; positive `dy` scrolls up.
    pub fn scrolled(&self, x: i32, y: i32, dx: i32, dy: i32, modifiers: Modifiers) {
        self.scrolled_exactly(x, y, (f64::from(dx), f64::from(dy)), false, modifiers);
    }

    /// Scrolls by `by`, across and down in logical pixels and parts of one, with the pointer at `x`, `y`; positive down scrolls up. `precise` says the distance came from a trackpad or another device that scrolls by the pixel, not from the notches of a wheel.
    pub fn scrolled_exactly(
        &self,
        x: i32,
        y: i32,
        by: (f64, f64),
        precise: bool,
        modifiers: Modifiers,
    ) {
        self.link.send(&Request::Scrolled {
            page: self.page,
            x,
            y,
            dx: by.0.trunc() as i32,
            dy: by.1.trunc() as i32,
            modifiers: modifiers.0,
            exact_dx: Some(by.0),
            exact_dy: Some(by.1),
            precise,
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

    /// Shows `text` as what an input method is composing, replacing what it showed before; empty `text` ends the composition without typing.
    pub fn compose_text(&self, text: &str) {
        self.link.send(&Request::Compose {
            page: self.page,
            text: text.to_owned(),
        });
    }

    /// Where the page last said its caret or the text being composed is: logical pixels from its top left, and the width and height.
    pub fn caret(&self) -> Option<(i32, i32, i32, i32)> {
        self.state.caret()
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
    ///
    /// `caller` names the agent session that made the call, whose recording keeps it.
    pub fn tool(&self, call: u64, tool: &str, arguments: &str, caller: &Caller) {
        self.link.send(&Request::Tool {
            page: self.page,
            call,
            tool: tool.to_owned(),
            arguments: arguments.to_owned(),
            session: caller.session.clone(),
            session_name: caller.name.clone(),
        });
    }

    /// Has the recording of `caller`'s session keep the call `call` of `tool`, which the window answered itself with `ok` and `text`, with a screenshot of this page after it when `picture`; the step comes from [`View::take_tool_results`] when pictured, and nothing comes when the session is not recorded. Without `picture` the helper is only reached through this page, the step is kept without a picture and the window is not told of it.
    pub fn record_step(
        &self,
        call: u64,
        tool: &str,
        arguments: &str,
        (ok, text): (bool, &str),
        caller: &Caller,
        picture: bool,
    ) {
        self.link.send(&Request::RecordStep {
            page: if picture { self.page } else { 0 },
            call,
            tool: tool.to_owned(),
            arguments: arguments.to_owned(),
            ok,
            text: text.to_owned(),
            session: caller.session.clone(),
            session_name: caller.name.clone(),
        });
    }

    /// The tool calls that ended since the last call.
    pub fn take_tool_results(&self) -> Vec<ToolResult> {
        self.state.take_tools()
    }

    /// Gives the helper the reader's answer to the question `id` of the page; the answer holds for that origin in the page's profile until the helper ends, or until the profile closes when it is kept in memory only, such as the private one.
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

    /// The text the reader copied or cut in the page since the last call, oldest first, for the window to put on its clipboard.
    pub fn take_copied(&self) -> Vec<Copied> {
        self.state.take_copied()
    }

    /// Puts `text` where the page's selection is, in place of what is marked, as pasting the window's clipboard does.
    pub fn paste_text(&self, text: &str) {
        self.paste_rich(text, None, &[]);
    }

    /// Pastes what the window's clipboard held into the page: `text`, with the rich text `html` and the files at `paths` when it held them, which the page is offered first as a paste event of its own, so rich editors keep the formatting and upload fields take the files; a page that does not take them gets the HTML in an editable region and the text in a plain field.
    pub fn paste_rich(&self, text: &str, html: Option<&str>, paths: &[String]) {
        self.link.send(&Request::PasteText {
            page: self.page,
            text: text.to_owned(),
            html: html.map(str::to_owned),
            files: paths.to_vec(),
        });
    }

    /// Cuts, copies, pastes, undoes or redoes in the page, or marks everything in it.
    pub fn edit(&self, action: EditAction) {
        self.link.send(&Request::Edit {
            page: self.page,
            action,
        });
    }

    /// Does `action` of the reader's on the page from the pane around it.
    pub fn act(&self, action: PageAction) {
        self.link.send(&Request::Chrome {
            page: self.page,
            action,
        });
    }

    /// What the page reported for the pane around it since the last call, oldest first.
    pub fn take_reports(&self) -> Vec<PageReport> {
        self.state.take_reports()
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

    /// The new windows the page asked for since the last call, each the address and whether a tool of an agent was running on the page when it asked.
    pub fn take_opened(&self) -> Vec<(String, bool)> {
        self.state.take_opened()
    }

    /// Whether the page has painted at least once, which a page that never loads does not.
    pub fn painted(&self) -> bool {
        self.state.painted()
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
    /// Stops the page loading.
    pub fn stop(&self) {
        self.link.send(&Request::Stop { page: self.page });
    }

    /// Where the page is in its loading, history and health, as the helper last said.
    pub fn lifecycle(&self) -> Lifecycle {
        self.state
            .life
            .lock()
            .map(|life| life.clone())
            .unwrap_or_default()
    }

    /// The windows the page opened with `window.open` since the last call; each waits for [`View::adopt_popup`] or [`View::drop_popup`].
    pub fn take_popups(&self) -> Vec<Popup> {
        self.state.take_popups()
    }

    /// Makes the window `popup` this page opened a page of its own, `width` by `height` logical pixels shown at display `scale`, which keeps its `window.opener`.
    pub fn adopt_popup(&self, popup: u64, width: u32, height: u32, scale: f32) -> Self {
        let (page, state) = self.link.register();
        self.link.send(&Request::AdoptPopup {
            page,
            popup,
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

    /// Closes the window `popup` this page opened, which will not be shown.
    pub fn drop_popup(&self, popup: u64) {
        self.link.send(&Request::DropPopup { popup });
    }
}

impl Drop for View {
    /// Closes the page.
    fn drop(&mut self) {
        self.link.send(&Request::Close { page: self.page });
        self.link.forget(self.page);
    }
}
