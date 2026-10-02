//! Language discovery and the editable server configuration for each language.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use pm_text::{Language, Server};
use pm_ui::{
    Div, Font, Paragraph, Styled, TextSize, Theme, button, h_flex, paragraph, text, v_flex,
};

use crate::config::{ServerList, extensions::Entry};
use crate::input::{Input, input_view};
use crate::message::Message;

/// A background catalogue read or completed extension mutation.
pub enum ResultMessage {
    /// Available packages or a retrieval error.
    Catalogue(Result<Vec<Entry>, String>),
    /// A completed install, import or removal.
    Changed(Result<(), String>),
}

/// A server being added or edited for one language.
pub struct ServerEditor {
    /// Canonical language name.
    pub language: String,
    /// Existing server position, or a new entry.
    pub index: Option<usize>,
    /// Executable, JSON arguments and JSON initialization options.
    pub fields: [Input; 3],
    /// Original installation recipe, retained when its command is unchanged.
    pub original: Option<Server>,
}

/// Transient state for the Languages page.
pub struct Languages {
    /// Local search across installed and available support.
    pub search: Input,
    /// Catalogue entries from the last successful retrieval.
    pub catalogue: Vec<Entry>,
    /// Current server form.
    pub editor: Option<ServerEditor>,
    /// Pending background operation.
    pub busy: bool,
    /// Whether the initial catalogue retrieval was attempted.
    pub requested: bool,
    /// Last operation error, displayed beside retry controls.
    pub error: Option<String>,
    /// Background results, drained on the window thread.
    pub results: Arc<Mutex<Vec<ResultMessage>>>,
}

impl Default for Languages {
    /// Creates an empty catalogue and search field.
    fn default() -> Self {
        Self {
            search: Input::one_line("Search languages"),
            catalogue: Vec::new(),
            editor: None,
            busy: false,
            requested: false,
            error: None,
            results: Arc::default(),
        }
    }
}

/// The data needed to render the Languages page.
pub struct LanguagesPage<'a> {
    /// Search, catalogue and active form.
    pub state: &'a Languages,
    /// Configured server overrides.
    pub servers: &'a BTreeMap<String, ServerList>,
    /// Focused text input; zero is the search and one through three are form fields.
    pub focus: Option<usize>,
    /// Visible caret phase.
    pub solid: bool,
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

/// Builds the language list, catalogue and active server form.
pub fn language_page(theme: &Theme, page: &LanguagesPage<'_>) -> Div<Message> {
    let query = page.state.search.value().to_lowercase();
    let installed = crate::config::extensions::installed();
    let mut view = v_flex()
        .w_full()
        .gap(3)
        .child(detail(
            theme,
            "Install language support and choose the servers that analyse your files.",
        ))
        .child(input_view(
            theme,
            &page.state.search,
            page.focus == Some(0),
            page.solid,
            1.0,
            |phase, anchor, head| Message::WriteLanguageField(0, phase, anchor, head),
            Message::ShowInputMenu,
        ))
        .child(
            h_flex()
                .gap(2)
                .child(button("Refresh Catalogue", Message::RefreshLanguageCatalogue).outlined())
                .child(
                    button("Import Local Extension", Message::ImportLanguageExtension).outlined(),
                ),
        )
        .when(page.state.busy, |view| {
            view.child(text("Working…").color(theme.colors.text_muted))
        })
        .when(page.state.error.is_some(), |view| {
            view.child(
                text(page.state.error.clone().unwrap_or_default()).color(theme.colors.text_muted),
            )
        })
        .child(text("Installed Languages").text_lg().font_semibold());
    for (index, language) in Language::all().into_iter().enumerate() {
        if !language.name().to_lowercase().contains(&query)
            && !language
                .extensions()
                .iter()
                .any(|extension| extension.contains(&query))
        {
            continue;
        }
        let configured = servers(language, page.servers);
        let associations = language.associations();
        let mut row = v_flex()
            .w_full()
            .gap(1.5)
            .p(3)
            .border_1(theme.colors.border_variant)
            .rounded(theme.radius.md)
            .child(
                h_flex()
                    .gap(2)
                    .items_center()
                    .child(text(language.name()).font_semibold())
                    .child(
                        text(if language.is_wasm() {
                            "Extension"
                        } else {
                            "Built-in"
                        })
                        .text_xs()
                        .color(theme.colors.text_muted),
                    ),
            )
            .child(detail(theme, associations.join(", ")));
        for (at, server) in configured.iter().enumerate() {
            row = row.child(
                v_flex()
                    .w_full()
                    .gap(1)
                    .child(detail(
                        theme,
                        format!("{} {}", server.command, server.arguments.join(" ")),
                    ))
                    .child(
                        h_flex()
                            .gap(1)
                            .child(
                                button("Edit", Message::EditLanguageServer(index, at)).outlined(),
                            )
                            .child(
                                button("Remove", Message::RemoveLanguageServer(index, at))
                                    .outlined(),
                            )
                            .when(server.install.is_some(), |row| {
                                row.child(
                                    button(
                                        "Install",
                                        Message::InstallLanguageServer(server.command),
                                    )
                                    .outlined(),
                                )
                            }),
                    ),
            );
        }
        row = row.child(
            h_flex()
                .gap(2)
                .child(button("Add Server", Message::AddLanguageServer(index)).outlined())
                .child(button("Restore Defaults", Message::ResetLanguageServers(index)).outlined()),
        );
        if page
            .state
            .editor
            .as_ref()
            .is_some_and(|editor| editor.language == language.name())
            && let Some(form) = server_form(theme, page)
        {
            row = row.child(form);
        }
        view = view.child(row);
    }
    view = view.child(text("Installed Extensions").text_lg().font_semibold());
    for (index, entry) in installed.iter().enumerate() {
        if !matches(entry, &query) {
            continue;
        }
        view = view.child(
            extension_card(theme, entry, Message::OpenLanguageSource(index, true)).child(
                h_flex()
                    .gap(2)
                    .child(button("Remove", Message::RemoveLanguageExtension(index)).outlined())
                    .when(
                        page.state.catalogue.iter().any(|available| {
                            available.id == entry.id
                                && semver::Version::parse(&available.version).ok()
                                    > semver::Version::parse(&entry.version).ok()
                        }),
                        |row| {
                            let at = page
                                .state
                                .catalogue
                                .iter()
                                .position(|available| available.id == entry.id)
                                .unwrap_or_default();
                            row.child(
                                button("Update", Message::InstallLanguageExtension(at)).filled(),
                            )
                        },
                    ),
            ),
        );
    }
    view = view.child(text("Available Languages").text_lg().font_semibold());
    for (index, entry) in page.state.catalogue.iter().enumerate() {
        if !matches(entry, &query) {
            continue;
        }
        let current = installed.iter().find(|installed| installed.id == entry.id);
        let supported =
            entry.platforms.is_empty() || entry.platforms.contains(&pm_text::install::platform());
        view = view.child(
            extension_card(theme, entry, Message::OpenLanguageSource(index, false))
                .child(detail(
                    theme,
                    format!(
                        "Platforms: {} · Requires: {}",
                        entry.platforms.join(", "),
                        if entry.prerequisites.is_empty() {
                            "No external tools".into()
                        } else {
                            entry.prerequisites.join(", ")
                        }
                    ),
                ))
                .when(!supported, |card| {
                    card.child(text("No server build for this platform").text_sm())
                })
                .when(
                    supported
                        && current.is_none_or(|installed| {
                            semver::Version::parse(&installed.version).ok()
                                < semver::Version::parse(&entry.version).ok()
                        }),
                    |card| {
                        card.child(
                            h_flex().child(
                                button(
                                    if current.is_some() {
                                        "Update"
                                    } else {
                                        "Install"
                                    },
                                    Message::InstallLanguageExtension(index),
                                )
                                .filled(),
                            ),
                        )
                    },
                ),
        );
    }
    view
}

/// Matches a catalogue search against package metadata.
fn matches(entry: &Entry, query: &str) -> bool {
    format!("{} {} {}", entry.name, entry.description, entry.publisher)
        .to_lowercase()
        .contains(query)
}

/// Displays package identity and source before offering an installation.
fn extension_card(theme: &Theme, entry: &Entry, source: Message) -> Div<Message> {
    v_flex()
        .w_full()
        .gap(1.5)
        .p(3)
        .border_1(theme.colors.border_variant)
        .rounded(theme.radius.md)
        .child(
            text(format!(
                "{} {} · {}",
                entry.name, entry.version, entry.publisher
            ))
            .font_semibold(),
        )
        .child(detail(theme, entry.description.clone()))
        .child(detail(theme, entry.source.clone()))
        .child(h_flex().child(button("View Source", source).ghost()))
}

/// Draws the server form inside its owning language card.
fn server_form(theme: &Theme, page: &LanguagesPage<'_>) -> Option<Div<Message>> {
    let editor = page.state.editor.as_ref()?;
    let mut form = v_flex()
        .w_full()
        .gap(2)
        .p(3)
        .border_1(theme.colors.border_variant)
        .child(text(format!("Server for {}", editor.language)).font_semibold());
    for (index, label) in [
        "Executable",
        "Arguments (JSON array)",
        "Initialization options (JSON object)",
    ]
    .into_iter()
    .enumerate()
    {
        form = form.child(text(label).text_sm()).child(input_view(
            theme,
            &editor.fields[index],
            page.focus == Some(index + 1),
            page.solid,
            1.0,
            move |phase, anchor, head| Message::WriteLanguageField(index + 1, phase, anchor, head),
            Message::ShowInputMenu,
        ));
    }
    Some(
        form.child(
            h_flex()
                .gap(2)
                .child(button("Save Server", Message::SaveLanguageServer).filled())
                .child(button("Cancel", Message::CancelLanguageServer).outlined()),
        ),
    )
}

/// Wraps language metadata and executable names to the space the pane provides.
fn detail(theme: &Theme, content: impl Into<String>) -> Paragraph {
    paragraph().w_full().break_long_words().span(
        content,
        Font::new(TextSize::Sm),
        theme.colors.text_muted,
    )
}
