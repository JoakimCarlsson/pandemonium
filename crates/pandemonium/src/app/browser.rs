//! The browser pane in the window: the engine that paints pages, the pointer and keyboard sent into them, and the pictures drawn from what they paint.

use pm_components::areas::{BrowserEvent, ComposerEvent};
use pm_components::target::Target;
use std::time::{Duration, Instant};

use pm_browser::{
    Button as Mouse, CaptureKind, Cursor as Shape, DownloadState, EditAction, Engine, Error,
    Key as Typed, Modifiers as Held, Notice, PickedElement, View, ZoomAction,
};
use pm_components::browser::{Command, HOME, Pointer, ZoomStep};
use pm_components::browser_keys::{Shortcut, shortcut_of};
use pm_components::browser_links::{Route, route};
use pm_components::browser_menu::{BROWSER_MENU_HEIGHT, BrowserCommand};
use pm_components::browser_safety::{Answer, Certificate, Download, Question, Safety, Saving};
use pm_components::browser_tabs::{TabInfo, changes_page, locked_message};
use pm_components::elements::Element;
use pm_components::exhibit::{Helper, PREVIEW_NEEDS_BROWSER, PREVIEW_NEEDS_UPDATE, PreviewRequest};
use pm_components::figures::Drawing;

use super::pointer::{Button, PointerState};
use rustle::{Key, KeyEvent};

use crate::keys;
use pm_editor::{Action, Pressed};
use pm_pages::Screen;
use rustle::CursorIcon as Cursor;
use rustle::ImageData;
use rustle::Rect;

use super::App;

/// How often the engine is given time to work while a page is open.
pub(super) const PUMP: Duration = Duration::from_millis(4);

/// The engine and the pages it holds.
#[derive(Default)]
pub(super) struct Web {
    /// The engine, once a page was first opened.
    engine: Option<Engine>,
    /// The page of each tab of the browser that has one, by the tab's number.
    pages: std::collections::HashMap<u32, Shown>,
    /// The developer tools, with the number of the tab whose page they are of, while they are open.
    tools: Option<(u32, View)>,
    /// The size the window of the tools was last given, in logical pixels, and the scale.
    tools_sized: Option<(u32, u32, f32)>,
    /// The newest frame of the tools that was handed to the window.
    tools_shown: u64,
    /// The texture every frame of the page shown is drawn into, and the tools' one.
    textures: [rustle::ImageKey; 2],
    /// Which window the primary button went down in, so the release goes to it too.
    pressed_in: Option<Target>,
    /// The last press, for counting clicks.
    last_press: Option<(Instant, (i32, i32), u8)>,
    /// The tab being shown and which window had the keyboard when the pages were last told.
    focused: Option<(Option<u32>, Option<Target>)>,
    /// Whether the engine could not start.
    failed: bool,
    /// The download of the browser, while it runs, and whether it already failed.
    installing: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    /// Whether the last download failed, so it is not started again until the setting is turned on again.
    install_failed: bool,
    /// Whether the servers were last told this window can run browser tools.
    announced: bool,
    /// The number the last tool call got in this window.
    next_call: u64,
    /// The tool calls running, by this window's number, with the machine and the server's number.
    calls: std::collections::HashMap<u64, (Option<String>, u64)>,
    /// Whether the setting was on when last looked at.
    was_enabled: bool,
    /// What the download says it is doing, written by its thread.
    download_note: std::sync::Arc<std::sync::Mutex<String>>,
    /// The line last handed to the settings page.
    shown_note: Option<String>,
    /// What was last said of the install once it was no longer running.
    install_note: Option<String>,
    /// How the helper was when the placeholders of previews were last told.
    shown_helper: Helper,
    /// Why the helper could not be started, shown whenever a page is asked for.
    reason: Option<String>,
    /// The local port that carries each remote port, by machine and remote port.
    forwards: std::collections::HashMap<(String, u16), u16>,
    /// The restriction on agent browsing and the origins listed, as the helper was last told.
    sent_origins: Option<(bool, Vec<String>)>,
    /// The number the last render of a sealed document got in this window.
    next_render: u64,
    /// The renders of sealed documents the helper is at, by this window's number: the key the picture is kept under and the width of the viewport it is rendered at, in CSS pixels.
    renders: pm_browser::render::Waiting<(u64, u32)>,
    /// Whether the helper that runs, or ran last, said it cannot open or render sealed documents.
    outdated: bool,
    /// Whether the reader asked for the outdated helper to be downloaded again, which the next turn starts.
    update_wanted: bool,
    /// Whether the helper was downloaded again because it was outdated, so that one still outdated after that says why the update did not help.
    updated: bool,
    /// Whether the folder of rendered pictures was cleared of old ones since the window started.
    renders_pruned: bool,
}

/// The page of one tab and what the window last handed it.
struct Shown {
    /// The page.
    view: View,
    /// The size the window of the page was last given, in logical pixels, and the scale.
    sized: Option<(u32, u32, f32)>,
    /// The newest frame of the page that was handed to the window.
    shown: u64,
    /// Whether the page shows a sealed document, which is never sent to an address.
    sealed: bool,
    /// When a sealed page was opened, until it first painted; one that does not paint within [`pm_browser::render::ANSWER_WITHIN`] is given up on.
    opened_at: Option<Instant>,
}

/// What the placeholders of previews say when the helper is still too old for sealed documents after it was downloaded again: what to do instead, which depends on where the helper comes from.
fn still_outdated() -> String {
    if pm_browser::install::named() {
        return "The browser PANDEMONIUM_BROWSER_HELPER names cannot render sealed pages. Point it at helpers/electron-browser/pandemonium-browser-electron in a current checkout.".to_owned();
    }
    format!(
        "The browser released for {} cannot render sealed pages. In a checkout, run make install, or start the window with make run.",
        env!("CARGO_PKG_VERSION")
    )
}

/// The picture the helper rendered of a sealed document into the file at `path`, at a viewport `width` CSS pixels wide, or why it cannot be shown.
fn preview_drawing(path: &std::path::Path, width: u32) -> Result<Drawing, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("The preview could not be read: {error}"))?;
    let picture = crate::work::picture::decode(&bytes, false)?;
    let density = (picture.width() as f32 / width.max(1) as f32).max(1.0);
    Ok(Drawing { picture, density })
}

/// Whether an agent may send the browser to `url`: a web address or the blank page, never a local file or an internal page of the browser.
fn agent_may_open(url: &str) -> bool {
    let url = url.trim();
    let scheme = url
        .split_once(':')
        .map_or("", |(scheme, _)| scheme)
        .to_ascii_lowercase();
    url.eq_ignore_ascii_case("about:blank")
        || (matches!(scheme.as_str(), "http" | "https") && url[scheme.len()..].starts_with("://"))
}

/// The scheme, host and port of a web address, in lower case with the scheme's usual port left out; nothing when it is not a plain `http` or `https` address, or has a login, a backslash or a space in it.
fn origin_parts(address: &str) -> Option<(String, String, String)> {
    let address = address.trim();
    if address
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return None;
    }
    let (scheme, rest) = address.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.strip_prefix('[') {
        Some(inner) => {
            let (host, after) = inner.split_once(']')?;
            (
                format!("[{host}]"),
                after.strip_prefix(':').unwrap_or("").to_owned(),
            )
        }
        None => {
            let (host, port) = authority.split_once(':').unwrap_or((authority, ""));
            (host.to_owned(), port.to_owned())
        }
    };
    let usual = if scheme == "http" { "80" } else { "443" };
    let port = if port == usual { "" } else { port.as_str() };
    Some((scheme, host.to_ascii_lowercase(), port.to_owned()))
}

/// Whether the restriction on agent browsing lets an agent open `url`: always when it is off or the blank page; otherwise only an address of `localhost`, `127.0.0.1` or `[::1]`, or of an origin in `origins`, as `helpers/electron-browser/src/origins.js` decides for the tools the helper runs.
fn origin_allowed(url: &str, restricted: bool, origins: &[String]) -> bool {
    if !restricted || url.trim().eq_ignore_ascii_case("about:blank") {
        return true;
    }
    let Some((scheme, host, port)) = origin_parts(url) else {
        return false;
    };
    if matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]") {
        return true;
    }
    origins.iter().any(|line| {
        let line = line.trim();
        let candidates = if line.contains("://") {
            vec![line.to_owned()]
        } else {
            vec![format!("http://{line}"), format!("https://{line}")]
        };
        candidates
            .iter()
            .filter_map(|candidate| origin_parts(candidate))
            .any(|entry| entry == (scheme.clone(), host.clone(), port.clone()))
    })
}

/// Takes in one thing the helper reported that the reader is to see or answer.
fn apply_notice(safety: &mut Safety, notice: Notice) {
    match notice {
        Notice::Permission {
            id,
            origin,
            permission,
        } => safety.ask(Question {
            id,
            origin,
            permission,
        }),
        Notice::PermissionCancelled { id } => safety.cancel(id),
        Notice::Certificate(info) => {
            safety.certificate = Some(Certificate {
                url: info.url,
                error: info.error,
                subject: info.subject,
                issuer: info.issuer,
                fingerprint: info.fingerprint,
                expires: info.expires,
                confirming: false,
            });
        }
        Notice::CertificateCleared => safety.certificate = None,
        Notice::Download { name, state } => {
            safety.download = Some(Download {
                name,
                saving: match state {
                    DownloadState::Started => Saving::Started,
                    DownloadState::Finished => Saving::Finished,
                    DownloadState::Failed => Saving::Failed,
                },
            });
        }
    }
}

/// What an agent is told of the tabs: one line for each, with its number and what marks it.
fn tabs_text(tabs: &[TabInfo]) -> String {
    if tabs.is_empty() {
        return "No tabs are open.".to_owned();
    }
    tabs.iter()
        .map(|tab| {
            let marks: Vec<&str> = [
                (tab.target, "the tools' tab"),
                (tab.viewing, "shown to the reader"),
                (tab.agent, "opened by an agent"),
                (tab.locked, "locked by the reader"),
            ]
            .into_iter()
            .filter_map(|(on, mark)| on.then_some(mark))
            .collect();
            let marks = if marks.is_empty() {
                String::new()
            } else {
                format!(" ({})", marks.join(", "))
            };
            format!("Tab {}{marks}: {} - {}", tab.id, tab.title, tab.address)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The element the pane's page reported, as the prompt holds it.
fn element_of(picked: &PickedElement) -> Element {
    Element {
        selector: picked.selector.clone(),
        tag: picked.tag.clone(),
        attributes: picked.attributes.clone(),
        text: picked.text.clone(),
        styles: picked.styles.clone(),
        width: picked.width as f32,
        height: picked.height as f32,
        component: picked.component.clone(),
        props: picked.props.clone(),
        url: picked.url.clone(),
        title: picked.title.clone(),
    }
}

/// An address cut at its host and port: what comes before the host, the host, the port when written, and what follows.
struct Parts<'a> {
    /// The scheme and the two slashes.
    before: &'a str,
    /// The host name or address.
    host: &'a str,
    /// The port, when the address names one.
    port: Option<u16>,
    /// The path, query and fragment.
    after: &'a str,
}

/// `address` cut into its parts, if it is a web address with a host.
fn parts(address: &str) -> Option<Parts<'_>> {
    let start = address.find("://")? + 3;
    let (authority, after) = address[start..]
        .find(['/', '?', '#'])
        .map_or((&address[start..], ""), |end| {
            (&address[start..start + end], &address[start + end..])
        });
    let authority = authority.rsplit('@').next()?;
    let (host, port) = if authority.starts_with('[') {
        let close = authority.find(']')? + 1;
        (&authority[..close], authority[close..].strip_prefix(':'))
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    let port = match port {
        Some(text) => Some(text.parse().ok()?),
        None => None,
    };
    Some(Parts {
        before: &address[..start],
        host,
        port,
        after,
    })
}

/// Whether `host` names the machine the address is opened on.
fn is_local(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "0.0.0.0")
}

/// The engine's modifiers for the keyboard keys held and the buttons down.
fn held(keys: rustle::Modifiers, pointer: PointerState) -> Held {
    let mut bits = 0;
    if keys.shift {
        bits |= Held::SHIFT;
    }
    if keys.ctrl {
        bits |= Held::CONTROL;
    }
    if keys.alt {
        bits |= Held::ALT;
    }
    if keys.meta {
        bits |= Held::COMMAND;
    }
    if pointer.left {
        bits |= Held::LEFT_BUTTON;
    }
    if pointer.right {
        bits |= Held::RIGHT_BUTTON;
    }
    Held(bits)
}

/// The Windows virtual-key code of `key`, which is how the engine names keys, and the text it types.
fn typed(event: &KeyEvent, modifiers: Held) -> Typed {
    let plain = modifiers.0 & (Held::CONTROL | Held::ALT | Held::COMMAND) == 0;
    let (code, text): (i32, Option<String>) = match &event.key {
        Key::Character(_) if keys::is_space(event) => (0x20, plain.then(|| " ".to_owned())),
        Key::Character(_) => {
            let character = event.code_point().unwrap_or_default();
            let upper = character.to_ascii_uppercase();
            let code = match upper {
                'A'..='Z' | '0'..='9' => upper as i32,
                ';' | ':' => 0xBA,
                '=' | '+' => 0xBB,
                ',' | '<' => 0xBC,
                '-' | '_' => 0xBD,
                '.' | '>' => 0xBE,
                '/' | '?' => 0xBF,
                '`' | '~' => 0xC0,
                '[' | '{' => 0xDB,
                '\\' | '|' => 0xDC,
                ']' | '}' => 0xDD,
                '\'' | '"' => 0xDE,
                _ => 0,
            };
            (code, plain.then(|| character.to_string()))
        }
        Key::Enter => (0x0D, Some("\r".to_owned())),
        Key::Escape => (0x1B, None),
        Key::Tab => (0x09, plain.then(|| "\t".to_owned())),
        Key::Backspace => (0x08, None),
        Key::Delete => (0x2E, None),
        Key::Home => (0x24, None),
        Key::End => (0x23, None),
        Key::PageUp => (0x21, None),
        Key::PageDown => (0x22, None),
        Key::ArrowLeft => (0x25, None),
        Key::ArrowUp => (0x26, None),
        Key::ArrowRight => (0x27, None),
        Key::ArrowDown => (0x28, None),
        Key::F(number) => (0x6F + i32::from(*number), None),
        _ => (0, None),
    };
    Typed {
        pressed: event.pressed,
        code,
        text,
        modifiers,
    }
}

/// How the pointer looks in the window when the page asks for `shape`.
fn cursor_of(pointer: Pointer) -> Cursor {
    match pointer {
        Pointer::Arrow => Cursor::Default,
        Pointer::Hand => Cursor::Pointer,
        Pointer::Text => Cursor::Text,
        Pointer::Cross => Cursor::Crosshair,
        Pointer::Wait => Cursor::Wait,
        Pointer::ResizeHorizontal => Cursor::ColResize,
        Pointer::ResizeVertical => Cursor::RowResize,
    }
}

/// How a page's pointer shape is told to the window.
fn pointer_of(shape: Shape) -> Pointer {
    match shape {
        Shape::Arrow | Shape::Help => Pointer::Arrow,
        Shape::Hand => Pointer::Hand,
        Shape::Text => Pointer::Text,
        Shape::Cross => Pointer::Cross,
        Shape::Wait => Pointer::Wait,
        Shape::ResizeHorizontal => Pointer::ResizeHorizontal,
        Shape::ResizeVertical => Pointer::ResizeVertical,
    }
}

impl App {
    /// Where the window of the page or of the tools was drawn, if it was.
    fn browser_region(&self, id: Target) -> Option<Rect> {
        self.place(id)
    }

    /// Whether the question about clearing cookies and site data is open, which covers the whole window.
    fn browser_clearing(&self) -> bool {
        self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser_clearing()
        })
    }

    /// The window of a page the point `at` is over, and where in it, in logical pixels; nothing where the bar for finding or the question about clearing is drawn over the page.
    fn browser_under(&self, at: (f32, f32)) -> Option<(Target, (i32, i32))> {
        let inside = |rect: Rect| {
            at.0 >= rect.origin.x
                && at.1 >= rect.origin.y
                && at.0 < rect.origin.x + rect.size.width
                && at.1 < rect.origin.y + rect.size.height
        };
        if self
            .browser_region(Target::Browser(BrowserEvent::BrowserFindBar))
            .is_some_and(inside)
            || self.browser_clearing()
        {
            return None;
        }
        [
            Target::Browser(BrowserEvent::BrowserBody),
            Target::Browser(BrowserEvent::BrowserToolsBody),
        ]
        .into_iter()
        .find_map(|id| {
            let rect = self.browser_region(id)?;
            inside(rect).then_some((
                id,
                ((at.0 - rect.origin.x) as i32, (at.1 - rect.origin.y) as i32),
            ))
        })
    }

    /// The number of the tab being shown, while the browser is open.
    fn browser_active(&self) -> Option<u32> {
        self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser.as_ref().map(|browser| browser.active_id())
        })
    }

    /// The page of the tab being shown.
    fn browser_page(&self) -> Option<&View> {
        let active = self.browser_active()?;
        self.web.pages.get(&active).map(|shown| &shown.view)
    }

    /// The view that shows in the window `id`.
    fn browser_view_of(&self, id: Target) -> Option<&View> {
        match id {
            Target::Browser(BrowserEvent::BrowserBody) => self.browser_page(),
            Target::Browser(BrowserEvent::BrowserToolsBody) => {
                self.web.tools.as_ref().map(|(_, view)| view)
            }
            _ => None,
        }
    }

    /// Whether a page or its tools have the keyboard.
    fn browser_focused(&self) -> Option<Target> {
        self.focus.current().filter(|id| {
            *id == Target::Browser(BrowserEvent::BrowserBody)
                || *id == Target::Browser(BrowserEvent::BrowserToolsBody)
        })
    }

    /// Carries out `command` of the menu over the page.
    pub(super) fn browser_menu_command(&mut self, command: BrowserCommand) {
        self.model
            .update(|screen| *screen = std::mem::take(screen).browser_command(command));
        if command == BrowserCommand::Find {
            self.focus
                .focus(Target::Browser(BrowserEvent::BrowserFindField));
        } else {
            self.focus.focus(Target::Browser(BrowserEvent::BrowserBody));
        }
    }

    /// What the pages reported for the reader's additions to the prompt since the last call, each with the number of the tab it came from.
    pub(super) fn browser_reports(&self) -> Vec<(u32, pm_browser::Capture)> {
        self.web
            .pages
            .iter()
            .flat_map(|(id, shown)| {
                shown
                    .view
                    .take_captures()
                    .into_iter()
                    .map(|capture| (*id, capture))
            })
            .collect()
    }

    /// Where a menu over the page opens: just under the pointer and inside the window.
    pub(super) fn browser_menu_place(&self) -> Option<(f32, f32)> {
        let (x, y) = self.pointer.position?;
        let (width, height) = self.window_size();
        Some((
            (x - 8.0).min(width - 260.0).max(0.0),
            (y + 4.0).min(height - BROWSER_MENU_HEIGHT - 8.0).max(0.0),
        ))
    }

    /// Opens the menu over the page just under the pointer and inside the window.
    pub(super) fn open_browser_menu(&mut self) {
        let Some(at) = self.browser_menu_place() else {
            return;
        };
        self.model
            .update(|screen| *screen = std::mem::take(screen).with_browser_menu(at));
    }

    /// Opens the browser, or shows it when it is open.
    pub(super) fn open_browser(&mut self) {
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            state.open_browser(HOME);
        });
        self.focus_browser_address = true;
    }

    /// Sends the pointer's move into the page under it, or out of the page it left.
    pub(super) fn browser_moved(&mut self, at: (f32, f32)) {
        let modifiers = held(self.modifiers, self.pointer);
        match self.browser_under(at) {
            Some((id, (x, y))) => {
                if let Some(view) = self.browser_view_of(id) {
                    view.pointer_moved(x, y, modifiers, false);
                }
            }
            None => {
                for id in [
                    Target::Browser(BrowserEvent::BrowserBody),
                    Target::Browser(BrowserEvent::BrowserToolsBody),
                ] {
                    if let Some(view) = self.browser_view_of(id) {
                        view.pointer_moved(0, 0, modifiers, true);
                    }
                }
            }
        }
    }

    /// Sends a press or release of `button` into the page under the pointer, or into the one the press began in, with how many presses in a row it makes by rustle's default view configuration, as the window counts presses no area counts.
    pub(super) fn browser_button(&mut self, button: Button, pressed: bool) {
        let Some(at) = self.pointer.position else {
            return;
        };
        if pressed && self.model.read(Screen::list_menu_open) {
            return;
        }
        let target = if pressed {
            self.browser_under(at)
        } else {
            self.web.pressed_in.and_then(|id| {
                let rect = self.browser_region(id)?;
                Some((
                    id,
                    ((at.0 - rect.origin.x) as i32, (at.1 - rect.origin.y) as i32),
                ))
            })
        };
        let Some((id, (x, y))) = target else {
            return;
        };
        let clicks = if pressed {
            let configuration = rustle::ViewConfiguration::default();
            let within = Duration::from_millis(configuration.double_tap_timeout_millis);
            let near = |last: (i32, i32)| {
                ((last.0 - x) as f32).hypot((last.1 - y) as f32) < configuration.touch_slop
            };
            let count = match self.web.last_press {
                Some((when, last, count)) if when.elapsed() < within && near(last) => count + 1,
                _ => 1,
            };
            self.web.last_press = Some((Instant::now(), (x, y), count));
            self.web.pressed_in = Some(id);
            count
        } else {
            self.web.pressed_in = None;
            self.web.last_press.map_or(1, |(_, _, count)| count)
        };
        let button = match button {
            Button::Left => Mouse::Left,
            Button::Right => Mouse::Right,
        };
        let modifiers = held(self.modifiers, self.pointer);
        if let Some(view) = self.browser_view_of(id) {
            view.pointer_button(x, y, button, pressed, i32::from(clicks), modifiers);
        }
    }

    /// Scrolls the page under the pointer by `pixels` of wheel, and says whether there was one.
    pub(super) fn browser_wheel(&mut self, dx: f32, dy: f32) -> bool {
        let Some(at) = self.pointer.position else {
            return false;
        };
        let Some((id, (x, y))) = self.browser_under(at) else {
            return false;
        };
        let modifiers = held(self.modifiers, self.pointer);
        if let Some(view) = self.browser_view_of(id) {
            view.scrolled(x, y, dx as i32, dy as i32, modifiers);
        }
        true
    }

    /// Takes the mouse's back or forward button into the page, when the pointer is over the page or the page holds the keyboard and is on screen, and says whether it did.
    pub(super) fn browser_history(&mut self, back: bool) -> bool {
        if self
            .browser_region(Target::Browser(BrowserEvent::BrowserBody))
            .is_none()
        {
            return false;
        }
        let over = self
            .pointer
            .position
            .and_then(|at| self.browser_under(at))
            .is_some_and(|(id, _)| id == Target::Browser(BrowserEvent::BrowserBody));
        if !over && self.browser_focused() != Some(Target::Browser(BrowserEvent::BrowserBody)) {
            return false;
        }
        let Some(page) = self.browser_page() else {
            return false;
        };
        if back {
            page.back();
        } else {
            page.forward();
        }
        true
    }

    /// Types the text an input method committed into the page that holds the keyboard, and says whether one did.
    pub(super) fn commit_to_browser(&mut self, text: &str) -> bool {
        let Some(id) = self.browser_focused() else {
            return false;
        };
        if let Some(view) = self.browser_view_of(id) {
            view.commit_text(text);
        }
        true
    }

    /// Takes a key meant for what the pane draws over the page, and says whether it did: while the question about clearing is open Escape declines it and every other key is ignored; in the bar for finding Escape closes it, Shift+Enter goes to the previous match and the keys for finding work.
    fn browser_overlay_key(&mut self, event: &KeyEvent) -> bool {
        if self.browser_clearing() {
            if matches!(event.key, Key::Escape) {
                self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    state.browser_clear_declined();
                });
            }
            return true;
        }
        if self.focus.current() != Some(Target::Browser(BrowserEvent::BrowserFindField)) {
            return false;
        }
        let shortcut = match event.key {
            Key::Escape => {
                self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    state.browser_find_closed();
                });
                self.focus.focus(Target::Browser(BrowserEvent::BrowserBody));
                return true;
            }
            Key::Enter if event.modifiers.shift => Shortcut::FindPrevious,
            _ => match shortcut_of(event) {
                Some(shortcut @ (Shortcut::Find | Shortcut::FindNext | Shortcut::FindPrevious)) => {
                    shortcut
                }
                _ => return false,
            },
        };
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser_shortcut(shortcut);
        });
        true
    }

    /// Sends a key into the page that holds the keyboard, unless the browser takes it for itself, as for finding, zooming and loading again, or the keymap binds it to something the window does, and says whether it did.
    pub(super) fn browser_key(&mut self, event: &KeyEvent) -> bool {
        if self.browser_overlay_key(event) {
            return true;
        }
        let Some(id) = self.browser_focused() else {
            return false;
        };
        if id == Target::Browser(BrowserEvent::BrowserBody)
            && let Some(shortcut) = shortcut_of(event)
        {
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.browser_shortcut(shortcut);
            });
            if shortcut == Shortcut::Find {
                self.focus
                    .focus(Target::Browser(BrowserEvent::BrowserFindField));
            }
            return true;
        }
        let modifiers = event.modifiers;
        if (modifiers.ctrl || modifiers.alt || modifiers.meta)
            && let Some(chord) = Self::chord_of(event)
            && !matches!(
                self.keymap()
                    .press_among(&self.pending, &chord, Action::everywhere),
                Pressed::Nothing
            )
        {
            return false;
        }
        let key = typed(event, held(modifiers, self.pointer));
        if let Some(view) = self.browser_view_of(id) {
            view.key(&key);
        }
        true
    }

    /// Tells the page that holds the keyboard a key came up, as pages that listen for `keyup` or track held keys expect, unless the browser took the press for itself.
    pub(super) fn browser_key_released(&mut self, event: &KeyEvent) {
        if self.browser_clearing() {
            return;
        }
        let Some(id) = self.browser_focused() else {
            return;
        };
        if id == Target::Browser(BrowserEvent::BrowserBody) && shortcut_of(event).is_some() {
            return;
        }
        let key = typed(event, held(event.modifiers, self.pointer));
        if let Some(view) = self.browser_view_of(id) {
            view.key(&key);
        }
    }

    /// How the pointer looks while it is over a page, as the page asks.
    pub(super) fn browser_cursor(&self) -> Option<Cursor> {
        let at = self.pointer.position?;
        self.browser_under(at)?;
        self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state
                .browser
                .as_ref()
                .map(|browser| cursor_of(browser.pointer))
        })
    }

    /// The address to open for `address` of a page on `host`: a local one is carried to that machine's port, and what no transport can carry is said.
    fn browser_resolve(&mut self, address: &str, host: Option<&str>) -> Result<String, String> {
        let Some(host) = host else {
            return Ok(address.to_owned());
        };
        let Some(cut) = parts(address).filter(|cut| is_local(cut.host)) else {
            return Ok(address.to_owned());
        };
        let port = cut.port.unwrap_or(if cut.before.starts_with("https") {
            443
        } else {
            80
        });
        match pm_protocol::route_of(Some(host)) {
            pm_protocol::Route::Ssh(destination) => {
                let key = (destination.to_owned(), port);
                let local = match self.web.forwards.get(&key) {
                    Some(local) => *local,
                    None => {
                        let local = pm_ssh::forward(destination, port)?;
                        self.web.forwards.insert(key, local);
                        local
                    }
                };
                Ok(format!("{}127.0.0.1:{local}{}", cut.before, cut.after))
            }
            pm_protocol::Route::Direct(target) => {
                let machine = target.rsplit_once(':').map_or(target, |(name, _)| name);
                Ok(format!("{}{machine}:{port}{}", cut.before, cut.after))
            }
            pm_protocol::Route::Wsl(_) => Ok(address.to_owned()),
            _ => Err(
                "Pages of this kind of machine cannot be opened here yet: only ssh, tcp and WSL machines are supported."
                    .to_owned(),
            ),
        }
    }

    /// How the page shows its address: a local port that carries a remote one is shown as the remote port on `localhost`, and a page of the machine `host` reached directly is shown on `localhost` too, so the address typed, the address shown and the address a server reports are the same.
    fn browser_unresolve(&self, address: &str, host: Option<&str>) -> String {
        let Some(cut) = parts(address) else {
            return address.to_owned();
        };
        if let pm_protocol::Route::Direct(target) = pm_protocol::route_of(host) {
            let machine = target.rsplit_once(':').map_or(target, |(name, _)| name);
            if let Some(port) = cut.port.filter(|_| cut.host == machine) {
                return format!("{}localhost:{port}{}", cut.before, cut.after);
            }
        }
        let Some(local) = cut.port.filter(|_| is_local(cut.host)) else {
            return address.to_owned();
        };
        match self
            .web
            .forwards
            .iter()
            .find(|(_, forwarded)| **forwarded == local)
        {
            Some(((_, remote), _)) => format!("{}localhost:{remote}{}", cut.before, cut.after),
            None => address.to_owned(),
        }
    }

    /// The machine the page of the tab numbered `id` is on, when it is not this one.
    fn browser_tab_host(&self, id: u32) -> Option<String> {
        self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state
                .browser
                .as_ref()
                .and_then(|browser| browser.tab(id))
                .and_then(|tab| tab.host.clone())
        })
    }

    /// Tells the helper which origins agents may open when that is not what it was last told; the helper enforces it, so it is sent as soon as the helper starts, before any page or tool.
    fn browser_origins(&mut self) {
        let general = &self.saved_window.general;
        let wanted = (
            general.restrict_agent_browsing,
            general.agent_origins.clone(),
        );
        if self.web.sent_origins.as_ref() == Some(&wanted) {
            return;
        }
        if let Some(engine) = self.web.engine.as_ref() {
            engine.set_agent_origins(wanted.0, &wanted.1);
            self.web.sent_origins = Some(wanted);
        }
    }

    /// Takes in what the helper says the reader is to see or answer, in each tab, and carries the reader's answers back to the tab they are for.
    fn browser_safety(&mut self) {
        let notices: Vec<(u32, Vec<Notice>)> = self
            .web
            .pages
            .iter()
            .map(|(id, shown)| (*id, shown.view.take_notices()))
            .filter(|(_, notices)| !notices.is_empty())
            .collect();
        if !notices.is_empty() {
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                for (id, notices) in notices {
                    state.browser_scoped_to_tab(id, |state| {
                        if let Some(tab) = state
                            .browser
                            .as_mut()
                            .and_then(|browser| browser.tab_mut(id))
                        {
                            for notice in notices {
                                apply_notice(&mut tab.safety, notice);
                            }
                        }
                    });
                }
            });
        }
        let waiting = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser.as_ref().is_some_and(|browser| {
                browser
                    .tabs
                    .iter()
                    .any(|tab| !tab.safety.answers.is_empty())
            })
        });
        if !waiting {
            return;
        }
        let mut answers = Vec::new();
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            answers = state.take_browser_answers();
        });
        for (tab, answer) in answers {
            let Some(page) = self.web.pages.get(&tab).map(|shown| &shown.view) else {
                continue;
            };
            match answer {
                Answer::Permission { id, allow } => page.answer_permission(id, allow),
                Answer::Certificate { proceed } => page.certificate_choice(proceed),
            }
        }
    }

    /// Starts the helper when it is not running, and says why when it cannot be, every time it is asked.
    fn browser_engine(&mut self) -> Option<&Engine> {
        if self.web.engine.is_none() && !self.web.failed {
            if self.saved_window.general.embedded_browser {
                match Engine::start() {
                    Ok(engine) => {
                        self.web.outdated = !engine.sealed_pages();
                        if self.web.outdated && self.web.updated {
                            self.web.install_failed = true;
                            self.web.install_note = Some(still_outdated());
                        }
                        self.web.engine = Some(engine);
                        self.web.sent_origins = None;
                        self.browser_origins();
                    }
                    Err(error) => {
                        self.web.failed = true;
                        self.web.reason = Some(match error {
                            Error::NotInstalled => {
                                "The embedded browser is not installed. It is downloaded when you turn it on in Settings under Browser, and this build has no download for it yet.".to_owned()
                            }
                            other => other.to_string(),
                        });
                    }
                }
            } else {
                self.web.failed = true;
                self.web.reason = Some(
                    "The embedded browser is turned off. Turn it on in Settings under Browser to download it.".to_owned(),
                );
            }
        }
        if self.web.engine.is_none() {
            let reason = self
                .web
                .install_note
                .clone()
                .filter(|_| self.web.install_failed)
                .or_else(|| self.web.reason.clone())
                .unwrap_or_else(|| "The embedded browser could not be started.".to_owned());
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.browser_notice(&reason);
            });
        }
        self.web.engine.as_ref()
    }

    /// The size a window of a page was last drawn at, in logical pixels, or a size to start from.
    fn browser_size(&self, id: Target, scale: f32) -> (u32, u32, f32) {
        self.browser_region(id).map_or((960, 640, scale), |rect| {
            (
                rect.size.width.max(1.0) as u32,
                rect.size.height.max(1.0) as u32,
                scale,
            )
        })
    }

    /// Carries out what the window asked of the engine, each command in the tab it is for.
    fn browser_commands(&mut self, scale: f32) {
        let pending = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state
                .browser
                .as_ref()
                .is_some_and(|browser| browser.tabs.iter().any(|tab| !tab.commands.is_empty()))
        });
        if !pending {
            return;
        }
        let mut commands = Vec::new();
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            commands = state.take_browser_commands();
        });
        for (tab, command) in commands {
            match command {
                Command::Load(address) => self.browser_load(tab, &address, scale),
                Command::LoadSealed => self.browser_load_sealed(tab, scale),
                Command::OpenTools => {
                    let (width, height, scale) =
                        self.browser_size(Target::Browser(BrowserEvent::BrowserToolsBody), scale);
                    if let Some(page) = self.web.pages.get(&tab).map(|shown| &shown.view) {
                        self.web.tools =
                            Some((tab, page.open_devtools(width.max(1), height, scale)));
                        self.web.tools_sized = None;
                        self.web.tools_shown = 0;
                    }
                }
                Command::CloseTools => {
                    self.web.tools = None;
                    self.web.tools_sized = None;
                    self.web.tools_shown = 0;
                }
                Command::Reload | Command::HardReload if self.browser_sealed_without_page(tab) => {
                    self.browser_load_sealed(tab, scale);
                }
                other => {
                    if let Some(page) = self.web.pages.get(&tab).map(|shown| &shown.view) {
                        Self::browser_page_command(page, other);
                    }
                }
            }
        }
    }

    /// Loads `address` in the tab `tab`, opening its page first when it has none.
    fn browser_load(&mut self, tab: u32, address: &str, scale: f32) {
        let host = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state
                .browser
                .as_ref()
                .and_then(|browser| browser.tab(tab))
                .and_then(|tab| tab.host.clone())
        });
        let address = match self.browser_resolve(address, host.as_deref()) {
            Ok(address) => address,
            Err(message) => {
                self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    state.browser_tab_notice(tab, &message);
                });
                return;
            }
        };
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser_notice_cleared(tab);
        });
        match self.web.pages.get(&tab) {
            Some(shown) if !shown.sealed => {
                shown.view.load(&address);
                return;
            }
            Some(_) => {
                self.web.pages.remove(&tab);
                self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    state.browser_tab_unsealed(tab);
                });
            }
            None => {}
        }
        let size = self.browser_size(Target::Browser(BrowserEvent::BrowserBody), scale);
        let Some(engine) = self.browser_engine() else {
            return;
        };
        match View::open(engine, &address, size.0, size.1, size.2) {
            Ok(view) => {
                self.web.pages.insert(
                    tab,
                    Shown {
                        view,
                        sized: Some(size),
                        shown: 0,
                        sealed: false,
                        opened_at: None,
                    },
                );
            }
            Err(error) => self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.browser_tab_notice(tab, &error.to_string());
            }),
        }
    }

    /// Shows the sealed document the tab `tab` holds in a page of its own, in place of any page the tab had: the helper denies that page the network and the tools of agents.
    fn browser_load_sealed(&mut self, tab: u32, scale: f32) {
        let sealed = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state
                .browser
                .as_ref()
                .and_then(|browser| browser.tab(tab))
                .and_then(|tab| tab.sealed.clone())
        });
        let Some(sealed) = sealed else {
            return;
        };
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser_notice_cleared(tab);
        });
        self.web.pages.remove(&tab);
        let size = self.browser_size(Target::Browser(BrowserEvent::BrowserBody), scale);
        let Some(engine) = self.browser_engine() else {
            return;
        };
        if !engine.sealed_pages() {
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.browser_tab_notice(tab, &format!("{PREVIEW_NEEDS_UPDATE}: the installed browser cannot open pages sealed from the network."));
            });
            return;
        }
        match View::open_html(engine, &sealed.html, &sealed.title, size) {
            Ok(view) => {
                self.web.pages.insert(
                    tab,
                    Shown {
                        view,
                        sized: Some(size),
                        shown: 0,
                        sealed: true,
                        opened_at: Some(Instant::now()),
                    },
                );
            }
            Err(error) => self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.browser_tab_notice(tab, &error.to_string());
            }),
        }
    }

    /// Hands the window the picture of a sealed document kept under `key`, or why there is none.
    fn browser_previewed(&mut self, key: u64, drawn: Result<Drawing, String>) {
        self.model
            .update(|screen| *screen = std::mem::take(screen).with_figure(key, drawn));
    }

    /// Has the picture `request` asks for made: taken from the folder of rendered pictures when the document was rendered before, and otherwise rendered by the helper, sealed from the network, into that folder. Without the embedded browser the picture is answered with the line that says to install it.
    fn browser_preview(&mut self, request: PreviewRequest) {
        let PreviewRequest {
            key,
            html,
            width,
            height,
        } = request;
        if let Some(reason) = pm_browser::render::refusal(&html) {
            self.browser_previewed(key, Err(reason));
            return;
        }
        let file = pm_browser::render::cache_file(&html, width, height);
        if let Some(kept) = file.as_deref().filter(|file| file.is_file()) {
            self.browser_previewed(key, preview_drawing(kept, width));
            return;
        }
        let usable = self.saved_window.general.embedded_browser
            && pm_browser::install::supported()
            && pm_browser::install::installed();
        let (Some(file), true) = (file, usable) else {
            self.browser_previewed(key, Err(PREVIEW_NEEDS_BROWSER.to_owned()));
            return;
        };
        if !self.web.renders_pruned {
            self.web.renders_pruned = true;
            if let Some(folder) = file.parent() {
                pm_browser::render::prune(folder);
            }
        }
        self.web.next_render += 1;
        let ticket = self.web.next_render;
        let Some(engine) = self.browser_engine() else {
            let reason = self
                .web
                .reason
                .clone()
                .unwrap_or_else(|| "The embedded browser could not be started.".to_owned());
            self.browser_previewed(key, Err(reason));
            return;
        };
        if !engine.sealed_pages() {
            self.browser_previewed(key, Err(PREVIEW_NEEDS_UPDATE.to_owned()));
            return;
        }
        engine.render_html(ticket, &html, width, height, &file);
        self.web
            .renders
            .insert(ticket, (key, width), Instant::now());
    }

    /// Has the pictures of sealed documents the window asked for made, takes in the ones the helper finished, and says whether any is still being rendered, which wants the loop to turn again soon.
    fn browser_previews(&mut self) -> bool {
        let pending = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state.preview_requests_pending()
        });
        if pending {
            let mut asked = Vec::new();
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                asked = state.take_preview_requests();
            });
            for request in asked {
                self.browser_preview(request);
            }
        }
        if self.web.renders.is_empty() {
            return false;
        }
        let rendered = self
            .web
            .engine
            .as_ref()
            .map(Engine::take_rendered)
            .unwrap_or_default();
        for done in rendered {
            let Some((key, width)) = self.web.renders.answered(done.ticket) else {
                continue;
            };
            let drawn = done.outcome.and_then(|path| preview_drawing(&path, width));
            self.browser_previewed(key, drawn);
        }
        for (key, _) in self.web.renders.overdue(Instant::now()) {
            self.browser_previewed(key, Err(pm_browser::render::NO_ANSWER.to_owned()));
        }
        let stopped = match self.web.engine.as_ref() {
            Some(engine) => engine.failure(),
            None => Some("The embedded browser stopped.".to_owned()),
        };
        if let Some(reason) = stopped {
            for (key, _) in self.web.renders.drain() {
                self.browser_previewed(key, Err(reason.clone()));
            }
        }
        !self.web.renders.is_empty()
    }

    /// Gives up on the sealed pages that have not painted within [`pm_browser::render::ANSWER_WITHIN`] of being opened: each is closed and its tab says so, and reloading the tab opens it again.
    fn browser_sealed_overdue(&mut self) {
        let now = Instant::now();
        let late: Vec<u32> = self
            .web
            .pages
            .iter_mut()
            .filter_map(|(tab, shown)| {
                let opened = shown.opened_at?;
                if shown.view.painted() {
                    shown.opened_at = None;
                    return None;
                }
                (now.saturating_duration_since(opened) > pm_browser::render::ANSWER_WITHIN)
                    .then_some(*tab)
            })
            .collect();
        for tab in late {
            self.web.pages.remove(&tab);
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.browser_tab_notice(
                    tab,
                    "The browser did not show this page within 15 seconds. Reload the tab to try again.",
                );
            });
        }
    }

    /// Whether the tab `tab` holds a sealed document but has no page, as after its page was given up on, so that reloading it opens the document again.
    fn browser_sealed_without_page(&self, tab: u32) -> bool {
        !self.web.pages.contains_key(&tab)
            && self.model.read(|screen| {
                let Screen::Workspace(state) = screen;
                state
                    .browser
                    .as_ref()
                    .and_then(|browser| browser.tab(tab))
                    .is_some_and(|tab| tab.sealed.is_some())
            })
    }

    /// Carries out the command of a page that is open: all the ones that need only the page.
    fn browser_page_command(page: &View, command: Command) {
        match command {
            Command::Cut => page.edit(EditAction::Cut),
            Command::Copy => page.edit(EditAction::Copy),
            Command::Paste => page.edit(EditAction::Paste),
            Command::SelectAll => page.edit(EditAction::SelectAll),
            Command::Pick(on) => page.pick(on),
            Command::Screenshot => page.capture(CaptureKind::Screenshot),
            Command::FullPage => page.capture(CaptureKind::FullPage),
            Command::Console => page.capture(CaptureKind::Console),
            Command::PickArea(on) => page.pick_area(on),
            Command::Back => page.back(),
            Command::Forward => page.forward(),
            Command::Reload => page.reload(),
            Command::HardReload => page.hard_reload(),
            Command::Find {
                text,
                forward,
                again,
            } => page.find(&text, forward, again),
            Command::StopFind => page.stop_find(),
            Command::Zoom(step) => page.zoom(match step {
                ZoomStep::In => ZoomAction::In,
                ZoomStep::Out => ZoomAction::Out,
                ZoomStep::Reset => ZoomAction::Reset,
            }),
            Command::OpenExternal => {
                if let Err(reason) = pm_harness::open_address(&page.address()) {
                    eprintln!("{reason}");
                }
            }
            Command::ClearData => page.clear_data(),
            Command::Load(_) | Command::LoadSealed | Command::OpenTools | Command::CloseTools => {}
        }
    }

    /// Closes the pages of tabs that were closed, and all of them when the browser was.
    fn browser_sync_tabs(&mut self) {
        let open: Vec<u32> = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser_tab_numbers()
        });
        self.web.pages.retain(|id, _| open.contains(id));
        if self
            .web
            .tools
            .as_ref()
            .is_some_and(|(tab, _)| !open.contains(tab))
        {
            self.web.tools = None;
            self.web.tools_sized = None;
            self.web.tools_shown = 0;
        }
    }

    /// Closes the pages and ends the helper, so the process can leave.
    pub(super) fn browser_shutdown(&mut self) {
        self.web.tools = None;
        self.web.pages.clear();
        self.web.engine = None;
    }

    /// Gives the page being shown and the tools the size of the room the pane has for them.
    fn browser_fit(&mut self, scale: f32) {
        let size_of = |rect: Rect| {
            (
                rect.size.width.max(1.0) as u32,
                rect.size.height.max(1.0) as u32,
                scale,
            )
        };
        if let Some(rect) = self.browser_region(Target::Browser(BrowserEvent::BrowserBody))
            && let Some(active) = self.browser_active()
            && let Some(shown) = self.web.pages.get_mut(&active)
        {
            let size = size_of(rect);
            if shown.sized != Some(size) {
                shown.sized = Some(size);
                shown.view.resize(size.0, size.1, size.2);
            }
        }
        if let Some(rect) = self.browser_region(Target::Browser(BrowserEvent::BrowserToolsBody))
            && let Some((_, view)) = self.web.tools.as_ref()
        {
            let size = size_of(rect);
            if self.web.tools_sized != Some(size) {
                self.web.tools_sized = Some(size);
                view.resize(size.0, size.1, size.2);
            }
        }
    }

    /// Takes in what each page says about itself, hands the window what the page being shown and the tools painted since it last looked, opens the tabs the pages asked for, and tells the pages which of them has the keyboard.
    fn browser_present(&mut self) {
        let ids: Vec<u32> = self.web.pages.keys().copied().collect();
        for id in &ids {
            let Some(view) = self.web.pages.get(id).map(|shown| &shown.view) else {
                continue;
            };
            let picked = view.take_pick();
            let found = view.take_found();
            let zoom = view.take_zoom();
            let opened = view.take_opened();
            if let Some(result) = picked {
                let element = result.as_ref().map(element_of);
                let picked = element.is_some();
                self.model.update(|screen| {
                    *screen = std::mem::take(screen).browser_picked(*id, element.clone());
                });
                if picked {
                    self.focus.focus(Target::Composer(ComposerEvent::Prompt));
                }
            }
            if found.is_some() || zoom.is_some() {
                self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    state.browser_scoped_to_tab(*id, |state| {
                        if let Some((active, total)) = found {
                            state.browser_found(*id, active, total);
                        }
                        if let Some(zoom) = zoom {
                            state.browser_zoomed(*id, zoom);
                        }
                    });
                });
            }
            for (url, agent) in opened {
                let host = self.browser_tab_host(*id);
                let address = self.browser_unresolve(&url, host.as_deref());
                self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    state.browser_scoped_to_tab(*id, |state| {
                        state.browser_tab_opened(&address, host, agent, !agent);
                    });
                });
            }
        }
        self.browser_captured();
        let active = self.browser_active();
        let focused = self.browser_focused();
        if self.web.focused != Some((active, focused)) {
            self.web.focused = Some((active, focused));
            for (id, shown) in &self.web.pages {
                shown.view.focus(
                    Some(*id) == active
                        && focused == Some(Target::Browser(BrowserEvent::BrowserBody)),
                );
            }
            if let Some((_, view)) = self.web.tools.as_ref() {
                view.focus(focused == Some(Target::Browser(BrowserEvent::BrowserToolsBody)));
            }
        }
        let picture = |view: &View, texture: rustle::ImageKey, shown: &mut u64| {
            let frame = view.frame().filter(|frame| frame.generation != *shown)?;
            *shown = frame.generation;
            ImageData::keyed(texture, frame.width, frame.height, frame.rgba.as_slice())
        };
        let [page_texture, tools_texture] = self.web.textures;
        let typing = self.focus.current() == Some(Target::Browser(BrowserEvent::BrowserAddress));
        let mut reports = Vec::new();
        for id in self.web.pages.keys().copied().collect::<Vec<_>>() {
            let host = self.browser_tab_host(id);
            let address = self
                .web
                .pages
                .get(&id)
                .map(|shown| self.browser_unresolve(&shown.view.address(), host.as_deref()))
                .unwrap_or_default();
            let Some(shown) = self.web.pages.get_mut(&id) else {
                continue;
            };
            let page = if Some(id) == active {
                picture(&shown.view, page_texture, &mut shown.shown)
            } else {
                None
            };
            reports.push((
                id,
                page,
                shown.view.title(),
                address,
                pointer_of(shown.view.cursor()),
            ));
        }
        let tools = self
            .web
            .tools
            .as_ref()
            .and_then(|(_, view)| picture(view, tools_texture, &mut self.web.tools_shown));
        let changed = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            reports.iter().any(|(id, page, title, address, pointer)| {
                page.is_some()
                    || state.browser_tab_anywhere(*id).is_some_and(|tab| {
                        tab.title != *title
                            || tab.pointer != *pointer
                            || (!(typing && Some(*id) == active)
                                && !address.is_empty()
                                && tab.address.text() != *address)
                    })
            })
        });
        if !changed && tools.is_none() {
            return;
        }
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            for (id, page, title, address, pointer) in reports {
                state.browser_scoped_to_tab(id, |state| {
                    state.browser_tab_painted(id, page, &title, &address, pointer, typing);
                });
            }
            if let Some(tools) = tools {
                state.browser_tools_painted(tools);
            }
        });
    }

    /// The line the General page says about the browser's install: what the download is doing, or how it ended, or that it is there.
    fn browser_status(&self, downloading: bool) -> Option<String> {
        if !self.saved_window.general.embedded_browser {
            return None;
        }
        if downloading {
            return self.web.download_note.lock().ok().map(|note| note.clone());
        }
        if let Some(note) = self.web.install_note.as_ref() {
            return Some(note.clone());
        }
        if !pm_browser::install::supported() {
            return Some("The embedded browser is not available on this system yet.".to_owned());
        }
        Some("The embedded browser is installed.".to_owned())
    }

    /// Forgets that the last download of the browser failed when the reader pressed the button of a preview's placeholder that asks for it again, so that [`Self::browser_install`] starts it as it does when the setting is turned on.
    fn browser_install_retry(&mut self) {
        let asked = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser_install_asked()
        });
        if !asked {
            return;
        }
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            state.take_browser_install_asked();
        });
        if self.web.installing.is_none() {
            self.web.install_failed = false;
            self.web.install_note = None;
            if self.web.outdated {
                if pm_browser::install::named() {
                    self.web.install_failed = true;
                    self.web.install_note = Some(still_outdated());
                } else {
                    self.web.update_wanted = true;
                }
            }
        }
    }

    /// How the helper is now, for the placeholders of previews: whether this system has one, whether it is installed, what its download says while `downloading`, and why the last download failed.
    fn browser_helper(&self, downloading: bool) -> Helper {
        Helper {
            supported: pm_browser::install::supported(),
            installed: pm_browser::install::installed(),
            outdated: self.web.outdated,
            downloading: downloading.then(|| {
                self.web
                    .download_note
                    .lock()
                    .map(|note| note.clone())
                    .unwrap_or_default()
            }),
            failed: self
                .web
                .install_note
                .clone()
                .filter(|_| self.web.install_failed),
        }
    }

    /// Downloads the browser when the setting is on and it is missing, takes in how the download went, and keeps the General page and the placeholders of previews saying what is happening.
    fn browser_install(&mut self) -> bool {
        self.browser_install_retry();
        let enabled = self.saved_window.general.embedded_browser;
        if enabled != self.web.was_enabled {
            self.web.was_enabled = enabled;
            self.web.install_failed = false;
            self.web.install_note = None;
            self.web.reason = None;
            if enabled {
                self.web.failed = false;
                self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    state.retry_previews();
                });
            }
        }
        let mut downloading = false;
        if let Some(download) = self.web.installing.as_ref() {
            match download.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => downloading = true,
                other => {
                    let outcome = other.unwrap_or_else(|_| Err("the download stopped".to_owned()));
                    self.web.installing = None;
                    self.web.install_failed = outcome.is_err();
                    self.web.failed = outcome.is_err();
                    if outcome.is_ok() && self.web.updated {
                        self.web.outdated = false;
                    }
                    self.web.install_note = Some(match &outcome {
                        Ok(()) => "The embedded browser is installed.".to_owned(),
                        Err(message) => {
                            format!("The embedded browser could not be installed: {message}")
                        }
                    });
                    self.model.update(|screen| {
                        let Screen::Workspace(state) = screen;
                        match &outcome {
                            Ok(()) => {
                                state.browser_retry();
                                state.retry_previews();
                            }
                            Err(message) => state.browser_notice(message),
                        }
                    });
                }
            }
        } else if enabled
            && !self.web.install_failed
            && pm_browser::install::supported()
            && (!pm_browser::install::installed() || self.web.update_wanted)
        {
            if std::mem::take(&mut self.web.update_wanted) {
                self.web.updated = true;
                self.web.engine = None;
                self.web.pages.clear();
                self.web.tools = None;
            }
            let (sender, receiver) = std::sync::mpsc::channel();
            let note = self.web.download_note.clone();
            std::thread::spawn(move || {
                let say = |text: String| {
                    if let Ok(mut held) = note.lock() {
                        *held = text;
                    }
                };
                let outcome = pm_browser::install::install(&|step| match step {
                    pm_browser::install::Step::Downloading(bytes) => say(format!(
                        "Downloading the embedded browser: {} of about 120 MB.",
                        bytes / 1_000_000
                    )),
                    pm_browser::install::Step::Verifying => {
                        say("Checking the embedded browser download.".to_owned());
                    }
                    pm_browser::install::Step::Unpacking => {
                        say("Unpacking the embedded browser.".to_owned());
                    }
                });
                let _ = sender.send(outcome);
            });
            self.web.installing = Some(receiver);
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.browser_notice("Downloading the embedded browser, about 120 MB...");
            });
            downloading = true;
        }
        let status = self.browser_status(downloading);
        if status != self.web.shown_note {
            self.web.shown_note.clone_from(&status);
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.set_browser_status(status);
            });
        }
        let helper = self.browser_helper(downloading);
        if helper != self.web.shown_helper {
            self.web.shown_helper.clone_from(&helper);
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.set_browser_helper(helper);
            });
        }
        downloading
    }

    /// Tells the servers whether this window can run browser tools when that changes, and runs the calls their agents made.
    fn browser_agent(&mut self, scale: f32) -> bool {
        let usable = self.saved_window.general.embedded_browser
            && pm_browser::install::supported()
            && pm_browser::install::installed();
        if usable != self.web.announced {
            self.web.announced = usable;
            self.work.announce_browser(usable);
        }
        for made in self.work.take_browser_calls() {
            let host = made.host;
            self.web.next_call += 1;
            let local = self.web.next_call;
            self.web.calls.insert(local, (host.clone(), made.call));
            let Some(position) = made.session else {
                self.browser_answer(
                    local,
                    false,
                    "This window does not know the session that made the call.".to_owned(),
                    Vec::new(),
                );
                continue;
            };
            let mut entered = None;
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                entered = state.browser_scope_enter(position);
            });
            let Some(entered) = entered else {
                self.browser_answer(
                    local,
                    false,
                    "This window does not know the session that made the call.".to_owned(),
                    Vec::new(),
                );
                continue;
            };
            let started = self.browser_tool(&host, local, &made.tool, &made.arguments, scale);
            self.browser_commands(scale);
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.browser_scope_leave(entered);
            });
            if !started {
                self.browser_answer(
                    local,
                    false,
                    "The embedded browser is not usable here.".to_owned(),
                    Vec::new(),
                );
            }
        }
        let results: Vec<_> = self
            .web
            .pages
            .values()
            .flat_map(|shown| shown.view.take_tool_results())
            .collect();
        for result in results {
            let image = result
                .image
                .as_deref()
                .and_then(|path| {
                    let bytes = std::fs::read(path).ok();
                    let _ = std::fs::remove_file(path);
                    bytes
                })
                .unwrap_or_default();
            self.browser_answer(result.call, result.ok, result.text, image);
        }
        !self.web.calls.is_empty()
    }

    /// Gives the server the answer of the tool call `local`.
    fn browser_answer(&mut self, local: u64, ok: bool, text: String, image: Vec<u8>) {
        if let Some((host, call)) = self.web.calls.remove(&local) {
            self.work.browser_result(&host, call, ok, text, image);
        }
    }

    /// Starts the tool `tool` an agent called; says whether it could be started, the answer coming later.
    ///
    /// A tool works on the tab its `tab` argument names, which then is the one tools work on when they name none; with no such argument, on the tab an agent chose or opened, and else on the one being shown. A tool that changes a page is refused on a tab the reader locked.
    fn browser_tool(
        &mut self,
        host: &Option<String>,
        local: u64,
        tool: &str,
        arguments: &str,
        scale: f32,
    ) -> bool {
        let mut arguments_json: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
        if tool == "browser_tabs" {
            self.browser_tabs_tool(host, local, &arguments_json, scale);
            return true;
        }
        let named = match arguments_json.get("tab") {
            None | Some(serde_json::Value::Null) => None,
            Some(value) => match value.as_u64().and_then(|id| u32::try_from(id).ok()) {
                Some(id) => Some(id),
                None => {
                    self.browser_answer(
                        local,
                        false,
                        "tab must be the number of a tab, as browser_tabs lists them".to_owned(),
                        Vec::new(),
                    );
                    return true;
                }
            },
        };
        if let Some(object) = arguments_json.as_object_mut() {
            object.remove("tab");
        }
        if tool == "browser_navigate" {
            let Some(url) = arguments_json["url"].as_str().map(str::to_owned) else {
                self.browser_answer(local, false, "no url was given".to_owned(), Vec::new());
                return true;
            };
            if !agent_may_open(&url) {
                self.browser_answer(
                    local,
                    false,
                    "Only http and https addresses can be opened.".to_owned(),
                    Vec::new(),
                );
                return true;
            }
            let resolved = match self.browser_resolve(&url, host.as_deref()) {
                Ok(resolved) => resolved,
                Err(message) => {
                    self.browser_answer(local, false, message, Vec::new());
                    return true;
                }
            };
            arguments_json["url"] = serde_json::Value::String(resolved);
        }
        let tabs = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser.as_ref().map(|browser| browser.tab_infos())
        });
        let target = match (named, tabs.as_deref()) {
            (Some(id), Some(tabs)) if tabs.iter().any(|tab| tab.id == id) => Some(id),
            (Some(id), _) => {
                self.browser_answer(
                    local,
                    false,
                    format!("There is no tab {id}; browser_tabs lists the tabs there are."),
                    Vec::new(),
                );
                return true;
            }
            (None, Some(tabs)) => tabs.iter().find(|tab| tab.target).map(|tab| tab.id),
            (None, None) => None,
        };
        let target = match target {
            Some(id) => id,
            None if tool == "browser_navigate" => {
                let mut opened = None;
                self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    opened = state.browser_tab_opened("about:blank", host.clone(), true, true);
                });
                self.browser_commands(scale);
                match opened {
                    Some(id) => id,
                    None => {
                        self.browser_answer(
                            local,
                            false,
                            "The browser holds too many tabs.".to_owned(),
                            Vec::new(),
                        );
                        return true;
                    }
                }
            }
            None => {
                self.browser_answer(
                    local,
                    false,
                    "No page is open in the browser; use browser_navigate first.".to_owned(),
                    Vec::new(),
                );
                return true;
            }
        };
        let locked = tabs
            .as_deref()
            .and_then(|tabs| tabs.iter().find(|tab| tab.id == target))
            .is_some_and(|tab| tab.locked);
        if locked && changes_page(tool) {
            self.browser_answer(local, false, locked_message(tool, target), Vec::new());
            return true;
        }
        if named.is_some() {
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                if let Some(browser) = state.browser.as_mut() {
                    browser.agent_tab = Some(target);
                }
            });
        }
        let Some(page) = self.web.pages.get(&target).map(|shown| &shown.view) else {
            self.browser_answer(
                local,
                false,
                format!(
                    "No page is open in tab {target}; use browser_navigate with tab {target} first."
                ),
                Vec::new(),
            );
            return true;
        };
        page.tool(local, tool, &arguments_json.to_string());
        true
    }

    /// Runs the tool `browser_tabs` an agent called, which the window answers itself: `list`, `open`, `select` and `close`.
    fn browser_tabs_tool(
        &mut self,
        host: &Option<String>,
        local: u64,
        arguments: &serde_json::Value,
        scale: f32,
    ) {
        let tab = arguments["tab"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok());
        let tabs = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state
                .browser
                .as_ref()
                .map(|browser| browser.tab_infos())
                .unwrap_or_default()
        });
        let (ok, text) = match arguments["action"].as_str().unwrap_or("list") {
            "list" => (true, tabs_text(&tabs)),
            "open" => {
                let url = arguments["url"].as_str().unwrap_or("about:blank").trim();
                if !agent_may_open(url) {
                    (
                        false,
                        "Only http and https addresses can be opened.".to_owned(),
                    )
                } else if !origin_allowed(
                    url,
                    self.saved_window.general.restrict_agent_browsing,
                    &self.saved_window.general.agent_origins,
                ) {
                    (
                        false,
                        format!(
                            "Blocked: {url} is not an origin agents may open. Agent browsing is restricted to localhost, 127.0.0.1, [::1] and the origins the reader added in Settings under Browser."
                        ),
                    )
                } else if let Err(message) = self.browser_resolve(url, host.as_deref()) {
                    (false, message)
                } else {
                    let mut opened = None;
                    self.model.update(|screen| {
                        let Screen::Workspace(state) = screen;
                        opened = state.browser_tab_opened(url, host.clone(), true, false);
                    });
                    self.browser_commands(scale);
                    match opened {
                        Some(id) => (
                            true,
                            format!(
                                "Opened tab {id} at {url}. It is the tab the tools work on from now on, and the reader sees it in the strip, marked as yours. Give tab {id} to a tool to name it."
                            ),
                        ),
                        None => (
                            false,
                            "The browser holds too many tabs; close one first.".to_owned(),
                        ),
                    }
                }
            }
            action @ ("select" | "close") => {
                match tab.filter(|id| tabs.iter().any(|t| t.id == *id)) {
                    None => (
                        false,
                        format!(
                            "Give tab as the number of a tab; the tabs are:\n{}",
                            tabs_text(&tabs)
                        ),
                    ),
                    Some(id) if action == "close" => {
                        if tabs.iter().any(|t| t.id == id && t.locked) {
                            (false, locked_message("browser_tabs close", id))
                        } else {
                            self.model.update(|screen| {
                                let Screen::Workspace(state) = screen;
                                state.browser_tab_closed(id);
                            });
                            (true, format!("Closed tab {id}."))
                        }
                    }
                    Some(id) => {
                        self.model.update(|screen| {
                            let Screen::Workspace(state) = screen;
                            if let Some(browser) = state.browser.as_mut() {
                                browser.agent_tab = Some(id);
                            }
                        });
                        let locked = tabs.iter().any(|t| t.id == id && t.locked);
                        (
                            true,
                            format!(
                                "Tools now work on tab {id}; what the reader sees is unchanged.{}",
                                if locked {
                                    " It is locked by the reader, so tools that change it are refused; reading it is allowed."
                                } else {
                                    ""
                                }
                            ),
                        )
                    }
                }
            }
            other => (
                false,
                format!("there is no action {other}; use list, open, select or close"),
            ),
        };
        self.browser_answer(local, ok, text, Vec::new());
    }

    /// Opens the web links the reader followed: a local one in the browser pane when the setting to do so is on and the embedded browser is on and installed, every other in the reader's browser.
    fn browser_links(&mut self) {
        let links = self.work.take_links();
        let general = &self.saved_window.general;
        let ready = general.embedded_browser
            && pm_browser::install::supported()
            && pm_browser::install::installed();
        let local_links = general.open_local_links;
        for link in links {
            match route(&link, local_links, ready) {
                Route::Pane(address) => self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    state.open_link_page(&address);
                }),
                Route::System(address) => {
                    if let Err(reason) = pm_harness::open_address(&address) {
                        eprintln!("{reason}");
                    }
                }
            }
        }
    }

    /// Opens the pages that programs started in shells serve, when the browser is on and installed and the setting to open them is.
    fn browser_served(&mut self) {
        let pages = self.work.take_served();
        let general = &self.saved_window.general;
        if !(general.embedded_browser
            && general.open_dev_servers
            && pm_browser::install::supported()
            && pm_browser::install::installed())
        {
            return;
        }
        let scale = self.scale;
        for (host, url, owner) in pages {
            let mut entered = None;
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                entered = owner.and_then(|position| state.browser_scope_enter(position));
                state.open_served_page(&url, host);
            });
            self.browser_commands(scale);
            if let Some(entered) = entered {
                self.model.update(|screen| {
                    let Screen::Workspace(state) = screen;
                    state.browser_scope_leave(entered);
                });
            }
        }
    }

    /// Does the browser's work for one turn of the event loop, and says whether a page is open, which wants the loop to turn again soon.
    pub(super) fn browser_tick(&mut self) -> bool {
        let scale = self.scale;
        if self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser_rescope_needed()
        }) {
            self.model.update(|screen| {
                let Screen::Workspace(state) = screen;
                state.rescope_browser();
            });
        }
        let downloading = self.browser_install();
        self.browser_origins();
        let calling = self.browser_agent(scale);
        self.browser_served();
        self.browser_links();
        self.browser_commands(scale);
        self.browser_sync_tabs();
        let rendering = self.browser_previews();
        self.browser_sealed_overdue();
        if self.web.pages.is_empty() {
            return downloading || calling || rendering;
        }
        self.browser_safety();
        self.browser_fit(scale);
        self.browser_present();
        true
    }
}

#[cfg(test)]
mod origin_tests {
    use super::origin_allowed;

    /// The restriction lets local addresses and listed origins through, and nothing else.
    #[test]
    fn restriction_holds_new_tabs_to_local_and_listed_origins() {
        let list = vec![
            "https://docs.rs".to_owned(),
            "staging.example.com:8443".to_owned(),
        ];
        assert!(origin_allowed("https://evil.com", false, &list));
        for ok in [
            "about:blank",
            "http://localhost:3000/x",
            "http://[::1]:80/",
            "https://docs.rs/a",
            "https://staging.example.com:8443/",
            "http://staging.example.com:8443",
        ] {
            assert!(origin_allowed(ok, true, &list), "{ok}");
        }
        for bad in [
            "https://evil.com",
            "http://docs.rs/",
            "https://docs.rs:444/",
            "http://localhost@evil.com/",
            "http://evil.com\\@localhost/",
            "file:///etc/passwd",
            "https://staging.example.com/",
            "http://localhost.evil.com",
        ] {
            assert!(!origin_allowed(bad, true, &list), "{bad}");
        }
    }
}
