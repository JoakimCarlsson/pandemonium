//! The servers page of the settings window: the server on this machine and the ones on other machines the window knows, each changed or taken out from its row, the form a server is added or changed with, and which one new projects go to.

use crate::kit::sp;
use pm_config::{KnownServer, Reach};
use pm_field::{Edit, Field};
use rustle::Modifier;
use rustle::{Flex, flex_column, flex_row};

use crate::kit::{FocusId, Role, Styled};

use crate::Button;
use crate::button::Variant;
use crate::input::Input;
use crate::parts::{clipped, line};
use crate::settings::Section;
use crate::settings_ids::{MACHINE_EDITS, MACHINE_REACHES, MACHINE_REMOVES, SINGLES};
use crate::settings_pages::page_rows;
use crate::settings_row::choices;
use crate::size::Size;
use crate::state::WorkspaceState;

/// Opens the form for adding a server.
pub const MACHINE_ADD: FocusId = SINGLES.at(70);

/// Keeps what the form holds.
pub const MACHINE_SAVE: FocusId = SINGLES.at(71);

/// Puts the form away without keeping it.
pub const MACHINE_CANCEL: FocusId = SINGLES.at(72);

/// The box the form's name is written in.
pub const MACHINE_NAME: FocusId = SINGLES.at(73);

/// The box the form's address is written in.
pub const MACHINE_ADDRESS: FocusId = SINGLES.at(74);

/// The box the form's token is written in.
pub const MACHINE_TOKEN: FocusId = SINGLES.at(75);

/// How the servers can be reached, in the order the form offers them.
const REACHES: [Reach; 5] = [
    Reach::Direct,
    Reach::Ssh,
    Reach::Wsl,
    Reach::Container,
    Reach::Tunnel,
];

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
    name: Field,
    /// How it is reached.
    reach: Reach,
    /// Where it is.
    address: Field,
    /// The token a direct one or one behind a tunnel wants.
    token: Field,
}

impl MachineForm {
    /// A form holding `server`, which is at `index` of the list, or nothing for a new one.
    fn holding(index: Option<usize>, server: &KnownServer) -> Self {
        Self {
            index,
            name: Field::new(server.name.clone()),
            reach: server.reach,
            address: Field::new(server.address.clone()),
            token: Field::new(server.token.clone()).masked(true),
        }
    }

    /// The server the form describes, once it has a name and an address.
    fn server(&self) -> Option<KnownServer> {
        let name = self.name.text().trim();
        let address = self.address.text().trim();
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

/// Whether `id` names a box of the form.
pub(crate) fn is_machine_field(id: FocusId) -> bool {
    [MACHINE_NAME, MACHINE_ADDRESS, MACHINE_TOKEN].contains(&id)
}

impl WorkspaceState {
    /// Carries out pressing `id` on the servers page, and says whether it was something of it.
    pub(crate) fn machines_pressed(&mut self, id: FocusId) -> bool {
        match id {
            MACHINE_ADD => {
                self.settings_page.server_form =
                    Some(MachineForm::holding(None, &KnownServer::default()));
            }
            MACHINE_CANCEL => self.settings_page.server_form = None,
            MACHINE_SAVE => self.save_machine(),
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

    /// Writes `edit` into the box `id` of the form.
    pub(crate) fn machine_edited(&mut self, id: FocusId, edit: Edit) {
        let Some(form) = self.settings_page.server_form.as_mut() else {
            return;
        };
        match id {
            MACHINE_NAME => form.name.apply(edit),
            MACHINE_ADDRESS => form.address.apply(edit),
            MACHINE_TOKEN => form.token.apply(edit),
            _ => {}
        }
    }

    /// The box `id` of the form, while it is open.
    pub(crate) fn machine_field(&self, id: FocusId) -> Option<&Field> {
        let form = self.settings_page.server_form.as_ref()?;
        match id {
            MACHINE_NAME => Some(&form.name),
            MACHINE_ADDRESS => Some(&form.address),
            MACHINE_TOKEN => Some(&form.token),
            _ => None,
        }
    }
}

/// Emits a box of the form under the words that say what is written in it.
#[track_caller]
fn labelled(words: &str, field: &Field, id: FocusId, placeholder: &str) {
    flex_column(Modifier::new(), Flex::new().gap(sp(1)), || {
        line(words, 12, 1.4, Role::Muted);
        Input::new(field, id)
            .size(Size::Sm)
            .placeholder(placeholder)
            .build();
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
    let picked = REACHES
        .iter()
        .position(|held| *held == form.reach)
        .unwrap_or(0);
    flex_column(Modifier::new().pad(3), Flex::new().gap(sp(3)), || {
        labelled("Name", &form.name, MACHINE_NAME, "e.g. Build box");
        flex_column(Modifier::new(), Flex::new().gap(sp(1)), || {
            line("Reached", 12, 1.4, Role::Muted);
            flex_row(Modifier::new(), Flex::new(), || {
                choices(
                    &["Direct", "SSH", "WSL", "Dev container", "Tunnel"],
                    picked,
                    |option| MACHINE_REACHES.id(option),
                );
            });
        });
        labelled(address_label, &form.address, MACHINE_ADDRESS, address_hint);
        if form.reach.keeps_token() {
            labelled(
                "Token",
                &form.token,
                MACHINE_TOKEN,
                "As in server.json on that machine",
            );
        }
        flex_row(
            Modifier::new(),
            Flex::new().items_center().justify_end().gap(sp(2)),
            || {
                Button::new("Cancel", MACHINE_CANCEL)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .build();
                Button::new("Save", MACHINE_SAVE)
                    .size(Size::Sm)
                    .disabled(form.server().is_none())
                    .build();
            },
        );
    });
}

/// Emits the row of a server: its name over how it is reached, and its buttons when it can be changed and the page has names for them.
#[track_caller]
fn machine_row(name: &str, reach: String, buttons: Option<usize>) {
    flex_row(
        Modifier::new().pad_x(3).pad_y(3),
        Flex::new().items_center().justify_between().gap(sp(3)),
        || {
            flex_column(Modifier::new().grow(), Flex::new().gap(sp(0.5)), || {
                clipped(name, 13, Role::Text);
                clipped(reach, 12, Role::Muted);
            });
            if let Some(index) = buttons {
                flex_row(
                    Modifier::new(),
                    Flex::new().items_center().gap(sp(1)),
                    || {
                        if let Some(edit) = MACHINE_EDITS.id(index) {
                            Button::new("Edit", edit)
                                .variant(Variant::Ghost)
                                .size(Size::Sm)
                                .build();
                        }
                        if let Some(remove) = MACHINE_REMOVES.id(index) {
                            Button::new("Remove", remove)
                                .variant(Variant::Ghost)
                                .size(Size::Sm)
                                .build();
                        }
                    },
                );
            }
        },
    );
}

/// How `server` is reached, in the words of its row.
fn reach_of(server: &KnownServer) -> String {
    match server.reach {
        Reach::Direct => format!("Direct · {}", server.address),
        Reach::Ssh => format!("SSH · {}", server.address),
        Reach::Wsl => format!("WSL · {}", server.address),
        Reach::Container => format!("Dev container · {}", server.address),
        Reach::Tunnel => format!("Tunnel · {}", server.address),
    }
}

/// Emits the body of the servers page, `room` logical pixels across: where new projects go, then every server with the form while it is open.
#[track_caller]
pub(crate) fn body(state: &WorkspaceState, room: f32) {
    let form = state.settings_page.server_form.as_ref();
    let adding = form.is_some_and(|form| form.index.is_none());
    page_rows(state, Section::Servers, room);
    flex_row(
        Modifier::new(),
        Flex::new().items_center().justify_between(),
        || {
            crate::settings_servers::heading_only("Servers", state.window.servers.known.len() + 1);
            Button::new("Add Server", MACHINE_ADD)
                .variant(Variant::Outline)
                .size(Size::Sm)
                .disabled(adding)
                .build();
        },
    );
    crate::settings_servers::boxed(|| {
        machine_row(
            "This machine",
            "Local · started by the window when none is running".to_owned(),
            None,
        );
        for (index, server) in state.window.servers.known.iter().enumerate() {
            rustle::key(index, || match form {
                Some(form) if form.index == Some(index) => form_node(form),
                _ => machine_row(&server.name, reach_of(server), Some(index)),
            });
        }
        if let (Some(form), true) = (form, adding) {
            form_node(form);
        }
    });
}
