//! The engine behind the browser pane: starts the helper, opens a page for each tab, carries out what the tabs queued, hands the pages the pointer and keys, and takes back their pictures, titles, addresses and questions.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use pm_browser::{EditAction, Engine, Error, PageInput, ToolResult, View, ZoomAction};
use pm_config::GeneralSettings;

use crate::browser::{Browser, Command, Picture, ZoomStep};
use crate::browser_action::Surface;
use crate::browser_safety::{ALLOW_DELAY, Answer};
use crate::browser_scope::BrowserShelf;
use crate::composer::{Capture, CaptureKind, PickedElement};

/// What wakes the window's loop.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// The size a page is opened at before its window was laid out, in logical pixels.
const FIRST_SIZE: (u32, u32) = (960, 640);

/// What a page handed back for the next prompt.
#[derive(Clone, Debug, PartialEq)]
pub enum Attachment {
    /// An element the reader picked.
    Element(PickedElement),
    /// A screenshot or the console the reader asked for.
    Capture(Capture),
    /// Why what the reader asked for could not be taken.
    Failed(String),
}

/// The element `element` picked on the page at `address`, as the composer attaches it.
fn picked_element(element: &pm_browser::PickedElement, address: &str) -> PickedElement {
    let attributes: String = element
        .attributes
        .iter()
        .map(|(name, value)| format!(" {name}=\"{value}\""))
        .collect();
    let label = if element.selector.is_empty() {
        element.tag.clone()
    } else {
        element.selector.clone()
    };
    PickedElement {
        label,
        address: address.to_owned(),
        markup: format!(
            "<{tag}{attributes}>{}</{tag}>",
            element.text,
            tag = element.tag
        ),
    }
}

/// What `capture` from the page at `address` attaches to the prompt: the console's lines, the first picture read from where the helper saved it, or why there is nothing.
fn captured(capture: pm_browser::Capture, address: &str) -> Option<Attachment> {
    if !capture.error.is_empty() {
        return Some(Attachment::Failed(capture.error));
    }
    let label = if capture.title.is_empty() {
        address.to_owned()
    } else {
        capture.title.clone()
    };
    if capture.kind == pm_browser::CaptureKind::Console {
        return Some(Attachment::Capture(Capture {
            kind: CaptureKind::Console,
            label,
            data: capture.lines.join("\n").into_bytes(),
        }));
    }
    let picture = capture.pictures.first()?;
    let read = std::fs::read(&picture.path);
    for taken in &capture.pictures {
        let _ = std::fs::remove_file(&taken.path);
    }
    Some(match read {
        Ok(data) => Attachment::Capture(Capture {
            kind: CaptureKind::Screenshot,
            label,
            data,
        }),
        Err(error) => Attachment::Failed(format!("Could not read the screenshot: {error}")),
    })
}

/// The page of one tab and what it was last handed.
struct Shown {
    /// The page.
    view: View,
    /// The size it was last given, in logical pixels, and the scale.
    sized: Option<(u32, u32, f32)>,
}

/// The engine and the pages it holds.
pub struct Web {
    /// What wakes the window when a page reports something.
    wake: Wake,
    /// The engine, once a page was first opened.
    engine: Option<Engine>,
    /// The page of each tab that has one, by the tab's number.
    pages: HashMap<u32, Shown>,
    /// The developer tools, with the number of the tab whose page they are of, while they are open.
    tools: Option<(u32, View)>,
    /// The size the developer tools were last given.
    tools_sized: Option<(u32, u32, f32)>,
    /// The size the pane last gave the window of the page, in logical pixels, and the scale.
    page_room: Option<(u32, u32, f32)>,
    /// The size the pane last gave the window of the developer tools.
    tools_room: Option<(u32, u32, f32)>,
    /// Whether the window of the page and of the tools hold the keyboard.
    focus: (bool, bool),
    /// The tab shown and the focus as the pages were last told.
    told_focus: Option<(Option<u32>, (bool, bool))>,
    /// Why the helper could not be started, once it could not.
    reason: Option<String>,
    /// The download of the browser while it runs.
    installing: Option<Receiver<Result<(), String>>>,
    /// What the download says it is doing, written by its thread.
    download_note: Arc<Mutex<String>>,
    /// Why the last download failed, so it is not started again until the setting is turned on again.
    install_failed: Option<String>,
    /// Whether the setting was on when last looked at.
    was_enabled: bool,
    /// The local port that carries each remote port, by machine and remote port.
    forwards: HashMap<(String, u16), u16>,
    /// The restriction on agent browsing and the origins listed, as the helper was last told.
    sent_origins: Option<(bool, Vec<String>)>,
    /// The tool calls of agents that ended and the server has not been told of.
    results: Vec<ToolResult>,
    /// Whether the server was last told this window can run the tools of agents.
    announced: Option<bool>,
    /// What the pages handed back for the next prompt and the link has not taken.
    attachments: Vec<Attachment>,
}

/// An address cut at its host and port.
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

/// Carries out a command that needs only the page; returns why it failed, when it did.
fn page_command(page: &View, command: Command) -> Option<String> {
    match command {
        Command::Cut => page.edit(EditAction::Cut),
        Command::Copy => page.edit(EditAction::Copy),
        Command::Paste => page.edit(EditAction::Paste),
        Command::SelectAll => page.edit(EditAction::SelectAll),
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
        Command::OpenExternal => return pm_harness::open_address(&page.address()).err(),
        Command::ClearData => page.clear_data(),
        Command::Pick(on) => page.pick(on),
        Command::Capture(kind) => page.capture(kind),
        Command::PickArea(on) => page.pick_area(on),
        Command::Load(_) | Command::OpenTools | Command::CloseTools => {}
    }
    None
}

impl Web {
    /// An engine not yet started, which calls `wake` when a page reports something.
    pub fn new(wake: Wake) -> Self {
        Self {
            wake,
            engine: None,
            pages: HashMap::new(),
            tools: None,
            tools_sized: None,
            page_room: None,
            tools_room: None,
            focus: (false, false),
            told_focus: None,
            reason: None,
            installing: None,
            download_note: Arc::new(Mutex::new(String::new())),
            install_failed: None,
            was_enabled: false,
            forwards: HashMap::new(),
            sent_origins: None,
            results: Vec::new(),
            announced: None,
            attachments: Vec::new(),
        }
    }

    /// Whether the server is to be told anew if this window can run the tools of agents, and what: the browser is turned on in `general` and installed.
    pub fn announcement(&mut self, general: &GeneralSettings) -> Option<bool> {
        let usable = Self::ready(general) && pm_browser::install::supported();
        (self.announced != Some(usable)).then(|| {
            self.announced = Some(usable);
            usable
        })
    }

    /// What the servers were last told about the agents' browser tools, once they were.
    pub fn announced(&self) -> Option<bool> {
        self.announced
    }

    /// Forgets what the server was told, as when the connection to it started again.
    pub fn forget_announcement(&mut self) {
        self.announced = None;
    }

    /// What the pages handed back for the next prompt since the last call.
    pub fn take_attachments(&mut self) -> Vec<Attachment> {
        std::mem::take(&mut self.attachments)
    }

    /// The tool calls of agents that ended since the last call.
    pub fn take_results(&mut self) -> Vec<ToolResult> {
        std::mem::take(&mut self.results)
    }

    /// Carries out what the tabs of `browser` queued and the answers given in them, without the rest of [`Web::run`], for work on the browser of a session not shown.
    pub fn carry_out(&mut self, browser: &mut Option<Browser>, general: &GeneralSettings) {
        self.commands(browser, general);
        self.answers(browser);
    }

    /// Runs the tool `tool` of an agent, numbered `call`, with `arguments` as JSON text, on the page of the tab `tab`; says whether the tab has a page to run it on.
    pub fn tool(&self, tab: u32, call: u64, tool: &str, arguments: &str) -> bool {
        match self.pages.get(&tab) {
            Some(page) => {
                page.view.tool(call, tool, arguments);
                true
            }
            None => false,
        }
    }

    /// The address an agent's tool opens for `address` of a page on `host`, carried to that machine's port when it is local there.
    ///
    /// # Errors
    ///
    /// Says so when no transport can carry the page of that machine.
    pub fn resolve_for_tool(
        &mut self,
        address: &str,
        host: Option<&str>,
    ) -> Result<String, String> {
        self.resolve(address, host)
    }

    /// Whether the browser is turned on in `general`, can run on this machine and is installed, so pages can be shown.
    pub fn ready(general: &GeneralSettings) -> bool {
        general.embedded_browser
            && pm_browser::install::supported()
            && pm_browser::install::installed()
    }

    /// Takes in a size the pane gave a page's window.
    pub fn resized(&mut self, surface: Surface, width: u32, height: u32, scale: f32) {
        let room = Some((width.max(1), height.max(1), scale));
        match surface {
            Surface::Page => self.page_room = room,
            Surface::Tools => self.tools_room = room,
        }
    }

    /// Takes in whether a page's window holds the keyboard.
    pub fn focused(&mut self, surface: Surface, focused: bool) {
        match surface {
            Surface::Page => self.focus.0 = focused,
            Surface::Tools => self.focus.1 = focused,
        }
    }

    /// Hands `input` to the page of the tab shown in `browser`, or to its developer tools.
    pub fn input(&self, browser: Option<&Browser>, surface: Surface, input: &PageInput) {
        let view = match surface {
            Surface::Page => browser
                .and_then(|browser| self.pages.get(&browser.active_id()))
                .map(|shown| &shown.view),
            Surface::Tools => self.tools.as_ref().map(|(_, view)| view),
        };
        if let Some(view) = view {
            view.input(input);
        }
    }

    /// Gives the engine time: downloads the browser when it is turned on and missing, carries out what the shown browser queued, closes the pages of tabs that are gone, and takes in what every page reported; returns whether the browsers changed.
    pub fn run(
        &mut self,
        shown: &mut Option<Browser>,
        shelf: &mut BrowserShelf,
        general: &GeneralSettings,
    ) -> bool {
        let mut changed = self.install(shown, shelf, general);
        self.origins(general);
        changed |= self.commands(shown, general);
        self.answers(shown);
        self.sync(shown, shelf);
        self.fit(shown);
        self.tell_focus(shown);
        changed |= self.present(shown, shelf);
        changed
    }

    /// Downloads the browser when it is turned on and missing, though no page is open, and takes in how a download went; returns whether that changed anything shown.
    pub fn prepare(
        &mut self,
        shown: &mut Option<Browser>,
        shelf: &mut BrowserShelf,
        general: &GeneralSettings,
    ) -> bool {
        self.install(shown, shelf, general)
    }

    /// How the download of the browser stands while one runs, or why the last one failed.
    pub fn download_status(&self) -> Option<String> {
        if self.installing.is_some() {
            let note = self
                .download_note
                .lock()
                .map(|held| held.clone())
                .unwrap_or_default();
            return Some(if note.is_empty() {
                "Downloading the embedded browser, about 120 MB…".to_owned()
            } else {
                note
            });
        }
        self.install_failed.clone()
    }

    /// Closes every page and ends the helper.
    pub fn shutdown(&mut self) {
        self.tools = None;
        self.pages.clear();
        self.engine = None;
    }

    /// Starts the download of the browser when it is turned on and missing, and takes in how a download went; returns whether the browsers changed.
    fn install(
        &mut self,
        shown: &mut Option<Browser>,
        shelf: &mut BrowserShelf,
        general: &GeneralSettings,
    ) -> bool {
        let enabled = general.embedded_browser;
        if enabled != self.was_enabled {
            self.was_enabled = enabled;
            self.install_failed = None;
            self.reason = None;
        }
        if let Some(download) = self.installing.as_ref() {
            let outcome = match download.try_recv() {
                Err(TryRecvError::Empty) => return false,
                Ok(outcome) => outcome,
                Err(TryRecvError::Disconnected) => Err("the download stopped".to_owned()),
            };
            self.installing = None;
            match outcome {
                Ok(()) => retry(shown, shelf),
                Err(message) => {
                    let note = format!("The embedded browser could not be installed: {message}");
                    notice_all(shown, shelf, &note);
                    self.install_failed = Some(note);
                }
            }
            return true;
        }
        if !enabled
            || self.install_failed.is_some()
            || !pm_browser::install::supported()
            || pm_browser::install::installed()
        {
            return false;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        let note = self.download_note.clone();
        let wake = self.wake.clone();
        std::thread::spawn(move || {
            let say = |text: String| {
                if let Ok(mut held) = note.lock()
                    && *held != text
                {
                    *held = text;
                    wake();
                }
            };
            let outcome = pm_browser::install::install(&|step| match step {
                pm_browser::install::Step::Downloading(bytes) => say(format!(
                    "Downloading the embedded browser: {} of about 120 MB.",
                    bytes / 1_000_000
                )),
                pm_browser::install::Step::Verifying => {
                    say("Checking the embedded browser download.".to_owned())
                }
                pm_browser::install::Step::Unpacking => {
                    say("Unpacking the embedded browser.".to_owned())
                }
            });
            let _ = sender.send(outcome);
            wake();
        });
        self.installing = Some(receiver);
        notice_all(
            shown,
            shelf,
            "Downloading the embedded browser, about 120 MB...",
        );
        true
    }

    /// Tells the helper which origins agents may open when that is not what it was last told.
    fn origins(&mut self, general: &GeneralSettings) {
        let wanted = (
            general.restrict_agent_browsing,
            general.agent_origins.clone(),
        );
        if self.sent_origins.as_ref() == Some(&wanted) {
            return;
        }
        if let Some(engine) = self.engine.as_ref() {
            engine.set_agent_origins(wanted.0, &wanted.1);
            self.sent_origins = Some(wanted);
        }
    }

    /// Starts the helper when it is not running; says why when it cannot be.
    fn engine(&mut self, general: &GeneralSettings) -> Result<&Engine, String> {
        if self.engine.is_none() && self.reason.is_none() {
            if !general.embedded_browser {
                return Err("The embedded browser is turned off. Turn it on in Settings under Browser to download it.".to_owned());
            }
            if self.installing.is_some() {
                return Err("Downloading the embedded browser, about 120 MB...".to_owned());
            }
            match Engine::start() {
                Ok(engine) => {
                    engine.set_waker(self.wake.clone());
                    self.engine = Some(engine);
                    self.sent_origins = None;
                    self.origins(general);
                }
                Err(Error::NotInstalled) => {
                    return Err(self.install_failed.clone().unwrap_or_else(|| {
                        "The embedded browser is not installed. It is downloaded when you turn it on in Settings under Browser.".to_owned()
                    }));
                }
                Err(other) => self.reason = Some(other.to_string()),
            }
        }
        match (&self.engine, &self.reason) {
            (Some(engine), _) => Ok(engine),
            (None, Some(reason)) => Err(reason.clone()),
            (None, None) => Err("The embedded browser could not be started.".to_owned()),
        }
    }

    /// The address to open for `address` of a page on `host`: a local one is carried to that machine's port.
    ///
    /// # Errors
    ///
    /// Says so when no transport can carry the page of that machine.
    fn resolve(&mut self, address: &str, host: Option<&str>) -> Result<String, String> {
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
                let local = match self.forwards.get(&key) {
                    Some(local) => *local,
                    None => {
                        let local = pm_ssh::forward(destination, port)?;
                        self.forwards.insert(key, local);
                        local
                    }
                };
                Ok(format!("{}127.0.0.1:{local}{}", cut.before, cut.after))
            }
            pm_protocol::Route::Direct(target) => {
                let machine = target.rsplit_once(':').map_or(target, |(name, _)| name);
                Ok(format!("{}{machine}:{port}{}", cut.before, cut.after))
            }
            pm_protocol::Route::Local | pm_protocol::Route::Wsl(_) => Ok(address.to_owned()),
            _ => Err(
                "Pages of this kind of machine cannot be opened here yet: only ssh, tcp and WSL machines are supported."
                    .to_owned(),
            ),
        }
    }

    /// How a page shows its address: a local port that carries a remote one is shown as the remote port on `localhost`, so the address typed, shown and reported by a server agree.
    fn unresolve(&self, address: &str, host: Option<&str>) -> String {
        let Some(cut) = parts(address) else {
            return address.to_owned();
        };
        if let pm_protocol::Route::Direct(target) = pm_protocol::route_of(host) {
            let machine = target.rsplit_once(':').map_or(target, |(name, _)| name);
            if let Some(port) = cut.port.filter(|_| cut.host == machine) {
                return format!("{}localhost:{port}{}", cut.before, cut.after);
            }
        }
        if host.is_none() {
            return address.to_owned();
        }
        let Some(local) = cut.port.filter(|_| is_local(cut.host)) else {
            return address.to_owned();
        };
        match self
            .forwards
            .iter()
            .find(|(_, forwarded)| **forwarded == local)
        {
            Some(((_, remote), _)) => format!("{}localhost:{remote}{}", cut.before, cut.after),
            None => address.to_owned(),
        }
    }

    /// Carries out what the tabs of the shown browser queued; returns whether a tab changed.
    fn commands(&mut self, shown: &mut Option<Browser>, general: &GeneralSettings) -> bool {
        let Some(browser) = shown.as_mut() else {
            return false;
        };
        let commands = browser.take_commands();
        let changed = !commands.is_empty();
        for (tab, command) in commands {
            match command {
                Command::Load(address) => self.load(browser, tab, &address, general),
                Command::OpenTools => {
                    let (width, height, scale) =
                        self.tools_room
                            .unwrap_or((FIRST_SIZE.0 / 2, FIRST_SIZE.1, 1.0));
                    if let Some(page) = self.pages.get(&tab) {
                        self.tools = Some((tab, page.view.open_devtools(width, height, scale)));
                        self.tools_sized = Some((width, height, scale));
                        self.told_focus = None;
                    }
                }
                Command::CloseTools => {
                    self.tools = None;
                    self.tools_sized = None;
                }
                other => {
                    if let Some(reason) = self
                        .pages
                        .get(&tab)
                        .and_then(|page| page_command(&page.view, other))
                    {
                        self.attachments.push(Attachment::Failed(reason));
                    }
                }
            }
        }
        changed
    }

    /// Loads `address` in the tab `tab` of `browser`, opening its page first when it has none.
    fn load(&mut self, browser: &mut Browser, tab: u32, address: &str, general: &GeneralSettings) {
        let host = browser.tab(tab).and_then(|held| held.host.clone());
        let notice = |browser: &mut Browser, notice: Option<String>| {
            if let Some(held) = browser.tab_mut(tab) {
                held.notice = notice;
            }
        };
        let address = match self.resolve(address, host.as_deref()) {
            Ok(address) => address,
            Err(message) => return notice(browser, Some(message)),
        };
        if let Some(page) = self.pages.get(&tab) {
            notice(browser, None);
            page.view.load(&address);
            return;
        }
        let size = self.page_room.unwrap_or((FIRST_SIZE.0, FIRST_SIZE.1, 1.0));
        let opened = self.engine(general).and_then(|engine| {
            View::open(engine, &address, size.0, size.1, size.2).map_err(|error| error.to_string())
        });
        match opened {
            Ok(view) => {
                notice(browser, None);
                self.told_focus = None;
                self.pages.insert(
                    tab,
                    Shown {
                        view,
                        sized: Some(size),
                    },
                );
            }
            Err(message) => notice(browser, Some(message)),
        }
    }

    /// Carries the reader's answers to the questions and certificates of the shown browser's tabs to their pages.
    fn answers(&mut self, shown: &mut Option<Browser>) {
        let Some(browser) = shown.as_mut() else {
            return;
        };
        for tab in &mut browser.tabs {
            let answers = std::mem::take(&mut tab.safety.answers);
            let Some(page) = self.pages.get(&tab.id) else {
                continue;
            };
            for answer in answers {
                match answer {
                    Answer::Permission { id, allow } => page.view.answer_permission(id, allow),
                    Answer::Certificate { proceed } => page.view.certificate_choice(proceed),
                }
            }
        }
    }

    /// Closes the pages of tabs that are gone, and the developer tools of a tab that is gone or no longer shown.
    fn sync(&mut self, shown: &Option<Browser>, shelf: &BrowserShelf) {
        let open = shelf.tab_numbers(shown);
        self.pages.retain(|id, _| open.contains(id));
        let showing = shown
            .as_ref()
            .filter(|browser| browser.tools)
            .map(Browser::active_id);
        if self
            .tools
            .as_ref()
            .is_some_and(|(tab, _)| Some(*tab) != showing)
        {
            self.tools = None;
            self.tools_sized = None;
        }
    }

    /// Gives the page shown and the developer tools the room the pane has for them.
    fn fit(&mut self, shown: &Option<Browser>) {
        if let (Some(room), Some(active)) = (self.page_room, shown.as_ref().map(Browser::active_id))
            && let Some(page) = self.pages.get_mut(&active)
            && page.sized != Some(room)
        {
            page.sized = Some(room);
            page.view.resize(room.0, room.1, room.2);
        }
        if let (Some(room), Some((_, view))) = (self.tools_room, self.tools.as_ref())
            && self.tools_sized != Some(room)
        {
            self.tools_sized = Some(room);
            view.resize(room.0, room.1, room.2);
        }
    }

    /// Tells the pages which of them holds the keyboard, when that changed.
    fn tell_focus(&mut self, shown: &Option<Browser>) {
        let active = shown.as_ref().map(Browser::active_id);
        let wanted = (active, self.focus);
        if self.told_focus == Some(wanted) {
            return;
        }
        self.told_focus = Some(wanted);
        for (id, page) in &self.pages {
            page.view.focus(Some(*id) == active && self.focus.0);
        }
        if let Some((_, view)) = self.tools.as_ref() {
            view.focus(self.focus.1);
        }
    }

    /// Takes in what each page reported: pictures, titles, addresses and pointers, counts of matches, zooms, questions and the tabs it asked for; returns whether a browser changed.
    fn present(&mut self, shown: &mut Option<Browser>, shelf: &mut BrowserShelf) -> bool {
        let mut changed = false;
        let active = shown.as_ref().map(Browser::active_id);
        let now = Instant::now();
        let ids: Vec<u32> = self.pages.keys().copied().collect();
        for id in ids {
            let Some(page) = self.pages.get(&id) else {
                continue;
            };
            let view = &page.view;
            let host = shelf.tab(shown, id).and_then(|tab| tab.host.clone());
            let address = self.unresolve(&view.address(), host.as_deref());
            let title = view.title();
            let pointer = view.cursor();
            let found = view.take_found();
            let zoom = view.take_zoom();
            let notices = view.take_notices();
            let opened = view.take_opened();
            self.results.extend(view.take_tool_results());
            let picked = view.take_pick();
            let captures = view.take_captures();
            let captured_any = !captures.is_empty();
            if let Some(element) = picked.clone().flatten() {
                self.attachments
                    .push(Attachment::Element(picked_element(&element, &address)));
            }
            self.attachments.extend(
                captures
                    .into_iter()
                    .filter_map(|capture| captured(capture, &address)),
            );
            let frame = (Some(id) == active).then(|| view.frame()).flatten();
            let asks = notices
                .iter()
                .any(|notice| matches!(notice, pm_browser::Notice::Permission { .. }));
            if asks {
                let wake = self.wake.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(ALLOW_DELAY);
                    wake();
                });
            }
            let opened: Vec<(String, bool)> = opened
                .into_iter()
                .map(|(url, agent)| (self.unresolve(&url, host.as_deref()), agent))
                .collect();
            shelf.with_tab(shown, id, |browser| {
                if let Some(tab) = browser.tab_mut(id) {
                    if let Some(frame) = frame
                        && tab
                            .page
                            .as_ref()
                            .is_none_or(|held| held.0.generation != frame.generation)
                    {
                        tab.page = Some(Picture(frame));
                        changed = true;
                    }
                    if tab.title != title {
                        tab.title.clone_from(&title);
                        changed = true;
                    }
                    if tab.pointer != pointer {
                        tab.pointer = pointer;
                        changed = true;
                    }
                    if tab.safety.note_ready(now) {
                        changed = true;
                    }
                    if !address.is_empty() && tab.seen != address {
                        tab.seen.clone_from(&address);
                        tab.editing = false;
                    }
                    if !tab.editing && !address.is_empty() && tab.address != address {
                        tab.address.clone_from(&address);
                        changed = true;
                    }
                    if let Some((current, total)) = found {
                        tab.found(current, total);
                        changed = true;
                    }
                    if let Some(zoom) = zoom {
                        tab.zoom = zoom;
                        changed = true;
                    }
                    for notice in notices {
                        tab.safety.take_notice(notice, now);
                        changed = true;
                    }
                    if captured_any && tab.area {
                        tab.area = false;
                        changed = true;
                    }
                    if picked.is_some() && tab.picking {
                        tab.picking = false;
                        changed = true;
                    }
                }
                for (url, agent) in opened {
                    browser.open_tab(&url, host.clone(), agent, !agent);
                    changed = true;
                }
            });
        }
        if let Some((_, view)) = self.tools.as_ref()
            && let Some(frame) = view.frame()
            && let Some(browser) = shown.as_mut()
            && browser
                .tools_page
                .as_ref()
                .is_none_or(|held| held.0.generation != frame.generation)
        {
            browser.tools_page = Some(Picture(frame));
            changed = true;
        }
        changed
    }
}

/// Says `notice` in every tab of every browser, as when no page can be shown.
fn notice_all(shown: &mut Option<Browser>, shelf: &mut BrowserShelf, notice: &str) {
    for browser in shown.iter_mut().chain(shelf.kept.values_mut()) {
        for tab in &mut browser.tabs {
            tab.notice = Some(notice.to_owned());
        }
    }
}

/// Clears the notice of every tab that has one and loads its address again, as after the browser was installed.
fn retry(shown: &mut Option<Browser>, shelf: &mut BrowserShelf) {
    for browser in shown.iter_mut().chain(shelf.kept.values_mut()) {
        for tab in &mut browser.tabs {
            if tab.notice.take().is_some() {
                let address = crate::browser_address::address_of(&tab.address);
                tab.commands.push(Command::Load(address));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Addresses are cut at their host and port.
    #[test]
    fn addresses_are_cut_into_parts() {
        let cut = parts("http://localhost:5173/a?b").expect("an address");
        assert_eq!(
            (cut.before, cut.host, cut.port, cut.after),
            ("http://", "localhost", Some(5173), "/a?b")
        );
        let cut = parts("http://[::1]/x").expect("an address");
        assert_eq!((cut.host, cut.port), ("[::1]", None));
        assert!(parts("about:blank").is_none());
    }

    /// A page of this machine is opened as it is.
    #[test]
    fn local_pages_are_not_carried() {
        let mut web = Web::new(Arc::new(|| {}));
        assert_eq!(
            web.resolve("http://localhost:3000/", None).as_deref(),
            Ok("http://localhost:3000/")
        );
        assert_eq!(
            web.resolve("https://example.com/", Some("box")).as_deref(),
            Ok("https://example.com/")
        );
        assert_eq!(
            web.unresolve("http://127.0.0.1:9/", None),
            "http://127.0.0.1:9/"
        );
    }
}
