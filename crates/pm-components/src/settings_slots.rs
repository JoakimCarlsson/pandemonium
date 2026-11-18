//! The model slots of the settings window: the models page, with the job slots the harness asks a model for, such as `fast` for summaries and `plan` for plan mode, each with its model, effort and backups, the file a change is saved to, and the presets that set every slot at once; and what the orchestration page shares with it, the work categories the agent hands work to in orchestrator mode, each also with its description and prompt addition.

use crate::kit::{
    Flex, FocusId, Hovered, IconName, Radius, Role, Side, Styled, Type, column, label, label_with,
    row, wrapping_with,
};
use pm_field::{Edit, Field};
use pm_harness::{PresetInfo, SlotChange, SlotInfo, SlotScope, SlotSource};
use rustle::Modifier;

use crate::button::Variant;
use crate::input::{self, Input};
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

/// The box a category of the reader's own is named in, on the orchestration page.
pub const CATEGORY_NEW_NAME: FocusId = SINGLES.at(98);

/// Adds the category named in [`CATEGORY_NEW_NAME`].
pub const CATEGORY_ADD: FocusId = SINGLES.at(99);

/// What the provider list calls the any-provider assignments.
const ALL_PROVIDERS: &str = "All providers";

/// How wide the provider picker is, in logical pixels.
const PROVIDER_PICKER_WIDTH: f32 = 160.0;

/// How many controls each slot has in [`SLOT_CONTROLS`]: its model picker, its suggestion, its clear button, its backup picker, one for each effort, then a category's description box, its prompt box, what Enter in either presses, and its card on the orchestration page.
pub const SLOT_COLUMNS: usize = 14;

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

/// The column of a category's box for what it is for.
pub(crate) const DESCRIPTION_COLUMN: usize = EFFORT_COLUMN + EFFORT_NAMES.len();

/// The column of a category's box for what it adds to a subagent's system prompt.
const PROMPT_COLUMN: usize = DESCRIPTION_COLUMN + 1;

/// The column Enter in either box of a category presses, which saves what is written in them.
const DESCRIBE_COLUMN: usize = PROMPT_COLUMN + 1;

/// The column of a category's card on the orchestration page, which opens and closes its editor.
const CARD_COLUMN: usize = DESCRIBE_COLUMN + 1;

/// How many lines tall a category's prompt box is at least.
const PROMPT_ROWS: u32 = 3;

/// How much narrower than the page the boxes of a row are, in logical pixels: the row's padding and the outline around the rows.
const ROW_INSET: f32 = 26.0;

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
    /// Whether it is a work category, which the agent picks from in orchestrator mode, and not one of the job slots.
    pub category: bool,
    /// What kind of work it is for, as the harness holds it, when it says.
    pub description: Option<String>,
}

/// What is written in the boxes of one category: what it is for and what it adds to a subagent's system prompt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotNotes {
    /// The category's name.
    pub name: String,
    /// The box for what kind of work the category is for, on one line.
    pub description: Field,
    /// The box for the text added to the system prompt of a subagent started in the category, on several lines.
    pub prompt: Field,
    /// Whether either box was changed since it was last saved.
    pub edited: bool,
}

impl SlotNotes {
    /// The boxes of the category `name`, holding `description` and `prompt` as the harness holds them.
    fn told(name: &str, description: Option<&str>, prompt: Option<&str>) -> Self {
        Self {
            name: name.to_owned(),
            description: Field::new(description.unwrap_or_default()),
            prompt: Field::new(prompt.unwrap_or_default()).multiline(true),
            edited: false,
        }
    }

    /// Whether the boxes hold `description` and `prompt`.
    fn holds(&self, description: Option<&str>, prompt: Option<&str>) -> bool {
        self.description.text() == description.unwrap_or_default()
            && self.prompt.text() == prompt.unwrap_or_default()
    }
}

/// What is written in `field` with the space around it taken off, or nothing when that is empty.
fn written(field: &Field) -> Option<String> {
    Some(field.text().trim().to_owned()).filter(|text| !text.is_empty())
}

/// What the models and orchestration pages keep: the slots and presets as the harness told them, the slots added here and not saved yet, where changes go, the list open, the category whose editor is open, the boxes, and why the last change failed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelSlots {
    /// Every slot, as the harness last told them.
    pub slots: Vec<SlotInfo>,
    /// Every preset, by name.
    pub presets: Vec<PresetInfo>,
    /// The categories the reader added on the orchestration page and gave no model yet, which the harness does not keep until they are set or described.
    pub added: Vec<String>,
    /// The slots the reader added on the models page and gave no model yet, which the harness does not keep until they are set; the models page lists them until then.
    pub plain: Vec<String>,
    /// The boxes of every category, in the order the orchestration page shows them.
    pub notes: Vec<SlotNotes>,
    /// The category whose editor is open on the orchestration page, by name, while one is.
    pub expanded: Option<String>,
    /// The file changes are saved to.
    pub scope: SlotScope,
    /// Whose assignments are shown and edited.
    pub view: SlotView,
    /// The list of models open, while one is.
    pub picking: Option<SlotPicking>,
    /// The box a new slot is named in, on the models page.
    pub new_slot: Field,
    /// The box a new category is named in, on the orchestration page.
    pub new_category: Field,
    /// The box a new preset is named in.
    pub preset_name: Field,
    /// Why the slots could not be read or changed, when the last attempt failed.
    pub error: Option<String>,
}

impl ModelSlots {
    /// Every slot the pages show, the harness's, then the categories added here, then the slots added on the models page, as it stands for `provider`, or for any provider when it is nothing; `session` is the provider of the conversation the harness resolved the slots for, when one is open.
    pub fn rows(&self, provider: Option<&str>, session: Option<&str>) -> Vec<SlotRow> {
        let pending = |name: &String, category: bool| SlotRow {
            name: name.clone(),
            unset: "Same as main".to_owned(),
            category,
            ..SlotRow::default()
        };
        self.slots
            .iter()
            .map(|slot| slot_row_of(slot, provider, session))
            .chain(self.added.iter().map(|name| pending(name, true)))
            .chain(self.plain.iter().map(|name| pending(name, false)))
            .collect()
    }

    /// The name of the slot at `row` of [`ModelSlots::rows`]: the harness's first, then the categories added here, then the slots added on the models page.
    fn name_at(&self, row: usize) -> Option<&str> {
        self.slots
            .iter()
            .map(|slot| slot.name.as_str())
            .chain(self.added.iter().map(String::as_str))
            .chain(self.plain.iter().map(String::as_str))
            .nth(row)
    }

    /// The boxes of the category `name`, when it is one.
    pub fn notes_of(&self, name: &str) -> Option<&SlotNotes> {
        self.notes.iter().find(|notes| notes.name == name)
    }

    /// Gives every category its boxes, holding what the harness holds for it; boxes the reader changed and has not saved, and boxes that already say the same, are kept as they are.
    fn sync_notes(&mut self) {
        let held = std::mem::take(&mut self.notes);
        let told = self
            .slots
            .iter()
            .filter(|slot| slot.category)
            .map(|slot| {
                (
                    slot.name.as_str(),
                    slot.description.as_deref(),
                    slot.prompt.as_deref(),
                )
            })
            .chain(self.added.iter().map(|name| (name.as_str(), None, None)));
        self.notes = told
            .map(|(name, description, prompt)| {
                held.iter()
                    .find(|notes| {
                        notes.name == name && (notes.edited || notes.holds(description, prompt))
                    })
                    .cloned()
                    .unwrap_or_else(|| SlotNotes::told(name, description, prompt))
            })
            .collect();
    }

    /// Writes `edit` into the box in `column` of the category at `row`, and marks its boxes as changed when the words did.
    fn edit_notes(&mut self, row: usize, column: usize, edit: Edit) {
        let Some(name) = self.name_at(row).map(str::to_owned) else {
            return;
        };
        let Some(notes) = self.notes.iter_mut().find(|notes| notes.name == name) else {
            return;
        };
        let field = if column == DESCRIPTION_COLUMN {
            &mut notes.description
        } else {
            &mut notes.prompt
        };
        let before = field.text().to_owned();
        field.apply(edit);
        if field.text() != before {
            notes.edited = true;
        }
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
        category: slot.category,
        description: slot.description.clone(),
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

/// The row and the column `id` names, when it names the description or the prompt box of a category.
fn notes_cell(id: FocusId) -> Option<(usize, usize)> {
    SLOT_CONTROLS
        .cell_of(id, SLOT_COLUMNS)
        .filter(|(_, column)| matches!(*column, DESCRIPTION_COLUMN | PROMPT_COLUMN))
}

/// Whether the box `id` saves what was written in it when the reader leaves it: the description and the prompt box of a category.
pub(crate) fn saves_on_leaving(id: FocusId) -> bool {
    notes_cell(id).is_some()
}

/// Whether `id` names a box of the models or the orchestration page.
pub(crate) fn is_slot_field(id: FocusId) -> bool {
    id == SLOT_NEW_NAME || id == CATEGORY_NEW_NAME || id == PRESET_NAME || notes_cell(id).is_some()
}

/// What Enter in the box `id` of the models or the orchestration page presses.
pub(crate) fn slot_submit(id: FocusId) -> FocusId {
    if let Some((row, _)) = notes_cell(id) {
        SLOT_CONTROLS
            .cell(row, DESCRIBE_COLUMN, SLOT_COLUMNS)
            .unwrap_or(id)
    } else if id == PRESET_NAME {
        PRESET_SAVE
    } else if id == CATEGORY_NEW_NAME {
        CATEGORY_ADD
    } else {
        SLOT_ADD
    }
}

/// The name of the card of the category at `row` of the slots, which opens and closes its editor.
pub fn card_id(row: usize) -> Option<FocusId> {
    SLOT_CONTROLS.cell(row, CARD_COLUMN, SLOT_COLUMNS)
}

impl WorkspaceState {
    /// The state with the slots and presets of `book`, as the harness holds them now; a slot added here that the harness now keeps is no longer held apart, and the editor of a category that is gone is closed.
    pub fn with_model_slots(mut self, book: pm_harness::SlotBook) -> Self {
        let page = &mut self.settings_page.slots;
        let kept = |name: &String| !book.slots.iter().any(|slot| slot.name == *name);
        page.added.retain(kept);
        page.plain.retain(kept);
        page.slots = book.slots;
        page.presets = book.presets;
        page.error = None;
        page.sync_notes();
        if let Some(open) = &page.expanded
            && page.notes_of(open).is_none()
        {
            page.expanded = None;
        }
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

    /// Has the harness save the boxes of every category the reader changed, both of them as they stand, in the file changes go to; an empty box clears what it held.
    fn send_descriptions(&mut self) {
        let scope = self.settings_page.slots.scope;
        let changes: Vec<SlotChange> = self
            .settings_page
            .slots
            .notes
            .iter_mut()
            .filter(|notes| notes.edited)
            .map(|notes| {
                notes.edited = false;
                SlotChange::Describe {
                    name: notes.name.clone(),
                    scope,
                    description: written(&notes.description),
                    prompt: written(&notes.prompt),
                }
            })
            .collect();
        for change in changes {
            self.change_slots(change);
        }
    }

    /// Saves what was written in the box `id` of the page when the reader leaves it, when it is a box of a category.
    pub(crate) fn slot_field_left(&mut self, id: FocusId) {
        if saves_on_leaving(id) {
            self.send_descriptions();
        }
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

    /// The slots the models page lists, each with its place among [`WorkspaceState::slot_rows`]: the job slots, then the slots added on that page; the work categories are on the orchestration page.
    pub fn model_rows(&self) -> Vec<(usize, SlotRow)> {
        self.slot_rows()
            .into_iter()
            .enumerate()
            .filter(|(_, slot)| !slot.category)
            .collect()
    }

    /// Every slot the pages show, for the provider viewed.
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

    /// Carries out pressing `id` on the models page, and says whether it was something of it; pressing anything but a box of a category first saves the boxes the reader changed.
    pub(crate) fn slots_pressed(&mut self, id: FocusId) -> bool {
        if notes_cell(id).is_some() {
            return true;
        }
        self.send_descriptions();
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
            CATEGORY_ADD => self.add_category(),
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
                let page = &mut self.settings_page.slots;
                page.added.retain(|name| *name != slot.name);
                page.plain.retain(|name| *name != slot.name);
                page.sync_notes();
                if page.slots.iter().any(|held| held.name == slot.name) {
                    let provider = self.slot_provider();
                    self.change_slots(SlotChange::Clear {
                        name: slot.name,
                        scope,
                        provider,
                    });
                } else if page.expanded.as_deref() == Some(slot.name.as_str()) {
                    page.expanded = None;
                }
            }
            CARD_COLUMN if slot.category => {
                let page = &mut self.settings_page.slots;
                page.expanded =
                    (page.expanded.as_deref() != Some(slot.name.as_str())).then_some(slot.name);
                page.picking = None;
                self.settings_page.filter = Field::default();
            }
            effort if (EFFORT_COLUMN..DESCRIPTION_COLUMN).contains(&effort) => {
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

    /// The name written in `field` with the space around it taken off, when it can name a slot and no slot the pages show has it.
    fn new_name(&self, field: &Field) -> Option<String> {
        let name = field.text().trim().to_owned();
        (valid_name(&name) && !self.slot_rows().iter().any(|slot| slot.name == name))
            .then_some(name)
    }

    /// Adds the slot named in the box of the models page as a row to give a model, when the name is valid and not taken, and empties the box.
    fn add_slot(&mut self) {
        let Some(name) = self.new_name(&self.settings_page.slots.new_slot) else {
            return;
        };
        let page = &mut self.settings_page.slots;
        page.plain.push(name);
        page.new_slot = Field::default();
    }

    /// Adds the category named in the box of the orchestration page with its editor open, to describe and give a model, when the name is valid and not taken, and empties the box.
    fn add_category(&mut self) {
        let Some(name) = self.new_name(&self.settings_page.slots.new_category) else {
            return;
        };
        let page = &mut self.settings_page.slots;
        page.added.push(name.clone());
        page.new_category = Field::default();
        page.expanded = Some(name);
        page.picking = None;
        page.sync_notes();
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
            CATEGORY_NEW_NAME => page.new_category.apply(edit),
            PRESET_NAME => page.preset_name.apply(edit),
            _ => {
                if let Some((row, column)) = notes_cell(id) {
                    page.edit_notes(row, column, edit);
                }
            }
        }
    }

    /// The box `id` of the page.
    pub(crate) fn slot_field(&self, id: FocusId) -> Option<&Field> {
        let page = &self.settings_page.slots;
        match id {
            SLOT_NEW_NAME => Some(&page.new_slot),
            CATEGORY_NEW_NAME => Some(&page.new_category),
            PRESET_NAME => Some(&page.preset_name),
            _ => {
                let (row, column) = notes_cell(id)?;
                let notes = page.notes_of(page.name_at(row)?)?;
                Some(if column == DESCRIPTION_COLUMN {
                    &notes.description
                } else {
                    &notes.prompt
                })
            }
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

/// Emits the list of models open under the slot at `row`, when the one open is for it as `kind` says.
#[track_caller]
fn open_list(state: &WorkspaceState, row: usize, kind: SlotPicking) {
    if state.settings_page.slots.picking != Some(kind) {
        return;
    }
    let options = state.slot_model_choices();
    let picked = match kind {
        SlotPicking::Model(_) => state.slot_rows().get(row).and_then(|slot| {
            let model = split_effort(slot.model.as_deref()?).0;
            options.iter().position(|option| option == model)
        }),
        SlotPicking::Backup(_) | SlotPicking::Provider => None,
    };
    if options.is_empty() {
        note("No signed-in agent or declared server of this provider has listed its models yet.");
        return;
    }
    picker_list(
        &options,
        picked,
        (&state.settings_page.filter, SETTINGS_PICK_FILTER),
    );
}

/// How wide the button that opens the list of backups to add is, in logical pixels.
const BACKUP_PICKER_WIDTH: f32 = 132.0;

/// Emits a small muted icon button showing `name` that answers to `id`, greyed out while `disabled`, when there is an `id`.
#[track_caller]
fn backup_button(name: IconName, id: Option<FocusId>, disabled: bool) {
    if let Some(id) = id {
        Button::icon(name, id)
            .size(Size::Sm)
            .tone(Role::Muted)
            .disabled(disabled)
            .build();
    }
}

/// Emits one backup of a slot as a chip: its place counted from one, `backup` itself, and the buttons `ids` names to move it up, move it down and remove it, the first greyed out when `first` and the second when `last`.
#[track_caller]
fn backup_chip(place: usize, backup: &str, ids: [Option<FocusId>; 3], first: bool, last: bool) {
    row(
        Modifier::new()
            .stroke(Role::Border, Radius::Md)
            .pad_side(Side::Left, 2),
        Flex::new().center().gap(0.5),
        || {
            line(format!("{}", place + 1), 11, 1.4, Role::Subtle);
            label(
                backup,
                Type::new(12).ratio(1.4).tone(Role::Text).truncate().mono(),
            );
            backup_button(IconName::ArrowUp, ids[0], first);
            backup_button(IconName::ArrowDown, ids[1], last);
            backup_button(IconName::Close, ids[2], false);
        },
    );
}

/// Emits the backups of the slot at `row`, in order, side by side, each with buttons that move it up and down and remove it, then the picker that adds one.
#[track_caller]
fn backups_node(state: &WorkspaceState, row: usize, slot: &SlotRow) {
    let cell = |place: usize, column: usize| {
        let index = row.checked_mul(MOST_BACKUPS)?.checked_add(place)?;
        (place < MOST_BACKUPS)
            .then(|| BACKUP_CONTROLS.cell(index, column, BACKUP_COLUMNS))
            .flatten()
    };
    let last = slot.backups.len().saturating_sub(1);
    let open = state.settings_page.slots.picking == Some(SlotPicking::Backup(row));
    column(Modifier::new(), Flex::new().gap(1), || {
        wrapping_with(Modifier::new(), Flex::new().gap(2).center(), || {
            line("Backups", 12, 1.4, Role::Muted);
            for (place, backup) in slot.backups.iter().enumerate() {
                backup_chip(
                    place,
                    backup,
                    [cell(place, 0), cell(place, 1), cell(place, 2)],
                    place == 0,
                    place == last,
                );
            }
            if let Some(id) = SLOT_CONTROLS.cell(row, BACKUP_COLUMN, SLOT_COLUMNS) {
                picker("Add backup", BACKUP_PICKER_WIDTH, id, open);
            }
        });
        open_list(state, row, SlotPicking::Backup(row));
    });
}

/// Emits a box of a category under what it is called, the two as wide as the row: `label_text` above what `field` emits.
#[track_caller]
fn labelled(label_text: &str, field: impl FnOnce()) {
    column(Modifier::new().fill_max_width(), Flex::new().gap(1), || {
        line(label_text, 12, 1.4, Role::Muted);
        field();
    });
}

/// Emits the boxes of the category at `row`, each `room` logical pixels across: what it is for on one line, then what it adds to a subagent's system prompt on several.
#[track_caller]
fn notes_node(row: usize, notes: &SlotNotes, room: f32) {
    let id = |column: usize| SLOT_CONTROLS.cell(row, column, SLOT_COLUMNS);
    column(Modifier::new().fill_max_width(), Flex::new().gap(2), || {
        if let Some(id) = id(DESCRIPTION_COLUMN) {
            labelled("Description", || {
                Input::new(&notes.description, id)
                    .variant(input::Variant::Bordered)
                    .size(Size::Sm)
                    .placeholder("What kind of work this category is for")
                    .build();
            });
        }
        if let Some(id) = id(PROMPT_COLUMN) {
            labelled("Prompt addition", || {
                column(
                    Modifier::new()
                        .fill_max_width()
                        .fill_rounded(Role::Background, Radius::Md)
                        .stroke_by_focus(id, Role::Border, Role::Accent, Radius::Md)
                        .focus_of(id),
                    Flex::new(),
                    || {
                        Input::new(&notes.prompt, id)
                            .variant(input::Variant::Multiline)
                            .placeholder("Added to a subagent's system prompt")
                            .rows(PROMPT_ROWS)
                            .wrap(input::inner_width(room))
                            .build();
                    },
                );
            });
        }
    });
}

/// Emits the badge saying which file or variable the model of `slot` comes from, when it is not the conversation's own.
#[track_caller]
pub(crate) fn source_badge(slot: &SlotRow) {
    if let Some(source) = slot.source.filter(|source| *source != SlotSource::Main) {
        Badge::new(source_name(source)).build();
    }
}

/// Emits the buttons of the slot at `row`, side by side, each only when it applies: one that takes its suggested model and one that clears it.
#[track_caller]
pub(crate) fn slot_actions(state: &WorkspaceState, row: usize, slot: &SlotRow) {
    let id = |column: usize| SLOT_CONTROLS.cell(row, column, SLOT_COLUMNS);
    let suggest = slot
        .suggested
        .as_ref()
        .filter(|_| slot.model.is_none())
        .and_then(|suggested| id(SUGGEST_COLUMN).map(|button| (suggested.clone(), button)));
    let page = &state.settings_page.slots;
    let clear = (slot.model.is_some()
        || !slot.backups.is_empty()
        || page.added.contains(&slot.name)
        || page.plain.contains(&slot.name))
    .then(|| id(CLEAR_COLUMN))
    .flatten();
    crate::kit::row(Modifier::new(), Flex::new().center().gap(1), || {
        if let Some((suggested, button)) = suggest {
            Button::new(format!("Use suggested: {suggested}"), button)
                .variant(Variant::Outline)
                .size(Size::Sm)
                .build();
        }
        if let Some(button) = clear {
            Button::new("Clear", button)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build();
        }
    });
}

/// Emits the controls of the slot at `row`, with `room` logical pixels across for its boxes: its model with its effort, the list of models while it is open, its backups, and for a category its boxes.
#[track_caller]
pub(crate) fn slot_controls(state: &WorkspaceState, row: usize, slot: &SlotRow, room: f32) {
    let id = |column: usize| SLOT_CONTROLS.cell(row, column, SLOT_COLUMNS);
    let notes = state
        .settings_page
        .slots
        .notes_of(&slot.name)
        .filter(|_| slot.category);
    let open = state.settings_page.slots.picking == Some(SlotPicking::Model(row));
    let effort = slot.model.as_deref().map(|model| {
        split_effort(model)
            .1
            .and_then(|effort| EFFORTS.iter().position(|held| *held == effort))
            .map_or(0, |place| place + 1)
    });
    let width = (PICKER_WIDTH * 1.4).min(room);
    column(Modifier::new().fill_max_width(), Flex::new().gap(2), || {
        wrapping_with(Modifier::new(), Flex::new().gap(2).center(), || {
            if let Some(button) = id(MODEL_COLUMN) {
                picker(&model_shown(slot), width, button, open);
            }
            if let Some(picked) = effort {
                choices(&EFFORT_NAMES, picked, |option| id(EFFORT_COLUMN + option));
            }
        });
        open_list(state, row, SlotPicking::Model(row));
        backups_node(state, row, slot);
        if let Some(notes) = notes {
            notes_node(row, notes, room);
        }
    });
}

/// Emits the row of the slot at `row` on the models and orchestration pages, `room` logical pixels across: its name, where its model comes from and, for a job slot, what it is for, its suggestion and clear buttons, then its controls.
#[track_caller]
pub(crate) fn slot_row(state: &WorkspaceState, row: usize, slot: &SlotRow, room: f32) {
    column(
        Modifier::new().pad_x(3).pad_y(3),
        Flex::new().gap(2),
        || {
            wrapping_with(
                Modifier::new().fill_max_width(),
                Flex::new().center().between().gap(2),
                || {
                    crate::kit::row(Modifier::new(), Flex::new().center().gap(2), || {
                        label(
                            format!("@{}", slot.name),
                            Type::new(13)
                                .ratio(1.4)
                                .tone(Role::Text)
                                .truncate()
                                .mono()
                                .medium(),
                        );
                        source_badge(slot);
                    });
                    slot_actions(state, row, slot);
                },
            );
            if !slot.category {
                clipped(pm_harness::slot_purpose(&slot.name), 12, Role::Muted);
            }
            slot_controls(state, row, slot, room - ROW_INSET);
        },
    );
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

/// Emits the row of the preset at `row`: its name and what it sets, with its apply and delete buttons.
#[track_caller]
fn preset_row(row: usize, preset: &PresetInfo) {
    let id = |column: usize| PRESET_CONTROLS.cell(row, column, PRESET_COLUMNS);
    crate::kit::row(
        Modifier::new().pad_x(3).pad_y(3),
        Flex::new().center().between().gap(3),
        || {
            column(Modifier::new().grow(), Flex::new().gap(0.5), || {
                label(
                    preset.name.clone(),
                    Type::new(13)
                        .ratio(1.4)
                        .tone(Role::Text)
                        .truncate()
                        .medium(),
                );
                label_with(
                    Modifier::new().grow(),
                    preset_summary(preset),
                    Type::new(12).ratio(1.4).tone(Role::Muted).truncate(),
                );
            });
            crate::kit::row(Modifier::new(), Flex::new().center().gap(1), || {
                if let Some(button) = id(0) {
                    Button::new("Apply", button)
                        .variant(Variant::Outline)
                        .size(Size::Sm)
                        .build();
                }
                if let Some(button) = id(1) {
                    Button::new("Delete", button)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build();
                }
            });
        },
    );
}

/// Emits a box and the button beside it that hands in what is written in it.
#[track_caller]
pub(crate) fn entry(field: &Field, id: FocusId, placeholder: &str, button: (&str, FocusId)) {
    row(Modifier::new(), Flex::new().center().gap(2), || {
        Input::new(field, id)
            .size(Size::Sm)
            .placeholder(placeholder)
            .modifier(Modifier::new().grow())
            .build();
        Button::new(button.0, button.1)
            .variant(Variant::Outline)
            .size(Size::Sm)
            .disabled(!valid_name(field.text().trim()))
            .build();
    });
}

/// What the page says about itself under its heading.
pub(crate) const SUMMARY: &str = "The models agents ask for each kind of work, for each provider. A provider's slot left unset uses the one set for all providers, else the conversation's own model. Name a slot as @name wherever a model is named, such as @fast in an agent definition.";

/// What the models page says under the box that adds a slot.
const OWN_SLOT_NOTE: &str = "A slot of your own is also a work category: once it has a model it is listed under Orchestration.";

/// Emits the choice of the file changes are saved to, after what it chooses.
#[track_caller]
pub(crate) fn scope_choice(state: &WorkspaceState) {
    let picked = usize::from(state.settings_page.slots.scope == SlotScope::Project);
    row(Modifier::new(), Flex::new().center().gap(2), || {
        line("Save changes to", 12, 1.4, Role::Muted);
        choices(&["User", "Project"], picked, |option| {
            [SLOTS_SCOPE_USER, SLOTS_SCOPE_PROJECT].get(option).copied()
        });
    });
}

/// Emits the picker of the provider whose assignments are shown, after what it picks, then the list of providers while it is open.
#[track_caller]
pub(crate) fn provider_choice(state: &WorkspaceState) {
    let open = state.settings_page.slots.picking == Some(SlotPicking::Provider);
    wrapping_with(Modifier::new(), Flex::new().gap(2).center(), || {
        line("Provider", 12, 1.4, Role::Muted);
        picker(
            &state.provider_shown(),
            PROVIDER_PICKER_WIDTH,
            SLOTS_PROVIDER,
            open,
        );
    });
    if open {
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
        );
    }
}

/// Emits the line saying why the slots could not be read or changed, in no more than `room` logical pixels across, when the last attempt failed.
#[track_caller]
pub(crate) fn error_note(state: &WorkspaceState, room: f32) {
    if let Some(reason) = state.settings_page.slots.error.as_deref() {
        wrapped_note(&format!("Could not change the slots: {reason}"), room);
    }
}

/// Emits the body of the models page, `room` logical pixels across, after what `page` emits, into the box the caller holds it in: what it is for, where changes go, the job slots and the slots added here, a box to add one, and the presets; the work categories are on the orchestration page.
#[track_caller]
pub(crate) fn body(state: &WorkspaceState, page: impl FnOnce(), room: f32) {
    let slots = &state.settings_page.slots;
    let rows = state.model_rows();
    page();
    paragraph(SUMMARY, room);
    provider_choice(state);
    error_note(state, room);
    wrapping_with(
        Modifier::new().fill_max_width(),
        Flex::new().center().between().gap(2),
        || {
            heading_only("Slots", rows.len());
            scope_choice(state);
        },
    );
    if rows.is_empty() {
        note("The agent has not told its slots yet.");
    } else {
        boxed(|| {
            for (row, slot) in rows.iter() {
                rustle::key(*row, || slot_row(state, *row, slot, room));
            }
        });
    }
    entry(
        &slots.new_slot,
        SLOT_NEW_NAME,
        "Add a slot of your own, e.g. docs",
        ("Add Slot", SLOT_ADD),
    );
    wrapped_note(OWN_SLOT_NOTE, room);
    heading_only("Presets", slots.presets.len());
    if slots.presets.is_empty() {
        note("No presets are saved.");
    } else {
        boxed(|| {
            for (row, preset) in slots.presets.iter().enumerate() {
                preset_row(row, preset);
            }
        });
    }
    entry(
        &slots.preset_name,
        PRESET_NAME,
        "Name the slots as they stand, e.g. cheap",
        ("Save Preset", PRESET_SAVE),
    );
}

/// How many job slots set a model of their own, in words, for the page's card on the Agents overview.
pub(crate) fn count(state: &WorkspaceState) -> String {
    let set = state
        .settings_page
        .slots
        .slots
        .iter()
        .filter(|slot| !slot.category && !slot.inherited)
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
            description: None,
            prompt: None,
            category: false,
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
        let added = state.model_rows().pop().unwrap().1;
        assert_eq!(added.name, "docs");
        assert!(!added.category);
        assert!(state.category_cards().is_empty());
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
        assert!(state.settings_page.slots.plain.is_empty());
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

    /// A category as the harness tells it: inheriting, saying `description` and adding `prompt`.
    fn category(name: &str, description: Option<&str>, prompt: Option<&str>) -> SlotInfo {
        SlotInfo {
            description: description.map(str::to_owned),
            prompt: prompt.map(str::to_owned),
            category: true,
            ..slot(name, None, None)
        }
    }

    /// The state of [`open`] with the categories `quick`, described, and `deep`, described and adding to the prompt, after its three job slots, and nothing waiting to be sent.
    fn with_categories() -> WorkspaceState {
        let state = open();
        let mut book = pm_harness::SlotBook {
            slots: state.settings_page.slots.slots.clone(),
            presets: Vec::new(),
        };
        book.slots
            .push(category("quick", Some("Small changes."), None));
        book.slots.push(category(
            "deep",
            Some("Hard problems."),
            Some("Read every caller first."),
        ));
        let mut state = state.with_model_slots(book);
        state.take_outbox();
        state
    }

    /// The name of the box in `column` of the slot at `row`.
    fn cell(row: usize, column: usize) -> FocusId {
        SLOT_CONTROLS.cell(row, column, SLOT_COLUMNS).unwrap()
    }

    /// Categories are told apart from the job slots, keep their place among the rows, and have boxes holding what the harness holds; job slots have none. The models page lists only the job slots and the orchestration page only the categories.
    #[test]
    fn categories_are_grouped_apart_from_job_slots() {
        let state = with_categories();
        let rows = state.slot_rows();
        let categories: Vec<&str> = rows
            .iter()
            .filter(|row| row.category)
            .map(|row| row.name.as_str())
            .collect();
        let jobs: Vec<&str> = rows
            .iter()
            .filter(|row| !row.category)
            .map(|row| row.name.as_str())
            .collect();
        assert_eq!(categories, ["quick", "deep"]);
        assert_eq!(jobs, ["main", "fast", "vision"]);
        let page = &state.settings_page.slots;
        assert!(page.notes_of("fast").is_none());
        assert_eq!(
            page.notes_of("deep").unwrap().prompt.text(),
            "Read every caller first."
        );
        assert_eq!(
            state.field(cell(3, DESCRIPTION_COLUMN)).unwrap().text(),
            "Small changes."
        );
        assert!(state.field(cell(4, PROMPT_COLUMN)).unwrap().is_multiline());
        assert!(state.field(cell(1, DESCRIPTION_COLUMN)).is_none());
        let listed: Vec<String> = state
            .model_rows()
            .into_iter()
            .map(|(_, row)| row.name)
            .collect();
        assert_eq!(listed, jobs);
        let cards: Vec<String> = state
            .category_cards()
            .into_iter()
            .map(|card| card.name)
            .collect();
        assert_eq!(cards, categories);
    }

    /// Editing a category's description and pressing Enter sends both boxes for the file chosen, and nothing is sent twice.
    #[test]
    fn editing_the_description_sends_both_fields() {
        let description = cell(4, DESCRIPTION_COLUMN);
        assert!(WorkspaceState::is_field(description));
        let state = with_categories()
            .settings_pressed(SLOTS_SCOPE_PROJECT)
            .edited(description, Edit::Paste(" Slowly.".to_owned()));
        assert!(state.settings_page.slots.notes_of("deep").unwrap().edited);
        let mut state = state.settings_pressed(description);
        assert!(state.take_outbox().is_empty());
        let mut state = state.settings_pressed(WorkspaceState::submit_of(description));
        assert_eq!(
            sent(&mut state),
            SlotChange::Describe {
                name: "deep".to_owned(),
                scope: SlotScope::Project,
                description: Some("Hard problems. Slowly.".to_owned()),
                prompt: Some("Read every caller first.".to_owned()),
            }
        );
        let mut state = state.settings_pressed(WorkspaceState::submit_of(description));
        assert!(state.take_outbox().is_empty());
    }

    /// Leaving a box that was changed sends it, an emptied box sends nothing for its field, and leaving a box that was not changed sends nothing.
    #[test]
    fn clearing_an_input_sends_nothing_for_it() {
        let prompt = cell(4, PROMPT_COLUMN);
        assert!(WorkspaceState::saves_on_leaving(prompt));
        assert!(!WorkspaceState::saves_on_leaving(SLOT_NEW_NAME));
        let mut state = with_categories().field_left(prompt);
        assert!(state.take_outbox().is_empty());
        let mut state = state
            .edited(prompt, Edit::SelectAll)
            .edited(prompt, Edit::Backspace)
            .field_left(prompt);
        assert_eq!(
            sent(&mut state),
            SlotChange::Describe {
                name: "deep".to_owned(),
                scope: SlotScope::User,
                description: Some("Hard problems.".to_owned()),
                prompt: None,
            }
        );
    }

    /// Boxes the reader changed are kept when the slots are told again, and pressing another control saves them first.
    #[test]
    fn changed_boxes_outlast_the_slots_being_told() {
        let description = cell(3, DESCRIPTION_COLUMN);
        let state = with_categories().edited(description, Edit::Paste(" Fast.".to_owned()));
        let book = pm_harness::SlotBook {
            slots: state.settings_page.slots.slots.clone(),
            presets: Vec::new(),
        };
        let state = state.with_model_slots(book);
        assert_eq!(
            state.field(description).unwrap().text(),
            "Small changes. Fast."
        );
        let mut state = state.settings_pressed(SLOTS_SCOPE_PROJECT);
        assert_eq!(
            sent(&mut state),
            SlotChange::Describe {
                name: "quick".to_owned(),
                scope: SlotScope::User,
                description: Some("Small changes. Fast.".to_owned()),
                prompt: None,
            }
        );
    }

    /// A category added on the orchestration page has empty boxes and its editor open, and describing it sends its name.
    #[test]
    fn an_added_category_is_described() {
        let state = with_categories()
            .edited(CATEGORY_NEW_NAME, Edit::Paste("docs".to_owned()))
            .settings_pressed(WorkspaceState::submit_of(CATEGORY_NEW_NAME));
        let rows = state.slot_rows();
        let added = rows.last().unwrap();
        assert_eq!(added.name, "docs");
        assert!(added.category);
        assert_eq!(state.settings_page.slots.expanded.as_deref(), Some("docs"));
        assert_eq!(state.settings_page.slots.new_category.text(), "");
        let description = cell(rows.len() - 1, DESCRIPTION_COLUMN);
        assert_eq!(state.field(description).unwrap().text(), "");
        let mut state = state
            .edited(
                description,
                Edit::Paste("Writing documentation.".to_owned()),
            )
            .field_left(description);
        assert_eq!(
            sent(&mut state),
            SlotChange::Describe {
                name: "docs".to_owned(),
                scope: SlotScope::User,
                description: Some("Writing documentation.".to_owned()),
                prompt: None,
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
