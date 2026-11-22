//! Where the page of a tab is in its life, and what the pane shows of it: the line that grows while it loads, the view that stands in for an address that did not load, the one for a page that crashed, the bar of a page that stopped answering and the note that the browser was started again; and the tabs that windows opened by pages become.

use crate::Button;
use crate::areas::BrowserEvent;
use crate::browser::{Browser, Command};
use crate::button::Variant;
use crate::parts::{clipped, glyph, wrapping};
use crate::state::WorkspaceState;
use crate::target::Target;
use crate::theme;
use crate::theme::{Frame, Radius, Role};
use rustle::phosphor;
use rustle::{Alignment, Arrangement, Modifier, TextFieldState, TextOverflow};

/// How tall the line that shows how far a load got is, in logical pixels.
pub const PROGRESS_LINE: f32 = 2.0;

/// How tall a bar over the page is, in logical pixels.
const BAR: f32 = 40.0;

/// What the note says that stands over every tab after the browser stopped and was started again.
pub const RESTARTED: &str = "The browser stopped unexpectedly and was started again. This tab was opened again at its last address.";

/// An address the page of a tab could not load, and why.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageFailure {
    /// The address, as the tab shows it.
    pub url: String,
    /// The engine's number for the error, such as -105.
    pub code: i32,
    /// The engine's name for it, such as `ERR_NAME_NOT_RESOLVED`.
    pub name: String,
}

/// Where the page of a tab is in its life, as the engine last reported it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageLife {
    /// Whether the page is loading.
    pub loading: bool,
    /// How far the load got, from 0 to 1.
    pub progress: f32,
    /// Whether the engine reports the page's history; until it does, back and forward stay pressable.
    pub history_known: bool,
    /// Whether the history has a page before the one shown.
    pub back: bool,
    /// Whether the history has a page after the one shown.
    pub forward: bool,
    /// The address the page could not load, until the next load starts.
    pub failure: Option<PageFailure>,
    /// Why the page's content process went away, until the next load starts.
    pub crashed: Option<String>,
    /// Whether the page stopped answering.
    pub unresponsive: bool,
    /// Whether the note that the browser was started again stands over the tab.
    pub restarted: bool,
}

impl PageLife {
    /// Whether back can be pressed: when the history has a page before, or the engine does not say.
    pub fn can_go_back(&self) -> bool {
        !self.history_known || self.back
    }

    /// Whether forward can be pressed: when the history has a page after, or the engine does not say.
    pub fn can_go_forward(&self) -> bool {
        !self.history_known || self.forward
    }

    /// Whether the pane shows a view of its own in place of the page: an address that did not load, or a crash.
    pub fn stands_in(&self) -> bool {
        self.failure.is_some() || self.crashed.is_some()
    }
}

/// The host of `address`, without its scheme, path, query or fragment; the whole address when it has no scheme.
fn host_of(address: &str) -> &str {
    let rest = address.split_once("://").map_or(address, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// The heading and the sentence the error view shows for the engine's error `code` on `host`.
pub fn failure_words(code: i32, host: &str) -> (&'static str, String) {
    match code {
        -105 | -137 => (
            "This site cannot be reached",
            format!("The address of {host} could not be found."),
        ),
        -102 => (
            "This site cannot be reached",
            format!("{host} refused to connect. Nothing may be listening on that port."),
        ),
        -7 | -118 => (
            "This site cannot be reached",
            format!("{host} took too long to respond."),
        ),
        -106 => (
            "No internet",
            "This machine is not connected to the internet.".to_owned(),
        ),
        -100 | -101 | -109 => (
            "This site cannot be reached",
            format!("The connection to {host} was cut off or could not be made."),
        ),
        _ => (
            "This page could not be loaded",
            format!("Loading {host} failed."),
        ),
    }
}

/// What the crash view says of the engine's word `reason` for a content process that went away.
fn crash_words(reason: &str) -> &'static str {
    match reason {
        "oom" => "The page ran out of memory.",
        "killed" => "The page was ended by the system.",
        "launch-failed" => "The page could not be started.",
        "integrity-failure" => "The page failed a check of the engine's code.",
        _ => "The page crashed.",
    }
}

impl WorkspaceState {
    /// Takes in where the page of tab `tab` is in its life, as the engine reported it; the note that the browser was started again stays as it is.
    pub fn browser_tab_life(&mut self, tab: u32, life: PageLife) {
        if let Some(tab) = self.browser_tab(tab) {
            let restarted = tab.life.restarted;
            tab.life = PageLife { restarted, ..life };
        }
    }

    /// Opens every tab of `tabs` again after the browser stopped and was started again: a tab on a sealed document shows it again and any other loads its last address, and each says that it was opened again.
    pub fn browser_restarted(&mut self, tabs: &[u32]) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        for tab in browser.tabs.iter_mut().filter(|tab| tabs.contains(&tab.id)) {
            tab.life = PageLife {
                restarted: true,
                ..PageLife::default()
            };
            tab.page = None;
            tab.picking = false;
            tab.picking_area = false;
            tab.commands
                .retain(|command| !matches!(command, Command::Load(_) | Command::LoadSealed));
            tab.commands.push(if tab.sealed.is_some() {
                Command::LoadSealed
            } else {
                Command::Load(crate::browser_address::address_of(&tab.address.text()))
            });
        }
    }

    /// Opens a tab for the window `popup` a page of the tab being scoped opened with `window.open`, on `address` on the machine `host`, for an agent when `agent` and shown otherwise; its page is that window, which keeps its `window.opener`. Gives the tab's number, or nothing when there are too many tabs.
    pub fn browser_popup_opened(
        &mut self,
        address: &str,
        host: Option<String>,
        agent: bool,
        popup: u64,
    ) -> Option<u32> {
        let id = self.browser_tab_opened(address, host, agent, !agent)?;
        if let Some(tab) = self.browser_tab(id) {
            tab.commands = vec![Command::AdoptPopup(popup)];
        }
        Some(id)
    }

    /// Carries out pressing `id` of what the pane shows of the life of the page being shown, and says whether `id` was one.
    pub(crate) fn browser_life_pressed(&mut self, id: Target) -> bool {
        let Some(browser) = self.browser.as_mut() else {
            return false;
        };
        match id {
            Target::Browser(BrowserEvent::BrowserStop) => browser.commands.push(Command::Stop),
            Target::Browser(BrowserEvent::BrowserRetry) => {
                let address = browser
                    .life
                    .failure
                    .as_ref()
                    .map(|failure| failure.url.clone())
                    .unwrap_or_else(|| browser.address.text().to_owned());
                browser.life.failure = None;
                browser.address = TextFieldState::new(&address);
                browser.commands.push(Command::Load(address));
            }
            Target::Browser(BrowserEvent::BrowserRevive) => {
                browser.life.crashed = None;
                browser.life.unresponsive = false;
                browser.commands.push(Command::Reload);
            }
            Target::Browser(BrowserEvent::BrowserRestartDismiss) => {
                browser.life.restarted = false;
            }
            _ => return false,
        }
        true
    }
}

/// Emits a bar as wide as its parent, holding `words` in `tone` and what `buttons` emits after them.
#[track_caller]
fn bar(words: &str, tone: Role, buttons: impl FnOnce()) {
    rustle::row_arranged(
        Modifier::new()
            .height(BAR)
            .fill_max_width()
            .background(theme::color(Role::Surface))
            .border_bottom(1.0, theme::color(Role::Border))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            clipped(words.to_owned(), 13, tone);
            buttons();
        },
    );
}

/// Emits the bars of the page being shown above it: that the browser was started again, and that the page stopped answering.
pub fn life_bars(browser: &Browser) {
    if browser.life.restarted {
        bar(RESTARTED, Role::Text, || {
            Button::icon(
                phosphor::X,
                Target::Browser(BrowserEvent::BrowserRestartDismiss),
            )
            .build();
        });
    }
    if browser.life.unresponsive && browser.life.crashed.is_none() {
        bar(
            "This page is not responding. Wait for it, or load it again.",
            Role::Danger,
            || {
                Button::new("Reload", Target::Browser(BrowserEvent::BrowserRevive))
                    .variant(Variant::Outline)
                    .build();
            },
        );
    }
}

/// Emits the thin line over the top of the page that grows as it loads; nothing while it is not loading.
pub fn progress_line(browser: &Browser) {
    if !browser.life.loading {
        return;
    }
    let progress = browser.life.progress.clamp(0.05, 1.0);
    rustle::linear_progress_indicator_with(
        move || progress,
        Modifier::new().fill_max_width().height(PROGRESS_LINE),
        theme::color(Role::Accent),
        rustle::Color::TRANSPARENT,
    );
}

/// Emits the view that stands in for a page whose life is `life` and whose address line shows `shown`, `width` by `height` logical pixels: why its address did not load with a button to try again, or that it crashed with a button to load it again.
pub fn life_page(life: &PageLife, shown: &str, width: f32, height: f32) {
    let across = (width - 48.0).clamp(0.0, 520.0);
    let room = (across - 40.0).max(0.0);
    let life = life.clone();
    let shown = shown.to_owned();
    rustle::row_arranged(
        Modifier::new()
            .width(width)
            .height(height)
            .background(theme::color(Role::Background))
            .padding(theme::sp(6)),
        Arrangement::Center,
        Alignment::Center,
        move || {
            rustle::column_arranged(
                Modifier::new()
                    .width(across)
                    .panel(Radius::Xl)
                    .padding(theme::sp(5)),
                Arrangement::SpacedBy(theme::sp(3)),
                Alignment::Start,
                || {
                    let (heading, sentence, detail, button, target) =
                        match (&life.crashed, &life.failure) {
                            (Some(reason), _) => (
                                "This page stopped",
                                crash_words(reason).to_owned(),
                                if reason.is_empty() {
                                    String::new()
                                } else {
                                    format!("The engine said: {reason}")
                                },
                                "Reload",
                                BrowserEvent::BrowserRevive,
                            ),
                            (None, Some(failure)) => {
                                let address = if failure.url.is_empty() {
                                    &shown
                                } else {
                                    &failure.url
                                };
                                let (heading, sentence) =
                                    failure_words(failure.code, host_of(address));
                                (
                                    heading,
                                    sentence,
                                    format!("{} ({})", failure.name, failure.code),
                                    "Try again",
                                    BrowserEvent::BrowserRetry,
                                )
                            }
                            (None, None) => return,
                        };
                    rustle::row_arranged(
                        Modifier::new(),
                        Arrangement::SpacedBy(theme::sp(2)),
                        Alignment::Center,
                        || {
                            glyph(phosphor::WARNING_CIRCLE, 5.0, Role::Danger);
                            rustle::basic_text(
                                Modifier::new(),
                                heading,
                                theme::text_style(18, 1.3)
                                    .font_weight(rustle::FontWeight::SEMI_BOLD)
                                    .color(theme::color(Role::Text))
                                    .max_lines(1),
                                TextOverflow::Ellipsis,
                            );
                        },
                    );
                    wrapping(
                        Modifier::new().width_in(0.0, room),
                        sentence,
                        13,
                        1.45,
                        Role::Text,
                    );
                    if !detail.is_empty() {
                        rustle::basic_text(
                            Modifier::new().width_in(0.0, room),
                            detail,
                            theme::in_mono(theme::line(12, Role::Muted)),
                            TextOverflow::Ellipsis,
                        );
                    }
                    Button::new(button, Target::Browser(target)).build();
                },
            );
        },
    );
}

#[cfg(test)]
/// Tests of what the pane makes of the life of a page.
mod tests {
    use super::*;

    /// Back and forward stay pressable until the engine reports the history, and then follow it.
    #[test]
    fn history_buttons_follow_the_reported_history() {
        let mut life = PageLife::default();
        assert!(life.can_go_back() && life.can_go_forward());
        life.history_known = true;
        assert!(!life.can_go_back() && !life.can_go_forward());
        life.back = true;
        assert!(life.can_go_back() && !life.can_go_forward());
    }

    /// The common failures name the host and say what went wrong.
    #[test]
    fn failures_are_said_in_words() {
        assert!(
            failure_words(-105, "example.test")
                .1
                .contains("example.test")
        );
        assert!(failure_words(-102, "localhost:3000").1.contains("refused"));
        assert!(failure_words(-118, "slow.test").1.contains("too long"));
        assert_eq!(
            host_of("https://example.test:8080/a?b#c"),
            "example.test:8080"
        );
    }
}
