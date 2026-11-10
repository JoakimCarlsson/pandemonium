//! Every name the settings window answers to, laid out in one place.
//!
//! The window's names run from [`SETTINGS_START`] up to [`SETTINGS_END`]. The
//! first [`SINGLES`] hold the controls there is one of, such as the button
//! closing the window; every other control repeats, once per row of a list,
//! and owns an [`IdRange`] of its own. Each range starts where the one before
//! it ends, so no two can share a name, and the assertions at the end of the
//! module stop the build when the ranges stop fitting below [`SETTINGS_END`]
//! or when two of the single controls are given the same name.
//!
//! A row past the end of its range gets no name from [`IdRange::id`], so it is
//! drawn without its control rather than answering to a name of another one.

use pm_input::FocusId;

/// The first name the settings window answers to.
pub const SETTINGS_START: u64 = 20_000_000;

/// One past the last name the settings window answers to; the debug tab's names start here.
pub const SETTINGS_END: u64 = 20_100_000;

/// A run of names that follow one another, one for each row of a list of controls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdRange {
    /// The first name of the run.
    base: u64,
    /// How many names the run holds.
    len: u64,
}

impl IdRange {
    /// The first run of the window, `len` names long, starting at [`SETTINGS_START`].
    pub const fn first(len: u64) -> Self {
        Self {
            base: SETTINGS_START,
            len,
        }
    }

    /// The run `len` names long that starts where this one ends.
    pub const fn after(self, len: u64) -> Self {
        Self {
            base: self.end(),
            len,
        }
    }

    /// The first name of the run.
    pub const fn base(self) -> u64 {
        self.base
    }

    /// How many names the run holds.
    pub const fn len(self) -> u64 {
        self.len
    }

    /// Whether the run holds no names.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// One past the last name of the run.
    pub const fn end(self) -> u64 {
        self.base + self.len
    }

    /// The name at `index`, for a control declared as a constant.
    ///
    /// # Panics
    ///
    /// Panics when `index` is past the end of the run, which stops the build
    /// when the constant is evaluated.
    pub const fn at(self, index: u64) -> FocusId {
        assert!(index < self.len, "the name is past the end of its range");
        FocusId(self.base + index)
    }

    /// Whether `id` is one of the run's names.
    pub const fn contains(self, id: FocusId) -> bool {
        id.0 >= self.base && id.0 < self.end()
    }

    /// The name of the row at `index`, or nothing when the run has no room for it.
    pub fn id(self, index: usize) -> Option<FocusId> {
        u64::try_from(index)
            .ok()
            .filter(|index| *index < self.len)
            .map(|index| FocusId(self.base + index))
    }

    /// The row `id` names, when it is one of the run's names.
    pub fn index_of(self, id: FocusId) -> Option<usize> {
        self.contains(id)
            .then(|| usize::try_from(id.0 - self.base).ok())
            .flatten()
    }

    /// The name of `column` of `row`, for a run holding `columns` names for each row, or nothing when either is past the room for it.
    pub fn cell(self, row: usize, column: usize, columns: usize) -> Option<FocusId> {
        if column >= columns {
            return None;
        }
        self.id(row.checked_mul(columns)?.checked_add(column)?)
    }

    /// The row and the column `id` names, for a run holding `columns` names for each row, when it is one of the run's names.
    pub fn cell_of(self, id: FocusId, columns: usize) -> Option<(usize, usize)> {
        let index = self.index_of(id)?;
        (columns > 0).then(|| (index / columns, index % columns))
    }
}

/// The controls there is one of, such as the button closing the window; each is declared with [`IdRange::at`] where it is used and listed in [`SINGLE_IDS`].
pub const SINGLES: IdRange = IdRange::first(100);

/// The choices of how the server in the servers page's form is reached.
pub const MACHINE_REACHES: IdRange = SINGLES.after(16);

/// The choices of where a new customization goes.
pub const DRAFT_SCOPES: IdRange = MACHINE_REACHES.after(16);

/// The sections down the left side, in the order of `Section::ALL`.
pub const SECTIONS: IdRange = DRAFT_SCOPES.after(64);

/// How many controls each setting has room for.
pub const PER_SETTING: usize = 8;

/// The controls of every setting, [`PER_SETTING`] for each, in the order of `Setting::ALL`.
pub const SETTING_CONTROLS: IdRange = SECTIONS.after(256 * PER_SETTING as u64);

/// How many controls each action of the keymap has room for.
pub const PER_ACTION: usize = 4;

/// The controls of every action of the keymap, [`PER_ACTION`] for each, in the order of `Action::ALL`.
pub const KEYMAP_CONTROLS: IdRange = SETTING_CONTROLS.after(1024 * PER_ACTION as u64);

/// The rows of the open list of choices of a setting, by their place among the rows shown.
pub const PICKS: IdRange = KEYMAP_CONTROLS.after(64);

/// The boxes a line of a setting written as a list is typed in, one for each such setting.
pub const LIST_FIELDS: IdRange = PICKS.after(16);

/// The buttons adding what is typed in a box of [`LIST_FIELDS`] to its list, one for each setting written as a list.
pub const LIST_ADDS: IdRange = LIST_FIELDS.after(16);

/// How many lines of one list the remove buttons have room for.
pub const LIST_LINES: usize = 1000;

/// The buttons removing a line of a list, [`LIST_LINES`] for each setting written as a list.
pub const LIST_REMOVES: IdRange = LIST_ADDS.after(8 * LIST_LINES as u64);

/// The buttons opening the form for a known server of the servers page.
pub const MACHINE_EDITS: IdRange = LIST_REMOVES.after(1000);

/// The buttons taking a known server out of the servers page.
pub const MACHINE_REMOVES: IdRange = MACHINE_EDITS.after(1000);

/// The buttons installing or removing a language, in the order the languages page lists them.
pub const LANGUAGE_ROWS: IdRange = MACHINE_REMOVES.after(4000);

/// The switches offering an agent for new sessions, by the agent's position.
pub const AGENT_SWITCHES: IdRange = LANGUAGE_ROWS.after(500);

/// The buttons opening the box for an agent's API key, by the agent's position.
pub const KEY_SETS: IdRange = AGENT_SWITCHES.after(500);

/// The buttons forgetting an agent's saved API key, by the agent's position.
pub const KEY_REMOVES: IdRange = KEY_SETS.after(500);

/// The buttons signing an agent's default account in, by the agent's position.
pub const SIGN_INS: IdRange = KEY_REMOVES.after(500);

/// The buttons adding an account to an agent, by the agent's position.
pub const ACCOUNT_ADDS: IdRange = SIGN_INS.after(500);

/// The buttons removing an added account, by the account's place among every agent's added accounts.
pub const ACCOUNT_REMOVES: IdRange = ACCOUNT_ADDS.after(2000);

/// The switches turning a tool server on or off, by its place in the list.
pub const SERVER_SWITCHES: IdRange = ACCOUNT_REMOVES.after(1000);

/// The buttons opening the menu of a tool server, by its place in the list.
pub const SERVER_MORES: IdRange = SERVER_SWITCHES.after(1000);

/// The buttons installing what the tool registry offers, by its place among the offers.
pub const INSTALLS: IdRange = SERVER_MORES.after(1000);

/// The switches turning a customization on or off, by its place among those read.
pub const ITEM_SWITCHES: IdRange = INSTALLS.after(5000);

/// The buttons opening the file of a customization, by its place among those read.
pub const ITEM_OPENS: IdRange = ITEM_SWITCHES.after(5000);

/// The switches turning one tool on or off, counted across every set.
pub const TOOL_SWITCHES: IdRange = ITEM_OPENS.after(20_000);

/// The switches turning a whole set of tools on or off, by the set's place.
pub const SET_SWITCHES: IdRange = TOOL_SWITCHES.after(1000);

/// The headings folding a set of tools away or showing it, by the set's place.
pub const SET_FOLDS: IdRange = SET_SWITCHES.after(1000);

/// The buttons making an agent's default account the one new sessions use, by the agent's place.
pub const DEFAULT_USES: IdRange = SET_FOLDS.after(500);

/// How many buttons each added account has: use it, sign it in, set its key, forget its key, rename it.
pub const ACCOUNT_COLUMNS: usize = 5;

/// The buttons of the added accounts, [`ACCOUNT_COLUMNS`] to a login, by the login's place in the list of logins.
pub const ACCOUNT_CONTROLS: IdRange = DEFAULT_USES.after(2000 * ACCOUNT_COLUMNS as u64);

/// The names after the last run, left free for runs added later; a new run goes before it.
pub const SPARE: IdRange = ACCOUNT_CONTROLS.after(SETTINGS_END - ACCOUNT_CONTROLS.end());

/// Every run of the window, in the order they are laid out.
pub const RANGES: [IdRange; 30] = [
    SINGLES,
    MACHINE_REACHES,
    DRAFT_SCOPES,
    SECTIONS,
    SETTING_CONTROLS,
    KEYMAP_CONTROLS,
    PICKS,
    LIST_FIELDS,
    LIST_ADDS,
    LIST_REMOVES,
    MACHINE_EDITS,
    MACHINE_REMOVES,
    LANGUAGE_ROWS,
    AGENT_SWITCHES,
    KEY_SETS,
    KEY_REMOVES,
    SIGN_INS,
    ACCOUNT_ADDS,
    ACCOUNT_REMOVES,
    SERVER_SWITCHES,
    SERVER_MORES,
    INSTALLS,
    ITEM_SWITCHES,
    ITEM_OPENS,
    TOOL_SWITCHES,
    SET_SWITCHES,
    SET_FOLDS,
    DEFAULT_USES,
    ACCOUNT_CONTROLS,
    SPARE,
];

/// Every control there is one of, which must each lie in [`SINGLES`] and differ from the others.
pub const SINGLE_IDS: [FocusId; 43] = [
    crate::settings::SETTINGS_CLOSE,
    crate::settings::SETTINGS_ADD,
    crate::settings::SETTINGS_SAVE,
    crate::settings::SETTINGS_CANCEL,
    crate::settings::SETTINGS_NAME,
    crate::settings::SETTINGS_TARGET,
    crate::settings::SETTINGS_ENV,
    crate::settings::SETTINGS_SEARCH,
    crate::settings::SETTINGS_SEARCH_GO,
    crate::settings::SETTINGS_TOGGLE_INSTALLED,
    crate::settings::SETTINGS_TOGGLE_AVAILABLE,
    crate::settings::SETTINGS_SCROLL,
    crate::settings::SETTINGS_NAV_SCROLL,
    crate::settings::SETTINGS_MENU_TOGGLE,
    crate::settings::SETTINGS_MENU_EDIT,
    crate::settings::SETTINGS_MENU_COPY,
    crate::settings::SETTINGS_MENU_WEBSITE,
    crate::settings::SETTINGS_MENU_REMOVE,
    crate::settings::SETTINGS_DOCS,
    crate::settings::SETTINGS_FIND,
    crate::settings::SETTINGS_PICK_FILTER,
    crate::settings::SETTINGS_KEYMAP_FIND,
    crate::settings::SETTINGS_FIND_CLEAR,
    crate::settings::SETTINGS_RELEASE,
    crate::settings::SETTINGS_PICK_FIRST,
    crate::settings_machines::MACHINE_ADD,
    crate::settings_machines::MACHINE_SAVE,
    crate::settings_machines::MACHINE_CANCEL,
    crate::settings_machines::MACHINE_NAME,
    crate::settings_machines::MACHINE_ADDRESS,
    crate::settings_machines::MACHINE_TOKEN,
    crate::settings_agents::KEY_FIELD,
    crate::settings_agents::KEY_SAVE,
    crate::settings_agents::KEY_CANCEL,
    crate::settings_agents::NAME_FIELD,
    crate::settings_agents::NAME_SAVE,
    crate::settings_agents::NAME_CANCEL,
    crate::settings_drafts::PROMPT_REPLACE,
    crate::settings_drafts::PROMPT_APPEND,
    crate::settings_drafts::DRAFT_NEW,
    crate::settings_drafts::DRAFT_NAME,
    crate::settings_drafts::DRAFT_DESCRIPTION,
    crate::settings_drafts::DRAFT_CREATE,
];

/// Whether `ranges` start at [`SETTINGS_START`], each where the one before it ends, none empty, and the last ending at [`SETTINGS_END`].
const fn laid_out(ranges: &[IdRange]) -> bool {
    let mut next = SETTINGS_START;
    let mut index = 0;
    while index < ranges.len() {
        if ranges[index].base != next || ranges[index].len == 0 {
            return false;
        }
        next = ranges[index].end();
        index += 1;
    }
    next == SETTINGS_END
}

/// Whether every one of `ids` lies in [`SINGLES`] and no two are the same.
const fn distinct_singles(ids: &[FocusId]) -> bool {
    let mut index = 0;
    while index < ids.len() {
        if !SINGLES.contains(ids[index]) {
            return false;
        }
        let mut other = index + 1;
        while other < ids.len() {
            if ids[index].0 == ids[other].0 {
                return false;
            }
            other += 1;
        }
        index += 1;
    }
    true
}

const _: () = assert!(
    laid_out(&RANGES),
    "the settings ranges overlap, leave a gap or run past SETTINGS_END"
);

const _: () = assert!(
    distinct_singles(&SINGLE_IDS),
    "two single settings controls share a name, or one lies outside SINGLES"
);

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
/// Tests for the layout of the window's names.
mod tests {
    use super::*;

    /// A run names its rows up to its end and none past it, and reads back only its own names.
    #[test]
    fn a_range_is_bounded_on_both_sides() {
        assert_eq!(PICKS.id(0), Some(FocusId(PICKS.base())));
        assert_eq!(PICKS.id(PICKS.len() as usize), None);
        assert_eq!(PICKS.index_of(FocusId(PICKS.base() + 3)), Some(3));
        assert_eq!(PICKS.index_of(FocusId(PICKS.base() - 1)), None);
        assert_eq!(PICKS.index_of(FocusId(PICKS.end())), None);
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

    /// Every name lies in exactly one run, and the runs cover the window's names and nothing else.
    #[test]
    fn every_name_has_one_range() {
        for number in [SETTINGS_START, SETTINGS_START + 99, SETTINGS_END - 1] {
            let holding = RANGES
                .iter()
                .filter(|range| range.contains(FocusId(number)))
                .count();
            assert_eq!(holding, 1, "{number}");
        }
        for range in RANGES {
            let holding = RANGES
                .iter()
                .filter(|other| other.contains(FocusId(range.base())))
                .count();
            assert_eq!(holding, 1);
        }
        assert!(
            !RANGES
                .iter()
                .any(|range| range.contains(FocusId(SETTINGS_END)))
        );
        assert!(crate::settings::is_settings(FocusId(SET_FOLDS.end() - 1)));
        assert!(!crate::settings::is_settings(FocusId(SETTINGS_END)));
        assert!(!crate::settings::is_settings(FocusId(SETTINGS_START - 1)));
    }
}
