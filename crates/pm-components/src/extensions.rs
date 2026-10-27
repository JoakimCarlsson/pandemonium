//! The extensions page: what the server on this machine has installed, then what it can install, narrowed by a search, each a row saying what the extension gives the files of a project and whether it works, and the button that installs one from a folder.

use pm_icon::{IconName, Svg};
use pm_input::FocusId;
use pm_protocol::{ExtensionAssets, ExtensionCommand, ExtensionInfo};
use pm_style::{
    Radius, Role, bg, border, border_b, border_color, flex, flex_1, flex_col, flex_none,
    font_medium, font_mono, gap, h, items_center, justify_between, max_w_px, min_h, min_w_px,
    mx_auto, overflow_hidden, pb, pt, px, py, rounded, size, text_color, text_px, w_full,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::checkbox::Variant as CheckboxVariant;
use crate::input::Input;
use crate::parts::{glyph, line};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Button, Checkbox};

/// Opens the extensions page.
pub const EXTENSIONS: FocusId = FocusId(308);

/// The box the extensions are searched with.
pub const EXTENSIONS_SEARCH: FocusId = FocusId(9_000_000);

/// The scrolling list of extensions.
pub const EXTENSIONS_SCROLL: FocusId = FocusId(9_000_001);

/// Asks for a folder and installs the extension in it on the server on this machine.
pub const INSTALL_EXTENSION: FocusId = FocusId(9_000_002);

/// Installs the extension the page asks about, with what the reader left switched on.
pub const PREVIEW_INSTALL: FocusId = FocusId(9_000_003);

/// Drops the extension the page asks about.
pub const PREVIEW_CANCEL: FocusId = FocusId(9_000_004);

/// The first of the switches of what the extension the page asks about asks to be allowed, in the order it asks.
const PREVIEW_GRANT_BASE: u64 = 9_000_010;

/// One past the last of those switches.
const PREVIEW_GRANT_END: u64 = 9_000_020;

/// The first of the buttons installing or uninstalling an extension, in the order the page lists them; the rest follow it.
const ROW_BASE: u64 = 9_001_000;

/// The first of the buttons updating an extension, in the order the page lists those that are behind; the rest follow it.
const UPDATE_BASE: u64 = 9_030_000;

/// The first of the switches that allow or refuse an installed extension a capability: a block of them for each row, in the order the page lists them.
const GRANT_BASE: u64 = 9_050_000;

/// How many switches each row has room for.
const GRANTS_PER_ROW: u64 = 8;

/// One past the last name the page answers to.
const EXTENSIONS_END: u64 = 9_100_000;

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
    pub search: Field,
    /// How far down the list is scrolled, in logical pixels.
    pub scroll: f32,
    /// The extension waiting to be installed, when one is.
    pub preview: Option<Preview>,
    /// The updates the reader turned down in the bar over a file, each as the language and the version offered, until the window is closed.
    pub dismissed_updates: Vec<String>,
    /// Why the last folder chosen could not be installed.
    pub problem: Option<String>,
}

/// Whether `id` names a button of the extensions page's rows or of the card asking about an extension.
pub fn is_extensions(id: FocusId) -> bool {
    (ROW_BASE..EXTENSIONS_END).contains(&id.0)
        || (PREVIEW_INSTALL.0..PREVIEW_GRANT_END).contains(&id.0)
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
    if extension.installed && extension.update.is_some() {
        return ("A newer version is available.".to_owned(), Role::Warning);
    }
    if extension.kind == "extension" {
        return (String::new(), Role::Subtle);
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
        crate::preview::learn_previews(&extensions);
        crate::figures::learn_renders(&extensions);
        self.extensions = extensions;
        self
    }

    /// The state after the server of `host` said what its extensions bring that the window uses itself: its themes join those that can be chosen, its icon themes draw the files of every project, and its commands are offered in the palette and behind the slash for the prompts of that machine.
    pub fn with_extension_assets(mut self, host: &Option<String>, assets: ExtensionAssets) -> Self {
        let families = assets
            .themes
            .iter()
            .map(|theme| pm_theme::family_from_colours(&theme.name, &theme.dark, &theme.light))
            .collect();
        pm_theme::set_extension_families(host.as_deref().unwrap_or_default(), families);
        crate::file_icon::learn_icon_themes(host, &assets.icon_themes);
        self.extension_assets.insert(host.clone(), assets);
        self
    }

    /// The commands extensions bring for the machine the prompt is for, each with that machine.
    pub(crate) fn extension_commands(&self) -> Vec<(Option<String>, ExtensionCommand)> {
        let host = self.prompt_place().and_then(|(host, _)| host);
        self.extension_assets
            .get(&host)
            .map(|assets| {
                assets
                    .commands
                    .iter()
                    .map(|command| (host.clone(), command.clone()))
                    .collect()
            })
            .unwrap_or_default()
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

    /// The state after `edit` was made to the box the extensions are searched with.
    pub(crate) fn extensions_searched(&mut self, edit: Edit) {
        self.extensions_page.search.apply(edit);
        self.extensions_page.scroll = 0.0;
    }

    /// The state after the extensions page is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_extensions(mut self, delta: f32, room: f32) -> Self {
        self.extensions_page.scroll = (self.extensions_page.scroll + delta).clamp(0.0, room);
        self
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
    fn extension_card_pressed(&mut self, action: FocusId) {
        if action == PREVIEW_CANCEL {
            self.extensions_page.preview = None;
        } else if action == PREVIEW_INSTALL {
            if let Some(preview) = self.extensions_page.preview.take() {
                self.outbox.push(Outgoing::InstallExtension {
                    path: preview.path,
                    grants: preview.granted,
                });
            }
        } else if let Some(preview) = self.extensions_page.preview.as_mut() {
            let place = (action.0 - PREVIEW_GRANT_BASE) as usize;
            if let Some(asked) = preview.extension.requests.get(place).cloned() {
                if preview.granted.contains(&asked) {
                    preview.granted.retain(|held| *held != asked);
                } else {
                    preview.granted.push(asked);
                }
            }
        }
    }

    /// The state after a switch of an installed extension's row was pressed: the capability is allowed, or refused if it was allowed.
    fn grant_pressed(&mut self, action: FocusId) {
        let at = action.0 - GRANT_BASE;
        let (place, which) = (
            (at / GRANTS_PER_ROW) as usize,
            (at % GRANTS_PER_ROW) as usize,
        );
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
    pub(crate) fn extensions_pressed(mut self, action: FocusId) -> Self {
        if action == INSTALL_EXTENSION {
            self.open_folder_picker(crate::folder_picker::Purpose::Extension, None);
            return self;
        }
        if action.0 < ROW_BASE {
            self.extension_card_pressed(action);
            return self;
        }
        if (UPDATE_BASE..GRANT_BASE).contains(&action.0) {
            let place = (action.0 - UPDATE_BASE) as usize;
            if let Some(extension) = self.behind_extensions().get(place).copied().cloned() {
                self.update_extension(&extension);
            }
            return self;
        }
        if action.0 >= GRANT_BASE {
            self.grant_pressed(action);
            return self;
        }
        let place = (action.0 - ROW_BASE) as usize;
        let Some(extension) = self.listed_extensions().get(place).copied().cloned() else {
            return self;
        };
        self.install_or_remove(&extension);
        self
    }

    /// The installed extensions that are behind the list, in the order the page lists them.
    pub(crate) fn behind_extensions(&self) -> Vec<&ExtensionInfo> {
        self.listed_extensions()
            .into_iter()
            .filter(|extension| extension.installed && extension.update.is_some())
            .collect()
    }

    /// Installs `extension` again from the list as it stands now, which brings it up to date, unless it is busy being installed; nothing is changed until the reader presses the button that asks for it.
    pub(crate) fn update_extension(&mut self, extension: &ExtensionInfo) {
        if extension.busy.is_none() {
            self.outbox.push(Outgoing::InstallExtensions {
                ids: vec![extension.id.clone()],
            });
        }
    }

    /// Installs `extension`, or uninstalls it when it is installed, unless it is busy being installed.
    pub(crate) fn install_or_remove(&mut self, extension: &ExtensionInfo) {
        if extension.busy.is_none() {
            self.outbox
                .push(if extension.installed && extension.kind == "extension" {
                    Outgoing::RemovePackage {
                        id: extension.id.clone(),
                    }
                } else if extension.installed {
                    Outgoing::RemoveExtension {
                        id: extension.id.clone(),
                    }
                } else {
                    Outgoing::InstallExtensions {
                        ids: vec![extension.id.clone()],
                    }
                });
        }
    }
}

/// The title of the page and the button that installs an extension from a folder.
fn heading() -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            justify_between(),
            h(12),
            px(5),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(line("Extensions", 14, 1.4, Role::Text).styles([font_medium(), text_px(14)]))
        .child(
            Button::new("Install Extension…", INSTALL_EXTENSION)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build(),
        )
}

/// The box the reader types into to narrow the list.
fn search(state: &WorkspaceState) -> Node {
    Input::search(&state.extensions_page.search, EXTENSIONS_SEARCH)
        .size(Size::Lg)
        .placeholder("Search extensions")
        .build()
}

/// The icon an extension's files are shown with, once it is installed and has one; a plain code file otherwise.
fn mark(extension: &ExtensionInfo) -> Node {
    let artwork = extension
        .icon
        .as_ref()
        .filter(|_| extension.installed)
        .map(|source| Svg::arrived(&format!("extension-{}", extension.id), source));
    match artwork {
        Some(svg) => {
            let (_, tone) = crate::file_icon::file_kind(&format!(
                "file.{}",
                extension.suffixes.first().cloned().unwrap_or_default()
            ));
            node()
                .styles([size(5), flex_none(), text_color(tone)])
                .artwork(svg)
        }
        None if extension.kind == "extension" => {
            glyph(IconName::Plug, 5.0, Role::Subtle).styles([flex_none()])
        }
        None => glyph(IconName::FileCode, 5.0, Role::Subtle).styles([flex_none()]),
    }
}

/// The row of `extension`: its icon, then its name and version over what it is for and how it stands, and the button, answering to `action`, that installs or uninstalls it; a language that is behind the list has a button before it, answering to `update`, that brings it up to date.
pub(crate) fn row(extension: &ExtensionInfo, action: FocusId, update: Option<FocusId>) -> Node {
    let updating = update
        .filter(|_| extension.installed && extension.busy.is_none())
        .zip(extension.update.as_ref())
        .map(|(id, version)| {
            let label = if *version == extension.version || version.is_empty() {
                "Update".to_owned()
            } else {
                format!("Update to {version}")
            };
            Button::new(label, id)
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .build()
        });
    let button = match (extension.busy.is_some(), extension.installed) {
        (true, _) => Button::new("Installing", action)
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .disabled(true)
            .build(),
        (false, true) => Button::new("Uninstall", action)
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .build(),
        (false, false) => Button::new("Install", action)
            .variant(Variant::Outline)
            .size(Size::Sm)
            .build(),
    };
    let (said, tone) = standing(extension);
    let mut name = node()
        .styles([flex(), items_center(), gap(2), min_w_px(0)])
        .child(line(extension.name.clone(), 14, 1.4, Role::Text).styles([font_medium()]));
    if !extension.version.is_empty() {
        name = name
            .child(line(extension.version.clone(), 12, 1.4, Role::Subtle).styles([font_mono()]));
    }
    if extension.local {
        name = name.child(line("your own", 12, 1.4, Role::Subtle));
    }
    let about = if extension.description.is_empty() {
        endings(extension)
    } else {
        format!("{}  {}", extension.description, endings(extension))
    };
    let brings = wrapped(&extension.provides, ROW_LINE)
        .into_iter()
        .map(|said| line(said, 12, 1.4, Role::Subtle));
    let owner = extension
        .owner
        .as_ref()
        .map(|owner| line(format!("From the {owner} extension"), 12, 1.4, Role::Subtle));
    node()
        .styles([flex(), items_center(), gap(3), px(4), py(3), flex_none()])
        .child(mark(extension))
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_w_px(0), overflow_hidden()])
                .child(name)
                .child(line(about, 12, 1.4, Role::Muted))
                .children(brings)
                .children(owner)
                .children((!said.is_empty()).then(|| line(said, 12, 1.4, tone))),
        )
        .children(updating)
        .child(button)
}

/// One line saying what a capability lets an extension do, with the switch, answering to `id`, that allows it or not.
fn permission(capability: &str, allowed: bool, id: FocusId) -> Node {
    node()
        .styles([flex(), items_center(), justify_between(), gap(3), py(1)])
        .child(line(describe_capability(capability), 12, 1.4, Role::Muted))
        .child(
            Checkbox::new(allowed, id)
                .variant(CheckboxVariant::Switch)
                .build(),
        )
}

/// The switches of what an installed extension package asked to be allowed, under its row; nothing for one that asked for nothing.
fn permissions(extension: &ExtensionInfo, place: usize) -> Option<Node> {
    if extension.kind != "extension" || !extension.installed || extension.requests.is_empty() {
        return None;
    }
    Some(
        node()
            .styles([flex(), flex_col(), px(4), pb(3), flex_none()])
            .child(line("Allowed to", 12, 1.4, Role::Subtle))
            .children(extension.requests.iter().enumerate().map(|(which, asked)| {
                permission(
                    asked,
                    extension.granted.contains(asked),
                    FocusId(GRANT_BASE + place as u64 * GRANTS_PER_ROW + which as u64),
                )
            })),
    )
}

/// The card that shows what an extension carries and asks to be allowed, with a switch for each thing it asks and the buttons that go ahead or drop it.
fn preview_card(preview: &Preview) -> Node {
    let extension = &preview.extension;
    let mut title = format!("Install {}", extension.name);
    if !extension.version.is_empty() {
        title = format!("{title} {}", extension.version);
    }
    node()
        .styles([
            flex(),
            flex_col(),
            gap(3),
            px(4),
            py(4),
            flex_none(),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(line(title, 14, 1.4, Role::Text).styles([font_medium()]))
        .children(
            (!extension.description.is_empty())
                .then(|| line(extension.description.clone(), 12, 1.4, Role::Muted)),
        )
        .children(
            wrapped(&extension.provides, CARD_LINE)
                .into_iter()
                .map(|said| line(said, 12, 1.4, Role::Subtle)),
        )
        .child(line("It asks to be allowed to", 12, 1.4, Role::Subtle))
        .children(extension.requests.iter().enumerate().map(|(which, asked)| {
            permission(
                asked,
                preview.granted.contains(asked),
                FocusId(PREVIEW_GRANT_BASE + which as u64),
            )
        }))
        .child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(
                    Button::new("Install", PREVIEW_INSTALL)
                        .size(Size::Sm)
                        .build(),
                )
                .child(
                    Button::new("Cancel", PREVIEW_CANCEL)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build(),
                ),
        )
}

/// A titled group of rows in one outlined box, or nothing when it has none.
pub(crate) fn group(title: &str, rows: Vec<Node>) -> Option<Node> {
    if rows.is_empty() {
        return None;
    }
    let last = rows.len() - 1;
    Some(
        node()
            .styles([flex(), flex_col(), gap(2), flex_none()])
            .child(line(title.to_owned(), 12, 1.4, Role::Subtle))
            .child(
                node()
                    .styles([
                        flex(),
                        flex_col(),
                        flex_none(),
                        rounded(Radius::Lg),
                        border(),
                        border_color(Role::Border),
                        bg(Role::Surface),
                        overflow_hidden(),
                    ])
                    .children(rows.into_iter().enumerate().map(|(at, row)| {
                        if at == last {
                            row
                        } else {
                            row.styles([border_b(), border_color(Role::Border)])
                        }
                    })),
            ),
    )
}

/// The extensions page for `state`.
pub fn extensions(state: &WorkspaceState) -> Node {
    let listed = state.listed_extensions();
    let mut installed = Vec::new();
    let mut available = Vec::new();
    let mut behind = 0;
    for (place, extension) in listed.iter().enumerate() {
        let update = (extension.installed && extension.update.is_some()).then(|| {
            behind += 1;
            FocusId(UPDATE_BASE + behind - 1)
        });
        let mut built = row(extension, FocusId(ROW_BASE + place as u64), update);
        if let Some(allowed) = permissions(extension, place) {
            built = node()
                .styles([flex(), flex_col(), flex_none()])
                .child(built)
                .child(allowed);
        }
        if extension.installed {
            installed.push(built);
        } else {
            available.push(built);
        }
    }
    let mut column = node()
        .styles([
            flex(),
            flex_col(),
            gap(6),
            w_full(),
            max_w_px(PAGE_WIDTH),
            mx_auto(),
            px(5),
            pt(5),
            pb(8),
            flex_none(),
        ])
        .child(search(state));
    column = column
        .children(
            state
                .extensions_page
                .problem
                .as_ref()
                .map(|problem| line(problem.clone(), 12, 1.4, Role::Danger)),
        )
        .children(state.extensions_page.preview.as_ref().map(preview_card));
    column = if state.extensions.is_empty() {
        column.child(line(
            "Asking the server which extensions there are…",
            13,
            1.4,
            Role::Subtle,
        ))
    } else if listed.is_empty() {
        column.child(line("No extension matches.", 13, 1.4, Role::Subtle))
    } else {
        column
            .children(group("Installed on this machine", installed))
            .children(group("Available", available))
    };
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
        .child(heading())
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
                .scroll_from_start(EXTENSIONS_SCROLL, state.extensions_page.scroll)
                .child(column),
        )
}
