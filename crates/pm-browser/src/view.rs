//! One web page, drawn offscreen.

use std::sync::Arc;

use cef::*;

use crate::engine::Engine;
use crate::error::Error;
use crate::frame::{Frame, PaintStats};
use crate::handler::{Shared, client_for};
use crate::input::{Button, Cursor, Key, Modifiers};

/// A page open in the engine, painted into [`Frame`]s instead of a window.
///
/// A view must be dropped before the [`Engine`] that opened it.
pub struct View {
    /// The size the page is told and the paints and reports it sends back.
    shared: Arc<Shared>,
}

/// The engine's name for a mouse position and the modifiers held.
fn mouse(x: i32, y: i32, modifiers: Modifiers) -> MouseEvent {
    MouseEvent {
        x,
        y,
        modifiers: modifiers.0,
    }
}

/// The window description of a page that has no window of its own.
fn offscreen(engine: &Engine) -> WindowInfo {
    WindowInfo {
        windowless_rendering_enabled: true as _,
        shared_texture_enabled: engine.shared_texture as _,
        ..Default::default()
    }
}

/// The settings of a page that paints at display rate.
fn settings() -> BrowserSettings {
    BrowserSettings {
        windowless_frame_rate: 60,
        ..Default::default()
    }
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
        let shared = Shared::new(width, height, scale);
        let mut client = client_for(&shared);
        let browser = browser_host_create_browser_sync(
            Some(&offscreen(engine)),
            Some(&mut client),
            Some(&url.into()),
            Some(&settings()),
            None,
            None,
        )
        .ok_or(Error::Page)?;
        shared.set_browser(browser);
        Ok(Self { shared })
    }

    /// The engine's handle on the page's host, once the page exists.
    fn host(&self) -> Option<BrowserHost> {
        self.shared.browser()?.host()
    }

    /// Navigates the page to `url`.
    pub fn load(&self, url: &str) {
        if let Some(frame) = self
            .shared
            .browser()
            .and_then(|browser| browser.main_frame())
        {
            frame.load_url(Some(&url.into()));
        }
    }

    /// Goes back one page in the history.
    pub fn back(&self) {
        if let Some(browser) = self.shared.browser() {
            browser.go_back();
        }
    }

    /// Goes forward one page in the history.
    pub fn forward(&self) {
        if let Some(browser) = self.shared.browser() {
            browser.go_forward();
        }
    }

    /// Loads the page again.
    pub fn reload(&self) {
        if let Some(browser) = self.shared.browser() {
            browser.reload();
        }
    }

    /// Resizes the page to `width` by `height` logical pixels at display `scale`.
    pub fn resize(&self, width: u32, height: u32, scale: f32) {
        self.shared.set_geometry(width, height, scale);
        if let Some(host) = self.host() {
            host.was_resized();
        }
    }

    /// Tells the page whether it has the keyboard.
    pub fn focus(&self, focused: bool) {
        if let Some(host) = self.host() {
            host.set_focus(focused as _);
        }
    }

    /// Moves the pointer to `x`, `y` logical pixels from the page's corner, or out of it when `left`.
    pub fn pointer_moved(&self, x: i32, y: i32, modifiers: Modifiers, left: bool) {
        if let Some(host) = self.host() {
            host.send_mouse_move_event(Some(&mouse(x, y, modifiers)), left as _);
        }
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
        let kind = match button {
            Button::Left => MouseButtonType::LEFT,
            Button::Middle => MouseButtonType::MIDDLE,
            Button::Right => MouseButtonType::RIGHT,
        };
        if let Some(host) = self.host() {
            host.send_mouse_click_event(
                Some(&mouse(x, y, modifiers)),
                kind,
                (!pressed) as _,
                clicks.max(1),
            );
        }
    }

    /// Scrolls by `dx`, `dy` logical pixels with the pointer at `x`, `y`; positive `dy` scrolls up.
    pub fn scrolled(&self, x: i32, y: i32, dx: i32, dy: i32, modifiers: Modifiers) {
        if let Some(host) = self.host() {
            host.send_mouse_wheel_event(Some(&mouse(x, y, modifiers)), dx, dy);
        }
    }

    /// Sends a key press or release, and the text it typed.
    pub fn key(&self, key: &Key) {
        let Some(host) = self.host() else {
            return;
        };
        let event = |kind: KeyEventType, character: u16| KeyEvent {
            size: std::mem::size_of::<KeyEvent>(),
            type_: kind,
            modifiers: key.modifiers.0,
            windows_key_code: key.code,
            native_key_code: 0,
            is_system_key: 0,
            character,
            unmodified_character: character,
            focus_on_editable_field: 0,
        };
        if !key.pressed {
            host.send_key_event(Some(&event(KeyEventType::KEYUP, 0)));
            return;
        }
        host.send_key_event(Some(&event(KeyEventType::RAWKEYDOWN, 0)));
        for unit in key.text.iter().flat_map(|text| text.encode_utf16()) {
            host.send_key_event(Some(&event(KeyEventType::CHAR, unit)));
        }
    }

    /// Types `text` as finished input, as an input method does.
    pub fn commit_text(&self, text: &str) {
        if let Some(host) = self.host() {
            host.ime_commit_text(Some(&text.into()), None, 0);
        }
    }

    /// The newest software picture of the page, once it has painted.
    pub fn frame(&self) -> Option<Frame> {
        self.shared.frame()
    }

    /// The page's title.
    pub fn title(&self) -> String {
        self.shared.about().0
    }

    /// The page's address.
    pub fn address(&self) -> String {
        self.shared.about().1
    }

    /// How the pointer should look over the page.
    pub fn cursor(&self) -> Cursor {
        self.shared.about().2
    }

    /// How many paints of each kind the page has delivered.
    pub fn stats(&self) -> PaintStats {
        self.shared.stats()
    }
}

impl Drop for View {
    /// Closes the page.
    fn drop(&mut self) {
        if let Some(host) = self.host() {
            host.close_browser(true as _);
        }
    }
}
