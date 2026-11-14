//! The models page of the settings window: the model slots the harness asks a model for, such as `fast` for summaries and `plan` for plan mode, each with its model, effort and backups, the file a change is saved to, and the presets that set every slot at once.

use pm_harness::{PresetInfo, SlotChange, SlotInfo, SlotScope, SlotSource};
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, border, border_color, flex, flex_1, flex_col, flex_none, flex_wrap, gap,
    items_center, justify_between, min_w_px, pl, px, py, rounded,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::input::Input;
use crate::parts::{clipped, line};
use crate::settings::SETTINGS_PICK_FILTER;
use crate::settings_ids::{BACKUP_CONTROLS, PICKS, PRESET_CONTROLS, SINGLES, SLOT_CONTROLS};
use crate::settings_row::{PICKER_WIDTH, choices, picker, picker_list, picker_shown};
use crate::settings_servers::{boxed, heading_only, note, paragraph, wrapped_note};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Badge, Button};

/// Saves changes to the user's file.
pub const SLOTS_SCOPE_USER: FocusId = SINGLES.at(21);

/// Saves changes to the project's file.
pub const SLOTS_SCOPE_PROJECT: FocusId = SINGLES.at(22);

/// The box a slot of the reader's own is named in.
pub const SLOT_NEW_NAME: FocusId = SINGLES.at(23);

/// Adds the slot named in [`SLOT_NEW_NAME`].
pub const SLOT_ADD: FocusId = SINGLES.at(24);

/// The box a preset is named in before it is saved.
pub const PRESET_NAME: FocusId = SINGLES.at(25);

/// Saves the slots as they stand as the preset named in [`PRESET_NAME`].
pub const PRESET_SAVE: FocusId = SINGLES.at(26);

/// Opens the list of providers whose assignments the page shows.
pub const SLOTS_PROVIDER: FocusId = SINGLES.at(27);

/// What the provider list calls the any-provider assignments.
const ALL_PROVIDERS: &str = "All providers";

/// How wide the provider picker is, in logical pixels.
const PROVIDER_PICKER_WIDTH: f32 = 160.0;

/// How many controls each slot has in [`SLOT_CONTROLS`]: its model picker, its suggestion, its clear button, its backup picker, then one for each effort.
pub const SLOT_COLUMNS: usize = 10;

/// The column of a slot's model picker.
const MODEL_COLUMN: usize = 0;

/// The column of a slot's button that takes the suggested model.
const SUGGEST_COLUMN: usize = 1;

/// The column of a slot's button that clears it.
const CLEAR_COLUMN: usize = 2;

/// The column of a slot's picker that adds a backup.
const BACKUP_COLUMN: usize = 3;

/// The column of a slot's first effort choice.
const EFFORT_COLUMN: usize = 4;

/// How many backups of a slot have controls.
pub const MOST_BACKUPS: usize = 16;

/// How many controls each backup has in [`BACKUP_CONTROLS`]: up, down and remove.
pub const BACKUP_COLUMNS: usize = 3;

/// How many controls each preset has in [`PRESET_CONTROLS`]: apply and delete.
pub const PRESET_COLUMNS: usize = 2;

/// The efforts a selector may end with, after a `:`, lowest first.
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// The effort choices of a slot as they are shown: none, then each of [`EFFORTS`].
const EFFORT_NAMES: [&str; 6] = ["Auto", "Low", "Medium", "High", "X-high", "Max"];

/// The slot that is the conversation's own model.
const MAIN: &str = "main";

/// Which list of models is open on the page, for which row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotPicking {
    /// The model of the slot at this row.
    Model(usize),
    /// A backup to add to the slot at this row.
    Backup(usize),
    /// The provider whose assignments the page shows.
    Provider,
}

/// Whose assignments of the slots the page shows and edits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SlotView {
    /// The open session's provider, or the first one offered when no session is open.
    #[default]
    Auto,
    /// The any-provider assignments, used by every provider that has none of its own.
    Any,
    /// The assignments of the provider of this id, such as `codex`.
    Provider(String),
}

/// One slot as the page shows it for the provider viewed: that provider's assignment, or the any-provider one.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SlotRow {
    /// The slot's name.
    pub name: String,
    /// The model selector the assignment sets; nothing when it sets none.
    pub model: Option<String>,
    /// The assignment's backups, in order.
    pub backups: Vec<String>,
    /// A model suggested when it sets none.
    pub suggested: Option<String>,
    /// Where its model comes from, when the harness said so for this provider.
    pub source: Option<SlotSource>,
    /// What its model picker says when the assignment sets no model: what applies instead.
    pub unset: String,
}

/// What the models page keeps: the slots and presets as the harness told them, the slots added here and not saved yet, where changes go, the list open, the boxes, and why the last change failed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelSlots {
    /// Every slot, as the harness last told them.
    pub slots: Vec<SlotInfo>,
    /// Every preset, by name.
    pub presets: Vec<PresetInfo>,
    /// The slots the reader added and gave no model yet, which the harness does not keep until they are set.
    pub added: Vec<String>,
    /// The file changes are saved to.
    pub scope: SlotScope,
    /// Whose assignments are shown and edited.
    pub view: SlotView,
    /// The list of models open, while one is.
    pub picking: Option<SlotPicking>,
    /// The box a new slot is named in.
    pub new_slot: Field,
    /// The box a new preset is named in.
    pub preset_name: Field,
    /// Why the slots could not be read or changed, when the last attempt failed.
    pub error: Option<String>,
}

impl ModelSlots {
    /// Every slot the page shows, the harness's then the ones added here, as it stands for `provider`, or for any provider when it is nothing; `session` is the provider of the conversation the harness resolved the slots for, when one is open.
    pub fn rows(&self, provider: Option<&str>, session: Option<&str>) -> Vec<SlotRow> {
        self.slots
            .iter()
            .map(|slot| slot_row_of(slot, provider, session))
            .chain(self.added.iter().map(|name| SlotRow {
                name: name.clone(),
                unset: "Same as main".to_owned(),
                ..SlotRow::default()
            }))
            .collect()
    }
}

/// What applies to `slot` when the assignment shown sets no model, for `provider` when it is one, with the conversation of `session`'s provider open: the any-provider model, the conversation's model when it is known, or main.
fn unset_of(slot: &SlotInfo, provider: Option<&str>, session: Option<&str>) -> String {
    let resolved = slot
        .resolved
        .as_deref()
        .filter(|_| slot.inherited && (provider.is_none() || provider == session));
    match (provider.and(slot.model.as_deref()), resolved) {
        (Some(any), _) => format!("{ALL_PROVIDERS} ({})", split_effort(any).0),
        (None, Some(resolved)) if slot.name == MAIN => format!("Conversation's model ({resolved})"),
        (None, Some(resolved)) => format!("Same as main ({resolved})"),
        (None, None) => "Same as main".to_owned(),
    }
}

/// The row of `slot` for `provider`, or for any provider when it is nothing, with the conversation of `session`'s provider open.
fn slot_row_of(slot: &SlotInfo, provider: Option<&str>, session: Option<&str>) -> SlotRow {
    let (model, backups, suggested) = match provider {
        Some(provider) => slot
            .providers
            .iter()
            .find(|held| held.provider == provider)
            .map(|held| {
                (
                    held.model.clone(),
                    held.backups.clone(),
                    held.suggested.clone(),
                )
            })
            .unwrap_or_default(),
        None => (
            slot.model.clone(),
            slot.backups.clone(),
            slot.suggested.clone().filter(|_| slot.model.is_none()),
        ),
    };
    let applies = model.is_some()
        && !slot.inherited
        && slot.resolved == model
        && (provider.is_none() || provider == session);
    SlotRow {
        name: slot.name.clone(),
        source: applies.then_some(slot.source),
        unset: unset_of(slot, provider, session),
        model,
        backups,
        suggested,
    }
}

/// The model `selector` names and the effort it ends with, when it ends with one of [`EFFORTS`].
pub fn split_effort(selector: &str) -> (&str, Option<&str>) {
    match selector.rsplit_once(':') {
        Some((model, effort)) if EFFORTS.contains(&effort) => (model, Some(effort)),
        _ => (selector, None),
    }
}

/// The selector naming `model` with `effort` after it, when there is one.
pub fn with_effort(model: &str, effort: Option<&str>) -> String {
    match effort {
        Some(effort) => format!("{model}:{effort}"),
        None => model.to_owned(),
    }
}

/// Whether `name` can name a slot or a preset: letters, digits, `-` and `_`.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '-' || letter == '_')
}

/// Whether `id` names a box of the page.
pub(crate) fn is_slot_field(id: FocusId) -> bool {
    id == SLOT_NEW_NAME || id == PRESET_NAME
}

/// What Enter in the box `id` of the page presses.
pub(crate) fn slot_submit(id: FocusId) -> FocusId {
    if id == PRESET_NAME {
        PRESET_SAVE
    } else {
        SLOT_ADD
    }
}

impl WorkspaceState {
    /// The state with the slots and presets of `book`, as the harness holds them now; a slot added here that the harness now keeps is no longer held apart.
    pub fn with_model_slots(mut self, book: pm_harness::SlotBook) -> Self {
        let page = &mut self.settings_page.slots;
        page.added
            .retain(|name| !book.slots.iter().any(|slot| slot.name == *name));
        page.slots = book.slots;
        page.presets = book.presets;
        page.error = None;
        self
    }

    /// The state after the slots could not be read or changed, saying why on the page.
    pub fn with_model_slots_failed(mut self, reason: String) -> Self {
        self.settings_page.slots.error = Some(reason);
        self
    }

    /// Asks for the slots and presets as they stand, through the open session's agent when there is one.
    pub(crate) fn ask_model_slots(&mut self) {
        self.outbox.push(Outgoing::ModelSlots {
            session: self.open_index(),
            change: None,
        });
    }

    /// Has the harness make `change`, through the open session's agent when there is one.
    fn change_slots(&mut self, change: SlotChange) {
        self.settings_page.slots.error = None;
        self.outbox.push(Outgoing::ModelSlots {
            session: self.open_index(),
            change: Some(change),
        });
    }

    /// The provider id of the agent at `index`: the harness provider a built-in agent runs on, or a declared server's own id.
    fn agent_provider(&self, index: usize) -> Option<String> {
        let entry = self.settings_servers.agents.get(index)?;
        Some(
            pm_harness::agents()
                .iter()
                .find(|agent| agent.id == entry.id)
                .map_or(entry.id.as_str(), |agent| agent.provider)
                .to_owned(),
        )
    }

    /// The agents whose models a slot can be given, signed in or declared servers, by position, with their provider ids and names.
    fn slot_agents(&self) -> Vec<(usize, String, String)> {
        self.offered_agents()
            .into_iter()
            .filter_map(|index| {
                let entry = self.settings_servers.agents.get(index)?;
                if !entry.server && !self.agent_ready(index) {
                    return None;
                }
                Some((index, self.agent_provider(index)?, entry.name.clone()))
            })
            .collect()
    }

    /// The providers the page's provider list offers, by id and name, each once, in the order the agents are listed.
    pub fn slot_providers(&self) -> Vec<(String, String)> {
        let mut found: Vec<(String, String)> = Vec::new();
        for (_, provider, name) in self.slot_agents() {
            if !found.iter().any(|(held, _)| *held == provider) {
                found.push((provider, name));
            }
        }
        found
    }

    /// The provider of the open session's agent, which the harness resolved the slots for, when a session is open.
    fn session_provider(&self) -> Option<String> {
        self.open()
            .and_then(|session| self.agent_provider(session.agent))
    }

    /// The provider whose assignments the page shows, or nothing for the any-provider ones: the one picked, else the open session's, else the first offered.
    pub fn slot_provider(&self) -> Option<String> {
        match &self.settings_page.slots.view {
            SlotView::Any => None,
            SlotView::Provider(provider) => Some(provider.clone()),
            SlotView::Auto => self.session_provider().or_else(|| {
                self.slot_providers()
                    .into_iter()
                    .next()
                    .map(|(provider, _)| provider)
            }),
        }
    }

    /// Every slot the page shows, for the provider viewed.
    pub fn slot_rows(&self) -> Vec<SlotRow> {
        self.settings_page.slots.rows(
            self.slot_provider().as_deref(),
            self.session_provider().as_deref(),
        )
    }

    /// The choices of the provider list: all providers first, then each provider offered by name.
    fn provider_choices(&self) -> Vec<String> {
        std::iter::once(ALL_PROVIDERS.to_owned())
            .chain(self.slot_providers().into_iter().map(|(_, name)| name))
            .collect()
    }

    /// What the provider picker says: the name of the provider viewed, or all providers.
    fn provider_shown(&self) -> String {
        let viewed = self.slot_provider();
        viewed
            .as_ref()
            .and_then(|provider| {
                self.slot_providers()
                    .into_iter()
                    .find(|(held, _)| held == provider)
                    .map(|(_, name)| name)
                    .or_else(|| Some(provider.clone()))
            })
            .unwrap_or_else(|| ALL_PROVIDERS.to_owned())
    }

    /// The models a slot can be given for the provider viewed: those of its agents, or of every agent signed in and every declared server when all providers are viewed, as `provider/model`, each once.
    pub fn slot_model_choices(&self) -> Vec<String> {
        let viewed = self.slot_provider();
        let mut found: Vec<String> = Vec::new();
        for (index, provider, _) in self.slot_agents() {
            if viewed.as_ref().is_some_and(|viewed| *viewed != provider) {
                continue;
            }
            let models = self
                .agent_settings
                .get(index)
                .and_then(|settings| settings.iter().find(|setting| setting.category == "model"))
                .map(|setting| setting.picks.as_slice())
                .unwrap_or_default();
            for pick in models.iter().filter(|pick| !pick.value.starts_with('@')) {
                let selector = format!("{provider}/{}", pick.value);
                if !found.contains(&selector) {
                    found.push(selector);
                }
            }
        }
        found
    }

    /// Carries out pressing `id` on the models page, and says whether it was something of it.
    pub(crate) fn slots_pressed(&mut self, id: FocusId) -> bool {
        if let Some(picking) = self.settings_page.slots.picking {
            let row = if id == crate::settings::SETTINGS_PICK_FIRST {
                Some(0)
            } else {
                PICKS.index_of(id)
            };
            if let Some(row) = row {
                if picking == SlotPicking::Provider {
                    self.pick_slot_provider(row);
                } else {
                    self.pick_slot_model(picking, row);
                }
                return true;
            }
        }
        match id {
            SLOTS_SCOPE_USER => self.settings_page.slots.scope = SlotScope::User,
            SLOTS_SCOPE_PROJECT => self.settings_page.slots.scope = SlotScope::Project,
            SLOTS_PROVIDER => {
                let page = &mut self.settings_page.slots;
                page.picking =
                    (page.picking != Some(SlotPicking::Provider)).then_some(SlotPicking::Provider);
                self.settings_page.filter = Field::default();
            }
            SLOT_ADD => self.add_slot(),
            PRESET_SAVE => self.save_preset(),
            _ if let Some((row, column)) = SLOT_CONTROLS.cell_of(id, SLOT_COLUMNS) => {
                self.slot_control(row, column);
            }
            _ if let Some((cell, column)) = BACKUP_CONTROLS.cell_of(id, BACKUP_COLUMNS) => {
                self.backup_control(cell / MOST_BACKUPS, cell % MOST_BACKUPS, column);
            }
            _ if let Some((row, column)) = PRESET_CONTROLS.cell_of(id, PRESET_COLUMNS) => {
                self.preset_control(row, column);
            }
            _ => return false,
        }
        true
    }

    /// Carries out the control in `column` of the slot at `row`.
    fn slot_control(&mut self, row: usize, column: usize) {
        let Some(slot) = self.slot_rows().get(row).cloned() else {
            return;
        };
        let scope = self.settings_page.slots.scope;
        match column {
            MODEL_COLUMN | BACKUP_COLUMN => {
                let wanted = if column == MODEL_COLUMN {
                    SlotPicking::Model(row)
                } else {
                    SlotPicking::Backup(row)
                };
                let page = &mut self.settings_page.slots;
                page.picking = (page.picking != Some(wanted)).then_some(wanted);
                self.settings_page.filter = Field::default();
            }
            SUGGEST_COLUMN => {
                if let Some(suggested) = slot.suggested.clone() {
                    self.set_slot(&slot, Some(suggested), slot.backups.clone());
                }
            }
            CLEAR_COLUMN => {
                self.settings_page
                    .slots
                    .added
                    .retain(|name| *name != slot.name);
                if self
                    .settings_page
                    .slots
                    .slots
                    .iter()
                    .any(|held| held.name == slot.name)
                {
                    let provider = self.slot_provider();
                    self.change_slots(SlotChange::Clear {
                        name: slot.name,
                        scope,
                        provider,
                    });
                }
            }
            effort if effort >= EFFORT_COLUMN => {
                let Some(model) = slot.model.as_deref() else {
                    return;
                };
                let effort = effort - EFFORT_COLUMN;
                let (bare, _) = split_effort(model);
                let chosen = effort
                    .checked_sub(1)
                    .and_then(|place| EFFORTS.get(place))
                    .copied();
                self.set_slot(&slot, Some(with_effort(bare, chosen)), slot.backups.clone());
            }
            _ => {}
        }
    }

    /// Carries out the control in `column` of the backup at `place` of the slot at `row`: moves it up or down, or removes it.
    fn backup_control(&mut self, row: usize, place: usize, column: usize) {
        let Some(slot) = self.slot_rows().get(row).cloned() else {
            return;
        };
        if place >= slot.backups.len() {
            return;
        }
        let mut backups = slot.backups.clone();
        match column {
            0 if place > 0 => backups.swap(place, place - 1),
            1 if place + 1 < backups.len() => backups.swap(place, place + 1),
            2 => {
                backups.remove(place);
            }
            _ => return,
        }
        self.set_slot(&slot, slot.model.clone(), backups);
    }

    /// Carries out the control in `column` of the preset at `row`: applies it to the file changes go to, or deletes it.
    fn preset_control(&mut self, row: usize, column: usize) {
        let Some(name) = self
            .settings_page
            .slots
            .presets
            .get(row)
            .map(|preset| preset.name.clone())
        else {
            return;
        };
        let change = if column == 0 {
            SlotChange::ApplyPreset {
                name,
                scope: self.settings_page.slots.scope,
            }
        } else {
            SlotChange::DeletePreset { name }
        };
        self.change_slots(change);
    }

    /// Takes the model at `row` of the open list, as narrowed, for what `picking` is: the slot's model, keeping its effort, or a backup added after the others.
    fn pick_slot_model(&mut self, picking: SlotPicking, row: usize) {
        let options = self.slot_model_choices();
        let shown = picker_shown(&options, &self.settings_page.filter);
        self.settings_page.slots.picking = None;
        self.settings_page.filter = Field::default();
        let Some(chosen) = shown
            .get(row)
            .and_then(|place| options.get(*place))
            .cloned()
        else {
            return;
        };
        let (SlotPicking::Model(index) | SlotPicking::Backup(index)) = picking else {
            return;
        };
        let Some(slot) = self.slot_rows().get(index).cloned() else {
            return;
        };
        match picking {
            SlotPicking::Model(_) => {
                let effort = slot
                    .model
                    .as_deref()
                    .and_then(|model| split_effort(model).1);
                self.set_slot(
                    &slot,
                    Some(with_effort(&chosen, effort)),
                    slot.backups.clone(),
                );
            }
            SlotPicking::Backup(_) => {
                let mut backups = slot.backups.clone();
                if !backups.contains(&chosen) {
                    backups.push(chosen);
                }
                self.set_slot(&slot, slot.model.clone(), backups);
            }
            SlotPicking::Provider => {}
        }
    }

    /// Shows the assignments of the provider at `row` of the provider list, as narrowed: all providers first, then each provider offered.
    fn pick_slot_provider(&mut self, row: usize) {
        let options = self.provider_choices();
        let shown = picker_shown(&options, &self.settings_page.filter);
        self.settings_page.slots.picking = None;
        self.settings_page.filter = Field::default();
        let Some(place) = shown.get(row).copied() else {
            return;
        };
        self.settings_page.slots.view = match place.checked_sub(1) {
            None => SlotView::Any,
            Some(place) => match self.slot_providers().into_iter().nth(place) {
                Some((provider, _)) => SlotView::Provider(provider),
                None => return,
            },
        };
    }

    /// Has the assignment of `slot` for the provider viewed set to `model` and `backups` in the file changes go to.
    fn set_slot(&mut self, slot: &SlotRow, model: Option<String>, backups: Vec<String>) {
        let scope = self.settings_page.slots.scope;
        let provider = self.slot_provider();
        self.change_slots(SlotChange::Set {
            name: slot.name.clone(),
            scope,
            model,
            backups,
            provider,
        });
    }

    /// Adds the slot named in the box as a row to give a model, when the name is valid and not taken, and empties the box.
    fn add_slot(&mut self) {
        let taken = self.slot_rows();
        let page = &mut self.settings_page.slots;
        let name = page.new_slot.text().trim().to_owned();
        if !valid_name(&name) || taken.iter().any(|slot| slot.name == name) {
            return;
        }
        page.added.push(name);
        page.new_slot = Field::default();
    }

    /// Saves the slots as they stand as the preset named in the box, when the name is valid, and empties the box.
    fn save_preset(&mut self) {
        let name = self
            .settings_page
            .slots
            .preset_name
            .text()
            .trim()
            .to_owned();
        if !valid_name(&name) {
            return;
        }
        self.settings_page.slots.preset_name = Field::default();
        self.change_slots(SlotChange::SavePreset { name, effort: None });
    }

    /// Writes `edit` into the box `id` of the page.
    pub(crate) fn slot_edited(&mut self, id: FocusId, edit: Edit) {
        let page = &mut self.settings_page.slots;
        match id {
            SLOT_NEW_NAME => page.new_slot.apply(edit),
            PRESET_NAME => page.preset_name.apply(edit),
            _ => {}
        }
    }

    /// The box `id` of the page.
    pub(crate) fn slot_field(&self, id: FocusId) -> Option<&Field> {
        let page = &self.settings_page.slots;
        match id {
            SLOT_NEW_NAME => Some(&page.new_slot),
            PRESET_NAME => Some(&page.preset_name),
            _ => None,
        }
    }
}

/// What the page calls where a slot's model comes from.
fn source_name(source: SlotSource) -> &'static str {
    match source {
        SlotSource::Environment => "Environment",
        SlotSource::Project => "Project",
        SlotSource::User => "User",
        SlotSource::Main => "Same as main",
    }
}

/// What the model picker of `slot` says: its model without its effort, which its own choices show, or what applies when it sets none.
fn model_shown(slot: &SlotRow) -> String {
    match &slot.model {
        Some(model) => split_effort(model).0.to_owned(),
        None => slot.unset.clone(),
    }
}

/// The list of models open under the slot at `row`, when the one open is for it as `kind` says.
fn open_list(state: &WorkspaceState, row: usize, kind: SlotPicking) -> Option<Node> {
    (state.settings_page.slots.picking == Some(kind)).then(|| {
        let options = state.slot_model_choices();
        let picked = match kind {
            SlotPicking::Model(_) => state.slot_rows().get(row).and_then(|slot| {
                let model = split_effort(slot.model.as_deref()?).0;
                options.iter().position(|option| option == model)
            }),
            SlotPicking::Backup(_) | SlotPicking::Provider => None,
        };
        if options.is_empty() {
            return note(
                "No signed-in agent or declared server of this provider has listed its models yet.",
            );
        }
        picker_list(
            &options,
            picked,
            (&state.settings_page.filter, SETTINGS_PICK_FILTER),
        )
    })
}

/// How wide the button that opens the list of backups to add is, in logical pixels.
const BACKUP_PICKER_WIDTH: f32 = 132.0;

/// The backups of the slot at `row`, in order, side by side, each with buttons that move it up and down and remove it, then the picker that adds one.
fn backups_node(state: &WorkspaceState, row: usize, slot: &SlotRow) -> Node {
    let cell = |place: usize, column: usize| {
        let index = row.checked_mul(MOST_BACKUPS)?.checked_add(place)?;
        (place < MOST_BACKUPS)
            .then(|| BACKUP_CONTROLS.cell(index, column, BACKUP_COLUMNS))
            .flatten()
    };
    let icon = |icon: IconName, id: Option<FocusId>, disabled: bool| {
        id.map(|id| {
            Button::icon(icon, id)
                .size(Size::Sm)
                .tone(Role::Muted)
                .disabled(disabled)
                .build()
        })
    };
    let last = slot.backups.len().saturating_sub(1);
    let chips = slot.backups.iter().enumerate().map(|(place, backup)| {
        node()
            .styles([
                flex(),
                items_center(),
                gap(0.5),
                flex_none(),
                pl(2),
                rounded(Radius::Md),
                border(),
                border_color(Role::Border),
            ])
            .child(line(format!("{}", place + 1), 11, 1.4, Role::Subtle))
            .child(line(backup.clone(), 12, 1.4, Role::Text).styles([pm_style::font_mono()]))
            .children(icon(IconName::ArrowUp, cell(place, 0), place == 0))
            .children(icon(IconName::ArrowDown, cell(place, 1), place == last))
            .children(icon(IconName::Close, cell(place, 2), false))
    });
    let open = state.settings_page.slots.picking == Some(SlotPicking::Backup(row));
    node()
        .styles([flex(), flex_col(), gap(1)])
        .child(
            node()
                .styles([flex(), items_center(), gap(2), flex_wrap()])
                .child(line("Backups", 12, 1.4, Role::Muted))
                .children(chips)
                .children(
                    SLOT_CONTROLS
                        .cell(row, BACKUP_COLUMN, SLOT_COLUMNS)
                        .map(|id| picker("Add backup", BACKUP_PICKER_WIDTH, id, open)),
                ),
        )
        .children(open_list(state, row, SlotPicking::Backup(row)))
}

/// The row of the slot at `row`: its name, where its model comes from and what it is for, its suggestion and clear buttons, its model with its effort, and its backups.
fn slot_row(state: &WorkspaceState, row: usize, slot: &SlotRow) -> Node {
    let id = |column: usize| SLOT_CONTROLS.cell(row, column, SLOT_COLUMNS);
    let source = slot
        .source
        .filter(|source| *source != SlotSource::Main)
        .map(|source| Badge::new(source_name(source)).build());
    let suggest = slot
        .suggested
        .as_ref()
        .filter(|_| slot.model.is_none())
        .and_then(|suggested| {
            id(SUGGEST_COLUMN).map(|button| {
                Button::new(format!("Use suggested: {suggested}"), button)
                    .variant(Variant::Outline)
                    .size(Size::Sm)
                    .build()
            })
        });
    let clear = (slot.model.is_some()
        || !slot.backups.is_empty()
        || state.settings_page.slots.added.contains(&slot.name))
    .then(|| id(CLEAR_COLUMN))
    .flatten()
    .map(|button| {
        Button::new("Clear", button)
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .build()
    });
    let heading = node()
        .styles([
            flex(),
            items_center(),
            justify_between(),
            gap(2),
            flex_wrap(),
        ])
        .child(
            node()
                .styles([flex(), items_center(), gap(2), min_w_px(0)])
                .child(
                    line(format!("@{}", slot.name), 13, 1.4, Role::Text)
                        .styles([pm_style::font_mono(), pm_style::font_medium()]),
                )
                .children(source),
        )
        .child(
            node()
                .styles([flex(), items_center(), gap(1), flex_none()])
                .children(suggest)
                .children(clear),
        );
    let open = state.settings_page.slots.picking == Some(SlotPicking::Model(row));
    let effort = slot.model.as_deref().map(|model| {
        let picked = split_effort(model)
            .1
            .and_then(|effort| EFFORTS.iter().position(|held| *held == effort))
            .map_or(0, |place| place + 1);
        choices(&EFFORT_NAMES, picked, |option| id(EFFORT_COLUMN + option))
    });
    let model = node()
        .styles([flex(), items_center(), gap(2), flex_wrap()])
        .children(
            id(MODEL_COLUMN)
                .map(|button| picker(&model_shown(slot), PICKER_WIDTH * 1.4, button, open)),
        )
        .children(effort);
    node()
        .styles([flex(), flex_col(), gap(2), px(3), py(3)])
        .child(heading)
        .child(clipped(
            pm_harness::slot_purpose(&slot.name),
            12,
            Role::Muted,
        ))
        .child(model)
        .children(open_list(state, row, SlotPicking::Model(row)))
        .child(backups_node(state, row, slot))
}

/// What `preset` sets, in a line: each slot's model, and the effort.
fn preset_summary(preset: &PresetInfo) -> String {
    let mut parts: Vec<String> = preset
        .slots
        .iter()
        .map(|slot| {
            let mut said = match &slot.model {
                Some(model) => format!("@{} {model}", slot.name),
                None => format!("@{} same as main", slot.name),
            };
            for held in &slot.providers {
                if let Some(model) = &held.model {
                    said.push_str(&format!(", {} {model}", held.provider));
                }
            }
            said
        })
        .collect();
    if let Some(effort) = &preset.effort {
        parts.push(format!("effort {effort}"));
    }
    if parts.is_empty() {
        "Sets no slot".to_owned()
    } else {
        parts.join(" · ")
    }
}

/// The row of the preset at `row`: its name and what it sets, with its apply and delete buttons.
fn preset_row(row: usize, preset: &PresetInfo) -> Node {
    let id = |column: usize| PRESET_CONTROLS.cell(row, column, PRESET_COLUMNS);
    node()
        .styles([
            flex(),
            items_center(),
            justify_between(),
            gap(3),
            px(3),
            py(3),
        ])
        .child(
            node()
                .styles([flex(), flex_col(), gap(0.5), flex_1(), min_w_px(0)])
                .child(
                    line(preset.name.clone(), 13, 1.4, Role::Text)
                        .styles([pm_style::font_medium()]),
                )
                .child(
                    clipped(preset_summary(preset), 12, Role::Muted)
                        .styles([pm_style::font_mono()]),
                ),
        )
        .child(
            node()
                .styles([flex(), items_center(), gap(1), flex_none()])
                .children(id(0).map(|button| {
                    Button::new("Apply", button)
                        .variant(Variant::Outline)
                        .size(Size::Sm)
                        .build()
                }))
                .children(id(1).map(|button| {
                    Button::new("Delete", button)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build()
                })),
        )
}

/// A box and the button beside it that hands in what is written in it.
fn entry(field: &Field, id: FocusId, placeholder: &str, button: (&str, FocusId)) -> Node {
    node()
        .styles([flex(), items_center(), gap(2)])
        .child(
            node().styles([flex_1(), min_w_px(0)]).child(
                Input::new(field, id)
                    .size(Size::Sm)
                    .placeholder(placeholder)
                    .build(),
            ),
        )
        .child(
            Button::new(button.0, button.1)
                .variant(Variant::Outline)
                .size(Size::Sm)
                .disabled(!valid_name(field.text().trim()))
                .build(),
        )
}

/// What the page says about itself under its heading.
pub(crate) const SUMMARY: &str = "The models agents ask for each kind of work, for each provider. A provider's slot left unset uses the one set for all providers, else the conversation's own model. Name a slot as @name wherever a model is named, such as @fast in an agent definition.";

/// The body of the models page, `room` logical pixels across: what it is for, where changes go, every slot, a box to add one, and the presets.
pub(crate) fn body(state: &WorkspaceState, page: Node, room: f32) -> Node {
    let slots = &state.settings_page.slots;
    let scope = choices(
        &["User", "Project"],
        usize::from(slots.scope == SlotScope::Project),
        |option| [SLOTS_SCOPE_USER, SLOTS_SCOPE_PROJECT].get(option).copied(),
    );
    let provider_open = slots.picking == Some(SlotPicking::Provider);
    let provider = node()
        .styles([flex(), items_center(), gap(2), flex_wrap(), flex_none()])
        .child(line("Provider", 12, 1.4, Role::Muted))
        .child(picker(
            &state.provider_shown(),
            PROVIDER_PICKER_WIDTH,
            SLOTS_PROVIDER,
            provider_open,
        ));
    let provider_list = provider_open.then(|| {
        let options = state.provider_choices();
        let picked = match state.slot_provider() {
            None => Some(0),
            Some(viewed) => state
                .slot_providers()
                .iter()
                .position(|(held, _)| *held == viewed)
                .map(|place| place + 1),
        };
        picker_list(
            &options,
            picked,
            (&state.settings_page.filter, SETTINGS_PICK_FILTER),
        )
    });
    let rows: Vec<Node> = state
        .slot_rows()
        .iter()
        .enumerate()
        .map(|(row, slot)| slot_row(state, row, slot))
        .collect();
    let mut page =
        page.child(paragraph(SUMMARY, room))
            .child(provider)
            .children(provider_list)
            .children(
                slots.error.as_deref().map(|reason| {
                    wrapped_note(&format!("Could not change the slots: {reason}"), room)
                }),
            )
            .child(
                node()
                    .styles([
                        flex(),
                        items_center(),
                        justify_between(),
                        gap(2),
                        flex_wrap(),
                        flex_none(),
                    ])
                    .child(heading_only("Slots", rows.len()))
                    .child(
                        node()
                            .styles([flex(), items_center(), gap(2)])
                            .child(line("Save changes to", 12, 1.4, Role::Muted))
                            .child(scope),
                    ),
            );
    page = if rows.is_empty() {
        page.child(note("The agent has not told its slots yet."))
    } else {
        page.child(boxed(rows))
    };
    let presets: Vec<Node> = slots
        .presets
        .iter()
        .enumerate()
        .map(|(row, preset)| preset_row(row, preset))
        .collect();
    page = page
        .child(entry(
            &slots.new_slot,
            SLOT_NEW_NAME,
            "Add a slot of your own, e.g. docs",
            ("Add Slot", SLOT_ADD),
        ))
        .child(heading_only("Presets", presets.len()));
    page = if presets.is_empty() {
        page.child(note("No presets are saved."))
    } else {
        page.child(boxed(presets))
    };
    page.child(entry(
        &slots.preset_name,
        PRESET_NAME,
        "Name the slots as they stand, e.g. cheap",
        ("Save Preset", PRESET_SAVE),
    ))
}

/// How many slots set a model of their own, in words, for the page's card on the Agents overview.
pub(crate) fn count(state: &WorkspaceState) -> String {
    let set = state
        .settings_page
        .slots
        .slots
        .iter()
        .filter(|slot| !slot.inherited)
        .count();
    format!("{set} set")
}

#[cfg(test)]
/// Tests for the models page: setting and clearing a slot, its effort and backups, its suggestion, slots of the reader's own, and presets.
mod tests {
    use super::*;
    use crate::settings::Section;

    /// A slot as the harness tells it: set to `model` from the user's file, or inheriting with `suggested` when there is none.
    fn slot(name: &str, model: Option<&str>, suggested: Option<&str>) -> SlotInfo {
        SlotInfo {
            name: name.to_owned(),
            model: model.map(str::to_owned),
            backups: Vec::new(),
            resolved: Some(model.unwrap_or("claude-opus-4-5").to_owned()),
            source: if model.is_some() {
                SlotSource::User
            } else {
                SlotSource::Main
            },
            inherited: model.is_none(),
            suggested: suggested.map(str::to_owned),
            providers: Vec::new(),
        }
    }

    /// The state with the settings window open on the models page, holding a set `fast` slot with two backups, an inheriting `vision` slot with a suggestion, and one preset, with nothing waiting to be sent.
    fn open() -> WorkspaceState {
        let mut fast = slot("fast", Some("claude/haiku:low"), None);
        fast.backups = vec!["openai/gpt-5-mini".to_owned(), "ollama/llama3".to_owned()];
        let book = pm_harness::SlotBook {
            slots: vec![
                slot("main", None, None),
                fast,
                slot("vision", None, Some("openai/gpt-5-mini")),
            ],
            presets: vec![PresetInfo {
                name: "cheap".to_owned(),
                slots: Vec::new(),
                effort: Some("low".to_owned()),
            }],
        };
        let mut state = WorkspaceState::new(&[])
            .with_settings()
            .settings_pressed(Section::ModelSlots.id())
            .with_model_slots(book);
        state.take_outbox();
        state
    }

    /// The one change sent since the last look, failing when there is none or more than one.
    fn sent(state: &mut WorkspaceState) -> SlotChange {
        let sent = state.take_outbox();
        let changes: Vec<SlotChange> = sent
            .into_iter()
            .filter_map(|outgoing| match outgoing {
                Outgoing::ModelSlots {
                    change: Some(change),
                    ..
                } => Some(change),
                _ => None,
            })
            .collect();
        assert_eq!(changes.len(), 1, "{changes:?}");
        changes.into_iter().next().unwrap()
    }

    /// Opening the page asks for the slots.
    #[test]
    fn opening_the_page_asks_for_the_slots() {
        let mut state = WorkspaceState::new(&[]).with_settings();
        state.take_outbox();
        let mut state = state.settings_pressed(Section::ModelSlots.id());
        assert!(
            state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::ModelSlots { change: None, .. }))
        );
    }

    /// Picking an effort keeps the slot's model and backups and saves to the scope chosen.
    #[test]
    fn an_effort_is_set_on_the_model() {
        let mut state = open()
            .settings_pressed(SLOTS_SCOPE_PROJECT)
            .settings_pressed(
                SLOT_CONTROLS
                    .cell(1, EFFORT_COLUMN + 3, SLOT_COLUMNS)
                    .unwrap(),
            );
        assert_eq!(
            sent(&mut state),
            SlotChange::Set {
                name: "fast".to_owned(),
                scope: SlotScope::Project,
                model: Some("claude/haiku:high".to_owned()),
                backups: vec!["openai/gpt-5-mini".to_owned(), "ollama/llama3".to_owned()],
                provider: None,
            }
        );
    }

    /// Clearing a slot takes it out of the file chosen, so it inherits again.
    #[test]
    fn clearing_a_slot_inherits_again() {
        let mut state =
            open().settings_pressed(SLOT_CONTROLS.cell(1, CLEAR_COLUMN, SLOT_COLUMNS).unwrap());
        assert_eq!(
            sent(&mut state),
            SlotChange::Clear {
                name: "fast".to_owned(),
                scope: SlotScope::User,
                provider: None,
            }
        );
    }

    /// Taking the suggestion sets the slot to the suggested model.
    #[test]
    fn the_suggestion_is_taken() {
        let mut state =
            open().settings_pressed(SLOT_CONTROLS.cell(2, SUGGEST_COLUMN, SLOT_COLUMNS).unwrap());
        assert_eq!(
            sent(&mut state),
            SlotChange::Set {
                name: "vision".to_owned(),
                scope: SlotScope::User,
                model: Some("openai/gpt-5-mini".to_owned()),
                backups: Vec::new(),
                provider: None,
            }
        );
    }

    /// A backup moves down, and another is removed, each keeping the slot's model.
    #[test]
    fn backups_are_reordered_and_removed() {
        let backup = |place: usize, column: usize| {
            BACKUP_CONTROLS
                .cell(MOST_BACKUPS + place, column, BACKUP_COLUMNS)
                .unwrap()
        };
        let mut state = open().settings_pressed(backup(0, 1));
        let SlotChange::Set { backups, model, .. } = sent(&mut state) else {
            panic!("the slot was not set");
        };
        assert_eq!(backups, ["ollama/llama3", "openai/gpt-5-mini"]);
        assert_eq!(model.as_deref(), Some("claude/haiku:low"));
        let mut state = state.settings_pressed(backup(0, 2));
        let SlotChange::Set { backups, .. } = sent(&mut state) else {
            panic!("the slot was not set");
        };
        assert_eq!(backups, ["ollama/llama3"]);
        let mut state = state.settings_pressed(backup(0, 0));
        assert!(state.take_outbox().is_empty());
    }

    /// A declared server of id `id` at a local address.
    fn server(id: &str) -> pm_harness::ModelServer {
        pm_harness::ModelServer {
            id: id.to_owned(),
            label: None,
            base_url: "http://localhost:8000/v1".to_owned(),
            wire: pm_harness::ServerWire::ChatCompletions,
            api_key_env: Vec::new(),
            api_key: None,
            has_api_key: false,
            headers: Vec::new(),
            default_model: None,
            models: Vec::new(),
            replay_reasoning: false,
        }
    }

    /// `state` with a declared server for each of `servers`, by id, each listing its one model, and nothing waiting to be sent.
    fn with_servers(state: WorkspaceState, servers: &[(&str, &str)]) -> WorkspaceState {
        let mut state =
            state.with_model_servers(servers.iter().map(|(id, _)| server(id)).collect());
        for (id, model) in servers {
            let place = state.agent_position(id).unwrap();
            state = state.with_discovery(
                place,
                pm_harness::Discovery {
                    settings: vec![pm_harness::Setting {
                        id: "model".to_owned(),
                        category: "model".to_owned(),
                        name: "Model".to_owned(),
                        current: (*model).to_owned(),
                        picks: vec![pm_harness::Pick {
                            value: (*model).to_owned(),
                            name: (*model).to_owned(),
                            description: String::new(),
                        }],
                    }],
                    commands: Vec::new(),
                },
            );
        }
        state.take_outbox();
        state
    }

    /// `state` with a declared server, `gateway`, that lists the model `qwen3-coder`, and nothing waiting to be sent.
    fn with_server_models(state: WorkspaceState) -> WorkspaceState {
        with_servers(state, &[("gateway", "qwen3-coder")])
    }

    /// `state` with the provider at `place` of the provider list picked: 0 for all providers, then each provider offered.
    fn viewing(state: WorkspaceState, place: u64) -> WorkspaceState {
        let state = state.settings_pressed(SLOTS_PROVIDER);
        assert_eq!(
            state.settings_page.slots.picking,
            Some(SlotPicking::Provider)
        );
        state.settings_pressed(PICKS.at(place))
    }

    /// A backup is added from the list of models, which lists the models of the agents ready, after the others.
    #[test]
    fn a_backup_is_added_from_the_list() {
        let state = viewing(with_server_models(open()), 0);
        assert_eq!(state.settings_page.slots.view, SlotView::Any);
        let state =
            state.settings_pressed(SLOT_CONTROLS.cell(1, BACKUP_COLUMN, SLOT_COLUMNS).unwrap());
        assert_eq!(
            state.settings_page.slots.picking,
            Some(SlotPicking::Backup(1))
        );
        let mut state = state.settings_pressed(PICKS.at(0));
        let SlotChange::Set { backups, .. } = sent(&mut state) else {
            panic!("the slot was not set");
        };
        assert_eq!(
            backups,
            ["openai/gpt-5-mini", "ollama/llama3", "gateway/qwen3-coder"]
        );
        assert_eq!(state.settings_page.slots.picking, None);
    }

    /// A slot of the reader's own shows until it is set, and a name that is not valid or taken is refused.
    #[test]
    fn a_slot_of_ones_own_is_added() {
        let state = open().edited(SLOT_NEW_NAME, Edit::Paste("docs".to_owned()));
        let state = state.settings_pressed(SLOT_ADD);
        assert_eq!(state.slot_rows().last().unwrap().name, "docs");
        let state = state
            .edited(SLOT_NEW_NAME, Edit::Paste("fast".to_owned()))
            .settings_pressed(SLOT_ADD);
        assert_eq!(state.slot_rows().len(), 4);
        let mut book = pm_harness::SlotBook {
            slots: state.settings_page.slots.slots.clone(),
            presets: Vec::new(),
        };
        book.slots.push(slot("docs", Some("claude/sonnet"), None));
        let state = state.with_model_slots(book);
        assert!(state.settings_page.slots.added.is_empty());
        assert_eq!(state.slot_rows().len(), 4);
    }

    /// Presets are saved under the name written, applied to the file chosen and deleted.
    #[test]
    fn presets_are_saved_applied_and_deleted() {
        let mut state = open()
            .edited(PRESET_NAME, Edit::Paste("night".to_owned()))
            .settings_pressed(PRESET_SAVE);
        assert_eq!(
            sent(&mut state),
            SlotChange::SavePreset {
                name: "night".to_owned(),
                effort: None,
            }
        );
        let mut state = state
            .settings_pressed(SLOTS_SCOPE_PROJECT)
            .settings_pressed(PRESET_CONTROLS.cell(0, 0, PRESET_COLUMNS).unwrap());
        assert_eq!(
            sent(&mut state),
            SlotChange::ApplyPreset {
                name: "cheap".to_owned(),
                scope: SlotScope::Project,
            }
        );
        let mut state = state.settings_pressed(PRESET_CONTROLS.cell(0, 1, PRESET_COLUMNS).unwrap());
        assert_eq!(
            sent(&mut state),
            SlotChange::DeletePreset {
                name: "cheap".to_owned()
            }
        );
    }

    /// A failure is shown on the page until the slots are told again.
    #[test]
    fn a_failure_is_shown_until_the_slots_come() {
        let state = open().with_model_slots_failed("no trusted project".to_owned());
        assert_eq!(
            state.settings_page.slots.error.as_deref(),
            Some("no trusted project")
        );
        let state = state.with_model_slots(pm_harness::SlotBook::default());
        assert!(state.settings_page.slots.error.is_none());
    }

    /// A selector's effort is read only when it is one, so an id with a colon stays whole.
    #[test]
    fn efforts_are_split_off_selectors() {
        assert_eq!(
            split_effort("claude/haiku:low"),
            ("claude/haiku", Some("low"))
        );
        assert_eq!(split_effort("ollama/llama3:8b"), ("ollama/llama3:8b", None));
        assert_eq!(with_effort("a", Some("max")), "a:max");
    }

    /// The page shows the first provider offered while no session is open; switching providers shows and edits that provider's assignments, lists only its models, says what applies instead when it sets none, and all providers edits the any-provider assignment.
    #[test]
    fn switching_providers_shows_their_assignments() {
        let mut state = open();
        let fast = &mut state.settings_page.slots.slots[1];
        fast.providers = vec![
            pm_harness::SlotProvider {
                provider: "gateway".to_owned(),
                model: Some("gateway/qwen3-coder:high".to_owned()),
                backups: vec!["gateway/qwen3-small".to_owned()],
                suggested: None,
            },
            pm_harness::SlotProvider {
                provider: "box".to_owned(),
                model: None,
                backups: Vec::new(),
                suggested: Some("box/llama3".to_owned()),
            },
        ];
        let state = with_servers(state, &[("gateway", "qwen3-coder"), ("box", "llama3")]);
        assert_eq!(state.settings_page.slots.view, SlotView::Auto);
        assert_eq!(state.slot_provider().as_deref(), Some("gateway"));
        let rows = state.slot_rows();
        assert_eq!(rows[1].model.as_deref(), Some("gateway/qwen3-coder:high"));
        assert_eq!(rows[1].backups, ["gateway/qwen3-small"]);
        assert_eq!(rows[2].unset, "Same as main");
        assert_eq!(state.slot_model_choices(), ["gateway/qwen3-coder"]);

        let state = viewing(state, 2);
        assert_eq!(
            state.settings_page.slots.view,
            SlotView::Provider("box".to_owned())
        );
        assert_eq!(state.provider_shown(), "box");
        let rows = state.slot_rows();
        assert_eq!(rows[1].model, None);
        assert!(rows[1].backups.is_empty());
        assert_eq!(rows[1].unset, "All providers (claude/haiku)");
        assert_eq!(rows[1].suggested.as_deref(), Some("box/llama3"));
        assert_eq!(model_shown(&rows[0]), "Same as main");
        assert_eq!(state.slot_model_choices(), ["box/llama3"]);
        let mut state =
            state.settings_pressed(SLOT_CONTROLS.cell(1, SUGGEST_COLUMN, SLOT_COLUMNS).unwrap());
        assert_eq!(
            sent(&mut state),
            SlotChange::Set {
                name: "fast".to_owned(),
                scope: SlotScope::User,
                model: Some("box/llama3".to_owned()),
                backups: Vec::new(),
                provider: Some("box".to_owned()),
            }
        );

        let state = viewing(state, 0);
        assert_eq!(state.slot_provider(), None);
        assert_eq!(state.provider_shown().as_str(), ALL_PROVIDERS);
        assert_eq!(state.slot_model_choices().len(), 2);
        let mut state =
            state.settings_pressed(SLOT_CONTROLS.cell(1, CLEAR_COLUMN, SLOT_COLUMNS).unwrap());
        assert_eq!(
            sent(&mut state),
            SlotChange::Clear {
                name: "fast".to_owned(),
                scope: SlotScope::User,
                provider: None,
            }
        );
    }
}
