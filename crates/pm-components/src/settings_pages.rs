//! The rows of settings on each page of the settings window, the page of what a search found, and what pressing a row's control or writing in its box does.

use pm_input::FocusId;
use pm_style::{
    Role, border_b, border_color, flex, flex_col, flex_none, font_medium, items_center, pb, pt,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::parts::line;
use crate::settings::{SETTINGS_PICK_FILTER, Section};
use crate::settings_catalog::{
    self, LIST_ADD, LIST_FIELD, OPEN_LIST, RESET, Setting, list_remove_id, list_remove_of, pick_id,
    pick_of,
};
use crate::settings_row::{
    NUMBER_WIDTH, PICKER_WIDTH, RESET_WIDTH, SWITCH_WIDTH, SettingRow, choices, choices_width,
    list, number, picker, picker_list, switch, text_box,
};
use crate::settings_values::Control;
use crate::state::WorkspaceState;

impl WorkspaceState {
    /// Carries out pressing the control `id` of a setting, or a choice of the open list, and says whether it was one.
    pub(crate) fn setting_pressed(&mut self, id: FocusId) -> bool {
        // the box and the buttons of a list are shared, so the list they work on is the one of the page that is open
        let listed = if self.settings == Some(Section::EditorVim) {
            Setting::VimBindings
        } else {
            Setting::WorktreeInclude
        };
        if id == LIST_ADD {
            let pattern = self.settings_page.list_add.text().trim().to_owned();
            if listed == Setting::VimBindings && !crate::vim::binding_reads(&pattern) {
                return true;
            }
            let mut lines = listed.lines(self);
            if !pattern.is_empty() && !lines.contains(&pattern) {
                lines.push(pattern);
                listed.set_lines(self, &lines);
                self.setting_changed(listed);
            }
            self.settings_page.list_add = Field::default();
            return true;
        }
        if let Some(place) = list_remove_of(id) {
            let mut lines = listed.lines(self);
            if place < lines.len() {
                lines.remove(place);
                listed.set_lines(self, &lines);
                self.setting_changed(listed);
            }
            return true;
        }
        if let Some(index) = pick_of(id) {
            if let Some(setting) = self.settings_page.picker.take() {
                setting.pick(self, index);
                self.settings_page.filter = Field::default();
                self.setting_changed(setting);
            }
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
            _ if setting.is_text() => return true,
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

    /// Writes `edit` into the box `id` of a setting written as text, or the box narrowing the open list, and puts what the box then holds to work.
    pub(crate) fn setting_edited(&mut self, id: FocusId, edit: Edit) {
        if id == SETTINGS_PICK_FILTER {
            self.settings_page.filter.apply(edit);
            return;
        }
        if id == LIST_FIELD {
            self.settings_page.list_add.apply(edit);
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

    /// The box `id` of a setting written as text, or the box narrowing the open list, once it has been written in.
    pub(crate) fn setting_field(&self, id: FocusId) -> Option<&Field> {
        if id == SETTINGS_PICK_FILTER {
            return Some(&self.settings_page.filter);
        }
        if id == LIST_FIELD {
            return Some(&self.settings_page.list_add);
        }
        self.settings_page.fields.get(&Setting::text_of(id)?)
    }
}

/// Whether `id` names a box of the settings written as text, or the one narrowing the open list.
pub(crate) fn is_setting_field(id: FocusId) -> bool {
    id == SETTINGS_PICK_FILTER || id == LIST_FIELD || Setting::text_of(id).is_some()
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
        | Setting::LanguageServers => state.settings_language().is_some(),
        _ => true,
    }
}

/// The row of `setting` as it stands in `state`, `room` logical pixels across.
pub(crate) fn setting_row(state: &WorkspaceState, setting: Setting, room: f32) -> Node {
    let (title, about) = setting.words();
    let changed = setting.changed(state);
    let reserved = if changed { RESET_WIDTH } else { 0.0 };
    let open = state.settings_page.picker == Some(setting);
    let row = SettingRow::new(title, about).reset(changed.then(|| setting.id(RESET)));
    let row = match setting.control(state) {
        Control::Switch(on) => row.control(switch(on, setting.id(0)), Some(SWITCH_WIDTH)),
        Control::Number(shown) => row.control(
            number(shown, setting.id(0), setting.id(1)),
            Some(NUMBER_WIDTH),
        ),
        Control::Text(placeholder) => {
            let held;
            let field = match state.settings_page.fields.get(&setting) {
                Some(field) => field,
                None => {
                    held = Field::new(setting.text(state));
                    &held
                }
            };
            row.control(text_box(field, setting.id(0), placeholder), None)
        }
        Control::Choice(names, picked) => {
            let width = choices_width(names);
            if width <= room - reserved {
                row.control(
                    choices(names, picked, |option| setting.id(option)),
                    Some(width),
                )
            } else {
                let options: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
                let width = PICKER_WIDTH.min(room - reserved);
                row.control(
                    picker(
                        names.get(picked).copied().unwrap_or_default(),
                        width,
                        setting.id(OPEN_LIST),
                        open,
                    ),
                    Some(width),
                )
                .below(open.then(|| {
                    picker_list(
                        &options,
                        Some(picked),
                        |option| setting.id(option),
                        (&state.settings_page.filter, SETTINGS_PICK_FILTER),
                    )
                }))
            }
        }
        Control::List(lines) => row.below(Some(list(
            &lines,
            list_remove_id,
            (&state.settings_page.list_add, LIST_FIELD),
            (LIST_ADD, setting.add_label()),
            setting.add_placeholder(),
        ))),
        Control::Picker(shown) => {
            let width = PICKER_WIDTH.min(room - reserved);
            row.control(
                picker(&shown, width, setting.id(OPEN_LIST), open),
                Some(width),
            )
            .below(open.then(|| {
                picker_list(
                    &setting.options(state),
                    setting.picked(state),
                    pick_id,
                    (&state.settings_page.filter, SETTINGS_PICK_FILTER),
                )
            }))
        }
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
