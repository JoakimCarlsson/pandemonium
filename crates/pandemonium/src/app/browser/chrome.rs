//! What the pane shows around each page from what the page reports, and what the reader does with it: dialogs, the menu of a right click, downloads, full screen, sound, the link under the pointer, tooltips, sharing the screen, printing, the zoom each site was left at, the addresses visited and the shortcuts that reach them.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use pm_browser::chrome::{
    DialogKind as Asked, DownloadProgress, PageAction, PageReport, ScreenShare,
};
use pm_components::areas::BrowserEvent;
use pm_components::browser_chrome::{
    ChromeAction, DialogKind, DownloadEntry, DownloadStatus, PageMenu, ScreenAsk,
};
use pm_components::browser_keys::{Shortcut, escape_of, shortcut_of};
use pm_components::browser_safety::{Download, Saving};
use pm_components::state::WorkspaceState;
use pm_components::target::Target;
use pm_protocol::{origin_of, zoom_percent};
use rustle::{Key, KeyEvent};

use super::super::App;

/// How long after the reader's right click the menu its report asks for opens where the press was; later it opens where the page says.
const MENU_PATIENCE: Duration = Duration::from_secs(1);

/// How long a zoom put back for a site is waited for before the zoom the page reports is taken as the reader's.
const ZOOM_PATIENCE: Duration = Duration::from_secs(2);

/// How many visited addresses the address bar suggests.
const SUGGESTED: usize = 6;

/// The difference between two zoom factors that counts as none.
const SAME_ZOOM: f32 = 0.005;

/// What the window keeps of the pages for what is drawn around them.
#[derive(Default)]
pub(super) struct ChromeWatch {
    /// Where in the window the reader last pressed the secondary button on the page, and when.
    menu_at: Option<((f32, f32), Instant)>,
    /// What is kept of each page, by the number of its tab.
    pages: HashMap<u32, PageWatch>,
    /// What was typed in the address bar when suggestions were last looked for, while it is typed in.
    query: Option<String>,
    /// Whether the window was made to fill the screen for a page in full screen.
    fullscreen: bool,
}

/// What the window keeps of one page.
#[derive(Default)]
struct PageWatch {
    /// The origin the page was last on.
    origin: Option<String>,
    /// The zoom the page last reported.
    zoom: f32,
    /// The zoom of the site that was put back, while the page has not reported it yet, and since when.
    pending: Option<(f32, Instant)>,
    /// The address last recorded as visited.
    url: String,
    /// The title last recorded for it.
    title: String,
}

/// The engine's form of what the reader did from the pane around a page.
pub(super) fn page_action_of(action: ChromeAction) -> PageAction {
    match action {
        ChromeAction::AnswerDialog { id, accept, text } => {
            PageAction::AnswerDialog { id, accept, text }
        }
        ChromeAction::ReplaceMisspelling(word) => PageAction::ReplaceMisspelling { word },
        ChromeAction::InspectAt(x, y) => PageAction::InspectAt { x, y },
        ChromeAction::CopyImageAt(x, y) => PageAction::CopyImageAt { x, y },
        ChromeAction::Save(url) => PageAction::Save { url },
        ChromeAction::CancelDownload(id) => PageAction::CancelDownload { id },
        ChromeAction::ShowDownload(id) => PageAction::ShowDownload { id },
        ChromeAction::OpenDownload(id) => PageAction::OpenDownload { id },
        ChromeAction::Mute(muted) => PageAction::Mute { muted },
        ChromeAction::LeaveFullscreen => PageAction::LeaveFullscreen,
        ChromeAction::Print => PageAction::Print,
        ChromeAction::ChooseScreen { id, source } => PageAction::ChooseScreen { id, source },
        ChromeAction::SetZoom(factor) => PageAction::SetZoom { factor },
    }
}

/// The pane's form of a dialog's kind.
fn dialog_kind(kind: Asked) -> DialogKind {
    match kind {
        Asked::Alert => DialogKind::Alert,
        Asked::Confirm => DialogKind::Confirm,
        Asked::Prompt => DialogKind::Prompt,
    }
}

/// The pane's form of how a download stands.
fn download_status(progress: DownloadProgress) -> DownloadStatus {
    match progress {
        DownloadProgress::Running => DownloadStatus::Running,
        DownloadProgress::Finished => DownloadStatus::Finished,
        DownloadProgress::Failed => DownloadStatus::Failed,
        DownloadProgress::Cancelled => DownloadStatus::Cancelled,
    }
}

/// The pane's form of a request to share the screen.
fn screen_ask(share: ScreenShare) -> ScreenAsk {
    ScreenAsk {
        id: share.id,
        origin: share.origin,
        sources: share
            .sources
            .into_iter()
            .map(|source| (source.id, source.name))
            .collect(),
    }
}

/// Takes the report of a printed page into the note about downloads of `state`'s tab `tab`: where the PDF went, or that it could not be made.
fn printed(state: &mut WorkspaceState, tab: u32, path: &str, error: &str) {
    let name = std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "the page as a PDF".to_owned());
    if let Some(tab) = state
        .browser
        .as_mut()
        .and_then(|browser| browser.tab_mut(tab))
    {
        tab.safety.download = Some(Download {
            name,
            saving: if error.is_empty() {
                Saving::Finished
            } else {
                Saving::Failed
            },
        });
    }
}

impl App {
    /// Remembers where the reader pressed the secondary button on the page, in the window's frame, for the menu the page's report then opens; the menu waits for the report, since a page that handles the press itself has none. A tab with no page, which reports nothing, opens the menu over the page at once.
    pub(super) fn browser_menu_pressed_at(&mut self, at: (f32, f32)) {
        let has_page = self
            .browser_active()
            .is_some_and(|id| self.web.pages.contains_key(&id));
        if has_page {
            self.web.chrome.menu_at = Some((at, Instant::now()));
        } else {
            self.model.change(|state| state.with_browser_menu(at));
        }
    }

    /// Puts `text` on the system's clipboard.
    pub(super) fn browser_copy_text(&self, text: &str) {
        if let Some(clipboard) = self.clipboard() {
            clipboard.set_text(text);
        }
    }

    /// Takes in what each page reported for the pane around it, puts back the zoom of the site each page is on and records the addresses visited, keeps the suggestions of the address bar and the window's full screen up to date.
    pub(super) fn browser_chrome(&mut self) {
        self.browser_chrome_reports();
        self.browser_chrome_sites();
        self.browser_suggest();
        self.browser_window_fullscreen();
    }

    /// Takes in what each page reported for the pane around it.
    fn browser_chrome_reports(&mut self) {
        let reports: Vec<(u32, Vec<PageReport>)> = self
            .web
            .pages
            .iter()
            .map(|(id, shown)| (*id, shown.view.take_reports()))
            .filter(|(_, reports)| !reports.is_empty())
            .collect();
        let tools: Vec<(u32, Vec<PageReport>)> = self
            .web
            .tools
            .as_ref()
            .map(|(tab, view)| {
                let kept = view
                    .take_reports()
                    .into_iter()
                    .filter(|report| {
                        matches!(report, PageReport::Download(_) | PageReport::Printed { .. })
                    })
                    .collect();
                (*tab, kept)
            })
            .into_iter()
            .collect();
        for (id, reports) in reports.into_iter().chain(tools) {
            for report in reports {
                self.browser_chrome_report(id, report);
            }
        }
    }

    /// Takes in one report of the page of tab `tab`.
    fn browser_chrome_report(&mut self, tab: u32, report: PageReport) {
        match report {
            PageReport::ContextMenu(menu) => self.browser_page_menu(tab, *menu),
            PageReport::Dialog(dialog) => {
                let prompt = dialog.kind == Asked::Prompt;
                let mut shown = false;
                self.model.update(|state| {
                    state.browser_scoped_to_tab(tab, |state| {
                        state.browser_dialog_opened(
                            tab,
                            dialog.id,
                            dialog_kind(dialog.kind),
                            &dialog.message,
                            &dialog.default_text,
                            &dialog.host,
                        );
                    });
                    shown = state
                        .browser
                        .as_ref()
                        .is_some_and(|browser| browser.active_id() == tab);
                });
                if prompt && shown {
                    self.focus
                        .request_focus(Target::Browser(BrowserEvent::BrowserDialogField));
                }
            }
            PageReport::HoveredLink { url } => {
                let host = self.browser_tab_host(tab);
                let url = if url.is_empty() {
                    url
                } else {
                    self.browser_unresolve(&url, host.as_deref())
                };
                self.browser_chrome_state(tab, move |state| state.browser_hovered(tab, &url));
            }
            other => self.browser_chrome_state(tab, move |state| match other {
                PageReport::DialogClosed { id } => state.browser_dialog_closed(tab, id),
                PageReport::Download(item) => state.browser_download(DownloadEntry {
                    id: item.id,
                    tab,
                    name: item.name,
                    received: item.received,
                    total: item.total,
                    status: download_status(item.progress),
                }),
                PageReport::Fullscreen { on } => state.browser_fullscreen(tab, on),
                PageReport::Audio { audible, muted } => state.browser_audio(tab, audible, muted),
                PageReport::Tooltip { text, x, y } => {
                    state.browser_tooltip(tab, &text, x as f32, y as f32)
                }
                PageReport::Printed { path, error } => printed(state, tab, &path, &error),
                PageReport::ScreenShare(share) => {
                    state.browser_screen_asked(tab, Some(screen_ask(*share)), 0)
                }
                PageReport::ScreenShareCancelled { id } => {
                    state.browser_screen_asked(tab, None, id)
                }
                PageReport::ContextMenu(_)
                | PageReport::Dialog(_)
                | PageReport::HoveredLink { .. } => {}
            }),
        }
    }

    /// Changes the state of tab `tab` with `change`, in the browser that holds it.
    fn browser_chrome_state(&mut self, tab: u32, change: impl FnOnce(&mut WorkspaceState)) {
        self.model.update(|state| {
            state.browser_scoped_to_tab(tab, change);
        });
    }

    /// Opens the menu over the page of tab `tab` for what `menu` says was under the pointer: where the reader pressed, when that was a moment ago, and otherwise at the point of the page the report gives.
    fn browser_page_menu(&mut self, tab: u32, menu: pm_browser::chrome::ContextMenu) {
        let pressed = self
            .web
            .chrome
            .menu_at
            .take()
            .filter(|(_, when)| when.elapsed() <= MENU_PATIENCE)
            .map(|(at, _)| at);
        let at = pressed.or_else(|| {
            let region = self.browser_region(Target::Browser(BrowserEvent::BrowserBody))?;
            Some((
                region.origin.x + menu.x as f32,
                region.origin.y + menu.y as f32,
            ))
        });
        let Some(at) = at else {
            return;
        };
        let host = self.browser_tab_host(tab);
        let unresolve = |url: &str| {
            if url.starts_with("http://") || url.starts_with("https://") {
                self.browser_unresolve(url, host.as_deref())
            } else {
                url.to_owned()
            }
        };
        let context = PageMenu {
            x: menu.x,
            y: menu.y,
            link: unresolve(&menu.link_url),
            source: unresolve(&menu.src_url),
            image: menu.is_image,
            selection: menu.selection_text,
            editable: menu.editable,
            can_cut: menu.can_cut,
            can_copy: menu.can_copy,
            can_paste: menu.can_paste,
            misspelled: menu.misspelled,
            suggestions: menu.suggestions,
            can_back: menu.can_back,
            can_forward: menu.can_forward,
        };
        self.model
            .change(|state| state.with_page_menu(tab, context, at));
    }

    /// Puts back the zoom the reader left each page's site at when the page goes to another site, remembers the zoom the reader gives a site, and records the addresses visited.
    fn browser_chrome_sites(&mut self) {
        let ids: Vec<u32> = self
            .web
            .pages
            .iter()
            .filter(|(_, shown)| !shown.sealed)
            .map(|(id, _)| *id)
            .collect();
        self.web
            .chrome
            .pages
            .retain(|id, _| self.web.pages.contains_key(id));
        for id in ids {
            let Some(view) = self.web.pages.get(&id).map(|shown| &shown.view) else {
                continue;
            };
            let host = self.browser_tab_host(id);
            let url = self.browser_unresolve(&view.address(), host.as_deref());
            let title = view.title();
            let zoom = self
                .model
                .read(|state| state.browser_tab_anywhere(id).map_or(1.0, |tab| tab.zoom));
            let origin = origin_of(&url);
            let remembered = self
                .work
                .browser_sites()
                .zoom_of(&url)
                .map(|percent| percent as f32 / 100.0);
            let watch = self.web.chrome.pages.entry(id).or_default();
            let mut put_back = None;
            if watch.origin != origin {
                watch.origin = origin.clone();
                watch.zoom = zoom;
                let wanted = remembered.unwrap_or(1.0);
                watch.pending = None;
                if origin.is_some() && (zoom - wanted).abs() > SAME_ZOOM {
                    watch.pending = Some((wanted, Instant::now()));
                    put_back = Some(wanted);
                }
            } else if (zoom - watch.zoom).abs() > SAME_ZOOM {
                watch.zoom = zoom;
                match watch.pending {
                    Some((wanted, _)) if (zoom - wanted).abs() <= SAME_ZOOM => watch.pending = None,
                    Some((_, since)) if since.elapsed() < ZOOM_PATIENCE => {}
                    _ => {
                        watch.pending = None;
                        if origin.is_some() {
                            self.work.set_site_zoom(&url, zoom_percent(zoom));
                        }
                    }
                }
            }
            let watch = self.web.chrome.pages.entry(id).or_default();
            let visited = origin.is_some() && watch.url != url;
            let titled = !visited && origin.is_some() && watch.title != title;
            if visited || titled {
                watch.url.clone_from(&url);
                watch.title.clone_from(&title);
            }
            if visited {
                self.work.browser_visited(&url, &title);
            } else if titled {
                self.work.browser_titled(&url, &title);
            }
            if let Some(factor) = put_back
                && let Some(shown) = self.web.pages.get(&id)
            {
                shown.view.act(PageAction::SetZoom { factor });
            }
        }
    }

    /// Fills the suggestions of the address bar from the addresses visited while the reader types in it, and empties them once the reader leaves it for anything but a suggestion.
    fn browser_suggest(&mut self) {
        let focused = self.focus.focused();
        let typing = focused == Some(Target::Browser(BrowserEvent::BrowserAddress));
        let choosing = matches!(
            focused,
            Some(Target::Browser(BrowserEvent::BrowserSuggestion(_)))
        );
        if !typing {
            if !choosing && self.web.chrome.query.take().is_some() {
                self.model.update(|state| {
                    if let Some(browser) = state.browser.as_mut() {
                        browser.suggestions.clear();
                    }
                });
            }
            return;
        }
        let typed = self.model.read(|state| {
            state
                .browser
                .as_ref()
                .map(|browser| browser.address.text().to_owned())
        });
        let Some(typed) = typed else {
            return;
        };
        if self.web.chrome.query.as_deref() == Some(typed.as_str()) {
            return;
        }
        let page = self
            .browser_active()
            .and_then(|id| {
                self.web
                    .pages
                    .get(&id)
                    .map(|shown| (id, shown.view.address()))
            })
            .map(|(id, address)| {
                let host = self.browser_tab_host(id);
                self.browser_unresolve(&address, host.as_deref())
            })
            .unwrap_or_default();
        let fresh = self.web.chrome.query.is_some() || typed != page;
        self.web.chrome.query = Some(typed.clone());
        let sites = self.work.browser_sites();
        let suggestions: Vec<(String, String)> = if fresh {
            sites
                .suggestions(&typed, SUGGESTED + 1)
                .into_iter()
                .filter(|visit| visit.url != typed)
                .take(SUGGESTED)
                .map(|visit| (visit.url, visit.title))
                .collect()
        } else {
            Vec::new()
        };
        let known = sites.bare_hosts();
        self.model.update(|state| {
            if let Some(browser) = state.browser.as_mut() {
                browser.suggestions = suggestions;
                browser.known_hosts = known;
            }
        });
    }

    /// Makes the window fill the screen while the page shown is in full screen, and gives it back its size when the page leaves it.
    fn browser_window_fullscreen(&mut self) {
        let wanted = self.model.read(|state| state.browser_in_fullscreen());
        if wanted == self.web.chrome.fullscreen {
            return;
        }
        self.web.chrome.fullscreen = wanted;
        if let Some(window) = self.window.clone() {
            window.set_fullscreen(wanted);
        }
    }

    /// Takes a key the browser gives the pane around the page while the page has the keyboard, and says whether it did: Escape and Enter answer the sheet of a dialog, which keeps every other key from the stopped page; Escape takes the page out of full screen or stops it loading; Control or Command L and F6 go to the address bar.
    pub(super) fn browser_chrome_key(&mut self, event: &KeyEvent) -> bool {
        let focused = self.focus.focused();
        if focused == Some(Target::Browser(BrowserEvent::BrowserDialogField)) {
            let plain = !event.modifiers.ctrl && !event.modifiers.meta && !event.modifiers.alt;
            if matches!(event.key, Key::Escape) && plain {
                self.model
                    .update(|state| state.browser_dialog_answered(false));
                self.focus
                    .request_focus(Target::Browser(BrowserEvent::BrowserBody));
                return true;
            }
            return false;
        }
        if focused != Some(Target::Browser(BrowserEvent::BrowserBody)) {
            return false;
        }
        if self
            .model
            .read(|state| state.browser_dialog_open())
            .is_some()
        {
            let plain = !event.modifiers.ctrl && !event.modifiers.meta && !event.modifiers.alt;
            match event.key {
                Key::Escape if plain => self
                    .model
                    .update(|state| state.browser_dialog_answered(false)),
                Key::Enter if plain => self
                    .model
                    .update(|state| state.browser_dialog_answered(true)),
                _ => {}
            }
            return true;
        }
        if let Some(shortcut) = self.model.read(|state| escape_of(event, state)) {
            self.model.update(|state| state.browser_shortcut(shortcut));
            return true;
        }
        if shortcut_of(event) == Some(Shortcut::FocusAddress) {
            self.focus
                .request_focus(Target::Browser(BrowserEvent::BrowserAddress));
            return true;
        }
        false
    }
}
