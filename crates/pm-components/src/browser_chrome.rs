//! What the pane draws around a page beyond its bar, from what the page reports: the sheet of a dialog the page opened, the menu of a right click, the list of downloads, the address of the link under the pointer, the page's own tooltips, the question about sharing the screen, full screen and the page's sound.

use crate::areas::BrowserEvent;
use crate::button;
use crate::events::{Event, Interactive, send};
use crate::find::SHADOW;
use crate::parts::{clipped, wrapping};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::target::Target;
use crate::theme;
use crate::theme::{Frame, Indicated, Radius, Role, ShadowLayers};
use crate::{Button, Input};
use rustle::phosphor;
use rustle::{Alignment, Arrangement, BoxAlignment, Modifier, TextFieldState};

use crate::browser::{Browser, Command};
use crate::button::Variant;

/// How wide the sheet of a dialog is at most, in logical pixels.
const SHEET_WIDTH: f32 = 420.0;

/// How wide the list of downloads is, in logical pixels.
const DOWNLOADS_WIDTH: f32 = 340.0;

/// How many downloads the list keeps; older ones that ended fall off.
const DOWNLOADS_KEPT: usize = 30;

/// Which JavaScript dialog a page opened.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DialogKind {
    /// A message with OK.
    #[default]
    Alert,
    /// A question with OK and Cancel.
    Confirm,
    /// A question with a line to type in, OK and Cancel.
    Prompt,
}

/// A dialog the page opened, which stops it until the reader answers.
#[derive(Clone, Debug, PartialEq)]
pub struct PageDialog {
    /// The number of the dialog, for the answer.
    pub id: u64,
    /// Which dialog it is.
    pub kind: DialogKind,
    /// What it says.
    pub message: String,
    /// The host of the page that opened it, or nothing.
    pub host: String,
    /// The line a prompt's answer is typed in.
    pub answer: TextFieldState,
}

/// What was under the pointer when the reader pressed the secondary button on the page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageMenu {
    /// Logical pixels from the left of the page.
    pub x: i32,
    /// Logical pixels from the top of the page.
    pub y: i32,
    /// The address of the link under the pointer, or nothing.
    pub link: String,
    /// The address of the picture or media under the pointer, or nothing.
    pub source: String,
    /// Whether what is under the pointer is a picture.
    pub image: bool,
    /// The text that is marked.
    pub selection: String,
    /// Whether the pointer is over something the reader can type in.
    pub editable: bool,
    /// Whether something can be cut.
    pub can_cut: bool,
    /// Whether something can be copied.
    pub can_copy: bool,
    /// Whether something can be pasted.
    pub can_paste: bool,
    /// The misspelt word under the pointer, or nothing.
    pub misspelled: String,
    /// The words the spelling checker suggests for it.
    pub suggestions: Vec<String>,
    /// Whether the page can go back.
    pub can_back: bool,
    /// Whether the page can go forward.
    pub can_forward: bool,
}

/// How a download stands.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DownloadStatus {
    /// Bytes are arriving.
    #[default]
    Running,
    /// The file was saved.
    Finished,
    /// The file could not be saved.
    Failed,
    /// The reader cancelled it.
    Cancelled,
}

/// One download of the browser.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DownloadEntry {
    /// The engine's number for it.
    pub id: u64,
    /// The tab whose page started it.
    pub tab: u32,
    /// The name it is saved under in the Downloads folder.
    pub name: String,
    /// How many bytes have arrived.
    pub received: u64,
    /// How many bytes there are in all, or 0 when unknown.
    pub total: u64,
    /// How it stands.
    pub status: DownloadStatus,
}

/// The downloads of the browser and whether their list is open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Downloads {
    /// The downloads, the newest first.
    pub items: Vec<DownloadEntry>,
    /// Whether the list shows over the page.
    pub open: bool,
}

impl Downloads {
    /// Whether any download is still running.
    pub fn running(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.status == DownloadStatus::Running)
    }

    /// Takes in how the download `entry` stands: a new one goes first and opens the list, a known one is updated where it is.
    pub fn update(&mut self, entry: DownloadEntry) {
        match self.items.iter_mut().find(|item| item.id == entry.id) {
            Some(item) => *item = entry,
            None => {
                self.items.insert(0, entry);
                self.open = true;
                self.trim();
            }
        }
    }

    /// Lets the oldest downloads that ended fall off the list past [`DOWNLOADS_KEPT`].
    fn trim(&mut self) {
        while self.items.len() > DOWNLOADS_KEPT {
            match self
                .items
                .iter()
                .rposition(|item| item.status != DownloadStatus::Running)
            {
                Some(at) => {
                    self.items.remove(at);
                }
                None => break,
            }
        }
    }
}

/// A page's request to share a screen or a window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScreenAsk {
    /// The number of the question.
    pub id: u64,
    /// The origin that asks.
    pub origin: String,
    /// What it may share: the engine's name and what it is called.
    pub sources: Vec<(String, String)>,
}

/// Something the reader does with a page from the pane around it, for the window to carry to the engine.
#[derive(Clone, Debug, PartialEq)]
pub enum ChromeAction {
    /// Answers the dialog `id`: OK when `accept`, with the text typed into a prompt.
    AnswerDialog {
        /// The dialog.
        id: u64,
        /// Whether OK was pressed.
        accept: bool,
        /// What was typed into a prompt.
        text: String,
    },
    /// Puts the word in place of the misspelt word under the caret.
    ReplaceMisspelling(String),
    /// Shows the element at a point of the page in the developer tools.
    InspectAt(i32, i32),
    /// Copies the picture at a point of the page to the clipboard.
    CopyImageAt(i32, i32),
    /// Downloads the address into the Downloads folder.
    Save(String),
    /// Stops the download.
    CancelDownload(u64),
    /// Shows the download's file in the system's file manager.
    ShowDownload(u64),
    /// Opens the download's file.
    OpenDownload(u64),
    /// Mutes the page, or unmutes it.
    Mute(bool),
    /// Takes the page out of full screen.
    LeaveFullscreen,
    /// Prints the page to a PDF in the Downloads folder and opens it.
    Print,
    /// Answers the question `id` about sharing the screen with the source chosen, or nothing to refuse.
    ChooseScreen {
        /// The question.
        id: u64,
        /// The source's engine name, or nothing.
        source: Option<String>,
    },
    /// Sets the zoom of the page as a factor.
    SetZoom(f32),
}

/// What the pane shows around one tab's page from what the page reported.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageChrome {
    /// The dialog the page opened, while it is open.
    pub dialog: Option<PageDialog>,
    /// What was under the pointer at the last right click, for the menu over the page.
    pub menu: Option<PageMenu>,
    /// Whether the page is in full screen.
    pub fullscreen: bool,
    /// Whether the page makes sound.
    pub audible: bool,
    /// Whether the page's sound is muted.
    pub muted: bool,
    /// The address of the link under the pointer, or nothing.
    pub hovered: String,
    /// The title the pointer rests on, and where, in logical pixels of the page.
    pub tooltip: Option<(String, f32, f32)>,
    /// The page's request to share the screen, while it is open.
    pub screen: Option<ScreenAsk>,
}

/// Emits `content` on one line in a column, `pixels` tall in `tone`, cut short at the column's edge.
#[track_caller]
fn one_line(content: &str, pixels: u32, tone: Role) {
    rustle::basic_text(
        Modifier::new().fill_max_width(),
        content,
        theme::line(pixels, tone).max_lines(1),
        rustle::TextOverflow::Ellipsis,
    );
}

/// Bytes in words, such as `1.2 MB`.
pub fn bytes_in_words(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// What the list of downloads says of how `item` stands.
pub fn download_words(item: &DownloadEntry) -> String {
    match item.status {
        DownloadStatus::Running if item.total > 0 => format!(
            "{} of {}",
            bytes_in_words(item.received),
            bytes_in_words(item.total)
        ),
        DownloadStatus::Running => bytes_in_words(item.received),
        DownloadStatus::Finished => format!("Done, {}", bytes_in_words(item.received)),
        DownloadStatus::Failed => "Failed".to_owned(),
        DownloadStatus::Cancelled => "Cancelled".to_owned(),
    }
}

impl Browser {
    /// Whether the page of the tab being shown is in full screen, so the pane shows nothing but the page.
    pub fn fullscreen(&self) -> bool {
        self.chrome.fullscreen
    }
}

impl WorkspaceState {
    /// Queues `action` for the page of the tab numbered `tab`.
    pub fn browser_chrome_action(&mut self, tab: u32, action: ChromeAction) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.commands.push(Command::Chrome(action));
        }
    }

    /// Queues `action` for the page of the tab being shown.
    fn browser_chrome_shown(&mut self, action: ChromeAction) {
        self.browser_queue(Command::Chrome(action));
    }

    /// Shows the sheet of the dialog the page of tab `tab` opened.
    pub fn browser_dialog_opened(
        &mut self,
        tab: u32,
        id: u64,
        kind: DialogKind,
        message: &str,
        default_text: &str,
        host: &str,
    ) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.chrome.dialog = Some(PageDialog {
                id,
                kind,
                message: message.to_owned(),
                host: host.to_owned(),
                answer: TextFieldState::new(default_text),
            });
        }
    }

    /// Takes away the sheet of the dialog `id` of tab `tab`, which closed another way.
    pub fn browser_dialog_closed(&mut self, tab: u32, id: u64) {
        if let Some(tab) = self.browser_tab(tab)
            && tab.chrome.dialog.as_ref().is_some_and(|open| open.id == id)
        {
            tab.chrome.dialog = None;
        }
    }

    /// Whether the tab being shown has the sheet of a dialog open, and whether it is a prompt.
    pub fn browser_dialog_open(&self) -> Option<DialogKind> {
        Some(self.browser.as_ref()?.chrome.dialog.as_ref()?.kind)
    }

    /// Answers the dialog of the tab being shown, OK when `accept`, and takes its sheet away.
    pub fn browser_dialog_answered(&mut self, accept: bool) {
        let Some(dialog) = self
            .browser
            .as_mut()
            .and_then(|browser| browser.chrome.dialog.take())
        else {
            return;
        };
        let text = if dialog.kind == DialogKind::Prompt {
            dialog.answer.text().to_owned()
        } else {
            String::new()
        };
        self.browser_chrome_shown(ChromeAction::AnswerDialog {
            id: dialog.id,
            accept,
            text,
        });
    }

    /// The state with the menu over the page of tab `tab` open at `at` for what `menu` says was under the pointer.
    pub fn with_page_menu(mut self, tab: u32, menu: PageMenu, at: (f32, f32)) -> Self {
        let shown = self
            .browser
            .as_ref()
            .is_some_and(|browser| browser.active_id() == tab);
        if !shown {
            return self;
        }
        if let Some(tab) = self.browser_tab(tab) {
            tab.chrome.menu = Some(menu);
        }
        self.with_browser_menu(at)
    }

    /// Takes in how a download of the page of tab `tab` stands.
    pub fn browser_download(&mut self, entry: DownloadEntry) {
        if let Some(browser) = self.browser.as_mut() {
            browser.downloads.update(entry);
        }
    }

    /// Notes whether the page of tab `tab` is in full screen.
    pub fn browser_fullscreen(&mut self, tab: u32, on: bool) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.chrome.fullscreen = on;
        }
    }

    /// Whether the page shown is in full screen.
    pub fn browser_in_fullscreen(&self) -> bool {
        self.browser.as_ref().is_some_and(Browser::fullscreen)
    }

    /// Takes the page shown out of full screen.
    pub fn browser_leave_fullscreen(&mut self) {
        if let Some(browser) = self.browser.as_mut()
            && browser.chrome.fullscreen
        {
            browser.chrome.fullscreen = false;
            browser
                .commands
                .push(Command::Chrome(ChromeAction::LeaveFullscreen));
        }
    }

    /// Notes whether the page of tab `tab` makes sound and whether it is muted.
    pub fn browser_audio(&mut self, tab: u32, audible: bool, muted: bool) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.chrome.audible = audible;
            tab.chrome.muted = muted;
        }
    }

    /// Notes the address of the link under the pointer on the page of tab `tab`.
    pub fn browser_hovered(&mut self, tab: u32, url: &str) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.chrome.hovered = url.to_owned();
        }
    }

    /// Notes the title the pointer rests on in the page of tab `tab`, at `x`, `y` of the page, or that it left it.
    pub fn browser_tooltip(&mut self, tab: u32, text: &str, x: f32, y: f32) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.chrome.tooltip = (!text.is_empty()).then(|| (text.to_owned(), x, y));
        }
    }

    /// Shows the question about sharing the screen the page of tab `tab` asked, or takes it away when `ask` is nothing and it had the number `cancelled`.
    pub fn browser_screen_asked(&mut self, tab: u32, ask: Option<ScreenAsk>, cancelled: u64) {
        if let Some(tab) = self.browser_tab(tab) {
            match ask {
                Some(ask) => tab.chrome.screen = Some(ask),
                None => {
                    if tab
                        .chrome
                        .screen
                        .as_ref()
                        .is_some_and(|open| open.id == cancelled)
                    {
                        tab.chrome.screen = None;
                    }
                }
            }
        }
    }

    /// Carries out pressing `id` of what the pane draws around the page, and says whether it was one of those.
    pub(crate) fn browser_chrome_pressed(&mut self, id: Target) -> bool {
        let Target::Browser(event) = id else {
            return false;
        };
        let Some(browser) = self.browser.as_mut() else {
            return false;
        };
        match event {
            BrowserEvent::BrowserDialogOk => self.browser_dialog_answered(true),
            BrowserEvent::BrowserDialogCancel => self.browser_dialog_answered(false),
            BrowserEvent::BrowserDialogSheet
            | BrowserEvent::BrowserDialogField
            | BrowserEvent::BrowserDownloadsPanel => {}
            BrowserEvent::BrowserDownloads => browser.downloads.open = !browser.downloads.open,
            BrowserEvent::BrowserDownloadsClear => {
                browser
                    .downloads
                    .items
                    .retain(|item| item.status == DownloadStatus::Running);
                browser.downloads.open = !browser.downloads.items.is_empty();
            }
            BrowserEvent::BrowserDownloadCancel(download) => {
                self.browser_download_action(download, ChromeAction::CancelDownload(download))
            }
            BrowserEvent::BrowserDownloadShow(download) => {
                self.browser_download_action(download, ChromeAction::ShowDownload(download))
            }
            BrowserEvent::BrowserDownloadOpen(download) => {
                self.browser_download_action(download, ChromeAction::OpenDownload(download))
            }
            BrowserEvent::BrowserTabMute(index) => {
                if let Some(tab) = browser.tabs.get_mut(index) {
                    tab.chrome.muted = !tab.chrome.muted;
                    let muted = tab.chrome.muted;
                    tab.commands
                        .push(Command::Chrome(ChromeAction::Mute(muted)));
                }
            }
            BrowserEvent::BrowserScreenSource(index) => {
                if let Some(ask) = browser.chrome.screen.take() {
                    let source = ask.sources.get(index).map(|(id, _)| id.clone());
                    browser
                        .commands
                        .push(Command::Chrome(ChromeAction::ChooseScreen {
                            id: ask.id,
                            source,
                        }));
                }
            }
            BrowserEvent::BrowserSuggestion(index) => {
                if let Some((url, _)) = browser.suggestions.get(index).cloned() {
                    let host = self.current_project_host();
                    if let Some(browser) = self.browser.as_mut() {
                        browser.suggestions.clear();
                        browser.address.set_text(&url);
                        browser.host = host;
                        browser.commands.push(Command::Load(url));
                    }
                }
            }
            BrowserEvent::BrowserScreenCancel => {
                if let Some(ask) = browser.chrome.screen.take() {
                    browser
                        .commands
                        .push(Command::Chrome(ChromeAction::ChooseScreen {
                            id: ask.id,
                            source: None,
                        }));
                }
            }
            _ => return false,
        }
        true
    }

    /// Sends `action` about the download `download` to the page of the tab that started it, or of the tab shown when that tab is gone; the engine knows every download by its number alone.
    fn browser_download_action(&mut self, download: u64, action: ChromeAction) {
        let Some(browser) = self.browser.as_ref() else {
            return;
        };
        let tab = browser
            .downloads
            .items
            .iter()
            .find(|item| item.id == download)
            .map(|item| item.tab)
            .filter(|tab| browser.tab(*tab).is_some())
            .unwrap_or_else(|| browser.active_id());
        self.browser_chrome_action(tab, action);
    }
}

/// Emits what the pane draws over the page of `browser`: the address of the link under the pointer, the page's tooltip, the question about sharing the screen, the list of downloads and the sheet of a dialog, telling `on_hover` when the pointer comes over one that takes it from the page and leaves it.
#[track_caller]
pub fn page_overlays(browser: &Browser, on_hover: impl Fn(bool) + Clone + 'static) {
    let chrome = &browser.chrome;
    if !chrome.hovered.is_empty() {
        status_bubble(&chrome.hovered);
    }
    if let Some((text, x, y)) = chrome.tooltip.as_ref() {
        page_tooltip(text, *x, *y);
    }
    if let Some(ask) = chrome.screen.as_ref() {
        screen_bar(ask, on_hover.clone());
    }
    if browser.downloads.open && !browser.downloads.items.is_empty() {
        downloads_panel(&browser.downloads, on_hover.clone());
    }
    if !browser.suggestions.is_empty() {
        suggestion_list(&browser.suggestions, on_hover.clone());
    }
    if let Some(dialog) = chrome.dialog.as_ref() {
        dialog_sheet(dialog, on_hover);
    }
}

/// Where the list of suggestions starts from the left of the page, under the address bar, in logical pixels: past the bar's buttons before the address.
const SUGGESTIONS_LEFT: f32 = 120.0;

/// Emits the visited addresses that match what is typed in the address bar, under it at the top of the page, each with its title; pressing one goes there.
#[track_caller]
fn suggestion_list(suggestions: &[(String, String)], on_hover: impl Fn(bool) + 'static) {
    rustle::column_arranged(
        Modifier::new()
            .align(BoxAlignment::TopStart)
            .offset(SUGGESTIONS_LEFT, theme::sp(0.5))
            .z_index(70)
            .width(520.0)
            .shadow_layers(&SHADOW, Radius::Md.px())
            .panel(Radius::Md)
            .on_hover(on_hover)
            .padding(theme::sp(1)),
        Arrangement::Start,
        Alignment::Start,
        || {
            for (index, (url, title)) in suggestions.iter().enumerate() {
                let id = Target::Browser(BrowserEvent::BrowserSuggestion(index));
                rustle::row_arranged(
                    crate::events::pressed(
                        Modifier::new().fill_max_width().height(28.0).indicated(
                            theme::selectable(false).radius(Radius::Sm),
                            theme::Focused::Control(crate::focus::is_focused(id)),
                        ),
                        id,
                    )
                    .padding_xy(theme::sp(2), 0.0),
                    Arrangement::SpacedBy(theme::sp(2)),
                    Alignment::Center,
                    || {
                        crate::parts::glyph(phosphor::CLOCK_COUNTER_CLOCKWISE, 3.5, Role::Muted);
                        rustle::row_arranged(
                            Modifier::new().weight(1.0),
                            Arrangement::Start,
                            Alignment::Center,
                            || clipped(url.clone(), 12, Role::Text),
                        );
                        if !title.is_empty() {
                            rustle::row_arranged(
                                Modifier::new().width_in(0.0, 200.0),
                                Arrangement::Start,
                                Alignment::Center,
                                || clipped(title.clone(), 12, Role::Muted),
                            );
                        }
                    },
                );
            }
        },
    );
}

/// Emits the button of the bar that shows or hides the list of downloads, while there are downloads, marked while one runs.
#[track_caller]
pub fn downloads_button(browser: &Browser) {
    if browser.downloads.items.is_empty() {
        return;
    }
    let running = browser.downloads.running();
    Button::icon(
        if running {
            phosphor::DOWNLOAD
        } else {
            phosphor::DOWNLOAD_SIMPLE
        },
        Target::Browser(BrowserEvent::BrowserDownloads),
    )
    .variant(if browser.downloads.open {
        Variant::Secondary
    } else {
        Variant::Ghost
    })
    .tone(if running { Role::Accent } else { Role::Muted })
    .tooltip(if running {
        "Downloading: show the downloads"
    } else {
        "Show the downloads"
    })
    .build();
}

/// Emits the address of the link under the pointer in a small box at the bottom left of the page, as browsers show it.
#[track_caller]
fn status_bubble(url: &str) {
    rustle::row_arranged(
        Modifier::new()
            .align(BoxAlignment::BottomStart)
            .z_index(30)
            .width_in(0.0, 520.0)
            .background(theme::color(Role::Surface))
            .border(1.0, theme::color(Role::Border), 0.0)
            .padding_xy(theme::sp(2), theme::sp(0.5)),
        Arrangement::Start,
        Alignment::Center,
        || clipped(url, 12, Role::Muted),
    );
}

/// Emits the title the pointer rests on in a box just below and right of the pointer at `x`, `y` of the page, as the page's own tooltip would show.
#[track_caller]
fn page_tooltip(text: &str, x: f32, y: f32) {
    rustle::column_arranged(
        Modifier::new()
            .align(BoxAlignment::TopStart)
            .offset(x + 12.0, y + 18.0)
            .z_index(50)
            .width_in(0.0, 360.0)
            .panel(Radius::Sm)
            .padding_xy(theme::sp(1.5), theme::sp(0.75)),
        Arrangement::Start,
        Alignment::Start,
        || {
            for line in text.lines().take(12) {
                wrapping(
                    Modifier::new().width_in(0.0, 340.0),
                    line,
                    12,
                    1.4,
                    Role::Text,
                );
            }
        },
    );
}

/// Emits the bar at the top of the page that asks which screen or window the page may share, with a button for each and one that refuses.
#[track_caller]
fn screen_bar(ask: &ScreenAsk, on_hover: impl Fn(bool) + 'static) {
    let host = ask
        .origin
        .split_once("://")
        .map_or(ask.origin.as_str(), |(_, host)| host)
        .to_owned();
    rustle::row_arranged(
        Modifier::new()
            .align(BoxAlignment::TopStart)
            .fill_max_width()
            .z_index(41)
            .height(crate::browser_safety::BAR)
            .background(theme::color(Role::Surface))
            .border_bottom(1.0, theme::color(Role::Border))
            .on_hover(on_hover)
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            clipped(
                format!("{host} wants to share your screen:"),
                13,
                Role::Text,
            );
            for (index, (_, name)) in ask.sources.iter().enumerate().take(6) {
                button::action(
                    name.clone(),
                    Target::Browser(BrowserEvent::BrowserScreenSource(index)),
                    Variant::Outline,
                );
            }
            button::action(
                "Don't share",
                Target::Browser(BrowserEvent::BrowserScreenCancel),
                Variant::Ghost,
            );
        },
    );
}

/// Emits the list of downloads over the top right of the page: each with its name, how far it got, and buttons to cancel it, show it in its folder or open it.
#[track_caller]
fn downloads_panel(downloads: &Downloads, on_hover: impl Fn(bool) + 'static) {
    rustle::column_arranged(
        Modifier::new()
            .align(BoxAlignment::TopEnd)
            .offset(-theme::sp(2), theme::sp(2))
            .z_index(42)
            .width(DOWNLOADS_WIDTH)
            .shadow_layers(&SHADOW, Radius::Md.px())
            .panel(Radius::Md)
            .on_hover(on_hover)
            .control(
                Target::Browser(BrowserEvent::BrowserDownloadsPanel),
                send(Event::Browser(BrowserEvent::BrowserDownloadsPanel)),
            )
            .padding(theme::sp(2)),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(1)),
                Alignment::Center,
                || {
                    rustle::row_arranged(
                        Modifier::new().weight(1.0),
                        Arrangement::Start,
                        Alignment::Center,
                        || clipped("Downloads", 13, Role::Text),
                    );
                    button::small_button(
                        "Clear",
                        Target::Browser(BrowserEvent::BrowserDownloadsClear),
                        Variant::Ghost,
                    )
                    .build();
                    Button::icon(phosphor::X, Target::Browser(BrowserEvent::BrowserDownloads))
                        .size(Size::Xs)
                        .build();
                },
            );
            for item in downloads.items.iter().take(8) {
                rustle::key(item.id, || download_row(item));
            }
        },
    );
}

/// Emits one row of the list of downloads.
#[track_caller]
fn download_row(item: &DownloadEntry) {
    let tone = match item.status {
        DownloadStatus::Failed => Role::Danger,
        DownloadStatus::Cancelled => Role::Muted,
        _ => Role::Text,
    };
    let id = item.id;
    let status = item.status;
    rustle::row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            rustle::column_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    one_line(&item.name, 12, tone);
                    one_line(&download_words(item), 11, Role::Muted);
                },
            );
            match status {
                DownloadStatus::Running => {
                    Button::icon(
                        phosphor::X,
                        Target::Browser(BrowserEvent::BrowserDownloadCancel(id)),
                    )
                    .size(Size::Xs)
                    .tooltip("Cancel")
                    .build();
                }
                DownloadStatus::Finished => {
                    Button::icon(
                        phosphor::FOLDER_OPEN,
                        Target::Browser(BrowserEvent::BrowserDownloadShow(id)),
                    )
                    .size(Size::Xs)
                    .tooltip("Show in folder")
                    .build();
                    Button::icon(
                        phosphor::ARROW_SQUARE_OUT,
                        Target::Browser(BrowserEvent::BrowserDownloadOpen(id)),
                    )
                    .size(Size::Xs)
                    .tooltip("Open")
                    .build();
                }
                _ => {}
            }
        },
    );
}

/// Emits the sheet of a dialog the page opened, over a dimmed page: who asks, what it says, the line for a prompt's answer, and OK, with Cancel for a question.
#[track_caller]
fn dialog_sheet(dialog: &PageDialog, on_hover: impl Fn(bool) + 'static) {
    let title = if dialog.host.is_empty() {
        "This page says".to_owned()
    } else {
        format!("{} says", dialog.host)
    };
    let dialog = dialog.clone();
    rustle::with_constraints(
        Modifier::new()
            .match_parent_size()
            .z_index(60)
            .on_hover(on_hover),
        move |room| {
            let across = (room.max_width - 32.0).clamp(0.0, SHEET_WIDTH);
            rustle::row_arranged(
                Modifier::new()
                    .fill_max_size()
                    .background(theme::color(Role::Background).copy(0.55))
                    .control(
                        Target::Browser(BrowserEvent::BrowserDialogSheet),
                        send(Event::Browser(BrowserEvent::BrowserDialogSheet)),
                    ),
                Arrangement::Center,
                Alignment::Start,
                || {
                    rustle::column_arranged(
                        Modifier::new()
                            .padding_each(0.0, theme::sp(8), 0.0, 0.0)
                            .width(across),
                        Arrangement::Start,
                        Alignment::Start,
                        || dialog_panel(&dialog, &title, across),
                    );
                },
            );
        },
    );
}

/// Emits the panel of a dialog's sheet, `across` logical pixels wide, headed `title`.
#[track_caller]
fn dialog_panel(dialog: &PageDialog, title: &str, across: f32) {
    let room = (across - 2.0 * theme::sp(4)).max(0.0);
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .shadow_layers(&SHADOW, Radius::Lg.px())
            .panel(Radius::Lg)
            .padding(theme::sp(4)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Start,
        || {
            one_line(title, 13, Role::Muted);
            for line in dialog.message.lines().take(30) {
                wrapping(
                    Modifier::new().width_in(0.0, room),
                    line,
                    13,
                    1.45,
                    Role::Text,
                );
            }
            if dialog.kind == DialogKind::Prompt {
                Input::new(
                    &dialog.answer,
                    Target::Browser(BrowserEvent::BrowserDialogField),
                )
                .size(Size::Sm)
                .modifier(Modifier::new().fill_max_width())
                .submits(Target::Browser(BrowserEvent::BrowserDialogOk))
                .build();
            }
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::End,
                Alignment::Center,
                || {
                    rustle::row_arranged(
                        Modifier::new(),
                        Arrangement::SpacedBy(theme::sp(2)),
                        Alignment::Center,
                        || {
                            if dialog.kind != DialogKind::Alert {
                                button::action(
                                    "Cancel",
                                    Target::Browser(BrowserEvent::BrowserDialogCancel),
                                    Variant::Outline,
                                );
                            }
                            button::action(
                                "OK",
                                Target::Browser(BrowserEvent::BrowserDialogOk),
                                Variant::Default,
                            );
                        },
                    );
                },
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes read as people write them.
    #[test]
    fn bytes_read_in_words() {
        assert_eq!(bytes_in_words(512), "512 B");
        assert_eq!(bytes_in_words(1_500), "1.5 KB");
        assert_eq!(bytes_in_words(2_300_000), "2.3 MB");
    }

    /// A new download goes first and opens the list; news of a known one changes it where it is.
    #[test]
    fn downloads_are_updated_in_place() {
        let mut downloads = Downloads::default();
        downloads.update(DownloadEntry {
            id: 1,
            name: "a.zip".into(),
            ..DownloadEntry::default()
        });
        downloads.open = false;
        downloads.update(DownloadEntry {
            id: 2,
            name: "b.zip".into(),
            ..DownloadEntry::default()
        });
        assert!(downloads.open);
        downloads.update(DownloadEntry {
            id: 1,
            name: "a.zip".into(),
            received: 10,
            status: DownloadStatus::Finished,
            ..DownloadEntry::default()
        });
        assert_eq!(downloads.items[0].id, 2);
        assert_eq!(downloads.items[1].status, DownloadStatus::Finished);
        assert!(downloads.running());
    }
}
