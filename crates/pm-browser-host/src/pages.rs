//! The pages the window asked for, and carrying out its requests on them.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use cef::*;
use pm_browser::wire::Request;
use pm_browser::{Button, EditAction, Key};

use crate::devtools::find_tools;
use crate::handler::{Out, Page, client_for};

/// A request for developer tools that waits for the engine to say where they are served.
struct PendingTools {
    /// The page the tools are shown in.
    page: u32,
    /// Where the lookup will answer.
    lookup: Receiver<Option<String>>,
    /// The size the tools' page was asked for.
    size: (u32, u32, f32),
}

/// Every open page.
pub(crate) struct Pages {
    /// The open pages by number.
    open: HashMap<u32, Arc<Page>>,
    /// Where reports go.
    out: Arc<Out>,
    /// The folder shared files are made in.
    folder: PathBuf,
    /// The port developer tools are served on.
    port: u16,
    /// Developer tools being looked up.
    pending: Vec<PendingTools>,
}

/// The engine's mouse event for a position and the modifiers held.
fn mouse(x: i32, y: i32, modifiers: u32) -> MouseEvent {
    MouseEvent { x, y, modifiers }
}

/// The engine's window description of a page with no window of its own.
fn offscreen() -> WindowInfo {
    WindowInfo {
        windowless_rendering_enabled: true as _,
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

impl Pages {
    /// No pages yet; reports go to `out`, shared files to `folder`, tools are served on `port`.
    pub(crate) fn new(out: Arc<Out>, folder: PathBuf, port: u16) -> Self {
        Self {
            open: HashMap::new(),
            out,
            folder,
            port,
            pending: Vec::new(),
        }
    }

    /// Opens `url` as page `id`.
    fn open(&mut self, id: u32, url: &str, width: u32, height: u32, scale: f32) {
        let page = Page::new(
            id,
            self.out.clone(),
            self.folder.clone(),
            width,
            height,
            scale,
        );
        let mut client = client_for(&page);
        let browser = browser_host_create_browser_sync(
            Some(&offscreen()),
            Some(&mut client),
            Some(&url.into()),
            Some(&settings()),
            None,
            None,
        );
        if let Some(browser) = browser {
            page.set_browser(browser);
            self.open.insert(id, page);
        }
    }

    /// The engine's host of page `id`.
    fn host(&self, id: u32) -> Option<BrowserHost> {
        self.open.get(&id)?.browser()?.host()
    }

    /// Opens the tools whose lookup has answered.
    pub(crate) fn poll_tools(&mut self) {
        let mut waiting = Vec::new();
        for pending in std::mem::take(&mut self.pending) {
            match pending.lookup.try_recv() {
                Ok(Some(url)) => {
                    let (width, height, scale) = pending.size;
                    self.open(pending.page, &url, width, height, scale);
                }
                Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
                Err(std::sync::mpsc::TryRecvError::Empty) => waiting.push(pending),
            }
        }
        self.pending = waiting;
    }

    /// Carries out `request`; says whether to go on.
    pub(crate) fn handle(&mut self, request: Request) -> bool {
        match request {
            Request::Open {
                page,
                url,
                width,
                height,
                scale,
            } => self.open(page, &url, width, height, scale),
            Request::OpenTools {
                page,
                of,
                width,
                height,
                scale,
            } => {
                if let Some(target) = self.open.get(&of) {
                    self.pending.push(PendingTools {
                        page,
                        lookup: find_tools(self.port, target.address()),
                        size: (width, height, scale),
                    });
                }
            }
            Request::Load { page, url } => {
                if let Some(frame) = self
                    .open
                    .get(&page)
                    .and_then(|p| p.browser())
                    .and_then(|b| b.main_frame())
                {
                    frame.load_url(Some(&url.as_str().into()));
                }
            }
            Request::Back { page } => {
                if let Some(browser) = self.open.get(&page).and_then(|p| p.browser()) {
                    browser.go_back();
                }
            }
            Request::Forward { page } => {
                if let Some(browser) = self.open.get(&page).and_then(|p| p.browser()) {
                    browser.go_forward();
                }
            }
            Request::Reload { page } => {
                if let Some(browser) = self.open.get(&page).and_then(|p| p.browser()) {
                    browser.reload();
                }
            }
            Request::Resize {
                page,
                width,
                height,
                scale,
            } => {
                if let Some(state) = self.open.get(&page) {
                    state.set_geometry(width, height, scale);
                }
                if let Some(host) = self.host(page) {
                    host.was_resized();
                }
            }
            Request::Focus { page, focused } => {
                if let Some(host) = self.host(page) {
                    host.set_focus(focused as _);
                }
            }
            Request::PointerMoved {
                page,
                x,
                y,
                modifiers,
                left,
            } => {
                if let Some(host) = self.host(page) {
                    host.send_mouse_move_event(Some(&mouse(x, y, modifiers)), left as _);
                }
            }
            Request::PointerButton {
                page,
                x,
                y,
                button,
                pressed,
                clicks,
                modifiers,
            } => {
                let kind = match button {
                    Button::Left => MouseButtonType::LEFT,
                    Button::Middle => MouseButtonType::MIDDLE,
                    Button::Right => MouseButtonType::RIGHT,
                };
                if let Some(host) = self.host(page) {
                    host.send_mouse_click_event(
                        Some(&mouse(x, y, modifiers)),
                        kind,
                        (!pressed) as _,
                        clicks.max(1),
                    );
                }
            }
            Request::Scrolled {
                page,
                x,
                y,
                dx,
                dy,
                modifiers,
            } => {
                if let Some(host) = self.host(page) {
                    host.send_mouse_wheel_event(Some(&mouse(x, y, modifiers)), dx, dy);
                }
            }
            Request::Key { page, key } => {
                if let Some(host) = self.host(page) {
                    send_key(&host, &key);
                }
            }
            Request::CommitText { page, text } => {
                if let Some(host) = self.host(page) {
                    host.ime_commit_text(Some(&text.as_str().into()), None, 0);
                }
            }
            Request::Edit { page, action } => {
                if let Some(frame) = self
                    .open
                    .get(&page)
                    .and_then(|p| p.browser())
                    .and_then(|b| b.focused_frame().or_else(|| b.main_frame()))
                {
                    match action {
                        EditAction::Cut => frame.cut(),
                        EditAction::Copy => frame.copy(),
                        EditAction::Paste => frame.paste(),
                        EditAction::SelectAll => frame.select_all(),
                    }
                }
            }
            Request::Close { page } => {
                if let Some(host) = self.host(page) {
                    host.close_browser(true as _);
                }
                self.open.remove(&page);
            }
            Request::Shutdown => {
                for page in std::mem::take(&mut self.open).into_values() {
                    if let Some(host) = page.browser().and_then(|b| b.host()) {
                        host.close_browser(true as _);
                    }
                }
                return false;
            }
        }
        true
    }
}

/// Sends a key press or release, and the text it typed, to `host`.
fn send_key(host: &BrowserHost, key: &Key) {
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
