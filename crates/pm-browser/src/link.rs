//! The connection to the helper: what is sent to it, and what it reported about each page.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::frame::Frame;
use crate::input::Cursor;
use crate::surface::SurfaceReader;
use crate::wire::{Event, PickedElement, Request, ToolResult, line};

/// What the helper reported about one page.
#[derive(Default)]
pub(crate) struct PageState {
    /// The newest picture that was copied out of the shared files.
    frame: Mutex<Option<Frame>>,
    /// The tool calls that ended since the window last asked.
    tools: Mutex<Vec<ToolResult>>,
    /// A pick the window has not taken yet: the element, or nothing when the mode ended without one.
    picked: Mutex<Option<Option<PickedElement>>>,
    /// Which file the newest paint is in, and how many paints there have been.
    latest: Mutex<Option<(u8, u64)>>,
    /// The title, the address and how the pointer should look.
    pub(crate) about: Mutex<(String, String, Cursor)>,
    /// The two files the page paints into, once it has said where they are.
    surfaces: Mutex<Option<[SurfaceReader; 2]>>,
    /// The size of a picture in those files.
    size: Mutex<(u32, u32)>,
}

impl PageState {
    /// The tool calls that ended since the last call.
    pub(crate) fn take_tools(&self) -> Vec<ToolResult> {
        self.tools
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }

    /// The pick the helper reported since the last call, if any.
    pub(crate) fn take_pick(&self) -> Option<Option<PickedElement>> {
        self.picked.lock().ok()?.take()
    }

    /// The newest picture, copied out of the shared files only when the page has painted since the last copy.
    pub(crate) fn frame(&self) -> Option<Frame> {
        let latest = *self.latest.lock().ok()?;
        let mut held = self.frame.lock().ok()?;
        let Some((buffer, generation)) = latest else {
            return held.clone();
        };
        if held
            .as_ref()
            .is_some_and(|frame| frame.generation >= generation)
        {
            return held.clone();
        }
        let size = self.size.lock().map(|size| *size).unwrap_or_default();
        let pixels = self
            .surfaces
            .lock()
            .ok()
            .and_then(|surfaces| surfaces.as_ref()?.get(usize::from(buffer))?.read());
        if let Some(pixels) = pixels {
            *held = Some(Frame {
                width: size.0,
                height: size.1,
                generation,
                rgba: Arc::new(pixels),
            });
        }
        held.clone()
    }
}

/// The connection to the helper.
pub(crate) struct Link {
    /// Where requests are written.
    writer: Mutex<Box<dyn Write + Send>>,
    /// The pages that are open, by number.
    pages: Mutex<HashMap<u32, Arc<PageState>>>,
    /// Why the helper failed, once it did.
    failure: Mutex<Option<String>>,
    /// The number the next page gets.
    next_page: AtomicU32,
}

impl Link {
    /// A connection that writes requests to `writer`.
    pub(crate) fn new(writer: Box<dyn Write + Send>) -> Arc<Self> {
        Arc::new(Self {
            writer: Mutex::new(writer),
            pages: Mutex::new(HashMap::new()),
            failure: Mutex::new(None),
            next_page: AtomicU32::new(1),
        })
    }

    /// Sends `request` to the helper.
    pub(crate) fn send(&self, request: &Request) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(&line(request));
            let _ = writer.flush();
        }
    }

    /// Makes room for a new page and gives its number.
    pub(crate) fn register(&self) -> (u32, Arc<PageState>) {
        let page = self.next_page.fetch_add(1, Ordering::Relaxed);
        let state = Arc::new(PageState::default());
        if let Ok(mut pages) = self.pages.lock() {
            pages.insert(page, state.clone());
        }
        (page, state)
    }

    /// Forgets a page.
    pub(crate) fn forget(&self, page: u32) {
        if let Ok(mut pages) = self.pages.lock() {
            pages.remove(&page);
        }
    }

    /// Why the helper failed, if it did.
    pub(crate) fn failure(&self) -> Option<String> {
        self.failure.lock().ok()?.clone()
    }

    /// The state of `page`, if it is open.
    fn page(&self, page: u32) -> Option<Arc<PageState>> {
        self.pages.lock().ok()?.get(&page).cloned()
    }

    /// Takes in what the helper reports until it hangs up.
    pub(crate) fn listen(&self, mut reader: Box<dyn BufRead + Send>) {
        let mut text = String::new();
        loop {
            text.clear();
            match reader.read_line(&mut text) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if let Ok(event) = serde_json::from_str::<Event>(&text) {
                self.handle(event);
            }
        }
        if let Ok(mut failure) = self.failure.lock() {
            failure.get_or_insert_with(|| "the helper stopped".to_owned());
        }
    }

    /// Records one report.
    fn handle(&self, event: Event) {
        match event {
            Event::Ready => {}
            Event::Failed { message } => {
                if let Ok(mut failure) = self.failure.lock() {
                    *failure = Some(message);
                }
            }
            Event::Surface {
                page,
                width,
                height,
                paths,
            } => {
                let Some(state) = self.page(page) else {
                    return;
                };
                let open = |path: &String| SurfaceReader::open(path.as_ref(), width, height).ok();
                if let (Some(first), Some(second)) = (open(&paths[0]), open(&paths[1]))
                    && let (Ok(mut surfaces), Ok(mut size)) =
                        (state.surfaces.lock(), state.size.lock())
                {
                    *surfaces = Some([first, second]);
                    *size = (width, height);
                }
            }
            Event::Painted {
                page,
                buffer,
                generation,
            } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut latest) = state.latest.lock()
                {
                    *latest = Some((buffer, generation));
                }
            }
            Event::Picked { page, element } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut picked) = state.picked.lock()
                {
                    *picked = Some(element.map(|element| *element));
                }
            }
            Event::ToolDone { page, result } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut tools) = state.tools.lock()
                {
                    tools.push(*result);
                }
            }
            Event::Title { page, title } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut about) = state.about.lock()
                {
                    about.0 = title;
                }
            }
            Event::Address { page, url } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut about) = state.about.lock()
                {
                    about.1 = url;
                }
            }
            Event::Cursor { page, cursor } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut about) = state.about.lock()
                {
                    about.2 = cursor;
                }
            }
        }
    }
}
