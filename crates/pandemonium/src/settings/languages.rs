//! The Languages page: language extensions to find, install and remove, and
//! what the Language Settings section next to it is drawn from.
//!
//! The extensions are laid out as a choice between
//! every extension, the installed ones and the ones not yet installed, and a
//! card per extension with the buttons that install or remove it. The
//! languages the editor ships are listed beneath, folded away.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use pm_text::{Language, Server};
use pm_ui::{Div, Styled, Theme, button, h_flex, space, text, toggle_row, v_flex};

use crate::config::{Preferences, ServerList, extensions::Entry};
use crate::input::Input;
use crate::message::Message;
use crate::settings::parts::{card, clipped, header, note};

/// How wide the choice of which extensions to list is, in spaces.
const FILTER_WIDTH: f32 = 110.0;

/// A background catalogue read or completed extension mutation.
pub enum ResultMessage {
    /// Available packages or a retrieval error.
    Catalogue(Result<Vec<Entry>, String>),
    /// A completed install, import or removal.
    Changed(Result<(), String>),
}

/// Which extensions the list shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Filter {
    /// Every extension, installed or not.
    #[default]
    All,
    /// Only the extensions that are installed.
    Installed,
    /// Only the extensions that are not installed.
    NotInstalled,
}

impl Filter {
    /// Every choice, in the order they are offered.
    pub const ALL: [Self; 3] = [Self::All, Self::Installed, Self::NotInstalled];

    /// What the choice is called.
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Installed => "Installed",
            Self::NotInstalled => "Not Installed",
        }
    }
}

/// A server being added or edited for one language.
pub struct ServerEditor {
    /// Canonical language name.
    pub language: &'static str,
    /// Existing server position, or a new entry.
    pub index: Option<usize>,
    /// Executable, JSON arguments and JSON initialization options.
    pub fields: [Input; 3],
    /// Original installation recipe, retained when its command is unchanged.
    pub original: Option<Server>,
}

/// Transient state for the Languages page.
pub struct Languages {
    /// Which extensions the list shows.
    pub filter: Filter,
    /// Whether the group of built-in languages is open.
    pub builtin_open: bool,
    /// The language the Language Settings section shows, by name.
    pub selected: Option<&'static str>,
    /// The server form that is open.
    pub editor: Option<ServerEditor>,
    /// Catalogue entries from the last successful retrieval.
    pub catalogue: Vec<Entry>,
    /// Whether an extension operation is running.
    pub busy: bool,
    /// Whether the catalogue was requested.
    pub requested: bool,
    /// Last retrieval or mutation error.
    pub error: Option<String>,
    /// Completed background results waiting for the UI thread.
    pub results: Arc<Mutex<Vec<ResultMessage>>>,
}

impl Default for Languages {
    /// Creates an empty catalogue.
    fn default() -> Self {
        Self {
            filter: Filter::default(),
            builtin_open: false,
            selected: None,
            editor: None,
            catalogue: Vec::new(),
            busy: false,
            requested: false,
            error: None,
            results: Arc::default(),
        }
    }
}

/// The data needed to render the Languages page.
pub struct LanguagesPage<'a> {
    /// Filter and catalogue.
    pub state: &'a Languages,
    /// The box of the server form that has the keyboard, while one does.
    pub field: Option<usize>,
    /// Visible caret phase.
    pub solid: bool,
    /// The preferences the language settings follow.
    pub preferences: &'a Preferences,
    /// Configured server overrides.
    pub servers: &'a BTreeMap<String, ServerList>,
    /// The language the Language Settings section shows.
    pub selected: Option<Language>,
}

/// Resolves replacement and additional server lists through the language defaults.
pub fn servers(language: Language, configured: &BTreeMap<String, ServerList>) -> Vec<Server> {
    match configured.get(language.name()) {
        Some(ServerList::Replace(servers)) => servers.clone(),
        Some(ServerList::Add(added)) => {
            let mut servers = language.servers().to_vec();
            for server in added {
                if !servers
                    .iter()
                    .any(|existing| existing.command == server.command)
                {
                    servers.push(*server);
                }
            }
            servers
        }
        None => language.servers().to_vec(),
    }
}

/// One extension of the list, with where it sits in the installed and offered lists.
struct Listed {
    /// The extension.
    entry: Entry,
    /// Its place among the installed extensions, when it is installed.
    installed: Option<usize>,
    /// Its place in the catalogue, when the catalogue offers it.
    offered: Option<usize>,
    /// Whether the catalogue offers a newer version than the installed one.
    outdated: bool,
    /// Its place among the remembered imports, when it was imported from a folder.
    remembered: Option<usize>,
}

/// Builds the Languages page.
pub fn language_page(theme: &Theme, page: &LanguagesPage<'_>) -> Div<Message> {
    let cards = listed(page.state)
        .into_iter()
        .filter(|listed| match page.state.filter {
            Filter::All => true,
            Filter::Installed => listed.installed.is_some(),
            Filter::NotInstalled => listed.installed.is_none(),
        })
        .map(|listed| card(theme, vec![extension_card(theme, page, &listed)]))
        .collect::<Vec<_>>();
    let cards = match (&page.state.error, page.state.busy, cards.is_empty()) {
        (_, true, true) => vec![card(
            theme,
            vec![note(theme, "Loading language extensions…")],
        )],
        (Some(error), _, true) => vec![card(
            theme,
            vec![note(
                theme,
                &format!("The language catalogue is unavailable: {error}"),
            )],
        )],
        (None, false, true) => Vec::new(),
        _ => cards,
    };
    let selected = Filter::ALL
        .iter()
        .position(|filter| *filter == page.state.filter);
    v_flex()
        .w_full()
        .gap(3)
        .child(
            h_flex()
                .w_full()
                .gap(2)
                .items_center()
                .justify_between()
                .child(
                    toggle_row(
                        Filter::ALL.into_iter().map(|filter| {
                            (
                                filter.label().to_owned(),
                                Message::SetLanguageFilter(filter),
                            )
                        }),
                        selected,
                    )
                    .w_px(space(FILTER_WIDTH)),
                )
                .child(button("Import Extension", Message::ImportLanguageExtension).outlined()),
        )
        .children(cards)
        .when(page.state.filter != Filter::NotInstalled, |column| {
            column.child(builtin_section(theme, page))
        })
}

/// The languages the editor ships, as one quiet group under the extensions.
///
/// The group stays folded until it is opened, so twenty-odd languages do not
/// push the extensions off the screen.
fn builtin_section(theme: &Theme, page: &LanguagesPage<'_>) -> Div<Message> {
    let mut languages = Language::all()
        .into_iter()
        .filter(|language| !language.is_wasm())
        .collect::<Vec<_>>();
    languages.sort_by_key(|language| language.name().to_lowercase());
    let open = page.state.builtin_open;
    let rows = languages
        .iter()
        .map(|language| {
            h_flex()
                .w_full()
                .px(3)
                .py(1.5)
                .gap(4)
                .items_center()
                .justify_between()
                .child(text(language.name()))
                .child(
                    text(clipped(&language.associations().join(", ")))
                        .text_xs()
                        .font_mono()
                        .color(theme.colors.text_subtle),
                )
        })
        .collect::<Vec<_>>();
    v_flex()
        .w_full()
        .gap(1.5)
        .child(header(
            theme,
            "Built-in languages",
            languages.len(),
            open,
            Message::ToggleBuiltinLanguages,
        ))
        .when(open, |section| section.child(card(theme, rows)))
}

/// The installed extensions followed by the ones only the catalogue offers.
fn listed(state: &Languages) -> Vec<Listed> {
    let installed = crate::config::extensions::installed();
    let mut listed = installed
        .iter()
        .enumerate()
        .map(|(place, entry)| {
            let offered = state
                .catalogue
                .iter()
                .position(|available| available.id == entry.id);
            let outdated = offered.is_some_and(|offered| {
                semver::Version::parse(&state.catalogue[offered].version).ok()
                    > semver::Version::parse(&entry.version).ok()
            });
            Listed {
                entry: entry.clone(),
                installed: Some(place),
                offered,
                outdated,
                remembered: None,
            }
        })
        .collect::<Vec<_>>();
    listed.extend(
        state
            .catalogue
            .iter()
            .enumerate()
            .filter(|(_, entry)| !installed.iter().any(|current| current.id == entry.id))
            .map(|(place, entry)| Listed {
                entry: entry.clone(),
                installed: None,
                offered: Some(place),
                outdated: false,
                remembered: None,
            }),
    );
    listed.extend(
        crate::config::extensions::imported()
            .into_iter()
            .enumerate()
            .filter(|(_, known)| !listed.iter().any(|shown| shown.entry.id == known.entry.id))
            .map(|(place, known)| Listed {
                entry: known.entry,
                installed: None,
                offered: None,
                outdated: false,
                remembered: Some(place),
            })
            .collect::<Vec<_>>(),
    );
    listed.sort_by_key(|listed| listed.entry.name.to_lowercase());
    listed
}

/// One extension: name and version over the buttons that
/// change it, then what it provides, then who publishes it and where its source is.
fn extension_card(theme: &Theme, page: &LanguagesPage<'_>, listed: &Listed) -> Div<Message> {
    let entry = &listed.entry;
    let supported =
        entry.platforms.is_empty() || entry.platforms.contains(&pm_text::install::platform());
    let mut actions = h_flex().gap(1.5).items_center();
    actions = match (page.state.busy, listed.installed, listed.offered) {
        (true, ..) => {
            actions.child(button("Working…", Message::RefreshLanguageCatalogue).outlined())
        }
        (_, Some(place), offered) => {
            if let (true, Some(offered)) = (listed.outdated, offered) {
                actions = actions
                    .child(button("Upgrade", Message::InstallLanguageExtension(offered)).filled());
            }
            actions.child(button("Uninstall", Message::RemoveLanguageExtension(place)).outlined())
        }
        (_, None, None) if listed.remembered.is_some() => actions.child(
            button(
                "Install",
                Message::ReinstallLanguageExtension(listed.remembered.unwrap_or_default()),
            )
            .filled(),
        ),
        (_, None, Some(offered)) if supported => {
            actions.child(button("Install", Message::InstallLanguageExtension(offered)).filled())
        }
        _ => actions.child(
            text("Unavailable on this platform")
                .text_sm()
                .color(theme.colors.text_muted),
        ),
    };
    v_flex()
        .w_full()
        .p(3)
        .gap(2)
        .child(
            h_flex()
                .w_full()
                .gap(2)
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .flex_1()
                        .gap(2)
                        .items_center()
                        .overflow_hidden()
                        .child(text(entry.name.clone()).font_semibold())
                        .child(
                            text(format!("v{}", entry.version))
                                .text_xs()
                                .color(theme.colors.text_muted),
                        ),
                )
                .child(actions),
        )
        .child(text(clipped(&entry.description)).text_sm())
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    text(entry.publisher.clone())
                        .text_sm()
                        .color(theme.colors.text_muted),
                )
                .when(listed.remembered.is_none(), |line| {
                    line.child(button("Repository", source_message(listed)).ghost())
                }),
        )
}

/// The message that opens the source of `listed`, wherever it was found.
fn source_message(listed: &Listed) -> Message {
    match (listed.installed, listed.offered) {
        (Some(place), _) => Message::OpenLanguageSource(place, true),
        (None, Some(place)) => Message::OpenLanguageSource(place, false),
        (None, None) => Message::RefreshLanguageCatalogue,
    }
}
