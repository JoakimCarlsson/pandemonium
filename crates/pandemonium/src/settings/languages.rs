//! Language discovery and the editable server configuration for each language.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use pm_text::{Language, Server};
use pm_ui::{
    Div, Font, IconName, Paragraph, Styled, TextSize, Theme, button, h_flex, icon_button,
    paragraph, text, v_flex,
};

use crate::config::{ServerList, extensions::Entry};
use crate::input::{Input, input_view};
use crate::message::Message;
use crate::settings::parts::{badge, card, clipped, header, note};

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
    /// Whether installed language support is expanded.
    pub installed_open: bool,
    /// Whether the catalogue is expanded.
    pub available_open: bool,
    /// Language whose server controls are expanded.
    pub expanded: Option<String>,
    /// Extension whose metadata is expanded.
    pub package: Option<String>,
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
            installed_open: true,
            available_open: true,
            expanded: None,
            package: None,
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

/// Builds the Languages page from the same sections and cards as MCP settings.
pub fn language_page(theme: &Theme, page: &LanguagesPage<'_>) -> Div<Message> {
    let query = page.state.search.value().trim().to_lowercase();
    let installed = crate::config::extensions::installed();
    v_flex().w_full().gap(4)
        .child(detail(theme, "Language support provides syntax highlighting, diagnostics and navigation. Manage installed languages or find more in the catalogue."))
        .child(input_view(
            theme, &page.state.search, page.focus == Some(0), page.solid, 1.0,
            |phase, anchor, head| Message::WriteLanguageField(0, phase, anchor, head),
            Message::ShowInputMenu,
        ).on_click(Message::FocusLanguageField(0)))
        .child(installed_section(theme, page, &query, &installed))
        .child(available_section(theme, page, &query, &installed))
}

/// Shows installed languages in one card, with configuration opened inside its row.
fn installed_section(
    theme: &Theme,
    page: &LanguagesPage<'_>,
    query: &str,
    installed: &[Entry],
) -> Div<Message> {
    let all = Language::all();
    let mut languages = all
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, language)| {
            language.name().to_lowercase().contains(query)
                || language
                    .associations()
                    .iter()
                    .any(|association| association.to_lowercase().contains(query))
        })
        .collect::<Vec<_>>();
    languages.sort_by_key(|(_, language)| language.name().to_lowercase());
    let mut rows = languages
        .iter()
        .map(|(index, language)| language_row(theme, page, *language, *index, installed))
        .collect::<Vec<_>>();
    for (index, entry) in installed.iter().enumerate() {
        if !all.iter().any(|language| language.name() == entry.name) && matches(entry, query) {
            rows.push(package_row(theme, page, entry, index, true, installed));
        }
    }
    let count = rows.len();
    if rows.is_empty() {
        rows.push(note(theme, "No installed languages match."));
    }
    let open = page.state.installed_open || page.state.editor.is_some();
    v_flex()
        .w_full()
        .gap(1.5)
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(header(
                    theme,
                    "Installed",
                    count,
                    open,
                    Message::ToggleLanguagesInstalled,
                ))
                .child(button("Import Extension", Message::ImportLanguageExtension).outlined()),
        )
        .when(open, |section| section.child(card(theme, rows)))
}

/// Shows catalogue results and retrieval status beneath the installed collection.
fn available_section(
    theme: &Theme,
    page: &LanguagesPage<'_>,
    query: &str,
    installed: &[Entry],
) -> Div<Message> {
    let offered = page
        .state
        .catalogue
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            matches(entry, query)
                && installed
                    .iter()
                    .find(|current| current.id == entry.id)
                    .is_none_or(|current| {
                        semver::Version::parse(&current.version).ok()
                            < semver::Version::parse(&entry.version).ok()
                    })
        })
        .collect::<Vec<_>>();
    let rows = match (&page.state.error, page.state.busy, offered.is_empty()) {
        (_, true, true) => vec![note(theme, "Loading language support…")],
        (Some(error), _, true) => vec![note(
            theme,
            &format!("The language catalogue is unavailable: {error}"),
        )],
        (None, false, true) => vec![note(theme, "No available language extensions match.")],
        _ => offered
            .iter()
            .map(|(index, entry)| package_row(theme, page, entry, *index, false, installed))
            .collect(),
    };
    v_flex().w_full().gap(1.5)
        .child(h_flex().w_full().items_center().justify_between()
            .child(header(theme, "Available", offered.len(), page.state.available_open, Message::ToggleLanguagesAvailable))
            .child(button("Refresh", Message::RefreshLanguageCatalogue).ghost()))
        .when(page.state.available_open, |section| {
            section.child(detail(theme, "Language extensions from the catalogue. Search by language name or file extension."))
                .child(card(theme, rows))
        })
}

/// Shows one language and expands its configuration only when requested.
fn language_row(
    theme: &Theme,
    page: &LanguagesPage<'_>,
    language: Language,
    index: usize,
    installed: &[Entry],
) -> Div<Message> {
    let configured = servers(language, page.servers);
    let expanded = page.state.expanded.as_deref() == Some(language.name());
    let mut body = v_flex().w_full().child(
        h_flex()
            .w_full()
            .px(3)
            .py(2.5)
            .gap(4)
            .items_center()
            .justify_between()
            .child(
                v_flex()
                    .flex_1()
                    .gap(0.5)
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .gap(1.5)
                            .items_center()
                            .child(text(language.name()).font_medium())
                            .child(badge(
                                theme,
                                if language.is_wasm() {
                                    "Extension"
                                } else {
                                    "Built-in"
                                },
                                theme.colors.text_muted,
                            )),
                    )
                    .child(
                        text(clipped(&language.associations().join(", ")))
                            .text_xs()
                            .font_mono()
                            .color(theme.colors.text_subtle),
                    ),
            )
            .child(
                icon_button(theme, IconName::More, Message::ShowLanguageMenu(index))
                    .tooltip("More actions"),
            ),
    );
    if !expanded {
        return body;
    }
    let mut row = v_flex()
        .w_full()
        .px(3)
        .pb(3)
        .gap(3)
        .child(text("Language servers").text_sm().font_semibold());
    if configured.is_empty() {
        row = row.child(detail(theme, "No language server configured."));
    }
    for (at, server) in configured.iter().enumerate() {
        let available = pm_text::program::installed_with_recipe(server.command, server.install)
            .or_else(|| pm_text::program::managed_fallback(server.command))
            .is_some();
        row = row.child(
            v_flex()
                .w_full()
                .gap(1)
                .child(detail(
                    theme,
                    format!(
                        "{} · {}",
                        server.command,
                        if available {
                            "Available"
                        } else {
                            "Not installed"
                        }
                    ),
                ))
                .child(
                    h_flex()
                        .gap(1)
                        .items_center()
                        .when(!available && server.install.is_some(), |controls| {
                            controls.child(
                                button("Install", Message::InstallLanguageServer(server.command))
                                    .filled(),
                            )
                        })
                        .child(button("Edit", Message::EditLanguageServer(index, at)).ghost())
                        .child(button("Remove", Message::RemoveLanguageServer(index, at)).ghost()),
                ),
        );
    }
    row = row.child(
        v_flex()
            .gap(1)
            .child(
                h_flex().child(button("Add server", Message::AddLanguageServer(index)).outlined()),
            )
            .when(page.servers.contains_key(language.name()), |controls| {
                controls.child(h_flex().child(
                    button("Restore defaults", Message::ResetLanguageServers(index)).ghost(),
                ))
            }),
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
    if let Some((at, entry)) = installed
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.name == language.name())
    {
        row = row.child(package_row(theme, page, entry, at, true, installed));
    }
    if let Some(error) = page
        .state
        .error
        .as_ref()
        .filter(|_| page.state.editor.is_some())
    {
        row = row.child(detail(theme, error.clone()));
    }
    body = body.child(row);
    body
}

/// Offers one clear installation action and optional package metadata.
fn package_row(
    theme: &Theme,
    page: &LanguagesPage<'_>,
    entry: &Entry,
    index: usize,
    installed_view: bool,
    installed: &[Entry],
) -> Div<Message> {
    let current = installed.iter().find(|current| current.id == entry.id);
    let update = page
        .state
        .catalogue
        .iter()
        .enumerate()
        .find(|(_, available)| {
            available.id == entry.id
                && current.is_some_and(|current| {
                    semver::Version::parse(&available.version).ok()
                        > semver::Version::parse(&current.version).ok()
                })
        })
        .map(|(index, _)| index);
    let supported =
        entry.platforms.is_empty() || entry.platforms.contains(&pm_text::install::platform());
    let expanded = page.state.package.as_deref() == Some(entry.id.as_str());
    let mut controls = h_flex().gap(2).items_center();
    if page.state.busy {
        controls = controls.child(text("Working…").text_sm().color(theme.colors.text_muted));
    } else if let Some(index) = update {
        controls =
            controls.child(button("Update", Message::InstallLanguageExtension(index)).filled());
    } else if current.is_some() {
        controls = controls.child(text("Installed").text_sm().color(theme.colors.text_muted));
    } else if supported {
        controls =
            controls.child(button("Install", Message::InstallLanguageExtension(index)).filled());
    } else {
        controls = controls.child(
            text("Unavailable on this platform")
                .text_sm()
                .color(theme.colors.text_muted),
        );
    }
    controls = controls.child(
        button(
            if expanded { "Less" } else { "Details" },
            Message::ToggleLanguagePackage(index, installed_view),
        )
        .ghost(),
    );
    let mut row = v_flex().w_full().child(crate::settings::parts::row(
        theme,
        &entry.name,
        &clipped(&entry.description),
        controls,
    ));
    if expanded {
        let mut details = v_flex()
            .w_full()
            .px(3)
            .pb(3)
            .gap(1.5)
            .child(detail(
                theme,
                format!("Version {} · {}", entry.version, entry.publisher),
            ))
            .child(detail(
                theme,
                format!(
                    "Requires: {}",
                    if entry.prerequisites.is_empty() {
                        "No external tools".into()
                    } else {
                        entry.prerequisites.join(", ")
                    }
                ),
            ))
            .child(detail(
                theme,
                format!(
                    "Platforms: {}",
                    if entry.platforms.is_empty() {
                        "All".into()
                    } else {
                        entry.platforms.join(", ")
                    }
                ),
            ))
            .child(
                h_flex().child(
                    button(
                        "View source",
                        Message::OpenLanguageSource(index, installed_view),
                    )
                    .ghost(),
                ),
            );
        if installed_view && !page.state.busy {
            details = details.child(
                h_flex().child(
                    button(
                        "Uninstall extension",
                        Message::RemoveLanguageExtension(index),
                    )
                    .outlined(),
                ),
            );
        }
        row = row.child(details);
    }
    row
}

/// Matches a catalogue search against package metadata.
fn matches(entry: &Entry, query: &str) -> bool {
    format!("{} {} {}", entry.name, entry.description, entry.publisher)
        .to_lowercase()
        .contains(query)
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
        form = form.child(text(label).text_sm()).child(
            input_view(
                theme,
                &editor.fields[index],
                page.focus == Some(index + 1),
                page.solid,
                1.0,
                move |phase, anchor, head| {
                    Message::WriteLanguageField(index + 1, phase, anchor, head)
                },
                Message::ShowInputMenu,
            )
            .on_click(Message::FocusLanguageField(index + 1)),
        );
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
