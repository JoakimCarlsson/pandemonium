//! The connection to the helper: what is sent to it, and what it reported about each page.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::chrome::PageReport;
use crate::damage::Damage;
use crate::frame::Frame;
use crate::frame::Region;
use crate::input::Cursor;
use crate::life::{Lifecycle, LoadFailure, Popup};
use crate::render::Rendered;
use crate::surface::SurfaceReader;
use crate::wire::{
    Capture, CertificateInfo, DownloadState, Event, PickedElement, Request, ToolResult, line,
};

/// Pixels of a replaced picture kept for reuse, with the paint they hold when they are known to hold a whole one.
type Spare = (Arc<[u8]>, Option<u64>);

/// What the reader copied or cut in a page, for the window to put on its clipboard.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Copied {
    /// What was marked, as plain text.
    pub text: String,
    /// What was marked, as HTML; empty when the page gave none.
    pub html: String,
}

/// Something the helper reported about a page that the reader is to see or answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    /// The page asks for something of the reader's machine, and waits for an answer.
    Permission {
        /// The number of the question, for the answer.
        id: u64,
        /// The origin that asks.
        origin: String,
        /// What it asks for, in words.
        permission: String,
    },
    /// A question is no longer open.
    PermissionCancelled {
        /// The number of the question.
        id: u64,
    },
    /// The certificate of the site being opened is not trusted.
    Certificate(Box<CertificateInfo>),
    /// The certificate error no longer applies.
    CertificateCleared,
    /// A download started, finished or failed.
    Download {
        /// The name the file is saved under.
        name: String,
        /// How it went.
        state: DownloadState,
    },
}

/// What the helper reported about one page.
#[derive(Default)]
pub(crate) struct PageState {
    /// What the reader is to see or answer, in the order it was reported, until the window takes it.
    notices: Mutex<Vec<Notice>>,
    /// The newest picture that was copied out of the shared files.
    frame: Mutex<Option<Frame>>,
    /// The tool calls that ended since the window last asked.
    tools: Mutex<Vec<ToolResult>>,
    /// A pick the window has not taken yet: the element, or nothing when the mode ended without one.
    picked: Mutex<Option<Option<PickedElement>>>,
    /// What the reader asked to add to the prompt that the window has not taken yet.
    captured: Mutex<Vec<Capture>>,
    /// The text the reader copied or cut in the page that the window has not put on its clipboard yet, oldest first.
    copied: Mutex<Vec<Copied>>,
    /// The new windows the page asked for that the window has not opened yet: the address, whether an agent's tool was running, and the profile a script's context asked for.
    opened: Mutex<Vec<(String, bool, Option<String>)>>,
    /// Which file the newest paint is in, and how many paints there have been.
    latest: Mutex<Option<(u8, u64)>>,
    /// The title, the address and how the pointer should look.
    pub(crate) about: Mutex<(String, String, Cursor)>,
    /// A count of the search's matches the window has not taken yet: which is current and how many there are.
    pub(crate) found: Mutex<Option<(u32, u32)>>,
    /// The page's zoom as a factor, when the helper reported it and the window has not taken it yet.
    pub(crate) zoom: Mutex<Option<f32>>,
    /// Where the page is in its loading, history and health, as the helper last said.
    pub(crate) life: Mutex<Lifecycle>,
    /// The windows the page opened with `window.open` that the window has not taken yet.
    popups: Mutex<Vec<Popup>>,
    /// The two files the page paints into, once it has said where they are.
    surfaces: Mutex<Option<[SurfaceReader; 2]>>,
    /// The size of a picture in those files.
    size: Mutex<(u32, u32)>,
    /// The pixels of a picture that was replaced, kept to copy the next one into once nothing else holds them, with
    /// the paint they hold when they are known to hold a whole one.
    spare: Mutex<Option<Spare>>,
    /// Which parts of the picture the recent paints changed.
    damage: Mutex<Damage>,
    /// What the page reported for the pane to show around it that the window has not taken yet, oldest first.
    reports: Mutex<Vec<PageReport>>,
    /// Where the page last said its caret or the text being composed is: left, top, width and height in logical pixels.
    caret: Mutex<Option<(i32, i32, i32, i32)>>,
}

impl PageState {
    /// Where the page last said its caret or the text being composed is, if it said.
    pub(crate) fn caret(&self) -> Option<(i32, i32, i32, i32)> {
        self.caret.lock().ok().and_then(|held| *held)
    }

    /// The tool calls that ended since the last call.
    pub(crate) fn take_tools(&self) -> Vec<ToolResult> {
        self.tools
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }

    /// The text copied or cut in the page since the last call, oldest first.
    pub(crate) fn take_copied(&self) -> Vec<Copied> {
        self.copied
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }

    /// What the page reported for the pane around it since the last call, oldest first.
    pub(crate) fn take_reports(&self) -> Vec<PageReport> {
        self.reports
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }

    /// The notices reported since the last call.
    pub(crate) fn take_notices(&self) -> Vec<Notice> {
        self.notices
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }

    /// Records a notice for the window.
    fn notify(&self, notice: Notice) {
        if let Ok(mut held) = self.notices.lock() {
            held.push(notice);
        }
    }

    /// The pick the helper reported since the last call, if any.
    pub(crate) fn take_pick(&self) -> Option<Option<PickedElement>> {
        self.picked.lock().ok()?.take()
    }

    /// What the helper reported for the reader's additions to the prompt since the last call.
    pub(crate) fn take_captures(&self) -> Vec<Capture> {
        self.captured
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }

    /// The windows the page opened with `window.open` since the last call.
    pub(crate) fn take_popups(&self) -> Vec<Popup> {
        self.popups
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }

    /// The new windows the page asked for since the last call.
    pub(crate) fn take_opened(&self) -> Vec<(String, bool, Option<String>)> {
        self.opened
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }

    /// Whether the page has painted at least once.
    pub(crate) fn painted(&self) -> bool {
        self.latest.lock().is_ok_and(|latest| latest.is_some())
    }

    /// The newest picture, copied out of the shared files only when the page has painted since the last copy.
    ///
    /// The copy goes into the pixels of the picture before the one shown when nothing else holds them any more,
    /// so a page painting steadily copies into two buffers in turn instead of allocating one a paint, and only the
    /// part the paints since that picture changed is copied, as the helper writes. The files and their size are read
    /// under one lock, so a picture is never labelled with the size of files it was not copied from while the page is
    /// resized.
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
        let surfaces = self.surfaces.lock().ok()?;
        let size = self.size.lock().map(|size| *size).unwrap_or_default();
        let Some(reader) = surfaces
            .as_ref()
            .and_then(|surfaces| surfaces.get(usize::from(buffer)))
        else {
            return held.clone();
        };
        let mut spare = self.spare.lock().ok()?;
        let damage = self.damage.lock().ok()?;
        let (mut pixels, holds) = spare
            .take()
            .filter(|(pixels, _)| pixels.len() == reader.len())
            .and_then(|(mut pixels, holds)| {
                Arc::get_mut(&mut pixels)
                    .is_some()
                    .then_some((pixels, holds))
            })
            .unwrap_or_else(|| (std::iter::repeat_n(0, reader.len()).collect(), None));
        let region = holds
            .and_then(|from| damage.since(from, generation))
            .unwrap_or(Region::whole(size.0, size.1));
        let changed = held
            .as_ref()
            .filter(|frame| (frame.width, frame.height) == size)
            .and_then(|frame| {
                damage
                    .since(frame.generation, generation)
                    .map(|region| (frame.generation, region))
            });
        let read =
            Arc::get_mut(&mut pixels).is_some_and(|out| reader.read_region_into(out, region));
        if read {
            let replaced = held.replace(Frame {
                width: size.0,
                height: size.1,
                generation,
                pixels,
                bgra: reader.bgra(),
                changed,
            });
            *spare = replaced.map(|frame| (frame.pixels, Some(frame.generation)));
        } else {
            *spare = Some((pixels, None));
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
    /// The renders of sealed documents that ended since the window last asked.
    rendered: Mutex<Vec<Rendered>>,
    /// The features the helper announced, kept once it said it is ready: nothing until then, and an empty list from a helper that announced none.
    features: Mutex<Option<Vec<String>>>,
    /// The features announced before the helper said it is ready.
    announced: Mutex<Vec<String>>,
}

impl Link {
    /// A connection that writes requests to `writer`.
    pub(crate) fn new(writer: Box<dyn Write + Send>) -> Arc<Self> {
        Arc::new(Self {
            writer: Mutex::new(writer),
            pages: Mutex::new(HashMap::new()),
            failure: Mutex::new(None),
            next_page: AtomicU32::new(1),
            rendered: Mutex::new(Vec::new()),
            features: Mutex::new(None),
            announced: Mutex::new(Vec::new()),
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

    /// The renders of sealed documents that ended since the last call.
    pub(crate) fn take_rendered(&self) -> Vec<Rendered> {
        self.rendered
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }

    /// Whether the helper announced `feature` before it said it is ready; nothing while it has not said so yet.
    pub(crate) fn supports(&self, feature: &str) -> Option<bool> {
        self.features
            .lock()
            .ok()?
            .as_ref()
            .map(|features| features.iter().any(|held| held == feature))
    }

    /// Whether the helper said it is ready, after which its features are known.
    fn ready(&self) -> bool {
        self.features
            .lock()
            .is_ok_and(|features| features.is_some())
    }

    /// Reads what the helper reports from `reader` until it says it is ready, the reader gives out or fails, as when its timeout passes; what it read is taken in as [`Link::listen`] takes it in.
    pub(crate) fn await_ready(&self, reader: &mut dyn BufRead) {
        let mut text = Vec::new();
        while !self.ready() {
            text.clear();
            match reader.read_until(b'\n', &mut text) {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            if let Ok(event) = serde_json::from_slice::<Event>(&text) {
                self.handle(event);
            }
        }
    }

    /// The state of `page`, if it is open.
    fn page(&self, page: u32) -> Option<Arc<PageState>> {
        self.pages.lock().ok()?.get(&page).cloned()
    }

    /// Changes what is known of the life of `page` with `change`, if the page is open.
    fn change_life(&self, page: u32, change: impl FnOnce(&mut Lifecycle)) {
        if let Some(state) = self.page(page)
            && let Ok(mut life) = state.life.lock()
        {
            change(&mut life);
        }
    }

    /// Takes in what the helper reports until it hangs up. A line is read as bytes up to its newline and only then
    /// decoded, so a line that is not valid text is skipped rather than ending the connection.
    pub(crate) fn listen(&self, mut reader: Box<dyn BufRead + Send>) {
        let mut text = Vec::new();
        loop {
            text.clear();
            match reader.read_until(b'\n', &mut text) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if let Ok(event) = serde_json::from_slice::<Event>(&text) {
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
            Event::Ready => {
                let announced = self
                    .announced
                    .lock()
                    .map(|mut held| std::mem::take(&mut *held))
                    .unwrap_or_default();
                if let Ok(mut features) = self.features.lock() {
                    features.get_or_insert(announced);
                }
            }
            Event::Features { features } => {
                if let Ok(mut announced) = self.announced.lock() {
                    announced.extend(features);
                }
            }
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
                let opened = match (open(&paths[0]), open(&paths[1])) {
                    (Some(first), Some(second)) => Some([first, second]),
                    _ => None,
                };
                if let (Ok(mut surfaces), Ok(mut size), Ok(mut damage)) = (
                    state.surfaces.lock(),
                    state.size.lock(),
                    state.damage.lock(),
                ) {
                    *surfaces = opened;
                    *size = (width, height);
                    damage.surface(width, height);
                }
            }
            Event::Painted {
                page,
                buffer,
                generation,
                dirty,
            } => {
                if let Some(state) = self.page(page) {
                    if let Ok(mut damage) = state.damage.lock() {
                        damage.painted(generation, dirty);
                    }
                    if let Ok(mut latest) = state.latest.lock() {
                        *latest = Some((buffer, generation));
                    }
                }
            }
            Event::Picked { page, element } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut picked) = state.picked.lock()
                {
                    *picked = Some(element.map(|element| *element));
                }
            }
            Event::Captured { page, capture } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut captured) = state.captured.lock()
                {
                    captured.push(*capture);
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
            Event::Caret {
                page,
                x,
                y,
                width,
                height,
            } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut caret) = state.caret.lock()
                {
                    *caret = Some((x, y, width, height));
                }
            }
            Event::Found {
                page,
                active,
                total,
            } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut found) = state.found.lock()
                {
                    *found = Some((active, total));
                }
            }
            Event::Zoomed { page, factor } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut zoom) = state.zoom.lock()
                {
                    *zoom = Some(factor);
                }
            }
            Event::PermissionAsked {
                page,
                id,
                origin,
                permission,
            } => {
                if let Some(state) = self.page(page) {
                    state.notify(Notice::Permission {
                        id,
                        origin,
                        permission,
                    });
                }
            }
            Event::PermissionCancelled { page, id } => {
                if let Some(state) = self.page(page) {
                    state.notify(Notice::PermissionCancelled { id });
                }
            }
            Event::CertificateError { page, info } => {
                if let Some(state) = self.page(page) {
                    state.notify(Notice::Certificate(info));
                }
            }
            Event::CertificateCleared { page } => {
                if let Some(state) = self.page(page) {
                    state.notify(Notice::CertificateCleared);
                }
            }
            Event::Download { page, name, state } => {
                if let Some(held) = self.page(page) {
                    held.notify(Notice::Download { name, state });
                }
            }
            Event::OpenRequested {
                page,
                url,
                agent,
                partition,
            } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut opened) = state.opened.lock()
                {
                    opened.push((url, agent, partition));
                }
            }
            Event::Chrome { page, report } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut reports) = state.reports.lock()
                {
                    reports.push(report);
                }
            }
            Event::Copied { page, text, html } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut copied) = state.copied.lock()
                {
                    copied.push(Copied { text, html });
                }
            }
            Event::Loading {
                page,
                loading,
                progress,
            } => self.change_life(page, |life| {
                if loading && !life.loading {
                    life.failure = None;
                    life.crashed = None;
                    life.unresponsive = false;
                }
                life.loading = loading;
                life.progress = progress;
            }),
            Event::LoadFailed {
                page,
                url,
                code,
                name,
            } => self.change_life(page, |life| {
                life.failure = Some(LoadFailure { url, code, name });
            }),
            Event::Crashed { page, reason } => self.change_life(page, |life| {
                life.crashed = Some(reason);
                life.loading = false;
            }),
            Event::Responsive { page, responsive } => {
                self.change_life(page, |life| life.unresponsive = !responsive);
            }
            Event::History {
                page,
                back,
                forward,
            } => self.change_life(page, |life| {
                life.history_known = true;
                life.back = back;
                life.forward = forward;
            }),
            Event::PageClosed { page } => self.change_life(page, |life| life.closed = true),
            Event::PopupOpened {
                page,
                popup,
                url,
                agent,
            } => {
                if let Some(state) = self.page(page)
                    && let Ok(mut popups) = state.popups.lock()
                {
                    popups.push(Popup { popup, url, agent });
                }
            }
            Event::HtmlRendered {
                ticket,
                path,
                error,
            } => {
                if let Ok(mut rendered) = self.rendered.lock() {
                    rendered.push(Rendered {
                        ticket,
                        outcome: if error.is_empty() && !path.is_empty() {
                            Ok(path.into())
                        } else if error.is_empty() {
                            Err("the helper rendered no picture".to_owned())
                        } else {
                            Err(error)
                        },
                    });
                }
            }
        }
    }
}

#[cfg(test)]
/// Tests for what the connection makes of the features the helper announces.
mod tests {
    use super::*;
    use crate::wire::SEALED_PAGES;

    /// A connection whose requests go nowhere.
    fn link() -> Arc<Link> {
        Link::new(Box::new(std::io::sink()))
    }

    /// Nothing is known of the helper's features before it is ready; a helper that announced sealed pages has them, and one that said only that it is ready, as older helpers do, has none.
    #[test]
    fn sealed_pages_are_known_from_what_the_helper_announced() {
        let current = link();
        assert_eq!(current.supports(SEALED_PAGES), None);
        current.handle(Event::Features {
            features: vec![SEALED_PAGES.to_owned()],
        });
        assert_eq!(current.supports(SEALED_PAGES), None);
        current.handle(Event::Ready);
        assert_eq!(current.supports(SEALED_PAGES), Some(true));
        assert_eq!(current.supports("something-else"), Some(false));
        let older = link();
        older.handle(Event::Ready);
        assert_eq!(older.supports(SEALED_PAGES), Some(false));
    }

    /// Waiting for the helper to be ready reads its announcement and stops at `Ready`, leaving what follows for the listener; a helper that hangs up first leaves the features unknown.
    #[test]
    fn waiting_for_ready_reads_the_announcement_and_stops_there() {
        let current = link();
        let mut lines = std::io::Cursor::new(
            b"{\"Features\":{\"features\":[\"sealed-pages\"]}}\n\"Ready\"\n{\"Title\":{\"page\":1,\"title\":\"x\"}}\n".to_vec(),
        );
        current.await_ready(&mut lines);
        assert_eq!(current.supports(SEALED_PAGES), Some(true));
        let mut rest = String::new();
        let _ = lines.read_line(&mut rest);
        assert!(rest.starts_with("{\"Title\""));
        let silent = link();
        silent.await_ready(&mut std::io::Cursor::new(Vec::new()));
        assert_eq!(silent.supports(SEALED_PAGES), None);
    }

    /// A failed load stays until the next load starts, and the history is known once the helper reports it.
    #[test]
    fn a_failed_load_lasts_until_the_next_load() {
        let current = link();
        let (page, state) = current.register();
        current.handle(Event::Loading {
            page,
            loading: true,
            progress: 0.1,
        });
        current.handle(Event::LoadFailed {
            page,
            url: "http://nowhere.test/".to_owned(),
            code: -105,
            name: "ERR_NAME_NOT_RESOLVED".to_owned(),
        });
        current.handle(Event::Loading {
            page,
            loading: false,
            progress: 1.0,
        });
        current.handle(Event::History {
            page,
            back: true,
            forward: false,
        });
        let life = state
            .life
            .lock()
            .map(|life| life.clone())
            .unwrap_or_default();
        assert_eq!(life.failure.map(|failure| failure.code), Some(-105));
        assert!(life.history_known && life.back && !life.forward && !life.loading);
        current.handle(Event::Loading {
            page,
            loading: true,
            progress: 0.1,
        });
        assert!(state.life.lock().is_ok_and(|life| life.failure.is_none()));
    }
}
