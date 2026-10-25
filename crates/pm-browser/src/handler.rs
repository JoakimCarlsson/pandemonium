//! The browser's callbacks: where a page is drawn, how large it is, and what it reports about itself.
//!
//! `PaintHandler` receives what the page paints; `LifeHandler` keeps the
//! browser the engine made; `ReportHandler` keeps the title, address and
//! cursor; `PageClient` hands the browser those three.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cef::*;

use crate::frame::{Frame as Picture, PaintStats};
use crate::input::Cursor;

/// What the browser callbacks and the [`crate::View`] share.
pub(crate) struct Shared {
    /// The browser the engine made, once it has.
    browser: Mutex<Option<Browser>>,
    /// The newest software paint, if the page has painted yet.
    frame: Mutex<Option<Picture>>,
    /// The page's size in logical pixels and the display scale it is drawn at.
    geometry: Mutex<(u32, u32, f32)>,
    /// The page's title, address and cursor.
    about: Mutex<(String, String, Cursor)>,
    /// Software paints delivered.
    software: AtomicU64,
    /// Shared-texture paints delivered.
    shared_texture: AtomicU64,
}

impl Shared {
    /// State for a page of `width` by `height` logical pixels at `scale`.
    pub(crate) fn new(width: u32, height: u32, scale: f32) -> Arc<Self> {
        Arc::new(Self {
            browser: Mutex::new(None),
            frame: Mutex::new(None),
            geometry: Mutex::new((width, height, scale)),
            about: Mutex::new((String::new(), String::new(), Cursor::Arrow)),
            software: AtomicU64::new(0),
            shared_texture: AtomicU64::new(0),
        })
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

    /// The newest software paint.
    pub(crate) fn frame(&self) -> Option<Picture> {
        self.frame.lock().ok()?.clone()
    }

    /// The page's title, address and cursor.
    pub(crate) fn about(&self) -> (String, String, Cursor) {
        self.about
            .lock()
            .map(|about| about.clone())
            .unwrap_or_default()
    }

    /// How many paints of each kind have arrived.
    pub(crate) fn stats(&self) -> PaintStats {
        PaintStats {
            software: self.software.load(Ordering::Relaxed),
            shared_texture: self.shared_texture.load(Ordering::Relaxed),
        }
    }

    /// Records a new size and scale for the next paint.
    pub(crate) fn set_geometry(&self, width: u32, height: u32, scale: f32) {
        if let Ok(mut geometry) = self.geometry.lock() {
            *geometry = (width, height, scale);
        }
    }

    /// Stores a paint of `width` by `height` pixels from the browser's BGRA `bytes`.
    fn store(&self, width: u32, height: u32, bytes: &[u8]) {
        let mut rgba = bytes.to_vec();
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let generation = self.software.fetch_add(1, Ordering::Relaxed) + 1;
        if let Ok(mut frame) = self.frame.lock() {
            *frame = Some(Picture {
                width,
                height,
                generation,
                rgba: Arc::new(rgba),
            });
        }
    }
}

/// How the engine's cursor kinds look to the application.
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
        shared: Arc<Shared>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let (width, height, _) = self.shared.geometry.lock().map(|g| *g).unwrap_or((1, 1, 1.0));
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
            let (_, _, scale) = self.shared.geometry.lock().map(|g| *g).unwrap_or((1, 1, 1.0));
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
            self.shared.store(width as u32, height as u32, bytes);
        }

        fn on_accelerated_paint(
            &self,
            _browser: Option<&mut Browser>,
            _type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            _info: Option<&AcceleratedPaintInfo>,
        ) {
            self.shared.shared_texture.fetch_add(1, Ordering::Relaxed);
        }
    }
}

wrap_life_span_handler! {
    pub(crate) struct LifeHandler {
        shared: Arc<Shared>,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(browser) = browser {
                self.shared.set_browser(browser.clone());
            }
        }
    }
}

wrap_display_handler! {
    pub(crate) struct ReportHandler {
        shared: Arc<Shared>,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let (Some(title), Ok(mut about)) = (title, self.shared.about.lock()) {
                about.0 = title.to_string();
            }
        }

        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut cef::Frame>,
            url: Option<&CefString>,
        ) {
            if let (Some(url), Ok(mut about)) = (url, self.shared.about.lock()) {
                about.1 = url.to_string();
            }
        }

        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: ::std::os::raw::c_ulong,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> ::std::os::raw::c_int {
            if let Ok(mut about) = self.shared.about.lock() {
                about.2 = cursor_of(type_);
            }
            true as _
        }
    }
}

wrap_client! {
    pub(crate) struct PageClient {
        render: RenderHandler,
        life: LifeSpanHandler,
        display: DisplayHandler,
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
    }
}

/// The handlers for a page whose callbacks fill `shared`.
pub(crate) fn client_for(shared: &Arc<Shared>) -> Client {
    PageClient::new(
        PaintHandler::new(shared.clone()),
        LifeHandler::new(shared.clone()),
        ReportHandler::new(shared.clone()),
    )
}
