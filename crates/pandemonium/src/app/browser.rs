//! The browser pane in the window: the engine that paints pages, the pointer and keyboard sent into them, and the pictures drawn from what they paint.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use pm_browser::{
    Button as Mouse, Cursor as Shape, Engine, Key as Typed, Modifiers as Held, Options, View,
};
use pm_components::browser::{
    BROWSER_BODY, BROWSER_TOOLS_BODY, Command, PAGE_SURFACE, Pointer, TOOLS_SURFACE,
};
use pm_editor::{Action, Pressed};
use pm_input::{Button, FocusId, Key, KeyEvent};
use pm_pages::Screen;
use pm_renderer::{Picture, Rect};
use pm_ui::Cursor;

use super::App;

/// Where a page opens when the command to open the browser was given with nothing typed.
const HOME: &str = "http://localhost:5173";

/// How often the engine is given time to work while a page is open.
pub(super) const PUMP: Duration = Duration::from_millis(4);

/// How soon after a press another press counts as part of a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(450);

/// The engine and the pages it holds.
#[derive(Default)]
pub(super) struct Web {
    /// The engine, once a page was first opened.
    engine: Option<Engine>,
    /// The page, while one is open.
    page: Option<View>,
    /// The developer tools of the page, while they are open.
    tools: Option<View>,
    /// The size each window of a page was last given, in logical pixels, and the scale.
    sized: [Option<(u32, u32, f32)>; 2],
    /// The newest frame of the page and of the tools that was handed to the window.
    shown: [u64; 2],
    /// Which window the primary button went down in, so the release goes to it too.
    pressed_in: Option<FocusId>,
    /// The last press, for counting clicks.
    last_press: Option<(Instant, (i32, i32), u8)>,
    /// Which window had the keyboard when the page was last told.
    focused: Option<FocusId>,
    /// Whether the engine could not start.
    failed: bool,
    /// The answer to the question of where the developer tools of the page are served, while it is awaited.
    tools_lookup: Option<std::sync::mpsc::Receiver<Option<String>>>,
}

/// Where the engine keeps cookies and caches.
fn cache_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    if cfg!(target_os = "macos") {
        home.join("Library/Caches/Pandemonium/browser")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"))
            .join("pandemonium/browser")
    }
}

/// The engine's modifiers for the keyboard keys held and the buttons down.
fn held(keys: pm_input::Modifiers, pointer: pm_input::PointerState) -> Held {
    let mut bits = 0;
    if keys.shift {
        bits |= Held::SHIFT;
    }
    if keys.control {
        bits |= Held::CONTROL;
    }
    if keys.alt {
        bits |= Held::ALT;
    }
    if keys.logo {
        bits |= Held::COMMAND;
    }
    if pointer.left {
        bits |= Held::LEFT_BUTTON;
    }
    if pointer.middle {
        bits |= Held::MIDDLE_BUTTON;
    }
    if pointer.right {
        bits |= Held::RIGHT_BUTTON;
    }
    Held(bits)
}

/// The Windows virtual-key code of `key`, which is how the engine names keys, and the text it types.
fn typed(event: KeyEvent, modifiers: Held) -> Typed {
    let plain = modifiers.0 & (Held::CONTROL | Held::ALT | Held::COMMAND) == 0;
    let (code, text): (i32, Option<String>) = match event.key {
        Key::Character(character) => {
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
        Key::Space => (0x20, plain.then(|| " ".to_owned())),
        Key::ArrowLeft => (0x25, None),
        Key::ArrowUp => (0x26, None),
        Key::ArrowRight => (0x27, None),
        Key::ArrowDown => (0x28, None),
        Key::Function(number) => (0x6F + i32::from(number), None),
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
    fn browser_region(&self, id: FocusId) -> Option<Rect> {
        self.output
            .regions
            .iter()
            .find(|region| region.id == id)
            .map(|region| region.rect)
    }

    /// The window of a page the point `at` is over, and where in it, in logical pixels.
    fn browser_under(&self, at: (f32, f32)) -> Option<(FocusId, (i32, i32))> {
        [BROWSER_BODY, BROWSER_TOOLS_BODY]
            .into_iter()
            .find_map(|id| {
                let rect = self.browser_region(id)?;
                let inside = at.0 >= rect.x
                    && at.1 >= rect.y
                    && at.0 < rect.x + rect.width
                    && at.1 < rect.y + rect.height;
                inside.then_some((id, ((at.0 - rect.x) as i32, (at.1 - rect.y) as i32)))
            })
    }

    /// The view that shows in the window `id`.
    fn browser_view_of(&self, id: FocusId) -> Option<&View> {
        match id {
            BROWSER_BODY => self.web.page.as_ref(),
            BROWSER_TOOLS_BODY => self.web.tools.as_ref(),
            _ => None,
        }
    }

    /// Whether a page or its tools have the keyboard.
    fn browser_focused(&self) -> Option<FocusId> {
        self.focus
            .current()
            .filter(|id| *id == BROWSER_BODY || *id == BROWSER_TOOLS_BODY)
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
                for id in [BROWSER_BODY, BROWSER_TOOLS_BODY] {
                    if let Some(view) = self.browser_view_of(id) {
                        view.pointer_moved(0, 0, modifiers, true);
                    }
                }
            }
        }
    }

    /// Sends a press or release of `button` into the page under the pointer, or into the one the press began in.
    pub(super) fn browser_button(&mut self, button: Button, pressed: bool) {
        let Some(at) = self.pointer.position else {
            return;
        };
        let target = if pressed {
            self.browser_under(at)
        } else {
            self.web.pressed_in.and_then(|id| {
                let rect = self.browser_region(id)?;
                Some((id, ((at.0 - rect.x) as i32, (at.1 - rect.y) as i32)))
            })
        };
        let Some((id, (x, y))) = target else {
            return;
        };
        let clicks = if pressed {
            let near = |last: (i32, i32)| (last.0 - x).abs() < 5 && (last.1 - y).abs() < 5;
            let count = match self.web.last_press {
                Some((when, last, count)) if when.elapsed() < DOUBLE_CLICK && near(last) => {
                    count + 1
                }
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
            Button::Middle => Mouse::Middle,
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

    /// Sends a key into the page that holds the keyboard, unless the keymap binds it to something the window does, and says whether it did.
    pub(super) fn browser_key(&mut self, event: KeyEvent) -> bool {
        let Some(id) = self.browser_focused() else {
            return false;
        };
        let modifiers = event.modifiers;
        if (modifiers.control || modifiers.alt || modifiers.logo)
            && let Some(chord) = Self::chord_of(&event)
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

    /// Starts the engine when it is not running.
    fn browser_engine(&mut self) -> Option<&Engine> {
        if self.web.engine.is_none() && !self.web.failed {
            let port = std::net::TcpListener::bind(("127.0.0.1", 0))
                .and_then(|listener| listener.local_addr())
                .map(|address| address.port())
                .ok();
            let options = Options {
                switches: vec![
                    "ozone-platform=x11".to_owned(),
                    format!(
                        "remote-allow-origins=http://127.0.0.1:{}",
                        port.unwrap_or(0)
                    ),
                ],
                shared_texture: false,
                debugging_port: port,
            };
            let cache = cache_dir();
            let _ = std::fs::create_dir_all(&cache);
            match Engine::start(&cache, &options) {
                Ok(engine) => self.web.engine = Some(engine),
                Err(error) => {
                    eprintln!("pandemonium: the browser could not start: {error}");
                    self.web.failed = true;
                }
            }
        }
        self.web.engine.as_ref()
    }

    /// The size a window of a page was last drawn at, in logical pixels, or a size to start from.
    fn browser_size(&self, id: FocusId, scale: f32) -> (u32, u32, f32) {
        self.browser_region(id).map_or((960, 640, scale), |rect| {
            (
                rect.width.max(1.0) as u32,
                rect.height.max(1.0) as u32,
                scale,
            )
        })
    }

    /// Carries out what the window asked of the engine.
    fn browser_commands(&mut self, scale: f32) {
        let pending = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state
                .browser
                .as_ref()
                .is_some_and(|browser| !browser.commands.is_empty())
        });
        if !pending {
            return;
        }
        let mut commands = Vec::new();
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            commands = state.take_browser_commands();
        });
        for command in commands {
            match command {
                Command::Load(address) => {
                    if let Some(page) = self.web.page.as_ref() {
                        page.load(&address);
                    } else {
                        let (width, height, scale) = self.browser_size(BROWSER_BODY, scale);
                        if let Some(engine) = self.browser_engine() {
                            match View::open(engine, &address, width, height, scale) {
                                Ok(view) => self.web.page = Some(view),
                                Err(error) => eprintln!("pandemonium: {error}"),
                            }
                        }
                    }
                }
                Command::Back => {
                    if let Some(page) = self.web.page.as_ref() {
                        page.back();
                    }
                }
                Command::Forward => {
                    if let Some(page) = self.web.page.as_ref() {
                        page.forward();
                    }
                }
                Command::Reload => {
                    if let Some(page) = self.web.page.as_ref() {
                        page.reload();
                    }
                }
                Command::OpenTools => {
                    if let (Some(port), Some(page)) = (
                        self.web.engine.as_ref().and_then(Engine::debugging_port),
                        self.web.page.as_ref(),
                    ) {
                        self.web.tools_lookup = Some(pm_browser::find_tools(port, page.address()));
                    }
                }
                Command::CloseTools => {
                    self.web.tools_lookup = None;
                    self.web.tools = None;
                    self.web.sized[1] = None;
                    self.web.shown[1] = 0;
                }
                Command::Close => {
                    self.web.tools = None;
                    self.web.page = None;
                    self.web.sized = [None, None];
                    self.web.shown = [0, 0];
                }
            }
        }
    }

    /// Opens the developer tools once the engine has said where they are served.
    fn browser_tools_ready(&mut self, scale: f32) {
        let Some(lookup) = self.web.tools_lookup.as_ref() else {
            return;
        };
        let address = match lookup.try_recv() {
            Ok(address) => address,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
        };
        self.web.tools_lookup = None;
        let (width, height, scale) = self.browser_size(BROWSER_TOOLS_BODY, scale);
        if let (Some(address), Some(engine)) = (address, self.web.engine.as_ref()) {
            match View::open(engine, &address, width.max(1), height, scale) {
                Ok(view) => self.web.tools = Some(view),
                Err(error) => eprintln!("pandemonium: {error}"),
            }
        }
    }

    /// Closes the pages and lets the engine finish, so the process can leave.
    pub(super) fn browser_shutdown(&mut self) {
        self.web.tools = None;
        self.web.page = None;
        if let Some(engine) = self.web.engine.take() {
            engine.settle(Duration::from_millis(300));
        }
    }

    /// Gives each window of a page the size of the room the pane has for it.
    fn browser_fit(&mut self, scale: f32) {
        for (slot, id) in [(0, BROWSER_BODY), (1, BROWSER_TOOLS_BODY)] {
            let Some(rect) = self.browser_region(id) else {
                continue;
            };
            let size = (
                rect.width.max(1.0) as u32,
                rect.height.max(1.0) as u32,
                scale,
            );
            if self.web.sized[slot] == Some(size) {
                continue;
            }
            self.web.sized[slot] = Some(size);
            if let Some(view) = self.browser_view_of(id) {
                view.resize(size.0, size.1, size.2);
            }
        }
    }

    /// Hands the window what the pages painted since it last looked and what they say about themselves, and tells the pages which of them has the keyboard.
    fn browser_present(&mut self) {
        let focused = self.browser_focused();
        if focused != self.web.focused {
            self.web.focused = focused;
            if let Some(view) = self.web.page.as_ref() {
                view.focus(focused == Some(BROWSER_BODY));
            }
            if let Some(view) = self.web.tools.as_ref() {
                view.focus(focused == Some(BROWSER_TOOLS_BODY));
            }
        }
        let picture = |view: &View, surface: u32, shown: &mut u64| {
            let frame = view.frame().filter(|frame| frame.generation != *shown)?;
            *shown = frame.generation;
            Some(Picture {
                key: Picture::live_key(surface, frame.generation),
                width: frame.width,
                height: frame.height,
                pixels: frame.rgba,
            })
        };
        let mut shown = self.web.shown;
        let page = self
            .web
            .page
            .as_ref()
            .and_then(|view| picture(view, PAGE_SURFACE, &mut shown[0]));
        let tools = self
            .web
            .tools
            .as_ref()
            .and_then(|view| picture(view, TOOLS_SURFACE, &mut shown[1]));
        let (title, address, pointer) = self.web.page.as_ref().map_or_else(
            || (String::new(), String::new(), Pointer::Arrow),
            |view| (view.title(), view.address(), pointer_of(view.cursor())),
        );
        let typing = self.focus.current() == Some(pm_components::browser::BROWSER_ADDRESS);
        let changed = self.model.read(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser.as_ref().is_some_and(|browser| {
                browser.title != title
                    || browser.pointer != pointer
                    || (!typing && !address.is_empty() && browser.address.text() != address)
            })
        });
        if page.is_none() && tools.is_none() && !changed {
            return;
        }
        self.web.shown = shown;
        self.model.update(|screen| {
            let Screen::Workspace(state) = screen;
            state.browser_painted(page, tools, &title, &address, pointer, typing);
        });
    }

    /// Does the browser's work for one turn of the event loop, and says whether a page is open, which wants the loop to turn again soon.
    pub(super) fn browser_tick(&mut self) -> bool {
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        self.browser_commands(scale);
        if self.web.page.is_none() {
            return false;
        }
        if let Some(engine) = self.web.engine.as_ref() {
            engine.pump();
        }
        self.browser_tools_ready(scale);
        self.browser_fit(scale);
        self.browser_present();
        true
    }
}
