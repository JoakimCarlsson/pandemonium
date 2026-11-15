//! The rows of the settings on a page, each with the control its kind of
//! value is changed with, and the groups and search results they are set in.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, Card, CardVariant, Divider, IconButton, Label, LabelVariant, OptionList,
    SectionHeader, SectionHeaderVariant, Segmented, Select, SettingRow, SettingRowVariant, Size,
    Stepper, Switch, TextInput,
};
use pm_shell::{Control, Section, Setting, SettingsChange, SettingsModel, found, on_page};

use crate::settings::Layout;
use crate::settings_actions::SettingsActions;

/// How wide a select box or a text box beside the words of a row is.
const CONTROL_WIDTH: f32 = 220.0;

/// How much of a wide row the words keep at least, beside a row of choices.
const WORDS_AT_LEAST: f32 = 240.0;

/// Emits the row of `setting` as it stands in `model`.
pub(crate) fn setting_row(
    model: &SettingsModel,
    actions: &SettingsActions,
    setting: Setting,
    layout: Layout,
) {
    key(setting, || {
        let (title, about) = setting.words();
        let about = match (&model.browser_note, setting) {
            (Some(note), Setting::EmbeddedBrowser) => format!("{about} {note}"),
            _ => about.to_owned(),
        };
        let beside = matches!(setting.control(model), Control::Switch(_));
        let mut row = SettingRow::new(title, about).variant(if layout.stacked && !beside {
            SettingRowVariant::Stacked
        } else {
            SettingRowVariant::Wide
        });
        if setting.changed(model) {
            row = row.on_reset(actions.send(SettingsChange::Reset(setting)));
        }
        let open = model.page.picker == Some(setting);
        let width = if layout.stacked {
            layout.room.min(CONTROL_WIDTH * 1.5)
        } else {
            CONTROL_WIDTH
        };
        let row = match setting.control(model) {
            Control::Switch(on) => row.control({
                let press = actions.send(SettingsChange::Press(setting, 0));
                move || Switch::new(on).on_change(move |_| press()).show()
            }),
            Control::Number(shown) => row.control({
                let step = actions.with(move |by: i32| {
                    SettingsChange::Press(setting, if by < 0 { 0 } else { 1 })
                });
                move || Stepper::new(shown).on_step(step).show()
            }),
            Control::Text(placeholder) => row.control({
                let text = setting.text(model);
                let change = actions.with(move |text| SettingsChange::Text(setting, text));
                move || {
                    TextInput::new(text)
                        .placeholder(placeholder)
                        .modifier(Modifier::new().width(width))
                        .on_change(change)
                        .show()
                }
            }),
            Control::Choice(names, picked) => {
                let options: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
                let fits = Segmented::width(&options, Size::Sm)
                    <= if layout.stacked {
                        layout.room
                    } else {
                        layout.room - WORDS_AT_LEAST
                    };
                if fits {
                    row.control({
                        let pick = actions.with(move |place| SettingsChange::Press(setting, place));
                        move || Segmented::new(options, picked).on_pick(pick).show()
                    })
                } else {
                    let choices = Choices {
                        shown: options.get(picked).cloned().unwrap_or_default(),
                        options,
                        picked: Some(picked),
                    };
                    picker(model, actions, row, setting, choices, open, width)
                }
            }
            Control::Picker(shown) => {
                let options = setting.options(model);
                let picked = setting.picked(model);
                let choices = Choices {
                    shown,
                    options,
                    picked,
                };
                picker(model, actions, row, setting, choices, open, width)
            }
            Control::List(lines) => row.below({
                let actions = actions.clone();
                move || list(&actions, setting, lines)
            }),
        };
        row.show();
    });
}

/// What a select box offers: the choice it shows, every option and the place of the one picked.
struct Choices {
    /// What the box shows.
    shown: String,
    /// Every option, as it reads.
    options: Vec<String>,
    /// The place of the option picked, when one is.
    picked: Option<usize>,
}

/// `row` with a select box `width` wide for `setting` offering `choices`, and the list of them under it while `open`.
fn picker(
    model: &SettingsModel,
    actions: &SettingsActions,
    row: SettingRow,
    setting: Setting,
    choices: Choices,
    open: bool,
    width: f32,
) -> SettingRow {
    let Choices {
        shown,
        options,
        picked,
    } = choices;
    let toggle = actions.send(SettingsChange::TogglePicker(setting));
    let row = row.control(move || {
        Select::new(shown)
            .open(open)
            .modifier(Modifier::new().width(width))
            .on_click(toggle)
            .show()
    });
    if !open {
        return row;
    }
    let filter = model.page.filter.clone();
    let on_filter = actions.with(SettingsChange::Filter);
    let on_pick = actions.with(move |place| SettingsChange::Pick(setting, place));
    row.below(move || {
        OptionList::new(options, picked)
            .filter(filter)
            .modifier(Modifier::new().fill_max_width())
            .on_filter(on_filter)
            .on_pick(on_pick)
            .show()
    })
}

/// The lines of the list `setting` holds, each with a button taking it out, and a box with a button adding one.
fn list(actions: &SettingsActions, setting: Setting, lines: Vec<String>) {
    let typed = remember(String::new);
    let refused = remember(|| false);
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(pm_components::spacing(6.0)),
        Alignment::Start,
        || {
            if !lines.is_empty() {
                Card::new()
                    .variant(CardVariant::Sunken)
                    .size(Size::Xs)
                    .modifier(Modifier::new().fill_max_width())
                    .show(|| {
                        for (place, line) in lines.iter().enumerate() {
                            key(place, || {
                                row_arranged(
                                    Modifier::new().fill_max_width().height(28.0),
                                    Arrangement::SpacedBy(pm_components::spacing(8.0)),
                                    Alignment::Center,
                                    || {
                                        Label::new(line.clone())
                                            .variant(LabelVariant::Code)
                                            .modifier(Modifier::new().weight(1.0))
                                            .show();
                                        IconButton::new(phosphor::TRASH)
                                            .size(Size::Xs)
                                            .on_click(
                                                actions.send(SettingsChange::RemoveLine(
                                                    setting, place,
                                                )),
                                            )
                                            .show();
                                    },
                                );
                            });
                        }
                    });
            }
            let add: Rc<dyn Fn(String)> = {
                let change = actions.change.clone();
                let typed = typed.clone();
                let refused = refused.clone();
                Rc::new(move |text: String| {
                    if !pm_shell::line_reads(setting, &text) {
                        refused.set(true);
                        return;
                    }
                    change(SettingsChange::AddLine(setting, text));
                    typed.set(String::new());
                })
            };
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(pm_components::spacing(8.0)),
                Alignment::Center,
                || {
                    let keep = typed.clone();
                    let calm = refused.clone();
                    let submit = add.clone();
                    TextInput::new(typed.get())
                        .placeholder(setting.add_placeholder())
                        .code()
                        .modifier(Modifier::new().weight(1.0))
                        .on_change(move |text| {
                            keep.set(text);
                            calm.set(false);
                        })
                        .on_submit(move |text| submit(text))
                        .show();
                    let typed = typed.clone();
                    Button::new(setting.add_label())
                        .icon(phosphor::PLUS)
                        .variant(ButtonVariant::Secondary)
                        .on_click(move || add(typed.get()))
                        .show();
                },
            );
            if refused.get() {
                Label::new(format!(
                    "That does not read as a line of this list ({}).",
                    setting.add_placeholder()
                ))
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .show();
            }
        },
    );
}

/// Emits the settings of the page `section` that show in `model`, with a hairline between each and the next.
pub(crate) fn page_rows(
    model: &SettingsModel,
    actions: &SettingsActions,
    section: Section,
    layout: Layout,
) {
    ruled(
        model,
        actions,
        on_page(section)
            .filter(|setting| model.shown(*setting))
            .collect(),
        layout,
    );
}

/// Emits the rows of `settings`, with a hairline between each and the next.
pub(crate) fn ruled(
    model: &SettingsModel,
    actions: &SettingsActions,
    settings: Vec<Setting>,
    layout: Layout,
) {
    column_with(Modifier::new().fill_max_width(), || {
        for (place, setting) in settings.into_iter().enumerate() {
            if place > 0 {
                Divider::new().show();
            }
            setting_row(model, actions, setting, layout);
        }
    });
}

/// Emits `title` over `rows`, as pages holding several groups set them.
pub(crate) fn group(title: &str, rows: impl FnOnce()) {
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(pm_components::spacing(4.0)),
        Alignment::Start,
        || {
            SectionHeader::new(title)
                .variant(SectionHeaderVariant::Group)
                .show();
            Divider::new().show();
            rows();
        },
    );
}

/// Emits what a search of every setting found, each under the page it is on, or a line saying nothing was.
pub(crate) fn results(model: &SettingsModel, actions: &SettingsActions, layout: Layout) {
    let found: Vec<Setting> = found(&model.page.find)
        .into_iter()
        .filter(|setting| model.shown(*setting))
        .collect();
    if found.is_empty() {
        Label::new("No setting matches.")
            .variant(LabelVariant::Muted)
            .show();
        return;
    }
    let mut pages: Vec<Section> = Vec::new();
    for setting in &found {
        if !pages.contains(&setting.page()) {
            pages.push(setting.page());
        }
    }
    for page in pages {
        let title = match page.parent() {
            Some(parent) => format!("{} › {}", parent.title(), page.title()),
            None => page.title().to_owned(),
        };
        let settings: Vec<Setting> = found
            .iter()
            .copied()
            .filter(|setting| setting.page() == page)
            .collect();
        key(page, || {
            group(&title, || ruled(model, actions, settings, layout));
        });
    }
}
