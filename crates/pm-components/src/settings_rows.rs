//! Every list of controls the settings window repeats once per row, laid out in one place.
//!
//! A control there is one of, such as the button closing the window, is a unit variant of
//! [`Target`]; a control repeated once per row of a list is a [`SettingsEvent::SettingsRow`](crate::areas::SettingsEvent::SettingsRow) naming
//! the [`SettingsList`] it belongs to and its place in it. Each list has [`Rows`] of its
//! own, which say how many rows it has room for: a row past the end of its list gets no
//! target from [`Rows::id`], so it is drawn without its control rather than answering for
//! another one.

use crate::areas::SettingsEvent;
use serde::{Deserialize, Serialize};

use crate::target::Target;

/// A list of controls the settings window repeats once per row.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum SettingsList {
    /// The choices of how the server in the servers page's form is reached.
    MachineReaches,
    /// The choices of where a new customization goes.
    DraftScopes,
    /// The sections down the left side, in the order of `Section::ALL`.
    Sections,
    /// The controls of every setting, [`PER_SETTING`] for each, in the order of `Setting::ALL`.
    SettingControls,
    /// The controls of every action of the keymap, [`PER_ACTION`] for each, in the order of `Action::ALL`.
    KeymapControls,
    /// The rows of the open list of choices of a setting, by their place among the rows shown.
    Picks,
    /// The boxes a line of a setting written as a list is typed in, one for each such setting.
    ListFields,
    /// The buttons adding what is typed in a box of [`LIST_FIELDS`] to its list, one for each setting written as a list.
    ListAdds,
    /// The buttons removing a line of a list, [`LIST_LINES`] for each setting written as a list.
    ListRemoves,
    /// The buttons opening the form for a known server of the servers page.
    MachineEdits,
    /// The buttons taking a known server out of the servers page.
    MachineRemoves,
    /// The buttons installing or removing a language, in the order the languages page lists them.
    LanguageRows,
    /// The switches offering an agent for new sessions, by the agent's position.
    AgentSwitches,
    /// The buttons opening the box for an agent's API key, by the agent's position.
    KeySets,
    /// The buttons forgetting an agent's saved API key, by the agent's position.
    KeyRemoves,
    /// The buttons signing an agent's default account in, by the agent's position.
    SignIns,
    /// The buttons adding an account to an agent, by the agent's position.
    AccountAdds,
    /// The buttons removing an added account, by the account's place among every agent's added accounts.
    AccountRemoves,
    /// The switches turning a tool server on or off, by its place in the list.
    ServerSwitches,
    /// The buttons opening the menu of a tool server, by its place in the list.
    ServerMores,
    /// The buttons installing what the tool registry offers, by its place among the offers.
    Installs,
    /// The switches turning a customization on or off, by its place among those read.
    ItemSwitches,
    /// The buttons opening the file of a customization, by its place among those read.
    ItemOpens,
    /// The switches turning one tool on or off, counted across every set.
    ToolSwitches,
    /// The switches turning a whole set of tools on or off, by the set's place.
    SetSwitches,
    /// The headings folding a set of tools away or showing it, by the set's place.
    SetFolds,
    /// The buttons making an agent's default account the one new sessions use, by the agent's place.
    DefaultUses,
    /// The buttons of the added accounts, [`ACCOUNT_COLUMNS`] to a login, by the login's place in the list of logins.
    AccountControls,
    /// The buttons that open the form of each declared model server, by its place in the list.
    ModelServerEdits,
    /// The buttons that take each declared model server out, by its place in the list.
    ModelServerRemoves,
    /// The boxes that list each model a server was found to offer in its form, by the model's place in what was found.
    FoundModelPicks,
    /// The controls of each model slot, [`crate::settings_slots::SLOT_COLUMNS`] to a slot, by the slot's place on the models page.
    SlotControls,
    /// The controls of each backup of each model slot, [`crate::settings_slots::BACKUP_COLUMNS`] to a backup, [`crate::settings_slots::MOST_BACKUPS`] backups to a slot.
    BackupControls,
    /// The controls of each preset of the models page, [`crate::settings_slots::PRESET_COLUMNS`] to a preset, by its place in the list.
    PresetControls,
    /// The choices of how many days the usage page covers, in the order of [`crate::settings_usage::RANGES`].
    UsageRanges,
}

/// The rows of one list of the settings window: which list, and how many rows it has room for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rows {
    /// The list the rows belong to.
    list: SettingsList,
    /// How many rows the list has room for.
    len: u64,
}

impl Rows {
    /// The rows of `list`, with room for `len` of them.
    pub const fn new(list: SettingsList, len: u64) -> Self {
        Self { list, len }
    }

    /// How many rows the list has room for.
    pub const fn len(self) -> u64 {
        self.len
    }

    /// Whether the list has room for no rows.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// The target of the row at `index`, for a control declared once.
    ///
    /// # Panics
    ///
    /// Panics when `index` is past the room of the list.
    pub fn at(self, index: usize) -> Target {
        self.id(index).expect("the row is past the end of its list")
    }

    /// Whether `id` is one of the list's rows.
    pub fn contains(self, id: Target) -> bool {
        self.index_of(id).is_some()
    }

    /// The target of the row at `index`, or nothing when the list has no room for it.
    pub fn id(self, index: usize) -> Option<Target> {
        u64::try_from(index)
            .ok()
            .filter(|place| *place < self.len)
            .map(|_| Target::Settings(SettingsEvent::SettingsRow(self.list, index)))
    }

    /// The row `id` names, when it is one of the list's rows.
    pub fn index_of(self, id: Target) -> Option<usize> {
        match id {
            Target::Settings(SettingsEvent::SettingsRow(list, index))
                if list == self.list
                    && u64::try_from(index).is_ok_and(|place| place < self.len) =>
            {
                Some(index)
            }
            _ => None,
        }
    }

    /// The target of `column` of `row`, for a list holding `columns` controls for each row, or nothing when either is past the room for it.
    pub fn cell(self, row: usize, column: usize, columns: usize) -> Option<Target> {
        if column >= columns {
            return None;
        }
        self.id(row.checked_mul(columns)?.checked_add(column)?)
    }

    /// The row and the column `id` names, for a list holding `columns` controls for each row, when it is one of the list's rows.
    pub fn cell_of(self, id: Target, columns: usize) -> Option<(usize, usize)> {
        let index = self.index_of(id)?;
        (columns > 0).then(|| (index / columns, index % columns))
    }
}

/// The choices of how the server in the servers page's form is reached.
pub const MACHINE_REACHES: Rows = Rows::new(SettingsList::MachineReaches, 16);

/// The choices of where a new customization goes.
pub const DRAFT_SCOPES: Rows = Rows::new(SettingsList::DraftScopes, 16);

/// The sections down the left side, in the order of `Section::ALL`.
pub const SECTIONS: Rows = Rows::new(SettingsList::Sections, 64);

/// How many controls each setting has room for.
pub const PER_SETTING: usize = 8;

/// The controls of every setting, [`PER_SETTING`] for each, in the order of `Setting::ALL`.
pub const SETTING_CONTROLS: Rows =
    Rows::new(SettingsList::SettingControls, 256 * PER_SETTING as u64);

/// How many controls each action of the keymap has room for.
pub const PER_ACTION: usize = 4;

/// The controls of every action of the keymap, [`PER_ACTION`] for each, in the order of `Action::ALL`.
pub const KEYMAP_CONTROLS: Rows = Rows::new(SettingsList::KeymapControls, 1024 * PER_ACTION as u64);

/// The rows of the open list of choices of a setting, by their place among the rows shown.
pub const PICKS: Rows = Rows::new(SettingsList::Picks, 64);

/// The boxes a line of a setting written as a list is typed in, one for each such setting.
pub const LIST_FIELDS: Rows = Rows::new(SettingsList::ListFields, 16);

/// The buttons adding what is typed in a box of [`LIST_FIELDS`] to its list, one for each setting written as a list.
pub const LIST_ADDS: Rows = Rows::new(SettingsList::ListAdds, 16);

/// How many lines of one list the remove buttons have room for.
pub const LIST_LINES: usize = 1000;

/// The buttons removing a line of a list, [`LIST_LINES`] for each setting written as a list.
pub const LIST_REMOVES: Rows = Rows::new(SettingsList::ListRemoves, 8 * LIST_LINES as u64);

/// The buttons opening the form for a known server of the servers page.
pub const MACHINE_EDITS: Rows = Rows::new(SettingsList::MachineEdits, 1000);

/// The buttons taking a known server out of the servers page.
pub const MACHINE_REMOVES: Rows = Rows::new(SettingsList::MachineRemoves, 1000);

/// The buttons installing or removing a language, in the order the languages page lists them.
pub const LANGUAGE_ROWS: Rows = Rows::new(SettingsList::LanguageRows, 4000);

/// The switches offering an agent for new sessions, by the agent's position.
pub const AGENT_SWITCHES: Rows = Rows::new(SettingsList::AgentSwitches, 500);

/// The buttons opening the box for an agent's API key, by the agent's position.
pub const KEY_SETS: Rows = Rows::new(SettingsList::KeySets, 500);

/// The buttons forgetting an agent's saved API key, by the agent's position.
pub const KEY_REMOVES: Rows = Rows::new(SettingsList::KeyRemoves, 500);

/// The buttons signing an agent's default account in, by the agent's position.
pub const SIGN_INS: Rows = Rows::new(SettingsList::SignIns, 500);

/// The buttons adding an account to an agent, by the agent's position.
pub const ACCOUNT_ADDS: Rows = Rows::new(SettingsList::AccountAdds, 500);

/// The buttons removing an added account, by the account's place among every agent's added accounts.
pub const ACCOUNT_REMOVES: Rows = Rows::new(SettingsList::AccountRemoves, 2000);

/// The switches turning a tool server on or off, by its place in the list.
pub const SERVER_SWITCHES: Rows = Rows::new(SettingsList::ServerSwitches, 1000);

/// The buttons opening the menu of a tool server, by its place in the list.
pub const SERVER_MORES: Rows = Rows::new(SettingsList::ServerMores, 1000);

/// The buttons installing what the tool registry offers, by its place among the offers.
pub const INSTALLS: Rows = Rows::new(SettingsList::Installs, 1000);

/// The switches turning a customization on or off, by its place among those read.
pub const ITEM_SWITCHES: Rows = Rows::new(SettingsList::ItemSwitches, 5000);

/// The buttons opening the file of a customization, by its place among those read.
pub const ITEM_OPENS: Rows = Rows::new(SettingsList::ItemOpens, 5000);

/// The switches turning one tool on or off, counted across every set.
pub const TOOL_SWITCHES: Rows = Rows::new(SettingsList::ToolSwitches, 20_000);

/// The switches turning a whole set of tools on or off, by the set's place.
pub const SET_SWITCHES: Rows = Rows::new(SettingsList::SetSwitches, 1000);

/// The headings folding a set of tools away or showing it, by the set's place.
pub const SET_FOLDS: Rows = Rows::new(SettingsList::SetFolds, 1000);

/// The buttons making an agent's default account the one new sessions use, by the agent's place.
pub const DEFAULT_USES: Rows = Rows::new(SettingsList::DefaultUses, 500);

/// How many buttons each added account has: use it, sign it in, set its key, forget its key, rename it.
pub const ACCOUNT_COLUMNS: usize = 5;

/// The buttons of the added accounts, [`ACCOUNT_COLUMNS`] to a login, by the login's place in the list of logins.
pub const ACCOUNT_CONTROLS: Rows =
    Rows::new(SettingsList::AccountControls, 2000 * ACCOUNT_COLUMNS as u64);

/// The buttons that open the form of each declared model server, by its place in the list.
pub const MODEL_SERVER_EDITS: Rows = Rows::new(SettingsList::ModelServerEdits, 500);

/// The buttons that take each declared model server out, by its place in the list.
pub const MODEL_SERVER_REMOVES: Rows = Rows::new(SettingsList::ModelServerRemoves, 500);

/// The boxes that list each model a server was found to offer in its form, by the model's place in what was found.
pub const FOUND_MODEL_PICKS: Rows = Rows::new(SettingsList::FoundModelPicks, 2000);

/// The controls of each model slot, [`crate::settings_slots::SLOT_COLUMNS`] to a slot, by the slot's place on the models page.
pub const SLOT_CONTROLS: Rows = Rows::new(
    SettingsList::SlotControls,
    100 * crate::settings_slots::SLOT_COLUMNS as u64,
);

/// The controls of each backup of each model slot, [`crate::settings_slots::BACKUP_COLUMNS`] to a backup, [`crate::settings_slots::MOST_BACKUPS`] backups to a slot.
pub const BACKUP_CONTROLS: Rows = Rows::new(
    SettingsList::BackupControls,
    100 * (crate::settings_slots::MOST_BACKUPS * crate::settings_slots::BACKUP_COLUMNS) as u64,
);

/// The controls of each preset of the models page, [`crate::settings_slots::PRESET_COLUMNS`] to a preset, by its place in the list.
pub const PRESET_CONTROLS: Rows = Rows::new(
    SettingsList::PresetControls,
    200 * crate::settings_slots::PRESET_COLUMNS as u64,
);

/// The choices of how many days the usage page covers, in the order of [`crate::settings_usage::RANGES`].
pub const USAGE_RANGES: Rows = Rows::new(SettingsList::UsageRanges, 8);

const _: () = assert!(
    crate::settings::Section::ALL.len() as u64 <= SECTIONS.len(),
    "SECTIONS has no room for every section"
);

const _: () = assert!(
    (crate::settings_catalog::Setting::ALL.len() * PER_SETTING) as u64 <= SETTING_CONTROLS.len(),
    "SETTING_CONTROLS has no room for every setting"
);

const _: () = assert!(
    (pm_editor::Action::ALL.len() * PER_ACTION) as u64 <= KEYMAP_CONTROLS.len(),
    "KEYMAP_CONTROLS has no room for every action"
);

#[cfg(test)]
/// Tests for the lists of the window.
mod tests {
    use super::*;
    use crate::areas::{ComposerEvent, DebugEvent};

    /// A list names its rows up to its end and none past it, and reads back only its own rows.
    #[test]
    fn a_range_is_bounded_on_both_sides() {
        assert_eq!(
            PICKS.id(0),
            Some(Target::Settings(SettingsEvent::SettingsRow(
                SettingsList::Picks,
                0
            )))
        );
        assert_eq!(PICKS.id(PICKS.len() as usize), None);
        assert_eq!(
            PICKS.index_of(Target::Settings(SettingsEvent::SettingsRow(
                SettingsList::Picks,
                3
            ))),
            Some(3)
        );
        assert_eq!(
            PICKS.index_of(Target::Settings(SettingsEvent::SettingsRow(
                SettingsList::KeymapControls,
                3
            ))),
            None
        );
        assert_eq!(
            PICKS.index_of(Target::Settings(SettingsEvent::SettingsRow(
                SettingsList::Picks,
                PICKS.len() as usize
            ))),
            None
        );
        assert_eq!(
            LIST_REMOVES.cell(1, 2, LIST_LINES),
            LIST_REMOVES.id(LIST_LINES + 2)
        );
        assert_eq!(LIST_REMOVES.cell(0, LIST_LINES, LIST_LINES), None);
        assert_eq!(
            LIST_REMOVES.cell_of(LIST_REMOVES.id(LIST_LINES + 2).unwrap(), LIST_LINES),
            Some((1, 2))
        );
    }

    /// Every row lies in exactly one list, and the settings window answers for its rows and controls and nothing else.
    #[test]
    fn every_name_has_one_range() {
        let lists = [PICKS, SECTIONS, SET_FOLDS, USAGE_RANGES, LIST_REMOVES];
        for list in lists {
            let first = list.id(0).unwrap();
            let holding = lists.iter().filter(|other| other.contains(first)).count();
            assert_eq!(holding, 1);
        }
        assert!(crate::settings::is_settings(
            SET_FOLDS.id(SET_FOLDS.len() as usize - 1).unwrap()
        ));
        assert!(crate::settings::is_settings(Target::Settings(
            SettingsEvent::SettingsClose
        )));
        assert!(!crate::settings::is_settings(Target::Debug(
            DebugEvent::DebugTab
        )));
        assert!(!crate::settings::is_settings(Target::Composer(
            ComposerEvent::Prompt
        )));
    }
}
