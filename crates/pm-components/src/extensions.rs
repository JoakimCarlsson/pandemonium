//! The extensions page: what the server on this machine has installed, then what it can install, narrowed by a search, each a row saying what the extension gives the files of a project and whether it works, and the button that installs one from a folder.

use crate::areas::SettingsEvent;
use crate::button;
use crate::icons::artwork_with;
use crate::overview::scrolled_page;
use crate::parts;
use crate::target::Target;
use crate::theme::Frame;
use crate::theme::{self, family_from_colours, set_extension_families};
use crate::theme::{Radius, Role, sp};
use pm_protocol::{ExtensionAssets, ExtensionCommand, ExtensionInfo};
use rustle::Modifier;
use rustle::TextFieldState;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, TextOverflow};

use crate::Checkbox;
use crate::button::Variant;
use crate::checkbox::Variant as CheckboxVariant;
use crate::input::Input;
use crate::parts::{glyph, line};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};

/// How wide the page's column is at most, in logical pixels.
const PAGE_WIDTH: f32 = 860.0;

/// How long a line of what an extension brings may be in a row, in characters.
const ROW_LINE: usize = 80;

/// How long a line of what an extension brings may be on the card asking about it, in characters.
const CARD_LINE: usize = 100;

/// How long a reason a row has room for, in characters.
const PROBLEM_LENGTH: usize = 96;

/// An extension the reader chose to install that the page has yet to be told to go ahead with: what it carries and which of what it asks to be allowed stays switched on.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    /// The folder it is installed from.
    pub path: String,
    /// What the server says it is.
    pub extension: ExtensionInfo,
    /// What it will be allowed, by the names of capabilities.
    pub granted: Vec<String>,
}

/// What the extensions page remembers: the search, how far it is scrolled and the extension waiting to be allowed or dropped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtensionsPage {
    /// What is searched for.
    pub search: TextFieldState,
    /// How far down the list is scrolled, in logical pixels.
    pub scroll: rustle::ScrollState,
    /// The extension waiting to be installed, when one is.
    pub preview: Option<Preview>,
    /// Why the last folder chosen could not be installed.
    pub problem: Option<String>,
}

/// Whether `id` names a button of the extensions page's rows or of the card asking about an extension.
pub fn is_extensions(id: Target) -> bool {
    matches!(
        id,
        Target::Settings(SettingsEvent::ExtensionRow(_))
            | Target::Settings(SettingsEvent::ExtensionGrant { .. })
            | Target::Settings(SettingsEvent::PreviewInstall)
            | Target::Settings(SettingsEvent::PreviewCancel)
            | Target::Settings(SettingsEvent::PreviewGrant(_))
    )
}

/// What the capability called `id` lets an extension do, in words.
fn describe_capability(id: &str) -> String {
    pm_protocol::CAPABILITIES
        .iter()
        .find(|(held, _)| *held == id)
        .map_or_else(|| id.to_owned(), |(_, said)| (*said).to_owned())
}

/// The extensions the reader picks from: every one but those installed only as part of another.
pub fn pickable(extensions: &[ExtensionInfo]) -> impl Iterator<Item = &ExtensionInfo> {
    extensions.iter().filter(|extension| !extension.hidden)
}

/// `words` cut to `most` characters, with an ellipsis where it was cut.
fn shortened(words: &str, most: usize) -> String {
    if words.chars().count() <= most {
        return words.to_owned();
    }
    let kept: String = words.chars().take(most - 1).collect();
    format!("{}…", kept.trim_end())
}

/// `items` joined with ` · ` and broken into lines of at most `most` characters, each item whole.
fn wrapped(items: &[String], most: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for item in items {
        match lines.last_mut() {
            Some(last) if last.chars().count() + 3 + item.chars().count() <= most => {
                last.push_str(" · ");
                last.push_str(item);
            }
            _ => lines.push(item.clone()),
        }
    }
    lines
}

/// What an extension is doing or has, in words, and the colour they are drawn in; nothing for one that is only available.
pub fn standing(extension: &ExtensionInfo) -> (String, Role) {
    if let Some(busy) = &extension.busy {
        return (format!("{busy}…"), Role::Muted);
    }
    if let Some(problem) = &extension.problem {
        return (shortened(problem, PROBLEM_LENGTH), Role::Danger);
    }
    if extension.kind == "extension" {
        return (String::new(), Role::Subtle);
    }
    if let Some(update) = &extension.update {
        return (update.clone(), Role::Accent);
    }
    match (&extension.server, extension.installed) {
        (Some(server), true) => (server.clone(), Role::Muted),
        (None, true) => ("No language server".to_owned(), Role::Subtle),
        (_, false) => (String::new(), Role::Subtle),
    }
}

/// The endings of the file names a language is written in, as `.rs .py`.
pub fn endings(extension: &ExtensionInfo) -> String {
    extension
        .suffixes
        .iter()
        .take(4)
        .map(|suffix| format!(".{suffix}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl WorkspaceState {
    /// The state after the server on this machine said which extensions there are.
    pub fn with_extensions(mut self, extensions: Vec<ExtensionInfo>) -> Self {
        crate::file_icon::learn_languages(&None, &extensions);
        self.extensions = extensions;
        self
    }

    /// The state after the server of `host` said what its extensions bring that the window uses itself: its themes join those that can be chosen, its icon themes draw the files of every project, and its commands are offered in the palette and behind the slash for the prompts of that machine.
    pub fn with_extension_assets(mut self, host: &Option<String>, assets: ExtensionAssets) -> Self {
        let families = assets
            .themes
            .iter()
            .map(|theme| family_from_colours(&theme.name, &theme.dark, &theme.light))
            .collect();
        set_extension_families(host.as_deref().unwrap_or_default(), families);
        crate::file_icon::learn_icon_themes(host, &assets.icon_themes);
        self.extension_assets.insert(host.clone(), assets);
        self
    }

    /// The commands extensions bring for the machine the prompt is for, each with that machine.
    pub(crate) fn extension_commands(&self) -> Vec<(Option<String>, ExtensionCommand)> {
        let host = self.prompt_machine();
        self.extension_assets
            .get(&host)
            .into_iter()
            .flat_map(|assets| &assets.commands)
            .map(|command| (host.clone(), command.clone()))
            .collect()
    }

    /// The state after the command `command`, which extensions of `host` bring, was taken: the server is asked to run it in the folder the prompt is for.
    pub fn extension_command_run(
        mut self,
        host: Option<String>,
        command: ExtensionCommand,
    ) -> Self {
        self.run_extension_command(host, command);
        self
    }

    /// Asks the server of `host` to run `command` in the folder the prompt is for.
    pub(crate) fn run_extension_command(
        &mut self,
        host: Option<String>,
        command: ExtensionCommand,
    ) {
        let directory = self.prompt_place().map(|(_, directory)| directory);
        let session = self.review_index();
        self.outbox.push(Outgoing::RunExtensionCommand {
            host,
            extension: command.extension,
            command: command.id,
            argument: String::new(),
            directory,
            session,
        });
    }

    /// The state after a command of an extension came to `prompt`, which is put in the prompt box, ready to be read, changed and sent.
    pub fn with_command_result(mut self, prompt: Option<String>) -> Self {
        if let Some(text) = prompt {
            self.prompt = crate::state::prompt_field(&text);
        }
        self
    }

    /// The extensions the page lists, as the search narrows them: those installed first, then those available, each by name.
    pub(crate) fn listed_extensions(&self) -> Vec<&ExtensionInfo> {
        let needle = self.extensions_page.search.text().trim().to_lowercase();
        let mut listed: Vec<&ExtensionInfo> = pickable(&self.extensions)
            .filter(|extension| {
                needle.is_empty()
                    || extension.name.to_lowercase().contains(&needle)
                    || extension.description.to_lowercase().contains(&needle)
                    || extension
                        .suffixes
                        .iter()
                        .any(|suffix| suffix.contains(&needle))
            })
            .collect();
        listed.sort_by_key(|extension| (!extension.installed, extension.name.to_lowercase()));
        listed
    }

    /// The state with the extensions page showing, asking the server on this machine for the extensions there are.
    pub(crate) fn with_extensions_page(mut self) -> Self {
        self.page = crate::state::Page::Extensions;
        self.outbox.push(Outgoing::Extensions);
        self
    }

    /// Takes in what the reader typed in the search box: the list goes back to its top.
    pub(crate) fn extensions_typed(&mut self) {
        self.extensions_page.scroll.jump_to(0.0);
    }

    /// The state after the reader chose the folder at `path` to install as an extension on the server on this machine: the server is asked what it carries first.
    pub fn extension_picked(mut self, path: String) -> Self {
        self.extensions_page.problem = None;
        self.outbox.push(Outgoing::InspectExtension { path });
        self
    }

    /// The state after the server said what the extension in the folder `path` is: one that asks for nothing is installed at once, and one that asks for something is shown with a switch for each thing, all on, for the reader to go ahead with.
    pub fn with_extension_preview(mut self, path: String, extension: ExtensionInfo) -> Self {
        if extension.requests.is_empty() {
            self.outbox.push(Outgoing::InstallExtension {
                path,
                grants: Vec::new(),
            });
            return self;
        }
        self.extensions_page.preview = Some(Preview {
            path,
            granted: extension.requests.clone(),
            extension,
        });
        self
    }

    /// The state after the server said why the folder chosen cannot be installed.
    pub fn with_extension_problem(mut self, problem: String) -> Self {
        self.extensions_page.problem = Some(problem);
        self
    }

    /// The state after a button of the card asking about an extension, or a switch of a row, was pressed.
    fn extension_card_pressed(&mut self, action: Target) {
        if action == Target::Settings(SettingsEvent::PreviewCancel) {
            self.extensions_page.preview = None;
        } else if action == Target::Settings(SettingsEvent::PreviewInstall) {
            if let Some(preview) = self.extensions_page.preview.take() {
                self.outbox.push(Outgoing::InstallExtension {
                    path: preview.path,
                    grants: preview.granted,
                });
            }
        } else if let Some(preview) = self.extensions_page.preview.as_mut()
            && let Target::Settings(SettingsEvent::PreviewGrant(place)) = action
            && let Some(asked) = preview.extension.requests.get(place).cloned()
        {
            if preview.granted.contains(&asked) {
                preview.granted.retain(|held| *held != asked);
            } else {
                preview.granted.push(asked);
            }
        }
    }

    /// The state after the switch of the `which`-th capability on the `place`-th row was pressed: the capability is allowed, or refused if it was allowed.
    fn grant_pressed(&mut self, place: usize, which: usize) {
        let Some(extension) = self.listed_extensions().get(place).copied().cloned() else {
            return;
        };
        let Some(capability) = extension.requests.get(which).cloned() else {
            return;
        };
        let granted = !extension.granted.contains(&capability);
        if let Some(held) = self
            .extensions
            .iter_mut()
            .find(|held| held.id == extension.id)
        {
            held.granted.retain(|name| *name != capability);
            if granted {
                held.granted.push(capability.clone());
            }
        }
        self.outbox.push(Outgoing::GrantExtension {
            id: extension.id,
            capability,
            granted,
        });
    }

    /// The state after the button of the `place`-th row, as `action` names it, was pressed: it installs the extension, or uninstalls it when it is installed.
    pub(crate) fn extensions_pressed(mut self, action: Target) -> Self {
        match action {
            Target::Settings(SettingsEvent::InstallExtension) => {
                self.open_folder_picker(crate::folder_picker::Purpose::Extension, None)
            }
            Target::Settings(SettingsEvent::ExtensionGrant { row, request }) => {
                self.grant_pressed(row, request)
            }
            Target::Settings(SettingsEvent::ExtensionRow(place)) => {
                if let Some(extension) = self.listed_extensions().get(place).copied().cloned() {
                    self.install_or_remove(&extension);
                }
            }
            _ => self.extension_card_pressed(action),
        }
        self
    }

    /// Installs `extension`, or uninstalls it when it is installed, unless it is busy being installed.
    pub(crate) fn install_or_remove(&mut self, extension: &ExtensionInfo) {
        if extension.busy.is_some() {
            return;
        }
        let id = extension.id.clone();
        self.outbox.push(match extension.installed {
            true if extension.update.is_some() => Outgoing::InstallExtensions { ids: vec![id] },
            true if extension.kind == "extension" => Outgoing::RemovePackage { id },
            true => Outgoing::RemoveExtension { id },
            false => Outgoing::InstallExtensions { ids: vec![id] },
        });
    }
}

/// Emits the title of the page and the button that installs an extension from a folder.
#[track_caller]
fn heading() {
    parts::bar(12, 5, 0, || {
        rustle::basic_text(
            Modifier::new().weight(1.0),
            "Extensions",
            theme::strong(14, Role::Text),
            TextOverflow::Ellipsis,
        );
        button::action(
            "Install Extension…",
            Target::Settings(SettingsEvent::InstallExtension),
            Variant::Ghost,
        );
    });
}

/// Emits the box the reader types into to narrow the list.
#[track_caller]
fn search(state: &WorkspaceState) {
    Input::search(
        &state.extensions_page.search,
        Target::Settings(SettingsEvent::ExtensionsSearch),
    )
    .size(Size::Lg)
    .placeholder("Search extensions")
    .build();
}

/// Emits the icon an extension's files are shown with, once it is installed and has one; a plain code file otherwise.
#[track_caller]
fn mark(extension: &ExtensionInfo) {
    let artwork = extension
        .icon
        .as_ref()
        .filter(|_| extension.installed)
        .and_then(|source| rustle::SvgPainter::from_str(source).ok());
    match artwork {
        Some(art) => {
            let (_, tone) = crate::file_icon::file_kind(&format!(
                "file.{}",
                extension.suffixes.first().cloned().unwrap_or_default()
            ));
            artwork_with(Modifier::new(), &art, sp(5.0), tone);
        }
        None if extension.kind == "extension" => glyph(phosphor::PLUG, 5.0, Role::Subtle),
        None => glyph(phosphor::CODE, 5.0, Role::Subtle),
    }
}

/// Emits the row of `extension`: its icon, then its name and version over what it is for and how it stands, and the button, answering to `action`, that installs or uninstalls it.
#[track_caller]
pub(crate) fn row(extension: &ExtensionInfo, action: Target) {
    let button = match (extension.busy.is_some(), extension.installed) {
        (true, _) => button::small_button("Installing", action, Variant::Ghost).disabled(true),
        (false, true) if extension.update.is_some() => {
            button::small_button("Update", action, Variant::Outline)
        }
        (false, true) => button::small_button("Uninstall", action, Variant::Ghost),
        (false, false) => button::small_button("Install", action, Variant::Outline),
    };
    let (said, tone) = standing(extension);
    let about = if extension.description.is_empty() {
        endings(extension)
    } else {
        format!("{}  {}", extension.description, endings(extension))
    };
    let brings = wrapped(&extension.provides, ROW_LINE);
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(theme::sp(4), theme::sp(3)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Center,
        || {
            mark(extension);
            rustle::column_arranged(
                Modifier::new().weight(1.0).clip(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    rustle::row_arranged(
                        Modifier::new(),
                        Arrangement::SpacedBy(theme::sp(2)),
                        Alignment::Center,
                        || {
                            rustle::basic_text(
                                Modifier::new(),
                                &extension.name,
                                theme::strong(14, Role::Text),
                                TextOverflow::Ellipsis,
                            );
                            if !extension.version.is_empty() {
                                rustle::basic_text(
                                    Modifier::new(),
                                    &extension.version,
                                    theme::code(12, Role::Subtle),
                                    TextOverflow::Ellipsis,
                                );
                            }
                            if extension.local {
                                line("your own", 12, 1.4, Role::Subtle);
                            }
                        },
                    );
                    line(about, 12, 1.4, Role::Muted);
                    for said in brings {
                        line(said, 12, 1.4, Role::Subtle);
                    }
                    if let Some(owner) = &extension.owner {
                        line(format!("From the {owner} extension"), 12, 1.4, Role::Subtle);
                    }
                    if !said.is_empty() {
                        line(said, 12, 1.4, tone);
                    }
                },
            );
            button.build();
        },
    );
}

/// Emits one line saying what a capability lets an extension do, with the switch, answering to `id`, that allows it or not.
#[track_caller]
fn permission(capability: &str, allowed: bool, id: Target) {
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(0.0, theme::sp(1)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Center,
        || {
            rustle::basic_text(
                Modifier::new().weight(1.0),
                describe_capability(capability),
                theme::line(12, Role::Muted),
                TextOverflow::Ellipsis,
            );
            Checkbox::new(allowed, id)
                .variant(CheckboxVariant::Switch)
                .build();
        },
    );
}

/// Emits the switches of what an installed extension package asked to be allowed, under its row; nothing for one that asked for nothing.
fn permissions(extension: &ExtensionInfo, place: usize) {
    if extension.kind != "extension" || !extension.installed || extension.requests.is_empty() {
        return;
    }
    rustle::column_arranged(
        Modifier::new().fill_max_width().padding_each(
            theme::sp(4),
            0.0,
            theme::sp(4),
            theme::sp(3),
        ),
        Arrangement::Start,
        Alignment::Start,
        || {
            line("Allowed to", 12, 1.4, Role::Subtle);
            for (which, asked) in extension.requests.iter().enumerate() {
                permission(
                    asked,
                    extension.granted.contains(asked),
                    Target::Settings(SettingsEvent::ExtensionGrant {
                        row: place,
                        request: which,
                    }),
                );
            }
        },
    );
}

/// Emits the card that shows what an extension carries and asks to be allowed, with a switch for each thing it asks and the buttons that go ahead or drop it.
fn preview_card(preview: &Preview) {
    let extension = &preview.extension;
    let title = format!("Install {} {}", extension.name, extension.version);
    let title = title.trim_end();
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .panel(Radius::Lg)
            .padding_xy(theme::sp(4), theme::sp(4)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Start,
        || {
            rustle::basic_text(
                Modifier::new(),
                title,
                theme::strong(14, Role::Text),
                TextOverflow::Ellipsis,
            );
            if !extension.description.is_empty() {
                line(extension.description.clone(), 12, 1.4, Role::Muted);
            }
            for said in wrapped(&extension.provides, CARD_LINE) {
                line(said, 12, 1.4, Role::Subtle);
            }
            line("It asks to be allowed to", 12, 1.4, Role::Subtle);
            for (which, asked) in extension.requests.iter().enumerate() {
                permission(
                    asked,
                    preview.granted.contains(asked),
                    Target::Settings(SettingsEvent::PreviewGrant(which)),
                );
            }
            button::confirm_row(
                "Install",
                Target::Settings(SettingsEvent::PreviewInstall),
                true,
                Target::Settings(SettingsEvent::PreviewCancel),
            );
        },
    );
}

/// Emits a titled group of the rows `rows` emits in one outlined box, with a
/// line under every row but the last that adds to the row's height, as a
/// bottom border does to a box sized by its content, or nothing when it emits none.
#[track_caller]
pub(crate) fn group(title: &str, rows: impl FnOnce()) {
    lined_group(title, false, rows);
}

/// Emits a titled group like [`group`] of rows of a fixed height, the line
/// under each drawn inside that height, as a bottom border is inside a box
/// of a set height.
#[track_caller]
pub(crate) fn group_of_fixed_rows(title: &str, rows: impl FnOnce()) {
    lined_group(title, true, rows);
}

/// Emits a titled group of the rows `rows` emits in one outlined box, the
/// line under every row but the last drawn `inside` its height or added to
/// it, or nothing when it emits none.
#[track_caller]
fn lined_group(title: &str, inside: bool, rows: impl FnOnce()) {
    let rows = rustle::collect(rows);
    if rows.is_empty() {
        return;
    }
    rustle::column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Start,
        move || {
            line(title.to_owned(), 12, 1.4, Role::Subtle);
            rustle::column_arranged(
                Modifier::new().fill_max_width().panel(Radius::Lg).clip(),
                Arrangement::Start,
                Alignment::Start,
                move || {
                    let last = rows.len() - 1;
                    let color = theme::color(Role::Border);
                    for (at, built) in rows.into_iter().enumerate() {
                        if at == last {
                            rustle::emit(built);
                            continue;
                        }
                        if inside {
                            rustle::column_arranged(
                                Modifier::new().fill_max_width().border_bottom(1.0, color),
                                Arrangement::Start,
                                Alignment::Start,
                                move || rustle::emit(built),
                            );
                        } else {
                            rustle::emit(built);
                            rustle::horizontal_divider(Modifier::new(), 1.0, color);
                        }
                    }
                },
            );
        },
    );
}

/// Emits the row of the extension listed at `place`, with the switches of what it is allowed under it.
#[track_caller]
fn entry(extension: &ExtensionInfo, place: usize) {
    rustle::column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        || {
            row(
                extension,
                Target::Settings(SettingsEvent::ExtensionRow(place)),
            );
            permissions(extension, place);
        },
    );
}

/// Emits the extensions page for `state`.
#[track_caller]
pub fn extensions(state: &WorkspaceState) {
    let listed = state.listed_extensions();
    let bounded = Modifier::new().width_in(0.0, PAGE_WIDTH).fill_max_width();
    scrolled_page(heading, &state.extensions_page.scroll, bounded, || {
        search(state);
        if let Some(problem) = &state.extensions_page.problem {
            line(problem.clone(), 12, 1.4, Role::Danger);
        }
        if let Some(preview) = &state.extensions_page.preview {
            preview_card(preview);
        }
        if state.extensions.is_empty() {
            line(
                "Asking the server which extensions there are…",
                13,
                1.4,
                Role::Subtle,
            );
        } else if listed.is_empty() {
            line("No extension matches.", 13, 1.4, Role::Subtle);
        } else {
            for (title, installed) in [("Installed on this machine", true), ("Available", false)] {
                rustle::key(installed, || {
                    group(title, || {
                        for (place, extension) in listed.iter().enumerate() {
                            if extension.installed == installed {
                                entry(extension, place);
                            }
                        }
                    });
                });
            }
        }
    });
}
