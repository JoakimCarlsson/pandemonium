//! What each section of the settings window holds under its heading: its
//! rows of settings, and the lists of the sections that show more than rows.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{
    BadgeVariant, Button, ButtonVariant, Card, CardRow, CardVariant, Divider, IconButton, KeyCap,
    KeyRecorder, Label, LabelVariant, SectionHeader, SectionHeaderVariant, Segmented, Size,
    TextInput, TextInputVariant,
};
use pm_shell::{
    REACHES, RUNNING, Release, Section, Setting, SettingsChange, SettingsModel, reach_of,
};

use crate::settings::Layout;
use crate::settings_actions::SettingsActions;
use crate::settings_rows::{group, page_rows, results, setting_row};

/// Emits what `section` holds under its heading.
pub(crate) fn body(
    model: &Rc<SettingsModel>,
    actions: &SettingsActions,
    section: Section,
    layout: Layout,
) {
    match section {
        Section::Results => results(model, actions, layout),
        Section::General => {
            page_rows(model, actions, section, layout);
            release_note(model, actions);
        }
        Section::Editor => {
            for page in Section::EDITOR_PAGES {
                key(page, || {
                    group(page.title(), || page_rows(model, actions, page, layout));
                });
            }
        }
        Section::Languages => languages(model, actions, layout),
        Section::Keymap => keymap(model, actions, layout),
        Section::Agents => {
            page_rows(model, actions, section, layout);
            explore(model, actions);
        }
        Section::Servers => servers(model, actions, layout),
        Section::AgentServers
        | Section::McpServers
        | Section::ModelServers
        | Section::ModelSlots => crate::settings_agents::body(model, actions, section),
        Section::Customizations
        | Section::Skills
        | Section::Instructions
        | Section::AgentDefinitions
        | Section::Hooks
        | Section::Tools => crate::settings_library::body(model, actions, section),
        _ => page_rows(model, actions, section, layout),
    }
}

/// Emits `rows` inside a card, as the lists of the settings window are set.
pub(crate) fn boxed(rows: impl FnOnce()) {
    Card::new()
        .variant(CardVariant::Outlined)
        .size(Size::Sm)
        .modifier(Modifier::new().fill_max_width())
        .show(rows);
}

/// Emits a heading `title` with `count` beside it, over a list.
pub(crate) fn heading(title: &str, count: usize) {
    SectionHeader::new(format!("{title} · {count}"))
        .variant(SectionHeaderVariant::Group)
        .show();
}

/// Emits a faint line saying `note`.
pub(crate) fn note(note: &str) {
    Label::new(note).variant(LabelVariant::Muted).wrap().show();
}

/// Emits each of `items` with `row`, with a hairline between each and the next.
pub(crate) fn divided<T>(items: Vec<T>, mut row: impl FnMut(usize, T)) {
    for (place, item) in items.into_iter().enumerate() {
        if place > 0 {
            Divider::new().show();
        }
        key(place, || row(place, item));
    }
}

/// Emits what looking for a newer release found, beside the version running, with the button opening its page when one is newer.
fn release_note(model: &SettingsModel, actions: &SettingsActions) {
    let Some(release) = &model.release else {
        return;
    };
    let (said, url) = match release {
        Release::Newer { name, url } => (
            format!("{name} is available; this is {RUNNING}."),
            Some(url.clone()),
        ),
        Release::Current => (format!("Pandemonium {RUNNING} is up to date."), None),
        Release::Unknown => (
            "Could not check for a newer release on this channel.".to_owned(),
            None,
        ),
    };
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpaceBetween,
        Alignment::Center,
        || {
            Label::new(said)
                .variant(LabelVariant::Muted)
                .wrap()
                .modifier(Modifier::new().weight(1.0))
                .show();
            if let Some(url) = url {
                let open = actions.open_url.clone();
                Button::new("Open Release Page")
                    .variant(ButtonVariant::Secondary)
                    .on_click(move || open(url.clone()))
                    .show();
            }
        },
    );
}

/// Emits the cards on the Agents page leading to the sections inside it, each with how many it lists.
fn explore(model: &SettingsModel, actions: &SettingsActions) {
    SectionHeader::new("Explore")
        .variant(SectionHeaderVariant::Group)
        .show();
    let count = |section: Section| match section {
        Section::AgentServers => {
            format!("{} of {} offered", model.shown_agents(), model.agents.len())
        }
        Section::McpServers if model.mcp_loaded => {
            format!("{} installed", model.mcp_servers.len())
        }
        Section::ModelServers if model.model_servers_loaded => {
            format!("{} declared", model.model_servers.len())
        }
        Section::ModelSlots if model.slots_loaded => format!("{} slots", model.slots.len()),
        _ => String::new(),
    };
    boxed(|| {
        divided(
            vec![
                Section::AgentServers,
                Section::McpServers,
                Section::ModelServers,
                Section::ModelSlots,
            ],
            |_, section| {
                let counted = count(section);
                let open = actions.send(SettingsChange::Open(section));
                column_with(
                    Modifier::new()
                        .fill_max_width()
                        .cursor(CursorIcon::Pointer)
                        .clickable(open),
                    || {
                        let mut row = CardRow::new(section.title())
                            .icon(crate::settings_nav::section_icon(section))
                            .detail(first_sentence(section.summary()))
                            .trailing(|| {
                                Label::new("").show();
                            });
                        if !counted.is_empty() {
                            row = row.badge(counted, BadgeVariant::Neutral);
                        }
                        row.show();
                    },
                );
            },
        );
    });
}

/// The first sentence of `text`.
fn first_sentence(text: &str) -> String {
    match text.find(". ") {
        Some(end) => text[..=end].to_owned(),
        None => text.to_owned(),
    }
}

/// Emits the languages page: how the language picked is handled, then every language installed and every one that can be, each with the button installing or removing it.
fn languages(model: &SettingsModel, actions: &SettingsActions, layout: Layout) {
    page_rows(model, actions, Section::Languages, layout);
    if !model.extensions_loaded {
        note("Asking the server which languages there are…");
        return;
    }
    let listed = model.languages();
    let (installed, available): (Vec<_>, Vec<_>) = listed
        .into_iter()
        .partition(|extension| extension.installed);
    for (title, extensions) in [
        ("Installed", installed),
        ("Available to install", available),
    ] {
        if extensions.is_empty() {
            continue;
        }
        heading(title, extensions.len());
        boxed(|| {
            divided(extensions, |_, extension| {
                let mut row = CardRow::new(extension.name.clone()).detail(
                    if extension.description.is_empty() {
                        extension.version.clone()
                    } else {
                        extension.description.clone()
                    },
                );
                if let Some(problem) = &extension.problem {
                    row = row.badge(problem.clone(), BadgeVariant::Danger);
                }
                let label = match (&extension.busy, extension.installed, &extension.update) {
                    (Some(busy), _, _) => busy.clone(),
                    (None, true, Some(_)) => "Update".to_owned(),
                    (None, true, None) => "Remove".to_owned(),
                    (None, false, _) => "Install".to_owned(),
                };
                let busy = extension.busy.is_some();
                let press = actions.send(SettingsChange::InstallOrRemove(extension.id.clone()));
                row.trailing(move || {
                    Button::new(label)
                        .variant(ButtonVariant::Secondary)
                        .disabled(busy)
                        .on_click(press)
                        .show();
                })
                .show();
            });
        });
    }
}

/// Emits the keymap page: the preset the keys start from, the box the actions are searched with, then every action found under its group with its keys.
fn keymap(model: &SettingsModel, actions: &SettingsActions, layout: Layout) {
    setting_row(model, actions, Setting::BaseKeymap, layout);
    TextInput::new(model.page.keymap_find.clone())
        .variant(TextInputVariant::Search)
        .placeholder("Search actions by name or keys")
        .modifier(Modifier::new().fill_max_width())
        .on_change(actions.with(SettingsChange::KeymapFind))
        .show();
    let recording = remember(|| None::<String>);
    let groups = model.keymap_groups();
    if groups.is_empty() {
        note("No action matches.");
    }
    for group in groups {
        key(group.title.clone(), || {
            SectionHeader::new(group.title.clone())
                .variant(SectionHeaderVariant::Group)
                .show();
            boxed(|| {
                divided(group.rows, |_, row| {
                    let detail = if row.clashes.is_empty() {
                        String::new()
                    } else {
                        format!("Also bound to {}", row.clashes.join(", "))
                    };
                    let reset = actions.send(SettingsChange::ResetKeys(row.key.clone()));
                    let clear = actions.send(SettingsChange::ClearKeys(row.key.clone()));
                    let keys = row.keys.clone();
                    let own = row.own;
                    let action = row.key.clone();
                    let held = recording.get().as_deref() == Some(action.as_str());
                    let start = {
                        let recording = recording.clone();
                        let action = action.clone();
                        move || recording.set(Some(action.clone()))
                    };
                    let cancel = {
                        let recording = recording.clone();
                        move || recording.set(None)
                    };
                    let record = {
                        let recording = recording.clone();
                        let change = actions.change.clone();
                        move |binding: String| {
                            change(SettingsChange::SetKeys(action.clone(), binding));
                            recording.set(None);
                        }
                    };
                    let mut shown = CardRow::new(row.title).size(Size::Xs).detail(detail);
                    if !row.clashes.is_empty() {
                        shown = shown.badge("Clash", BadgeVariant::Warning);
                    }
                    shown
                        .trailing(move || {
                            if keys.is_empty() && !held {
                                Label::new("Unbound").variant(LabelVariant::Muted).show();
                            }
                            if !held {
                                for binding in &keys {
                                    KeyCap::new(binding.clone()).show();
                                }
                            }
                            KeyRecorder::new(held)
                                .on_start(start)
                                .on_cancel(cancel)
                                .on_record(record)
                                .show();
                            if own {
                                Button::new("Reset").size(Size::Xs).on_click(reset).show();
                            }
                            if !keys.is_empty() {
                                Button::new("Clear").size(Size::Xs).on_click(clear).show();
                            }
                        })
                        .show();
                });
            });
        });
    }
}

/// Emits the servers page: where new projects go, then this machine and every server the window knows, and the form adding one.
fn servers(model: &SettingsModel, actions: &SettingsActions, layout: Layout) {
    page_rows(model, actions, Section::Servers, layout);
    heading("Servers", model.window.servers.known.len() + 1);
    boxed(|| {
        CardRow::new("This machine")
            .icon(phosphor::DESKTOP)
            .detail("Local · started by the window when none is running")
            .show();
        for (place, server) in model.window.servers.known.iter().enumerate() {
            key(place, || {
                Divider::new().show();
                let remove = actions.send(SettingsChange::RemoveServer(place));
                CardRow::new(server.name.clone())
                    .icon(phosphor::HARD_DRIVES)
                    .detail(reach_of(server))
                    .trailing(move || {
                        IconButton::new(phosphor::TRASH)
                            .size(Size::Xs)
                            .on_click(remove)
                            .show();
                    })
                    .show();
            });
        }
    });
    server_form(actions, layout);
}

/// Emits the form adding a server: its name, how it is reached, its address, and for a server reached straight or through a tunnel the token it lets the window in with.
fn server_form(actions: &SettingsActions, layout: Layout) {
    let name = remember(String::new);
    let address = remember(String::new);
    let token = remember(String::new);
    let reach = remember(|| 0_usize);
    SectionHeader::new("Add a server")
        .variant(SectionHeaderVariant::Group)
        .show();
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Start,
        || {
            let options: Vec<String> = REACHES.iter().map(|(_, name)| (*name).to_owned()).collect();
            let fits = Segmented::width(&options, Size::Sm) <= layout.room;
            let picked = reach.clone();
            if fits {
                Segmented::new(options, reach.get())
                    .on_pick(move |place| picked.set(place))
                    .show();
            } else {
                let cycle = reach.clone();
                Button::new(REACHES[reach.get().min(REACHES.len() - 1)].1)
                    .trailing_icon(phosphor::CARET_UP_DOWN)
                    .variant(ButtonVariant::Secondary)
                    .on_click(move || cycle.update(|place| *place = (*place + 1) % REACHES.len()))
                    .show();
            }
            let keep_name = name.clone();
            TextInput::new(name.get())
                .placeholder("Name, e.g. Lab")
                .modifier(Modifier::new().fill_max_width())
                .on_change(move |text| keep_name.set(text))
                .show();
            let keep_address = address.clone();
            TextInput::new(address.get())
                .placeholder(match reach.get() {
                    1 => "user@host",
                    2 => "Distribution, e.g. Ubuntu",
                    3 => "Folder with a dev container",
                    4 => "Tunnel address",
                    _ => "host:port",
                })
                .code()
                .modifier(Modifier::new().fill_max_width())
                .on_change(move |text| keep_address.set(text))
                .show();
            let keeps_token = REACHES
                .get(reach.get())
                .is_some_and(|(how, _)| how.keeps_token());
            if keeps_token {
                let keep_token = token.clone();
                TextInput::new(token.get())
                    .placeholder("Token the server lets the window in with")
                    .code()
                    .modifier(Modifier::new().fill_max_width())
                    .on_change(move |text| keep_token.set(text))
                    .show();
            }
            let change = actions.change.clone();
            let (name, address, reach, token) =
                (name.clone(), address.clone(), reach.clone(), token.clone());
            let ready = !name.get().trim().is_empty();
            Button::new("Add Server")
                .icon(phosphor::PLUS)
                .variant(ButtonVariant::Primary)
                .disabled(!ready)
                .on_click(move || {
                    change(SettingsChange::AddServer(
                        name.get(),
                        address.get(),
                        reach.get(),
                        token.get(),
                    ));
                    name.set(String::new());
                    address.set(String::new());
                    token.set(String::new());
                })
                .show();
        },
    );
}
