//! The link's side of the browser pane: carries out what the pane asks, keeps each session's browser its own, opens the pages servers start and local links, hands the start page's browser and shells to the session started from it, and gives the engine time.

use std::time::Instant;

use pm_browser::{CaptureKind, PageInput};

use super::Link;
use crate::browser::{Browser, Command, HOME, ZoomStep};
use crate::browser_action::{BrowserAction, PageCommand, Surface};
use crate::browser_links::{Route, route};
use crate::browser_scope::NO_SCOPE;
use crate::open_files::SessionTab;
use crate::page::Page;
use crate::web::{Attachment, Web};

impl Link {
    /// Carries out `action` of the browser pane; returns whether the workspace changed.
    pub fn browser(&mut self, action: BrowserAction) -> bool {
        match action {
            BrowserAction::Input(surface, input) => {
                self.browser_input(surface, &input);
                return false;
            }
            BrowserAction::Resized(surface, width, height, scale) => {
                self.web.resized(surface, width, height, scale);
                return self.run_web();
            }
            BrowserAction::Focused(surface, focused) => {
                self.web.focused(surface, focused);
                return self.run_web();
            }
            BrowserAction::Open => self.open_browser(None),
            BrowserAction::Close => self.close_browser(),
            other => self.browser_change(other),
        }
        self.run_web();
        self.browser_agent();
        true
    }

    /// Closes every page and ends the browser's helper, as the window closes.
    pub fn shutdown_browser(&mut self) {
        self.web.shutdown();
    }

    /// Hands `input` to the page of the tab shown or to its developer tools.
    fn browser_input(&mut self, surface: Surface, input: &PageInput) {
        self.web
            .input(self.workspace.browser.as_ref(), surface, input);
    }

    /// Carries out an action that changes the browser shown, when it is open.
    fn browser_change(&mut self, action: BrowserAction) {
        let host = self.browser_host();
        let now = Instant::now();
        let mut closed = false;
        let Some(browser) = self.workspace.browser.as_mut() else {
            return;
        };
        match action {
            BrowserAction::Address(text) => {
                let tab = browser.shown_mut();
                tab.address = text;
                tab.editing = true;
            }
            BrowserAction::Go => browser.go(host),
            BrowserAction::Back => browser.queue(Command::Back),
            BrowserAction::Forward => browser.queue(Command::Forward),
            BrowserAction::Reload => browser.queue(Command::Reload),
            BrowserAction::HardReload => browser.queue(Command::HardReload),
            BrowserAction::NewTab => {
                browser.open_tab("about:blank", host, false, true);
            }
            BrowserAction::SelectTab(index) => browser.select(index),
            BrowserAction::CloseTab(index) => closed = browser.close_tab(index),
            BrowserAction::CloseShownTab => {
                let index = browser.active;
                closed = browser.close_tab(index);
            }
            BrowserAction::Lock => {
                let tab = browser.shown_mut();
                tab.locked = !tab.locked && !tab.agent;
            }
            BrowserAction::ToggleTools => browser.toggle_tools(),
            BrowserAction::ToolsWidth(width) => browser.tools_width = width,
            BrowserAction::Find => browser.shown_mut().find_opened(),
            BrowserAction::FindTyped(query) => browser.shown_mut().find_typed(query),
            BrowserAction::FindStep { back } => browser.shown_mut().find_stepped(back),
            BrowserAction::FindClose => browser.shown_mut().find_closed(),
            BrowserAction::Zoom(step) => browser.queue(Command::Zoom(step)),
            BrowserAction::Menu(at) => {
                browser.menu = at.map(|at| crate::browser::BrowserMenu { at });
            }
            BrowserAction::Command(command) => page_command(browser, command),
            BrowserAction::Pick => toggle_pick(browser),
            BrowserAction::Capture(kind) => browser.queue(Command::Capture(kind)),
            BrowserAction::ClearConfirm => {
                if std::mem::take(&mut browser.clearing) {
                    browser.queue(Command::ClearData);
                }
            }
            BrowserAction::ClearCancel => browser.clearing = false,
            BrowserAction::Permission { id, allow } => {
                browser.shown_mut().safety.answer_at(id, allow, now);
            }
            BrowserAction::DismissDownload => browser.shown_mut().safety.download = None,
            BrowserAction::CertificateBack => browser.shown_mut().safety.certificate_back(),
            BrowserAction::CertificateProceed => {
                browser.shown_mut().safety.certificate_proceed();
            }
            BrowserAction::Open
            | BrowserAction::Close
            | BrowserAction::Input(..)
            | BrowserAction::Resized(..)
            | BrowserAction::Focused(..) => {}
        }
        if closed {
            self.close_browser();
        }
    }

    /// Opens the browser on `address`, or on the page a dev server is usually at, and shows it; an open browser loads `address` in the tab shown.
    pub fn open_browser(&mut self, address: Option<&str>) {
        let host = self.browser_host();
        match (self.workspace.browser.as_mut(), address) {
            (Some(browser), Some(address)) => browser.shown_mut().load(address, host),
            (Some(_), None) => {}
            (None, address) => {
                self.workspace.browser = Some(Browser::new(address.unwrap_or(HOME), host));
            }
        }
        self.workspace.open_files.tab = SessionTab::Browser;
        self.run_web();
    }

    /// Closes the browser shown, and shows the conversation when the browser was showing.
    fn close_browser(&mut self) {
        self.workspace.browser = None;
        if self.workspace.open_files.tab == SessionTab::Browser {
            self.workspace.open_files.tab = SessionTab::Chat;
        }
    }

    /// The machine the pages of the browser shown are on, when it is not this one: the machine of the session shown, or of the project the start page starts work in.
    fn browser_host(&self) -> Option<String> {
        let folder = match (&self.workspace.starting, &self.workspace.page) {
            (Some(start), _) => start
                .at
                .as_ref()
                .and_then(|at| self.paths.get(*at.first()?))
                .cloned(),
            (None, Page::Session(id)) => self
                .sessions
                .iter()
                .find(|info| info.session == *id)
                .map(|info| info.directory.clone()),
            _ => None,
        }?;
        pm_protocol::host_of(&folder).map(|(host, _)| host.to_owned())
    }

    /// The scope whose browser the page shown wants: the session shown, or no session.
    fn browser_scope_wanted(&self) -> u64 {
        match (&self.workspace.starting, &self.workspace.page) {
            (None, Page::Session(id)) => *id,
            _ => NO_SCOPE,
        }
    }

    /// Shows the browser of the session shown, putting the one shown before away, and forgets the browsers of sessions that are gone.
    pub(super) fn rescope_browser(&mut self) {
        let wanted = self.browser_scope_wanted();
        let workspace = &mut self.workspace;
        workspace.shelf.swap(&mut workspace.browser, wanted);
        if workspace.browser.is_none() && workspace.open_files.tab == SessionTab::Browser {
            workspace.open_files.tab = SessionTab::Chat;
        }
        if !self.sessions.is_empty() {
            let kept: Vec<u64> = self.sessions.iter().map(|info| info.session).collect();
            self.workspace.shelf.retain(&kept);
        }
    }

    /// Hands what the start page holds to the session `session` it started, working in `directory`: its browser, and the shells of its dock unless they are another session's.
    pub(super) fn hand_over(&mut self, session: u64, directory: &str) {
        let workspace = &mut self.workspace;
        workspace.shelf.hand_over(&mut workspace.browser, session);
        let place = self.dock.place();
        let owned = self
            .sessions
            .iter()
            .any(|info| info.session != session && info.directory == place);
        if !owned && !directory.is_empty() {
            let directory = directory.to_owned();
            self.change_dock(|dock| dock.hand_over(directory));
        }
    }

    /// Gives the engine time; returns whether a browser changed.
    pub(super) fn run_web(&mut self) -> bool {
        let general = &self.settings.window.general;
        let workspace = &mut self.workspace;
        let changed = if workspace.browser.is_none() && workspace.shelf.kept.is_empty() {
            self.web.shutdown();
            self.web
                .prepare(&mut workspace.browser, &mut workspace.shelf, general)
        } else {
            self.web
                .run(&mut workspace.browser, &mut workspace.shelf, general)
        };
        let note = self.web.download_status();
        let noted = note != self.settings.browser_note;
        self.settings.browser_note = note;
        changed || noted || self.take_web_attachments()
    }

    /// Attaches to the next prompt the elements picked and the captures taken in the browser since the last call, and says in the status line why a capture failed; returns whether anything was attached or said.
    fn take_web_attachments(&mut self) -> bool {
        let attachments = self.web.take_attachments();
        let changed = !attachments.is_empty();
        for attachment in attachments {
            match attachment {
                Attachment::Element(element) => self.attach_element(element),
                Attachment::Capture(capture) => self.attach_capture(capture),
                Attachment::Failed(reason) => self.workspace.status = Some(reason),
            }
        }
        changed
    }

    /// Opens the page a program of the session `session`, or of the shells shown when `None`, serves at `url`, when the settings let dev servers open in the browser.
    pub(super) fn served(&mut self, session: Option<u64>, url: &str) {
        let general = &self.settings.window.general;
        if !(general.open_dev_servers && Web::ready(general)) {
            return;
        }
        let scope = session.unwrap_or_else(|| self.browser_scope_wanted());
        let host = self.browser_host();
        let workspace = &mut self.workspace;
        if scope == workspace.shelf.scope {
            match workspace.browser.as_mut() {
                Some(browser) => browser.serve(url, host),
                None => workspace.browser = Some(Browser::new(url, host)),
            }
            workspace.open_files.tab = SessionTab::Browser;
        } else {
            match workspace.shelf.kept.get_mut(&scope) {
                Some(browser) => browser.serve(url, host),
                None => {
                    workspace.shelf.kept.insert(scope, Browser::new(url, host));
                }
            }
        }
        self.run_web();
    }

    /// Opens `address` in the browser pane when it is a page of this machine and the settings let local links open there, and in the reader's browser otherwise, saying why in the status line when it cannot.
    pub fn open_address(&mut self, address: &str) {
        let general = &self.settings.window.general;
        match route(address, general.open_local_links, Web::ready(general)) {
            Route::Pane(local) => self.open_browser(Some(&local)),
            Route::System(address) => {
                if let Err(reason) = pm_harness::open_address(&address) {
                    self.workspace.status = Some(reason);
                }
            }
        }
    }
}

/// Carries out the row `command` of the menu over the page of `browser`, closing the menu.
fn page_command(browser: &mut Browser, command: PageCommand) {
    browser.menu = None;
    match command {
        PageCommand::Back => browser.queue(Command::Back),
        PageCommand::Forward => browser.queue(Command::Forward),
        PageCommand::Reload => browser.queue(Command::Reload),
        PageCommand::HardReload => browser.queue(Command::HardReload),
        PageCommand::Cut => browser.queue(Command::Cut),
        PageCommand::Copy => browser.queue(Command::Copy),
        PageCommand::Paste => browser.queue(Command::Paste),
        PageCommand::SelectAll => browser.queue(Command::SelectAll),
        PageCommand::Find => browser.shown_mut().find_opened(),
        PageCommand::ZoomIn => browser.queue(Command::Zoom(ZoomStep::In)),
        PageCommand::ZoomOut => browser.queue(Command::Zoom(ZoomStep::Out)),
        PageCommand::ZoomReset => browser.queue(Command::Zoom(ZoomStep::Reset)),
        PageCommand::OpenExternal => browser.queue(Command::OpenExternal),
        PageCommand::ClearData => browser.clearing = true,
        PageCommand::Inspect => browser.show_tools(),
        PageCommand::PickElement => toggle_pick(browser),
        PageCommand::Screenshot => browser.queue(Command::Capture(CaptureKind::Screenshot)),
        PageCommand::FullPage => browser.queue(Command::Capture(CaptureKind::FullPage)),
        PageCommand::Area => browser.queue(Command::Capture(CaptureKind::Area)),
        PageCommand::Console => browser.queue(Command::Capture(CaptureKind::Console)),
    }
}

/// Starts picking an element of the page of the tab shown of `browser` for the prompt, or stops when it is picking.
fn toggle_pick(browser: &mut Browser) {
    let tab = browser.shown_mut();
    tab.picking = !tab.picking;
    let picking = tab.picking;
    browser.queue(Command::Pick(picking));
}
