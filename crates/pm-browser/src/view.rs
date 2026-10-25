//! One web page, painted by the helper and shown here as pictures.

use std::sync::Arc;

use crate::engine::Engine;
use crate::error::Error;
use crate::frame::Frame;
use crate::input::{Button, Cursor, Key, Modifiers};
use crate::link::{Link, PageState};
use crate::wire::{EditAction, PickedElement, Request};

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

    /// Cuts, copies or pastes in the page, or marks everything in it.
    pub fn edit(&self, action: EditAction) {
        self.link.send(&Request::Edit {
            page: self.page,
            action,
        });
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
