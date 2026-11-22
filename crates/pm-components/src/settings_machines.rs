//! The servers page of the settings window: the server on this machine and the ones on other machines the window knows, each changed or taken out from its row, the form a server is added or changed with, and which one new projects go to.

use crate::areas::SettingsEvent;
use crate::button;
use crate::target::Target;
use crate::theme;
use pm_config::{KnownServer, Reach};
use rustle::Modifier;
use rustle::TextFieldState;
use rustle::{Alignment, Arrangement};

use crate::theme::Role;

use crate::button::Variant;
use crate::input::Input;
use crate::parts::line;
use crate::settings::Section;
use crate::settings_pages::page_rows;
use crate::settings_row::choices;
use crate::settings_rows::{MACHINE_EDITS, MACHINE_REACHES, MACHINE_REMOVES};
use crate::settings_servers::{add_heading, boxed, flat_row, form_buttons, note};
use crate::size::Size;
use crate::state::WorkspaceState;

/// How the servers can be reached, in the order the form offers them.
const REACHES: [Reach; 5] = [
    Reach::Direct,
    Reach::Ssh,
    Reach::Wsl,
    Reach::Container,
    Reach::Tunnel,
];

/// What the form and the rows call each way of [`REACHES`], in its order.
const REACH_NAMES: [&str; 5] = ["Direct", "SSH", "WSL", "Dev container", "Tunnel"];

const _: () = assert!(
    REACHES.len() as u64 <= MACHINE_REACHES.len(),
    "MACHINE_REACHES has no room for every way a server is reached"
);

/// A server being described: which one of the list it replaces, and the boxes it is written in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MachineForm {
    /// The place in the list of the server being changed, or nothing for a new one.
    index: Option<usize>,
    /// What it is called.
    name: TextFieldState,
    /// How it is reached.
    reach: Reach,
    /// Where it is.
    address: TextFieldState,
    /// The token a direct one or one behind a tunnel wants.
    token: TextFieldState,
}

impl MachineForm {
    /// A form holding `server`, which is at `index` of the list, or nothing for a new one.
    fn holding(index: Option<usize>, server: &KnownServer) -> Self {
        Self {
            index,
            name: TextFieldState::new(server.name.clone()),
            reach: server.reach,
            address: TextFieldState::new(server.address.clone()),
            token: TextFieldState::new(server.token.clone()),
        }
    }

    /// The server the form describes, once it has a name and an address.
    fn server(&self) -> Option<KnownServer> {
        let name = self.name.text();
        let name = name.trim();
        let address = self.address.text();
        let address = address.trim();
        (!name.is_empty() && !address.is_empty()).then(|| KnownServer {
            name: name.to_owned(),
            reach: self.reach,
            address: address.to_owned(),
            token: if self.reach.keeps_token() {
                self.token.text().trim().to_owned()
            } else {
                String::new()
            },
        })
    }
}

/// The boxes of the form: its name, address and token.
const FIELDS: [Target; 3] = [
    Target::Settings(SettingsEvent::MachineName),
    Target::Settings(SettingsEvent::MachineAddress),
    Target::Settings(SettingsEvent::MachineToken),
];

/// Whether `id` names a box of the form.
pub(crate) fn is_machine_field(id: Target) -> bool {
    FIELDS.contains(&id)
}

impl WorkspaceState {
    /// Carries out pressing `id` on the servers page, and says whether it was something of it.
    pub(crate) fn machines_pressed(&mut self, id: Target) -> bool {
        match id {
            Target::Settings(SettingsEvent::MachineAdd) => {
                self.settings_page.server_form =
                    Some(MachineForm::holding(None, &KnownServer::default()));
            }
            Target::Settings(SettingsEvent::MachineCancel) => self.settings_page.server_form = None,
            Target::Settings(SettingsEvent::MachineSave) => self.save_machine(),
            _ if let Some(option) = MACHINE_REACHES.index_of(id) => {
                if let (Some(form), Some(reach)) =
                    (self.settings_page.server_form.as_mut(), REACHES.get(option))
                {
                    form.reach = *reach;
                }
            }
            _ if let Some(index) = MACHINE_EDITS.index_of(id) => {
                self.settings_page.server_form = self
                    .window
                    .servers
                    .known
                    .get(index)
                    .map(|server| MachineForm::holding(Some(index), server));
            }
            _ if let Some(index) = MACHINE_REMOVES.index_of(id) => self.remove_machine(index),
            _ => return false,
        }
        true
    }

    /// Keeps the server the form describes, in place of the one it changes, and puts the form away; a form missing its name or address stays open.
    fn save_machine(&mut self) {
        let Some(form) = self.settings_page.server_form.as_ref() else {
            return;
        };
        let Some(server) = form.server() else {
            return;
        };
        let servers = &mut self.window.servers;
        match form.index {
            Some(index) if index < servers.known.len() => {
                let renamed = servers.known[index].name.clone();
                if servers.default_server.as_deref() == Some(renamed.as_str()) {
                    servers.default_server = Some(server.name.clone());
                }
                servers.known[index] = server;
            }
            _ => servers.known.push(server),
        }
        self.settings_page.server_form = None;
    }

    /// Takes the server at `index` out of the list, and new projects back to this machine when they went to it.
    fn remove_machine(&mut self, index: usize) {
        let servers = &mut self.window.servers;
        if index >= servers.known.len() {
            return;
        }
        let gone = servers.known.remove(index);
        if servers.default_server.as_deref() == Some(gone.name.as_str()) {
            servers.default_server = None;
        }
        self.settings_page.server_form = None;
    }

    /// The box `id` of the form, while it is open.
    pub(crate) fn machine_field(&self, id: Target) -> Option<&TextFieldState> {
        let form = self.settings_page.server_form.as_ref()?;
        let place = FIELDS.iter().position(|held| *held == id)?;
        Some([&form.name, &form.address, &form.token][place])
    }
}

/// Emits the box of a form `input` describes, small, under the words `label` that say
/// what is written in it.
#[track_caller]
pub(crate) fn labelled(label: &str, input: Input<'_>) {
    rustle::key(input.answers_to(), || {
        labelled_box(label, || input.size(Size::Sm).build());
    });
}

/// Emits what `field` emits under the words `label` that say what it holds,
/// the two as wide as the form.
#[track_caller]
pub(crate) fn labelled_box(label: &str, field: impl FnOnce()) {
    rustle::column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Start,
        || {
            note(label);
            field();
        },
    );
}

/// Emits the choices `names` of a form, the one at `picked` marked and each answering to what `id_of` names it, under the words `label` that say what is chosen.
#[track_caller]
pub(crate) fn labelled_choices(
    label: &str,
    names: &[&str],
    picked: usize,
    id_of: impl Fn(usize) -> Option<Target>,
) {
    rustle::column_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Start,
        || {
            note(label);
            rustle::row_arranged(
                Modifier::new(),
                Arrangement::Start,
                Alignment::Start,
                || choices(names, picked, id_of),
            );
        },
    );
}

/// Emits a row of a list that can be changed: what `lines` emits, one line under another, then, while `buttons` holds them, the Edit and Remove buttons answering to the names in it, each left out when it has none.
#[track_caller]
pub(crate) fn editable_row(
    lines: impl FnOnce(),
    buttons: Option<(Option<Target>, Option<Target>)>,
) {
    flat_row(3, || {
        rustle::column_arranged(
            Modifier::new().weight(1.0),
            Arrangement::SpacedBy(theme::sp(0.5)),
            Alignment::Start,
            lines,
        );
        if let Some((edit, remove)) = buttons {
            rustle::row_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(theme::sp(1)),
                Alignment::Center,
                || {
                    if let Some(edit) = edit {
                        button::action("Edit", edit, Variant::Ghost);
                    }
                    if let Some(remove) = remove {
                        button::action("Remove", remove, Variant::Ghost);
                    }
                },
            );
        }
    });
}

/// Emits the form a server is added or changed with.
#[track_caller]
fn form_node(form: &MachineForm) {
    let (address_label, address_hint) = match form.reach {
        Reach::Direct => ("Address", "host:port, e.g. 100.64.0.7:6868"),
        Reach::Ssh => ("SSH destination", "user@host, or a host of ~/.ssh/config"),
        Reach::Wsl => ("Distribution", "As wsl.exe -l lists it, e.g. Ubuntu"),
        Reach::Container => (
            "Folder with a devcontainer.json",
            "A folder on this machine, e.g. /home/me/app",
        ),
        Reach::Tunnel => ("Tunnel hostname", "e.g. build.example.com"),
    };
    rustle::column_arranged(
        Modifier::new().fill_max_width().padding(theme::sp(3)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Start,
        || {
            labelled(
                "Name",
                Input::new(&form.name, Target::Settings(SettingsEvent::MachineName))
                    .placeholder("e.g. Build box")
                    .submits(Target::Settings(SettingsEvent::MachineSave)),
            );
            labelled_choices("Reached", &REACH_NAMES, reach_place(form.reach), |option| {
                MACHINE_REACHES.id(option)
            });
            labelled(
                address_label,
                Input::new(
                    &form.address,
                    Target::Settings(SettingsEvent::MachineAddress),
                )
                .placeholder(address_hint)
                .submits(Target::Settings(SettingsEvent::MachineSave)),
            );
            if form.reach.keeps_token() {
                labelled(
                    "Token",
                    Input::new(&form.token, Target::Settings(SettingsEvent::MachineToken))
                        .placeholder("As in server.json on that machine")
                        .submits(Target::Settings(SettingsEvent::MachineSave))
                        .secure(true),
                );
            }
            form_buttons(
                Modifier::new().fill_max_width(),
                Target::Settings(SettingsEvent::MachineCancel),
                Target::Settings(SettingsEvent::MachineSave),
                form.server().is_some(),
            );
        },
    );
}

/// The place of `reach` among [`REACHES`].
fn reach_place(reach: Reach) -> usize {
    REACHES.iter().position(|held| *held == reach).unwrap_or(0)
}

/// Emits the body of the servers page, `room` logical pixels across: where new projects go, then every server with the form while it is open.
#[track_caller]
pub(crate) fn body(state: &WorkspaceState, room: f32) {
    let form = state.settings_page.server_form.as_ref();
    let adding = form.is_some_and(|form| form.index.is_none());
    page_rows(state, Section::Servers, room);
    add_heading(
        "Servers",
        state.window.servers.known.len() + 1,
        None,
        Target::Settings(SettingsEvent::MachineAdd),
        adding,
    );
    boxed(|| {
        editable_row(
            || {
                line("This machine", 13, 1.4, Role::Text);
                note("Local · started by the window when none is running");
            },
            None,
        );
        for (index, server) in state.window.servers.known.iter().enumerate() {
            rustle::key(index, || match form {
                Some(form) if form.index == Some(index) => form_node(form),
                _ => editable_row(
                    || {
                        line(&server.name, 13, 1.4, Role::Text);
                        note(&format!(
                            "{} · {}",
                            REACH_NAMES[reach_place(server.reach)],
                            server.address
                        ));
                    },
                    Some((MACHINE_EDITS.id(index), MACHINE_REMOVES.id(index))),
                ),
            });
        }
        if let (Some(form), true) = (form, adding) {
            form_node(form);
        }
    });
}
