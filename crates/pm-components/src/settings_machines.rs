//! The servers page of the settings window: the server on this machine and the ones on other machines the window knows, each changed or taken out from its row, the form a server is added or changed with, and which one new projects go to.

use pm_config::{KnownServer, Reach};
use pm_input::FocusId;
use pm_style::{
    Role, flex, flex_1, flex_col, flex_none, gap, items_center, justify_between, justify_end,
    min_w_px, p,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::{Size, Variant};
use crate::input::{self, Input};
use crate::parts::{clipped, line};
use crate::settings::Section;
use crate::settings_pages::page_rows;
use crate::settings_row::choices;
use crate::state::WorkspaceState;

/// Opens the form for adding a server.
pub const MACHINE_ADD: FocusId = FocusId(20_000_070);

/// Keeps what the form holds.
pub const MACHINE_SAVE: FocusId = FocusId(20_000_071);

/// Puts the form away without keeping it.
pub const MACHINE_CANCEL: FocusId = FocusId(20_000_072);

/// The box the form's name is written in.
pub const MACHINE_NAME: FocusId = FocusId(20_000_073);

/// The box the form's address is written in.
pub const MACHINE_ADDRESS: FocusId = FocusId(20_000_074);

/// The box the form's token is written in.
pub const MACHINE_TOKEN: FocusId = FocusId(20_000_075);

/// The first of the choices of how the form's server is reached; the others follow it.
const MACHINE_REACH_BASE: u64 = 20_000_076;

/// The first of the buttons changing or taking out a server; each server has two after its own base, its place among them times two.
const MACHINE_ROW_BASE: u64 = 20_005_000;

/// One past the last of them.
const MACHINE_ROW_END: u64 = 20_005_400;

/// How the servers can be reached, in the order the form offers them.
const REACHES: [Reach; 5] = [
    Reach::Direct,
    Reach::Ssh,
    Reach::Wsl,
    Reach::Container,
    Reach::Tunnel,
];

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
            FocusId(number)
                if (MACHINE_REACH_BASE..MACHINE_REACH_BASE + REACHES.len() as u64)
                    .contains(&number) =>
            {
                if let Some(form) = self.settings_page.server_form.as_mut() {
                    form.reach = REACHES[(number - MACHINE_REACH_BASE) as usize];
                }
            }
            FocusId(number) if (MACHINE_ROW_BASE..MACHINE_ROW_END).contains(&number) => {
                let index = ((number - MACHINE_ROW_BASE) / 2) as usize;
                if (number - MACHINE_ROW_BASE).is_multiple_of(2) {
                    self.settings_page.server_form = self
                        .window
                        .servers
                        .known
                        .get(index)
                        .map(|server| MachineForm::holding(Some(index), server));
                } else {
                    self.remove_machine(index);
                }
            }
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

/// A box of the form under the words that say what is written in it.
fn labelled(label: &str, field: &Field, id: FocusId, placeholder: &str) -> Node {
    node()
        .styles([flex(), flex_col(), gap(1)])
        .child(line(label, 12, 1.4, Role::Muted))
        .child(
            Input::new(field, id)
                .size(input::Size::Sm)
                .placeholder(placeholder)
                .build(),
        )
}

/// The form a server is added or changed with.
fn form_node(form: &MachineForm) -> Node {
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
    let reach = choices(
        &["Direct", "SSH", "WSL", "Dev container", "Tunnel"],
        REACHES
            .iter()
            .position(|held| *held == form.reach)
            .unwrap_or(0),
        |option| FocusId(MACHINE_REACH_BASE + option as u64),
    );
    let mut fields = node()
        .styles([flex(), flex_col(), gap(3), p(3)])
        .child(labelled("Name", &form.name, MACHINE_NAME, "e.g. Build box"))
        .child(
            node()
                .styles([flex(), flex_col(), gap(1)])
                .child(line("Reached", 12, 1.4, Role::Muted))
                .child(node().styles([flex()]).child(reach)),
        )
        .child(labelled(
            address_label,
            &form.address,
            MACHINE_ADDRESS,
            address_hint,
        ));
    if form.reach.keeps_token() {
        fields = fields.child(labelled(
            "Token",
            &form.token,
            MACHINE_TOKEN,
            "As in server.json on that machine",
        ));
    }
    fields.child(
        node()
            .styles([flex(), items_center(), justify_end(), gap(2)])
            .child(
                Button::new("Cancel", MACHINE_CANCEL)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .build(),
            )
            .child(
                Button::new("Save", MACHINE_SAVE)
                    .size(Size::Sm)
                    .disabled(form.server().is_none())
                    .build(),
            ),
    )
}

/// The row of a server: its name over how it is reached, and its buttons when it can be changed.
fn machine_row(name: &str, reach: String, buttons: Option<usize>) -> Node {
    let words = node()
        .styles([flex(), flex_col(), gap(0.5), flex_1(), min_w_px(0)])
        .child(clipped(name, 13, Role::Text))
        .child(clipped(reach, 12, Role::Muted));
    let row = node()
        .styles([
            flex(),
            items_center(),
            justify_between(),
            gap(3),
            flex_none(),
            pm_style::px(3),
            pm_style::py(3),
        ])
        .child(words);
    match buttons {
        Some(index) => row.child(
            node()
                .styles([flex(), items_center(), gap(1), flex_none()])
                .child(
                    Button::new("Edit", FocusId(MACHINE_ROW_BASE + index as u64 * 2))
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build(),
                )
                .child(
                    Button::new("Remove", FocusId(MACHINE_ROW_BASE + index as u64 * 2 + 1))
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build(),
                ),
        ),
        None => row,
    }
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

/// The body of the servers page, `room` logical pixels across: where new projects go, then every server with the form while it is open.
pub(crate) fn body(state: &WorkspaceState, room: f32) -> Vec<Node> {
    let form = state.settings_page.server_form.as_ref();
    let adding = form.is_some_and(|form| form.index.is_none());
    let mut rows = vec![machine_row(
        "This machine",
        "Local · started by the window when none is running".to_owned(),
        None,
    )];
    rows.extend(
        state
            .window
            .servers
            .known
            .iter()
            .enumerate()
            .map(|(index, server)| match form {
                Some(form) if form.index == Some(index) => form_node(form),
                _ => machine_row(&server.name, reach_of(server), Some(index)),
            }),
    );
    if let (Some(form), true) = (form, adding) {
        rows.push(form_node(form));
    }
    vec![
        page_rows(state, Section::Servers, room),
        node()
            .styles([flex(), items_center(), justify_between(), flex_none()])
            .child(crate::settings_servers::heading_only(
                "Servers",
                state.window.servers.known.len() + 1,
            ))
            .child(
                Button::new("Add Server", MACHINE_ADD)
                    .variant(Variant::Outline)
                    .size(Size::Sm)
                    .disabled(adding)
                    .build(),
            ),
        crate::settings_servers::boxed(rows),
    ]
}
