//! The engine's callbacks for one page: where it is drawn and how large it is, and what it reports about itself.
//!
//! `MenuHandler` empties the page's context menu, which cannot be drawn without a window; `PaintHandler` writes what the page paints into shared files; `LifeHandler`
//! keeps the browser the engine made; `ReportHandler` tells the window the
//! title, address and cursor; `PageClient` hands the engine those three.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cef::*;
use pm_browser::surface::SurfaceWriter;
use pm_browser::wire::{Event, line};
use pm_browser::{Cursor, PickedElement};

/// Where reports to the window are written.
pub(crate) struct Out(Mutex<Box<dyn Write + Send>>);

impl Out {
    /// A sink writing to `writer`.
    pub(crate) fn new(writer: Box<dyn Write + Send>) -> Arc<Self> {
        Arc::new(Self(Mutex::new(writer)))
    }

    /// Sends `event` to the window.
    pub(crate) fn send(&self, event: &Event) {
        if let Ok(mut writer) = self.0.lock() {
            let _ = writer.write_all(&line(event));
            let _ = writer.flush();
        }
    }
}

/// The two shared files a page paints into, taken in turn.
struct Surface {
    /// Width and height of a picture in pixels.
    size: (u32, u32),
    /// The files' writers.
    writers: [SurfaceWriter; 2],
    /// Where the files are.
    paths: [PathBuf; 2],
    /// Which file the next paint goes into.
    next: usize,
}

impl Drop for Surface {
    /// Removes the files.
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// What the engine's callbacks and the host's loop share about one page.
pub(crate) struct Page {
    /// The number the window knows the page by.
    id: u32,
    /// Where reports go.
    out: Arc<Out>,
    /// The folder the shared files are made in.
    folder: PathBuf,
    /// The browser the engine made, once it has.
    browser: Mutex<Option<Browser>>,
    /// The page's size in logical pixels and the display scale it is drawn at.
    geometry: Mutex<(u32, u32, f32)>,
    /// The page's address, as last reported.
    address: Mutex<String>,
    /// The files painted into.
    surface: Mutex<Option<Surface>>,
    /// The secret the pick script was given when pick mode was turned on, while it is on; only a report that carries it is believed.
    pick_secret: Mutex<Option<String>>,
    /// How many files have been made, for naming.
    serial: AtomicU64,
    /// How many paints there have been.
    generation: AtomicU64,
}

impl Page {
    /// State for page `id` of `width` by `height` logical pixels at `scale`.
    pub(crate) fn new(
        id: u32,
        out: Arc<Out>,
        folder: PathBuf,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            out,
            folder,
            browser: Mutex::new(None),
            geometry: Mutex::new((width, height, scale)),
            address: Mutex::new(String::new()),
            surface: Mutex::new(None),
            pick_secret: Mutex::new(None),
            serial: AtomicU64::new(0),
            generation: AtomicU64::new(0),
        })
    }

    /// Remembers the secret of the pick mode just turned on, or forgets it when there is none.
    pub(crate) fn set_pick_secret(&self, secret: Option<String>) {
        if let Ok(mut held) = self.pick_secret.lock() {
            *held = secret;
        }
    }

    /// What follows `prefix` in `message` when pick mode is on and the message carries its secret; the secret is used up by a message that does.
    fn pick_report<'a>(&self, message: &'a str, prefix: &str) -> Option<&'a str> {
        let rest = message.strip_prefix(prefix)?;
        let mut held = self.pick_secret.lock().ok()?;
        let rest = rest.strip_prefix(held.as_deref()?)?;
        *held = None;
        Some(rest)
    }

    /// The browser the engine made, once it has.
    pub(crate) fn browser(&self) -> Option<Browser> {
        self.browser.lock().ok()?.clone()
    }

    /// Keeps the browser the engine made.
    pub(crate) fn set_browser(&self, browser: Browser) {
        if let Ok(mut slot) = self.browser.lock() {
            *slot = Some(browser);
        }
    }

    /// The page's address, as last reported.
    pub(crate) fn address(&self) -> String {
        self.address.lock().map(|a| a.clone()).unwrap_or_default()
    }

    /// Records a new size and scale for the next paint.
    pub(crate) fn set_geometry(&self, width: u32, height: u32, scale: f32) {
        if let Ok(mut geometry) = self.geometry.lock() {
            *geometry = (width, height, scale);
        }
    }

    /// Writes a paint of `width` by `height` pixels from the engine's BGRA `bytes` into the next shared file and tells the window.
    fn paint(&self, width: u32, height: u32, bytes: &[u8]) {
        let Ok(mut slot) = self.surface.lock() else {
            return;
        };
        if slot
            .as_ref()
            .is_none_or(|surface| surface.size != (width, height))
        {
            let serial = self.serial.fetch_add(1, Ordering::Relaxed);
            let paths = [0, 1].map(|n| self.folder.join(format!("p{}-{serial}-{n}.px", self.id)));
            let made = (
                SurfaceWriter::create(&paths[0], width, height),
                SurfaceWriter::create(&paths[1], width, height),
            );
            let (Ok(first), Ok(second)) = made else {
                return;
            };
            self.out.send(&Event::Surface {
                page: self.id,
                width,
                height,
                paths: paths
                    .clone()
                    .map(|path| path.to_string_lossy().into_owned()),
            });
            *slot = Some(Surface {
                size: (width, height),
                writers: [first, second],
                paths,
                next: 0,
            });
        }
        let Some(surface) = slot.as_mut() else {
            return;
        };
        let buffer = surface.next;
        surface.writers[buffer].write(bytes);
        surface.next = 1 - buffer;
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.out.send(&Event::Painted {
            page: self.id,
            buffer: buffer as u8,
            generation,
        });
    }
}

/// `text` cut to at most `most` characters.
fn cut(text: &mut String, most: usize) {
    if let Some((end, _)) = text.char_indices().nth(most) {
        text.truncate(end);
    }
}

/// A picked element with every field cut to a size a prompt can take, as a page chooses what it says.
fn limited(mut element: PickedElement) -> PickedElement {
    for (text, most) in [
        (&mut element.selector, 300),
        (&mut element.tag, 40),
        (&mut element.id, 200),
        (&mut element.text, 400),
        (&mut element.html, 2000),
        (&mut element.url, 500),
        (&mut element.title, 200),
    ] {
        cut(text, most);
    }
    element.classes.truncate(20);
    element.classes.iter_mut().for_each(|class| cut(class, 100));
    element.styles.truncate(20);
    for (name, value) in &mut element.styles {
        cut(name, 40);
        cut(value, 200);
    }
    element
}

/// How the engine's cursor kinds look to the window.
fn cursor_of(kind: CursorType) -> Cursor {
    if kind == CursorType::HAND {
        Cursor::Hand
    } else if kind == CursorType::IBEAM {
        Cursor::Text
    } else if kind == CursorType::CROSS {
        Cursor::Cross
    } else if kind == CursorType::WAIT {
        Cursor::Wait
    } else if kind == CursorType::HELP {
        Cursor::Help
    } else if kind == CursorType::EASTRESIZE || kind == CursorType::WESTRESIZE {
        Cursor::ResizeHorizontal
    } else if kind == CursorType::NORTHRESIZE || kind == CursorType::SOUTHRESIZE {
        Cursor::ResizeVertical
    } else {
        Cursor::Arrow
    }
}

wrap_render_handler! {
    pub(crate) struct PaintHandler {
        page: Arc<Page>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let (width, height, _) = self.page.geometry.lock().map(|g| *g).unwrap_or((1, 1, 1.0));
            if let Some(rect) = rect {
                rect.width = width.max(1) as _;
                rect.height = height.max(1) as _;
            }
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            let (_, _, scale) = self.page.geometry.lock().map(|g| *g).unwrap_or((1, 1, 1.0));
            match screen_info {
                Some(info) => {
                    info.device_scale_factor = scale;
                    true as _
                }
                None => false as _,
            }
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if type_ != PaintElementType::default() || buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let length = width as usize * height as usize * 4;
            let bytes = unsafe { std::slice::from_raw_parts(buffer, length) };
            self.page.paint(width as u32, height as u32, bytes);
        }
    }
}

wrap_life_span_handler! {
    pub(crate) struct LifeHandler {
        page: Arc<Page>,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(browser) = browser {
                self.page.set_browser(browser.clone());
            }
        }
    }
}

wrap_display_handler! {
    pub(crate) struct ReportHandler {
        page: Arc<Page>,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(title) = title {
                self.page.out.send(&Event::Title {
                    page: self.page.id,
                    title: title.to_string(),
                });
            }
        }

        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            if let Some(url) = url {
                if let Ok(mut address) = self.page.address.lock() {
                    *address = url.to_string();
                }
                self.page.out.send(&Event::Address {
                    page: self.page.id,
                    url: url.to_string(),
                });
            }
        }

        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            _source: Option<&CefString>,
            _line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let Some(message) = message.map(ToString::to_string) else {
                return false as _;
            };
            if let Some(json) = self.page.pick_report(&message, "__pm_pick__") {
                self.page.out.send(&Event::Picked {
                    page: self.page.id,
                    element: serde_json::from_str(json).ok().map(limited),
                });
                return true as _;
            }
            if self.page.pick_report(&message, "__pm_pick_cancel__").is_some() {
                self.page.out.send(&Event::Picked {
                    page: self.page.id,
                    element: None,
                });
                return true as _;
            }
            false as _
        }

        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: ::std::os::raw::c_ulong,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            self.page.out.send(&Event::Cursor {
                page: self.page.id,
                cursor: cursor_of(type_),
            });
            true as _
        }
    }
}

wrap_context_menu_handler! {
    pub(crate) struct MenuHandler;

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            if let Some(model) = model {
                model.clear();
            }
        }
    }
}

wrap_client! {
    pub(crate) struct PageClient {
        render: RenderHandler,
        life: LifeSpanHandler,
        display: DisplayHandler,
        menu: ContextMenuHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render.clone())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life.clone())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display.clone())
        }

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(self.menu.clone())
        }
    }
}

/// The handlers for a page whose callbacks fill `page`.
pub(crate) fn client_for(page: &Arc<Page>) -> Client {
    PageClient::new(
        PaintHandler::new(page.clone()),
        LifeHandler::new(page.clone()),
        ReportHandler::new(page.clone()),
        MenuHandler::new(),
    )
}
