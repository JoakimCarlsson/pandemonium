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

/// Declares [`SettingsList`] with each of `$list`, documented by its `$doc`, and beside each the [`Rows`] of that list as the constant `$rows`, with room for `$len` rows and the same documentation.
macro_rules! lists {
    ($($(#[$doc:meta])* $list:ident => $rows:ident, $len:expr;)*) => {
        /// A list of controls the settings window repeats once per row.
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        pub enum SettingsList {
            $($(#[$doc])* $list,)*
        }

        $($(#[$doc])* pub const $rows: Rows = Rows::new(SettingsList::$list, $len);)*
    };
}

lists! {
    /// The choices of how the server in the servers page's form is reached.
    MachineReaches => MACHINE_REACHES, 16;
    /// The choices of where a new customization goes.
    DraftScopes => DRAFT_SCOPES, 16;
    /// The sections down the left side, in the order of `Section::ALL`.
    Sections => SECTIONS, 64;
    /// The controls of every setting, [`PER_SETTING`] for each, in the order of `Setting::ALL`.
    SettingControls => SETTING_CONTROLS, 256 * PER_SETTING as u64;
    /// The controls of every action of the keymap, [`PER_ACTION`] for each, in the order of `Action::ALL`.
    KeymapControls => KEYMAP_CONTROLS, 1024 * PER_ACTION as u64;
    /// The rows of the open list of choices of a setting, by their place among the rows shown.
    Picks => PICKS, 64;
    /// The boxes a line of a setting written as a list is typed in, one for each such setting.
    ListFields => LIST_FIELDS, 16;
    /// The buttons adding what is typed in a box of [`LIST_FIELDS`] to its list, one for each setting written as a list.
    ListAdds => LIST_ADDS, 16;
    /// The buttons removing a line of a list, [`LIST_LINES`] for each setting written as a list.
    ListRemoves => LIST_REMOVES, 8 * LIST_LINES as u64;
    /// The buttons opening the form for a known server of the servers page.
    MachineEdits => MACHINE_EDITS, 1000;
    /// The buttons taking a known server out of the servers page.
    MachineRemoves => MACHINE_REMOVES, 1000;
    /// The buttons installing or removing a language, in the order the languages page lists them.
    LanguageRows => LANGUAGE_ROWS, 4000;
    /// The switches offering an agent for new sessions, by the agent's position.
    AgentSwitches => AGENT_SWITCHES, 500;
    /// The buttons opening the box for an agent's API key, by the agent's position.
    KeySets => KEY_SETS, 500;
    /// The buttons forgetting an agent's saved API key, by the agent's position.
    KeyRemoves => KEY_REMOVES, 500;
    /// The buttons signing an agent's default account in, by the agent's position.
    SignIns => SIGN_INS, 500;
    /// The buttons adding an account to an agent, by the agent's position.
    AccountAdds => ACCOUNT_ADDS, 500;
    /// The buttons removing an added account, by the account's place among every agent's added accounts.
    AccountRemoves => ACCOUNT_REMOVES, 2000;
    /// The switches turning a tool server on or off, by its place in the list.
    ServerSwitches => SERVER_SWITCHES, 1000;
    /// The buttons opening the menu of a tool server, by its place in the list.
    ServerMores => SERVER_MORES, 1000;
    /// The buttons installing what the tool registry offers, by its place among the offers.
    Installs => INSTALLS, 1000;
    /// The switches turning a customization on or off, by its place among those read.
    ItemSwitches => ITEM_SWITCHES, 5000;
    /// The buttons opening the file of a customization, by its place among those read.
    ItemOpens => ITEM_OPENS, 5000;
    /// The switches turning one tool on or off, counted across every set.
    ToolSwitches => TOOL_SWITCHES, 20_000;
    /// The switches turning a whole set of tools on or off, by the set's place.
    SetSwitches => SET_SWITCHES, 1000;
    /// The headings folding a set of tools away or showing it, by the set's place.
    SetFolds => SET_FOLDS, 1000;
    /// The buttons making an agent's default account the one new sessions use, by the agent's place.
    DefaultUses => DEFAULT_USES, 500;
    /// The buttons of the added accounts, [`ACCOUNT_COLUMNS`] to a login, by the login's place in the list of logins.
    AccountControls => ACCOUNT_CONTROLS, 2000 * ACCOUNT_COLUMNS as u64;
    /// The buttons that open the form of each declared model server, by its place in the list.
    ModelServerEdits => MODEL_SERVER_EDITS, 500;
    /// The buttons that take each declared model server out, by its place in the list.
    ModelServerRemoves => MODEL_SERVER_REMOVES, 500;
    /// The boxes that list each model a server was found to offer in its form, by the model's place in what was found.
    FoundModelPicks => FOUND_MODEL_PICKS, 2000;
    /// The controls of each model slot, [`crate::settings_slots::SLOT_COLUMNS`] to a slot, by the slot's place on the models page.
    SlotControls => SLOT_CONTROLS, 100 * crate::settings_slots::SLOT_COLUMNS as u64;
    /// The controls of each backup of each model slot, [`crate::settings_slots::BACKUP_COLUMNS`] to a backup, [`crate::settings_slots::MOST_BACKUPS`] backups to a slot.
    BackupControls => BACKUP_CONTROLS, 100 * (crate::settings_slots::MOST_BACKUPS * crate::settings_slots::BACKUP_COLUMNS) as u64;
    /// The controls of each preset of the models page, [`crate::settings_slots::PRESET_COLUMNS`] to a preset, by its place in the list.
    PresetControls => PRESET_CONTROLS, 200 * crate::settings_slots::PRESET_COLUMNS as u64;
    /// The choices of how many days the usage page covers, in the order of [`crate::settings_usage::RANGES`].
    UsageRanges => USAGE_RANGES, 8;
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

/// How many controls each setting has room for.
pub const PER_SETTING: usize = 8;

/// How many controls each action of the keymap has room for.
pub const PER_ACTION: usize = 4;

/// How many lines of one list the remove buttons have room for.
pub const LIST_LINES: usize = 1000;

/// How many buttons each added account has: use it, sign it in, set its key, forget its key, rename it.
pub const ACCOUNT_COLUMNS: usize = 5;

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
