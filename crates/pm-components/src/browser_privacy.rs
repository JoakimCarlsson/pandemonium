//! What the pane shows of the reader's privacy in the browser: the shield in the toolbar that counts the ads and trackers blocked on the page and lets its site through, the bar that offers to save a login the page sent, the list of saved logins under a login field that fills one in, and the sheet asking for the primary password that seals saved logins where the system has no keyring. Saved passwords never reach the window; only user names do, and the primary password the reader types goes to the helper.

use crate::Button;
use crate::areas::BrowserEvent;
use crate::browser::{Browser, Command};
use crate::browser_chrome::ChromeAction;
use crate::button;
use crate::button::Variant;
use crate::events::{Event, Interactive, send};
use crate::find::SHADOW;
use crate::parts::clipped;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::target::Target;
use crate::theme;
use crate::theme::{Frame, Indicated, Radius, Role, ShadowLayers};
use crate::{Input, parts::wrapping};
use rustle::phosphor;
use rustle::{Alignment, Arrangement, BoxAlignment, Modifier, TextFieldState};

/// How wide the sheet asking for the primary password is at most, in logical pixels.
const PRIMARY_WIDTH: f32 = 420.0;

/// How wide the list of saved logins is at least, in logical pixels.
const LOGINS_WIDTH: f32 = 240.0;

/// How tall one row of the list of saved logins is, in logical pixels.
const LOGIN_ROW: f32 = 30.0;

/// How many saved logins the list shows at most.
const LOGINS_SHOWN: usize = 6;

/// How the blocker of ads and trackers stands on a page.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SiteBlocking {
    /// Whether it blocks on the page's site.
    pub on: bool,
    /// How many requests of the page it stopped since the page last loaded.
    pub blocked: u32,
}

/// A page's offer to save a login the reader sent from it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PasswordAsk {
    /// The number of the offer, for the answer.
    pub id: u64,
    /// The origin the login is for.
    pub origin: String,
    /// The user name, or nothing when the form had none.
    pub username: String,
    /// Whether a password is saved for that user name already, so saving replaces it.
    pub update: bool,
}

/// A login field holding the keyboard on a page whose origin has saved logins, and where it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoginChoices {
    /// Logical pixels from the left of the page to the field.
    pub x: f32,
    /// Logical pixels from the top of the page to the field.
    pub y: f32,
    /// The field's width in logical pixels.
    pub width: f32,
    /// The field's height in logical pixels.
    pub height: f32,
    /// The user names saved for the origin.
    pub usernames: Vec<String>,
}

/// The helper's question for the primary password that seals saved logins, and what the reader typed so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrimaryAsk {
    /// Whether no primary password is set yet, so the reader chooses one and types it twice.
    pub create: bool,
    /// The origin of the login that needs the saved logins open.
    pub origin: String,
    /// Why the last answer was refused, or nothing.
    pub problem: String,
    /// The line the password is typed in.
    pub password: TextFieldState,
    /// The line a new password is typed in again.
    pub confirm: TextFieldState,
    /// Whether an answer went to the helper and is being checked.
    pub checking: bool,
}

impl PrimaryAsk {
    /// A question as the helper asked it, with empty lines.
    pub fn new(create: bool, origin: &str, problem: &str) -> Self {
        Self {
            create,
            origin: origin.to_owned(),
            problem: problem.to_owned(),
            ..Self::default()
        }
    }
}

/// The host of an origin, such as `example.com` for `https://example.com`.
fn host_of(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, host)| host)
}

/// What the sheet asking for the primary password says: its title and the line under it.
pub fn primary_words(ask: &PrimaryAsk) -> (String, String) {
    if ask.create {
        (
            "Set a primary password for saved logins".to_owned(),
            "This system has no keyring to protect saved passwords, so they are sealed with a password you choose. You type it once each time the browser starts; it cannot be recovered if you forget it.".to_owned(),
        )
    } else {
        (
            "Unlock saved logins".to_owned(),
            format!(
                "Type your primary password to save or fill in a login for {}.",
                host_of(&ask.origin)
            ),
        )
    }
}

/// What the shield's tooltip says of how the blocker stands.
pub fn shield_words(blocking: SiteBlocking) -> String {
    match (blocking.on, blocking.blocked) {
        (false, _) => {
            "Ads and trackers are not blocked on this site. Press to block them.".to_owned()
        }
        (true, 0) => {
            "Blocking ads and trackers: none on this page so far. Press to let this site through."
                .to_owned()
        }
        (true, 1) => {
            "Blocked 1 ad or tracker on this page. Press to let this site through.".to_owned()
        }
        (true, count) => format!(
            "Blocked {count} ads and trackers on this page. Press to let this site through."
        ),
    }
}

/// What the bar offering to save a login says.
pub fn offer_words(ask: &PasswordAsk) -> String {
    let host = host_of(&ask.origin);
    let verb = if ask.update {
        "Update the saved password"
    } else {
        "Save the password"
    };
    if ask.username.is_empty() {
        format!("{verb} for {host}?")
    } else {
        format!("{verb} of {} for {host}?", ask.username)
    }
}

impl WorkspaceState {
    /// Notes how the blocker of ads and trackers stands on the page of tab `tab`.
    pub fn browser_blocking(&mut self, tab: u32, on: bool, blocked: u32) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.chrome.blocking = Some(SiteBlocking { on, blocked });
        }
    }

    /// Shows the offer to save a login the page of tab `tab` made, or takes it away when `ask` is nothing and it had the number `closed`.
    pub fn browser_password_offered(&mut self, tab: u32, ask: Option<PasswordAsk>, closed: u64) {
        if let Some(tab) = self.browser_tab(tab) {
            match ask {
                Some(ask) => tab.chrome.password = Some(ask),
                None => {
                    if tab
                        .chrome
                        .password
                        .as_ref()
                        .is_some_and(|open| open.id == closed)
                    {
                        tab.chrome.password = None;
                    }
                }
            }
        }
    }

    /// Shows the saved logins under the login field that holds the keyboard on the page of tab `tab`, or takes them away when `choices` is nothing.
    pub fn browser_login_field(&mut self, tab: u32, choices: Option<LoginChoices>) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.chrome.logins = choices;
        }
    }

    /// Shows the question for the primary password the helper asked on the page of tab `tab`, or takes it away when `ask` is nothing.
    pub fn browser_primary_asked(&mut self, tab: u32, ask: Option<PrimaryAsk>) {
        if let Some(tab) = self.browser_tab(tab) {
            tab.chrome.primary = ask;
        }
    }

    /// Whether the tab being shown asks for the primary password.
    pub fn browser_primary_open(&self) -> bool {
        self.browser
            .as_ref()
            .is_some_and(|browser| browser.chrome.primary.is_some())
    }

    /// Answers the question for the primary password of the tab being shown: with what was typed when `accept`, refusing locally a new password typed differently twice, otherwise turning it down and taking the sheet away.
    pub fn browser_primary_answered(&mut self, accept: bool) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        let action = if accept {
            let Some(ask) = browser.chrome.primary.as_mut() else {
                return;
            };
            let typed = ask.password.text().to_owned();
            if typed.is_empty() || ask.checking {
                return;
            }
            if ask.create && ask.confirm.text().is_empty() {
                return;
            }
            if ask.create && ask.confirm.text() != typed {
                ask.problem = "The two passwords differ.".to_owned();
                return;
            }
            ask.checking = true;
            ask.problem.clear();
            ChromeAction::PrimaryPassword(Some(typed))
        } else {
            if browser.chrome.primary.take().is_none() {
                return;
            }
            ChromeAction::PrimaryPassword(None)
        };
        browser.commands.push(Command::Chrome(action));
    }

    /// Carries out pressing `id` of the shield, the offer to save a login or the list of saved logins, and says whether it was one of those.
    pub(crate) fn browser_privacy_pressed(&mut self, id: Target) -> bool {
        let Target::Browser(event) = id else {
            return false;
        };
        let Some(browser) = self.browser.as_mut() else {
            return false;
        };
        let action = match event {
            BrowserEvent::BrowserShield => {
                let Some(blocking) = browser.chrome.blocking.as_mut() else {
                    return true;
                };
                blocking.on = !blocking.on;
                blocking.blocked = 0;
                ChromeAction::SiteBlocking(blocking.on)
            }
            BrowserEvent::BrowserPasswordSave
            | BrowserEvent::BrowserPasswordNever
            | BrowserEvent::BrowserPasswordDismiss => {
                let Some(ask) = browser.chrome.password.take() else {
                    return true;
                };
                ChromeAction::AnswerPassword {
                    id: ask.id,
                    save: event == BrowserEvent::BrowserPasswordSave,
                    never: event == BrowserEvent::BrowserPasswordNever,
                }
            }
            BrowserEvent::BrowserLogin(index) => {
                let Some(choices) = browser.chrome.logins.take() else {
                    return true;
                };
                let Some(username) = choices.usernames.get(index).cloned() else {
                    return true;
                };
                ChromeAction::FillLogin(username)
            }
            BrowserEvent::BrowserPrimaryOk => {
                self.browser_primary_answered(true);
                return true;
            }
            BrowserEvent::BrowserPrimaryCancel => {
                self.browser_primary_answered(false);
                return true;
            }
            BrowserEvent::BrowserLoginList
            | BrowserEvent::BrowserPasswordBar
            | BrowserEvent::BrowserPrimaryField
            | BrowserEvent::BrowserPrimaryConfirm
            | BrowserEvent::BrowserPrimarySheet => return true,
            _ => return false,
        };
        browser.commands.push(Command::Chrome(action));
        true
    }
}

/// Emits the shield of the toolbar, once the helper said how its blocker stands on the page: accented with the count while it blocks, muted and crossed out while the site is let through; pressing it switches.
#[track_caller]
pub fn shield_button(browser: &Browser) {
    let Some(blocking) = browser.chrome.blocking else {
        return;
    };
    let id = Target::Browser(BrowserEvent::BrowserShield);
    let shield = if blocking.on && blocking.blocked > 0 {
        Button::new(blocking.blocked.min(999).to_string(), id)
            .with_icon(phosphor::SHIELD_CHECK)
            .tone(Role::Accent)
    } else if blocking.on {
        Button::icon(phosphor::SHIELD_CHECK, id).tone(Role::Muted)
    } else {
        Button::icon(phosphor::SHIELD_SLASH, id).tone(Role::Muted)
    };
    shield
        .variant(Variant::Ghost)
        .tooltip(shield_words(blocking))
        .build();
}

/// Emits what the pane draws over the page of `browser` for logins: the bar offering to save one, the list of saved ones under the field that holds the keyboard and the sheet asking for the primary password, telling `on_hover` when the pointer comes over one and leaves it.
#[track_caller]
pub fn privacy_overlays(browser: &Browser, on_hover: impl Fn(bool) + Clone + 'static) {
    if let Some(ask) = browser.chrome.primary.as_ref() {
        primary_sheet(ask, on_hover.clone());
    }
    if let Some(ask) = browser.chrome.password.as_ref() {
        password_bar(ask, on_hover.clone());
    }
    if let Some(choices) = browser.chrome.logins.as_ref()
        && !choices.usernames.is_empty()
    {
        login_list(choices, on_hover);
    }
}

/// Emits the bar at the top of the page that offers to save the login the page sent, with Save, Never for this site and Not now.
#[track_caller]
fn password_bar(ask: &PasswordAsk, on_hover: impl Fn(bool) + 'static) {
    rustle::row_arranged(
        Modifier::new()
            .align(BoxAlignment::TopStart)
            .fill_max_width()
            .z_index(41)
            .height(crate::browser_safety::BAR)
            .background(theme::color(Role::Surface))
            .border_bottom(1.0, theme::color(Role::Border))
            .on_hover(on_hover)
            .control(
                Target::Browser(BrowserEvent::BrowserPasswordBar),
                send(Event::Browser(BrowserEvent::BrowserPasswordBar)),
            )
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            crate::parts::glyph(phosphor::KEY, 4.0, Role::Accent);
            rustle::row_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Center,
                || clipped(offer_words(ask), 13, Role::Text),
            );
            button::action(
                if ask.update { "Update" } else { "Save" },
                Target::Browser(BrowserEvent::BrowserPasswordSave),
                Variant::Default,
            );
            if !ask.update {
                button::action(
                    "Never for this site",
                    Target::Browser(BrowserEvent::BrowserPasswordNever),
                    Variant::Outline,
                );
            }
            button::action(
                "Not now",
                Target::Browser(BrowserEvent::BrowserPasswordDismiss),
                Variant::Ghost,
            );
        },
    );
}

/// Emits the saved logins in a list just under the login field that holds the keyboard; pressing one fills the form with it.
#[track_caller]
fn login_list(choices: &LoginChoices, on_hover: impl Fn(bool) + 'static) {
    rustle::column_arranged(
        Modifier::new()
            .align(BoxAlignment::TopStart)
            .offset(
                choices.x.max(0.0),
                (choices.y + choices.height).max(0.0) + theme::sp(0.5),
            )
            .z_index(60)
            .width(choices.width.max(LOGINS_WIDTH))
            .shadow_layers(&SHADOW, Radius::Md.px())
            .panel(Radius::Md)
            .on_hover(on_hover)
            .control(
                Target::Browser(BrowserEvent::BrowserLoginList),
                send(Event::Browser(BrowserEvent::BrowserLoginList)),
            )
            .padding(theme::sp(1)),
        Arrangement::Start,
        Alignment::Start,
        || {
            for (index, username) in choices.usernames.iter().enumerate().take(LOGINS_SHOWN) {
                let id = Target::Browser(BrowserEvent::BrowserLogin(index));
                rustle::row_arranged(
                    crate::events::pressed(
                        Modifier::new()
                            .fill_max_width()
                            .height(LOGIN_ROW)
                            .indicated(
                                theme::selectable(false).radius(Radius::Sm),
                                theme::Focused::Control(crate::focus::is_focused(id)),
                            ),
                        id,
                    )
                    .padding_xy(theme::sp(2), 0.0),
                    Arrangement::SpacedBy(theme::sp(2)),
                    Alignment::Center,
                    || {
                        crate::parts::glyph(phosphor::KEY, 3.5, Role::Muted);
                        rustle::row_arranged(
                            Modifier::new().weight(1.0),
                            Arrangement::Start,
                            Alignment::Center,
                            || {
                                clipped(
                                    if username.is_empty() {
                                        "Saved password".to_owned()
                                    } else {
                                        username.clone()
                                    },
                                    12,
                                    Role::Text,
                                )
                            },
                        );
                    },
                );
            }
        },
    );
}

/// Emits the sheet over the page asking for the primary password: to choose one, typed twice, or to unlock the saved logins with it.
#[track_caller]
fn primary_sheet(ask: &PrimaryAsk, on_hover: impl Fn(bool) + 'static) {
    let ask = ask.clone();
    rustle::with_constraints(
        Modifier::new()
            .match_parent_size()
            .z_index(61)
            .on_hover(on_hover),
        move |room| {
            let across = (room.max_width - 32.0).clamp(0.0, PRIMARY_WIDTH);
            rustle::row_arranged(
                Modifier::new()
                    .fill_max_size()
                    .background(theme::color(Role::Background).copy(0.55))
                    .control(
                        Target::Browser(BrowserEvent::BrowserPrimarySheet),
                        send(Event::Browser(BrowserEvent::BrowserPrimarySheet)),
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
                        || primary_panel(&ask, across),
                    );
                },
            );
        },
    );
}

/// Emits the panel of the sheet asking for the primary password, `across` logical pixels wide.
#[track_caller]
fn primary_panel(ask: &PrimaryAsk, across: f32) {
    let room = (across - 2.0 * theme::sp(4)).max(0.0);
    let (title, line) = primary_words(ask);
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .shadow_layers(&SHADOW, Radius::Lg.px())
            .panel(Radius::Lg)
            .padding(theme::sp(4)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(2)),
                Alignment::Center,
                || {
                    crate::parts::glyph(phosphor::LOCK_KEY, 4.0, Role::Accent);
                    rustle::row_arranged(
                        Modifier::new().weight(1.0),
                        Arrangement::Start,
                        Alignment::Center,
                        || clipped(title.clone(), 14, Role::Text),
                    );
                },
            );
            wrapping(
                Modifier::new().width_in(0.0, room),
                line.as_str(),
                13,
                1.45,
                Role::Muted,
            );
            Input::new(
                &ask.password,
                Target::Browser(BrowserEvent::BrowserPrimaryField),
            )
            .secure(true)
            .placeholder("Primary password")
            .size(Size::Sm)
            .invalid(!ask.problem.is_empty())
            .modifier(Modifier::new().fill_max_width())
            .submits(Target::Browser(BrowserEvent::BrowserPrimaryOk))
            .build();
            if ask.create {
                Input::new(
                    &ask.confirm,
                    Target::Browser(BrowserEvent::BrowserPrimaryConfirm),
                )
                .secure(true)
                .placeholder("Type it again")
                .size(Size::Sm)
                .modifier(Modifier::new().fill_max_width())
                .submits(Target::Browser(BrowserEvent::BrowserPrimaryOk))
                .build();
            }
            if !ask.problem.is_empty() {
                wrapping(
                    Modifier::new().width_in(0.0, room),
                    ask.problem.as_str(),
                    12,
                    1.4,
                    Role::Danger,
                );
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
                            button::action(
                                "Cancel",
                                Target::Browser(BrowserEvent::BrowserPrimaryCancel),
                                Variant::Outline,
                            );
                            let label = match (ask.checking, ask.create) {
                                (true, _) => "Checking…",
                                (false, true) => "Set password",
                                (false, false) => "Unlock",
                            };
                            button::action(
                                label,
                                Target::Browser(BrowserEvent::BrowserPrimaryOk),
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

    /// The shield says what pressing it does, and counts in words.
    #[test]
    fn shield_words_count_and_say_what_a_press_does() {
        assert!(
            shield_words(SiteBlocking {
                on: true,
                blocked: 0
            })
            .contains("none")
        );
        assert!(
            shield_words(SiteBlocking {
                on: true,
                blocked: 1
            })
            .starts_with("Blocked 1 ad or tracker")
        );
        assert!(
            shield_words(SiteBlocking {
                on: true,
                blocked: 12
            })
            .starts_with("Blocked 12 ads")
        );
        assert!(
            shield_words(SiteBlocking {
                on: false,
                blocked: 3
            })
            .contains("not blocked")
        );
    }

    /// A new primary password typed differently twice is refused in the pane; typed alike it goes to the helper once, and turning the question down sends nothing for a password and takes the sheet away.
    #[test]
    fn primary_password_answers_go_to_the_helper_once() {
        use crate::browser::Command;
        let mut state = WorkspaceState::sample();
        state.open_browser("https://example.com/sign-in");
        let tab = state
            .browser
            .as_ref()
            .map_or(0, |browser| browser.active_id());
        let ask = PrimaryAsk::new(true, "https://example.com", "");
        ask.password.set_text("correct horse");
        ask.confirm.set_text("correct hrose");
        state.browser_primary_asked(tab, Some(ask));
        let sent = |state: &mut WorkspaceState| {
            state.browser.as_mut().map_or(Vec::new(), |browser| {
                browser
                    .commands
                    .drain(..)
                    .filter_map(|command| match command {
                        Command::Chrome(ChromeAction::PrimaryPassword(typed)) => Some(typed),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
        };
        state.browser_primary_answered(true);
        assert!(sent(&mut state).is_empty());
        let problem = state
            .browser
            .as_ref()
            .and_then(|browser| browser.chrome.primary.as_ref())
            .map(|ask| ask.problem.clone());
        assert_eq!(problem.as_deref(), Some("The two passwords differ."));
        if let Some(ask) = state
            .browser
            .as_mut()
            .and_then(|browser| browser.chrome.primary.as_mut())
        {
            ask.confirm.set_text("correct horse");
        }
        state.browser_primary_answered(true);
        state.browser_primary_answered(true);
        assert_eq!(sent(&mut state), vec![Some("correct horse".to_owned())]);
        state.browser_primary_answered(false);
        assert_eq!(sent(&mut state), vec![None]);
        assert!(!state.browser_primary_open());
    }

    /// The offer names the user and the host, and says when it replaces a saved password.
    #[test]
    fn offer_words_name_the_user_and_the_host() {
        let mut ask = PasswordAsk {
            id: 1,
            origin: "https://example.com".to_owned(),
            username: "ada".to_owned(),
            update: false,
        };
        assert_eq!(
            offer_words(&ask),
            "Save the password of ada for example.com?"
        );
        ask.update = true;
        ask.username.clear();
        assert_eq!(
            offer_words(&ask),
            "Update the saved password for example.com?"
        );
    }
}
