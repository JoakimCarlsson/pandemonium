//! The rows of settings on each page of the settings window, the page of what a search found, and what pressing a row's control or writing in its box does.

use pm_input::FocusId;
use pm_style::{
    Role, border_b, border_color, flex, flex_col, flex_none, font_medium, items_center, pb, pt,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::parts::line;
use crate::settings::{SETTINGS_PICK_FILTER, SETTINGS_PICK_FIRST, Section};
use crate::settings_catalog::{self, LISTS, OPEN_LIST, RESET, Setting};
use crate::settings_ids::{IdRange, LIST_ADDS, LIST_FIELDS, LIST_LINES, LIST_REMOVES, PICKS};
use crate::settings_row::{
    NUMBER_WIDTH, PICKER_WIDTH, RESET_WIDTH, SWITCH_WIDTH, SettingRow, choices, choices_width,
    list, number, picker, picker_list, picker_shown, switch, text_box,
};
use crate::settings_values::Control;
use crate::state::WorkspaceState;

/// Whether `text` reads as an origin agents may be allowed to open: a host, optionally with a port, or a web address that begins with `http://` or `https://`, with no white space and no user name.
fn origin_reads(text: &str) -> bool {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .unwrap_or(text);
    !rest.is_empty()
        && !rest.contains("://")
        && !text.contains(char::is_whitespace)
        && !rest.contains('@')
        && !rest.starts_with(['/', ':'])
}

const _: () = assert!(
    LISTS.len() as u64 <= LIST_FIELDS.len()
        && LISTS.len() as u64 <= LIST_ADDS.len()
        && (LISTS.len() * LIST_LINES) as u64 <= LIST_REMOVES.len(),
    "the ranges of list controls have no room for every setting written as a list"
);

/// The box a line of `setting`'s list is typed in, when it is written as a list.
fn list_field_id(setting: Setting) -> Option<FocusId> {
    LIST_FIELDS.id(setting.list_place()?)
}

/// The button adding what is typed in `setting`'s box to its list, when it is written as a list.
fn list_add_id(setting: Setting) -> Option<FocusId> {
    LIST_ADDS.id(setting.list_place()?)
}

/// The button removing the line at `line` of `setting`'s list, when it is written as a list and its buttons have room for the line.
fn list_remove_id(setting: Setting, line: usize) -> Option<FocusId> {
    LIST_REMOVES.cell(setting.list_place()?, line, LIST_LINES)
}

/// The setting written as a list whose control in `range` is `id`, when it is one.
fn list_of(range: IdRange, id: FocusId) -> Option<Setting> {
    LISTS.get(range.index_of(id)?).copied()
}

/// The setting written as a list, and the line of it, that the remove button `id` removes.
fn list_line_of(id: FocusId) -> Option<(Setting, usize)> {
    let (list, line) = LIST_REMOVES.cell_of(id, LIST_LINES)?;
    Some((*LISTS.get(list)?, line))
}

/// What Enter in the box `id` presses, when `id` is the box of a list or the one narrowing the open list of choices.
pub(crate) fn list_submit_of(id: FocusId) -> Option<FocusId> {
    if id == SETTINGS_PICK_FILTER {
        return Some(SETTINGS_PICK_FIRST);
    }
    list_add_id(list_of(LIST_FIELDS, id)?)
}

/// What the open list of choices of `setting` holds: the choices of a setting picked from a few, or the options of one picked from a long list.
fn picker_options(setting: Setting, state: &WorkspaceState) -> Vec<String> {
    match setting.control(state) {
        Control::Choice(names, _) => names.iter().map(|name| (*name).to_owned()).collect(),
        _ => setting.options(state),
    }
}

impl WorkspaceState {
    /// Carries out pressing the control `id` of a setting, or a choice of the open list, and says whether it was one.
    pub(crate) fn setting_pressed(&mut self, id: FocusId) -> bool {
        if let Some(listed) = list_of(LIST_ADDS, id) {
            self.add_line(listed);
            return true;
        }
        if let Some((listed, place)) = list_line_of(id) {
            self.remove_line(listed, place);
            return true;
        }
        if id == SETTINGS_PICK_FIRST {
            self.pick_row(0);
            return true;
        }
        if let Some(row) = PICKS.index_of(id) {
            self.pick_row(row);
            return true;
        }
        let Some((setting, option)) = Setting::of(id) else {
            return false;
        };
        match option {
            RESET => {
                setting.reset(self);
                self.settings_page.picker = None;
            }
            OPEN_LIST => {
                let open = self.settings_page.picker == Some(setting);
                self.settings_page.picker = (!open).then_some(setting);
                self.settings_page.filter = Field::default();
                return true;
            }
            _ if setting.is_text() || setting.list_place().is_some() => return true,
            option => {
                setting.press(self, option);
                if self.settings_page.picker == Some(setting) {
                    self.settings_page.picker = None;
                }
            }
        }
        self.setting_changed(setting);
        true
    }

    /// Adds what is typed in the box of `listed`'s list to the list, when it reads as a line of it and is not there yet, and empties the box.
    fn add_line(&mut self, listed: Setting) {
        let pattern = self
            .settings_page
            .list_adds
            .get(&listed)
            .map(|field| field.text().trim().to_owned())
            .unwrap_or_default();
        let reads = match listed {
            Setting::VimBindings => crate::vim::binding_reads(&pattern),
            Setting::AgentOrigins => origin_reads(&pattern),
            _ => true,
        };
        if !reads {
            return;
        }
        let mut lines = listed.lines(self);
        if !pattern.is_empty() && !lines.contains(&pattern) {
            lines.push(pattern);
            listed.set_lines(self, &lines);
            self.setting_changed(listed);
        }
        self.settings_page
            .list_adds
            .insert(listed, Field::default());
    }

    /// Takes the line at `place` out of `listed`'s list, when it has one there.
    fn remove_line(&mut self, listed: Setting, place: usize) {
        let mut lines = listed.lines(self);
        if place < lines.len() {
            lines.remove(place);
            listed.set_lines(self, &lines);
            self.setting_changed(listed);
        }
    }

    /// Picks the choice shown at `row` of the open list of choices, as narrowed by what its box holds, and closes the list; nothing happens when no choice shows there.
    fn pick_row(&mut self, row: usize) {
        let Some(setting) = self.settings_page.picker else {
            return;
        };
        let options = picker_options(setting, self);
        let Some(index) = picker_shown(&options, &self.settings_page.filter)
            .get(row)
            .copied()
        else {
            return;
        };
        self.settings_page.picker = None;
        self.settings_page.filter = Field::default();
        match setting.control(self) {
            Control::Choice(..) => setting.press(self, index),
            _ => setting.pick(self, index),
        }
        self.setting_changed(setting);
    }

    /// Writes `edit` into the box `id` of a setting written as text, or the box narrowing the open list, and puts what the box then holds to work.
    pub(crate) fn setting_edited(&mut self, id: FocusId, edit: Edit) {
        if id == SETTINGS_PICK_FILTER {
            self.settings_page.filter.apply(edit);
            return;
        }
        if let Some(listed) = list_of(LIST_FIELDS, id) {
            self.settings_page
                .list_adds
                .entry(listed)
                .or_default()
                .apply(edit);
            return;
        }
        let Some(setting) = Setting::text_of(id) else {
            return;
        };
        let text = setting.text(self);
        let field = self
            .settings_page
            .fields
            .entry(setting)
            .or_insert_with(|| Field::new(text));
        field.apply(edit);
        let written = field.text().to_owned();
        if written != setting.text(self) {
            setting.set_text(self, written);
            self.setting_changed(setting);
        }
    }

    /// The box `id` of a setting written as text, or of a list, or the box narrowing the open list, once it has been written in.
    pub(crate) fn setting_field(&self, id: FocusId) -> Option<&Field> {
        if id == SETTINGS_PICK_FILTER {
            return Some(&self.settings_page.filter);
        }
        if let Some(listed) = list_of(LIST_FIELDS, id) {
            return self.settings_page.list_adds.get(&listed);
        }
        self.settings_page.fields.get(&Setting::text_of(id)?)
    }
}

/// Whether `id` names a box of the settings written as text or as a list, or the one narrowing the open list.
pub(crate) fn is_setting_field(id: FocusId) -> bool {
    id == SETTINGS_PICK_FILTER
        || list_of(LIST_FIELDS, id).is_some()
        || Setting::text_of(id).is_some()
}

/// Whether `setting` shows in `state`: the pickers of themes only once there is more than one theme, and a language's settings only once a language is installed.
fn shown(setting: Setting, state: &WorkspaceState) -> bool {
    match setting {
        Setting::LightTheme | Setting::DarkTheme => pm_theme::families().len() > 1,
        Setting::LanguageTabSize
        | Setting::LanguageIndent
        | Setting::LanguageFormat
        | Setting::LanguageServer
        | Setting::LanguageServerCommand
        | Setting::LanguageServers
        | Setting::LanguageOrganize
        | Setting::LanguageFixAll => state.settings_language().is_some(),
        _ => true,
    }
}

/// The row of `setting` as it stands in `state`, `room` logical pixels across; a control the window has no name for is left out rather than answering to another's.
pub(crate) fn setting_row(state: &WorkspaceState, setting: Setting, room: f32) -> Node {
    let (title, about) = setting.words();
    let changed = setting.changed(state);
    let reserved = if changed { RESET_WIDTH } else { 0.0 };
    let open = state.settings_page.picker == Some(setting);
    let row = SettingRow::new(title, about).reset(changed.then(|| setting.id(RESET)).flatten());
    let opener = setting.id(OPEN_LIST);
    let list_below = |options: &[String], picked: Option<usize>| {
        picker_list(
            options,
            picked,
            (&state.settings_page.filter, SETTINGS_PICK_FILTER),
        )
    };
    let row = match setting.control(state) {
        Control::Switch(on) => row.control(switch(on, setting.id(0)), Some(SWITCH_WIDTH)),
        Control::Number(shown) => match (setting.id(0), setting.id(1)) {
            (Some(smaller), Some(bigger)) => {
                row.control(number(shown, smaller, bigger), Some(NUMBER_WIDTH))
            }
            _ => row,
        },
        Control::Text(placeholder) => {
            let held;
            let field = match state.settings_page.fields.get(&setting) {
                Some(field) => field,
                None => {
                    held = Field::new(setting.text(state));
                    &held
                }
            };
            match setting.id(0) {
                Some(id) => row.control(text_box(field, id, placeholder), None),
                None => row,
            }
        }
        Control::Choice(names, picked) => {
            let width = choices_width(names);
            match opener {
                Some(_) if width <= room - reserved => row.control(
                    choices(names, picked, |option| setting.choice_id(option)),
                    Some(width),
                ),
                Some(opener) => {
                    let options: Vec<String> =
                        names.iter().map(|name| (*name).to_owned()).collect();
                    let width = PICKER_WIDTH.min(room - reserved);
                    row.control(
                        picker(
                            names.get(picked).copied().unwrap_or_default(),
                            width,
                            opener,
                            open,
                        ),
                        Some(width),
                    )
                    .below(open.then(|| list_below(&options, Some(picked))))
                }
                None => row,
            }
        }
        Control::List(lines) => {
            let empty = Field::default();
            let field = state
                .settings_page
                .list_adds
                .get(&setting)
                .unwrap_or(&empty);
            match (list_field_id(setting), list_add_id(setting)) {
                (Some(field_id), Some(add)) => row.below(Some(list(
                    &lines,
                    |line| list_remove_id(setting, line),
                    (field, field_id),
                    (add, setting.add_label()),
                    setting.add_placeholder(),
                ))),
                _ => row,
            }
        }
        Control::Picker(shown) => match opener {
            Some(opener) => {
                let width = PICKER_WIDTH.min(room - reserved);
                row.control(picker(&shown, width, opener, open), Some(width))
                    .below(open.then(|| list_below(&setting.options(state), setting.picked(state))))
            }
            None => row,
        },
    };
    row.build(room)
}

/// `rows` one under another, with a hairline between each and the next.
pub(crate) fn ruled(rows: Vec<Node>) -> Node {
    let last = rows.len().saturating_sub(1);
    node()
        .styles([flex(), flex_col(), flex_none()])
        .children(rows.into_iter().enumerate().map(|(place, row)| {
            if place < last {
                row.styles([border_b(), border_color(Role::Border)])
            } else {
                row
            }
        }))
}

/// The rows of the settings on the page `section` that show in `state`, `room` logical pixels across.
pub(crate) fn page_rows(state: &WorkspaceState, section: Section, room: f32) -> Node {
    ruled(
        settings_catalog::on_page(section)
            .filter(|setting| shown(*setting, state))
            .map(|setting| setting_row(state, setting, room))
            .collect(),
    )
}

/// `title` over the rows of a group of settings, as the pages that hold several groups set it.
pub(crate) fn group(title: &str, rows: Node) -> Node {
    node()
        .styles([flex(), flex_col(), flex_none(), pt(2)])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    flex_none(),
                    pb(1),
                    border_b(),
                    border_color(Role::Border),
                ])
                .child(line(title, 12, 1.4, Role::Muted).styles([font_medium()])),
        )
        .child(rows)
}

/// What a search of every setting found, each under the page it is on, `room` logical pixels across; or a line saying nothing was.
pub(crate) fn results(state: &WorkspaceState, room: f32) -> Vec<Node> {
    let found: Vec<Setting> = settings_catalog::found(state.settings_page.find.text())
        .into_iter()
        .filter(|setting| shown(*setting, state))
        .collect();
    if found.is_empty() {
        return vec![line("No setting matches.", 13, 1.5, Role::Muted)];
    }
    let mut pages: Vec<Section> = Vec::new();
    for setting in &found {
        if !pages.contains(&setting.page()) {
            pages.push(setting.page());
        }
    }
    pages
        .into_iter()
        .map(|page| {
            let title = match page.parent() {
                Some(parent) => format!("{} › {}", parent.title(), page.title()),
                None => page.title().to_owned(),
            };
            group(
                &title,
                ruled(
                    found
                        .iter()
                        .filter(|setting| setting.page() == page)
                        .map(|setting| setting_row(state, *setting, room))
                        .collect(),
                ),
            )
        })
        .collect()
}
