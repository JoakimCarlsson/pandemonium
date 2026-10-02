//! The Language Settings section: how files of one language are indented,
//! saved and served, over what every language shares.
//!
//! The language is chosen first, then
//! each setting is a row that follows the shared preference until it is
//! changed, and carries an undo mark while it is not following it.

use pm_text::{Language, Server};
use pm_ui::{
    Div, IconName, Styled, Theme, button, h_flex, icon_button, space, switch, text, toggle_row,
    v_flex,
};

use crate::config::languages::{Formatter, FormatterKind, LanguageSetting};
use crate::config::{Preferences, Step};
use crate::input::input_view;
use crate::message::Message;
use crate::settings::languages::{LanguagesPage, servers};
use crate::settings::page::action;
use crate::settings::parts::{badge, clipped};

/// How wide a row's choice of formatter is, in spaces.
const CHOICE_WIDTH: f32 = 110.0;

/// How wide the number of a stepper is, in spaces.
const VALUE_WIDTH: f32 = 14.0;

/// How wide the button naming the language is, in spaces.
const LANGUAGE_WIDTH: f32 = 48.0;

/// The rows of the section, top to bottom.
pub fn language_settings_rows(theme: &Theme, page: &LanguagesPage<'_>) -> Vec<Div<Message>> {
    let Some(language) = page.selected else {
        return vec![action(
            theme,
            "Language",
            "No language is installed.",
            h_flex(),
        )];
    };
    let name = language.name();
    let preferences = page.preferences;
    let settings = preferences.language(Some(name));
    let row = |setting, title: &str, description: &str, control: Div<Message>| {
        setting_row(
            theme,
            preferences,
            name,
            setting,
            title,
            description,
            control,
        )
    };
    let toggle = |setting, title: &str, description: &str, on: bool| {
        row(
            setting,
            title,
            description,
            h_flex().child(switch(on, Message::ToggleLanguageSetting(setting))),
        )
    };
    let mut rows = vec![
        action(
            theme,
            "Language",
            "The language the settings below apply to",
            h_flex()
                .gap(2)
                .items_center()
                .when(preferences.customized(name), |controls| {
                    controls.child(button("Reset All", Message::ResetLanguageSettings).ghost())
                })
                .child(
                    button(name, Message::PickSettingsLanguage)
                        .outlined()
                        .w_px(space(LANGUAGE_WIDTH)),
                ),
        ),
        row(
            LanguageSetting::TabSize,
            "Tab Size",
            "How wide a step of indentation and a tab are, where a file does not say",
            stepper(theme, settings.indent.width),
        ),
        toggle(
            LanguageSetting::HardTabs,
            "Hard Tabs",
            "Write indentation as tabs rather than spaces, where a file does not say",
            settings.indent.tabs,
        ),
        toggle(
            LanguageSetting::FormatOnSave,
            "Format on Save",
            "Lay a file out with its formatter every time it is written",
            settings.format_on_save,
        ),
        row(
            LanguageSetting::Formatter,
            "Formatter",
            "What lays a file out: its language servers, or a program the file is piped through",
            formatter_choice(&settings.formatter),
        ),
    ];
    if let Formatter::External(command) = &settings.formatter {
        rows.push(action(
            theme,
            "Formatter Command",
            "Gets the file on its standard input; {path} stands for the file's path",
            button(clipped(command), Message::AskLanguageFormatter).outlined(),
        ));
    }
    rows.extend([
        toggle(
            LanguageSetting::OrganizeImportsOnSave,
            "Organize Imports on Save",
            "Have the language server put a file's imports in order every time it is written",
            settings.organize_imports_on_save,
        ),
        toggle(
            LanguageSetting::FixOnSave,
            "Fix on Save",
            "Have the language server make the fixes it can make on its own every time a file is written",
            settings.fix_on_save,
        ),
        toggle(
            LanguageSetting::TrimWhitespace,
            "Remove Trailing Whitespace",
            "Take the spaces and tabs off the ends of lines when a file is saved",
            settings.trim_whitespace,
        ),
        toggle(
            LanguageSetting::FinalNewline,
            "Ensure Final Newline",
            "End a file with a line break when it is saved",
            settings.final_newline,
        ),
        text_row(theme, "Language Servers", "What is started for a file in the language"),
    ]);
    rows.extend(server_rows(theme, page, language));
    rows
}

/// A row of a setting: its name with the undo mark while it is set away
/// from the shared preference, what it does, and its control.
fn setting_row(
    theme: &Theme,
    preferences: &Preferences,
    name: &str,
    setting: LanguageSetting,
    title: &str,
    description: &str,
    control: Div<Message>,
) -> Div<Message> {
    h_flex()
        .w_full()
        .py(3)
        .gap(6)
        .items_center()
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .gap(0.5)
                .child(
                    h_flex()
                        .h_px(theme.size.icon_control)
                        .gap(1)
                        .items_center()
                        .child(text(title.to_owned()).font_medium())
                        .when(preferences.overrides(name, setting), |line| {
                            line.child(
                                icon_button(
                                    theme,
                                    IconName::Undo,
                                    Message::ResetLanguageSetting(setting),
                                )
                                .tooltip("Follow the shared setting"),
                            )
                        }),
                )
                .child(
                    text(description.to_owned())
                        .text_sm()
                        .color(theme.colors.text_muted),
                ),
        )
        .child(control)
}

/// A line that names the group of rows after it.
fn text_row(theme: &Theme, title: &str, description: &str) -> Div<Message> {
    v_flex()
        .w_full()
        .py(3)
        .gap(0.5)
        .child(text(title.to_owned()).font_medium())
        .child(
            text(description.to_owned())
                .text_sm()
                .color(theme.colors.text_muted),
        )
}

/// A number with a step down and a step up either side of it.
fn stepper(theme: &Theme, value: usize) -> Div<Message> {
    let step = |icon, tip, step| {
        icon_button(
            theme,
            icon,
            Message::StepLanguageSetting(LanguageSetting::TabSize, step),
        )
        .tooltip(tip)
    };
    h_flex()
        .gap(1)
        .items_center()
        .child(step(IconName::Minus, "Less", Step::Down))
        .child(
            h_flex()
                .w_px(space(VALUE_WIDTH))
                .justify_center()
                .child(text(value.to_string()).text_sm().font_mono()),
        )
        .child(step(IconName::Plus, "More", Step::Up))
}

/// The kinds of formatter, the one the language uses lit.
fn formatter_choice(formatter: &Formatter) -> Div<Message> {
    let selected = FormatterKind::ALL
        .iter()
        .position(|kind| *kind == formatter.kind());
    toggle_row(
        FormatterKind::ALL.into_iter().map(|kind| {
            let message = match kind {
                FormatterKind::External => Message::AskLanguageFormatter,
                other => Message::SetLanguageFormatter(other),
            };
            (kind.label().to_owned(), message)
        }),
        selected,
    )
    .w_px(space(CHOICE_WIDTH))
}

/// The servers of the language, the way to add one, and the form while one is open.
fn server_rows(theme: &Theme, page: &LanguagesPage<'_>, language: Language) -> Vec<Div<Message>> {
    let mut rows = servers(language, page.servers)
        .iter()
        .enumerate()
        .map(|(at, server)| server_row(theme, at, server))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        rows.push(text_row(
            theme,
            "No language server",
            "Add one to get diagnostics and navigation",
        ));
    }
    match server_form(theme, page, language) {
        Some(form) => rows.push(form),
        None => rows.push(
            h_flex()
                .w_full()
                .py(3)
                .gap(1.5)
                .child(button("Add Server", Message::AddLanguageServer).outlined())
                .when(page.servers.contains_key(language.name()), |controls| {
                    controls
                        .child(button("Restore Defaults", Message::ResetLanguageServers).ghost())
                }),
        ),
    }
    rows
}

/// One server: its command, whether it can run, and what can be done to it.
fn server_row(theme: &Theme, at: usize, server: &Server) -> Div<Message> {
    let available = pm_text::program::installed_with_recipe(server.command, server.install)
        .or_else(|| pm_text::program::managed_fallback(server.command))
        .is_some();
    let (standing, tone) = match available {
        true => ("Installed", theme.colors.accent),
        false => ("Not installed", theme.colors.text_subtle),
    };
    h_flex()
        .w_full()
        .py(3)
        .gap(4)
        .items_center()
        .justify_between()
        .child(
            h_flex()
                .flex_1()
                .gap(1.5)
                .items_center()
                .overflow_hidden()
                .child(text(clipped(server.command)).font_mono().text_sm())
                .child(badge(theme, standing, tone)),
        )
        .child(
            h_flex()
                .gap(1.5)
                .items_center()
                .when(!available && server.install.is_some(), |controls| {
                    controls.child(
                        button("Install", Message::InstallLanguageServer(server.command)).filled(),
                    )
                })
                .child(button("Edit", Message::EditLanguageServer(at)).outlined())
                .child(button("Remove", Message::RemoveLanguageServer(at)).outlined()),
        )
}

/// The form a server is described in, while one is open for `language`.
fn server_form(
    theme: &Theme,
    page: &LanguagesPage<'_>,
    language: Language,
) -> Option<Div<Message>> {
    let editor = page
        .state
        .editor
        .as_ref()
        .filter(|editor| editor.language == language.name())?;
    let mut form = v_flex().w_full().py(3).gap(2.5);
    for (index, label) in [
        "Executable",
        "Arguments (JSON array)",
        "Initialization options (JSON object)",
    ]
    .into_iter()
    .enumerate()
    {
        form = form.child(
            v_flex()
                .w_full()
                .gap(0.5)
                .child(text(label).text_sm().color(theme.colors.text_muted))
                .child(
                    input_view(
                        theme,
                        &editor.fields[index],
                        page.field == Some(index),
                        page.solid,
                        1.0,
                        move |phase, anchor, head| {
                            Message::WriteLanguageServerField(index, phase, anchor, head)
                        },
                        Message::ShowInputMenu,
                    )
                    .on_click(Message::FocusLanguageServerField(index)),
                ),
        );
    }
    if let Some(error) = page.state.error.as_ref() {
        form = form.child(text(error.clone()).text_sm().color(theme.colors.text_muted));
    }
    Some(
        form.child(
            h_flex()
                .gap(1.5)
                .child(button("Save", Message::SaveLanguageServer).filled())
                .child(button("Cancel", Message::CancelLanguageServer).outlined()),
        ),
    )
}
