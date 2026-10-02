//! Applies language catalogue operations and per-language settings through existing seams.

use pm_text::Language;

use crate::app::{App, Wake, Writing};
use crate::config::languages::{Formatter, FormatterKind, LanguageSetting};
use crate::config::{self, ServerList, Step, StoredServer};
use crate::input::Input;
use crate::picker::{Choice, Kind, Row};
use crate::settings::languages::{ResultMessage, ServerEditor, servers};

impl App {
    /// Fetches catalogue metadata without blocking the window.
    pub(super) fn refresh_language_catalogue(&mut self) {
        self.language_operation(|| ResultMessage::Catalogue(config::extensions::catalogue()));
    }

    /// Installs the selected package through the validated extension installer.
    pub(super) fn install_language_extension(&mut self, index: usize) {
        let Some(entry) = self.languages.catalogue.get(index).cloned() else {
            return;
        };
        self.language_operation(move || {
            ResultMessage::Changed(config::extensions::install(&entry))
        });
    }

    /// Imports a directory chosen in the platform's native file picker.
    pub(super) fn import_language_extension(&mut self) {
        self.language_operation(|| {
            let result = match rfd::FileDialog::new()
                .set_title("Import Language Extension")
                .pick_folder()
            {
                Some(path) => config::extensions::import(&path),
                None => Ok(()),
            };
            ResultMessage::Changed(result)
        });
    }

    /// Installs the remembered import at this position again from its folder.
    pub(super) fn reinstall_language_extension(&mut self, index: usize) {
        let Some(imported) = config::extensions::imported().get(index).cloned() else {
            return;
        };
        self.language_operation(move || {
            ResultMessage::Changed(config::extensions::import(&imported.path))
        });
    }

    /// Removes only the selected user extension.
    pub(super) fn remove_language_extension(&mut self, index: usize) {
        let Some(entry) = config::extensions::installed().get(index).cloned() else {
            return;
        };
        self.language_operation(move || {
            ResultMessage::Changed(config::extensions::remove(&entry.id))
        });
    }

    /// Serializes extension mutations and returns their results on the window thread.
    fn language_operation(&mut self, operation: impl FnOnce() -> ResultMessage + Send + 'static) {
        if self.languages.busy {
            return;
        }
        self.languages.busy = true;
        self.languages.requested = true;
        self.languages.error = None;
        let results = self.languages.results.clone();
        let wake = self.waker(Wake::Install);
        std::thread::spawn(move || {
            results.lock().unwrap().push(operation());
            wake();
        });
        self.request_redraw();
    }

    /// Reports completed operations and updates matching open files and servers.
    pub(super) fn finish_language_operations(&mut self) {
        let results = std::mem::take(&mut *self.languages.results.lock().unwrap());
        for result in results {
            self.languages.busy = false;
            let result = match result {
                ResultMessage::Catalogue(Ok(entries)) => {
                    self.languages.catalogue = entries;
                    if self.picker.as_ref().is_some_and(|picker| {
                        picker.kind() == crate::picker::Kind::LanguageExtensions
                    }) {
                        let rows = self.rows_for(crate::picker::Kind::LanguageExtensions);
                        if let Some(picker) = self.picker.as_mut() {
                            picker.refill_preserving_selection(rows);
                        }
                    }
                    Ok(())
                }
                ResultMessage::Catalogue(Err(error)) => Err(error),
                ResultMessage::Changed(Ok(())) => {
                    for error in config::reload_extensions(&mut self.preferences) {
                        self.notices.trouble(error, None);
                    }
                    self.activate_languages();
                    Ok(())
                }
                ResultMessage::Changed(Err(error)) => Err(error),
            };
            if let Err(error) = result {
                self.languages.error = Some(error.clone());
                self.notices.trouble(error, None);
            }
        }
        self.request_redraw();
    }

    /// Reidentifies open files and starts configured servers in their owning worktrees.
    pub(super) fn activate_languages(&mut self) {
        self.editor.reload_languages();
        self.apply_language_servers();
        self.offered_servers.clear();
        self.offer_missing_servers();
        self.follow_keymap();
        self.store();
        self.request_redraw();
    }

    /// The language the Language Settings section shows: the one picked, else
    /// the one the focused file is written in, else the first there is.
    pub(super) fn settings_language(&self) -> Option<Language> {
        let all = Language::all();
        let named = |name: &str| all.iter().copied().find(|language| language.name() == name);
        self.languages
            .selected
            .and_then(named)
            .or_else(|| {
                self.active_file()
                    .and_then(|file| file.borrow().buffer().language())
            })
            .or_else(|| all.first().copied())
    }

    /// Asks which language the Language Settings section shows.
    pub(super) fn pick_settings_language(&mut self) {
        let current = self.settings_language().map(Language::name);
        let mut languages = Language::all();
        languages.sort_by_key(|language| language.name().to_lowercase());
        let rows = languages
            .into_iter()
            .map(|language| Row {
                section: None,
                detail: match (
                    self.preferences.customized(language.name()),
                    Some(language.name()) == current,
                ) {
                    (true, _) => "Customized".to_owned(),
                    (false, true) => "Current".to_owned(),
                    (false, false) => String::new(),
                },
                label: language.name().to_owned(),
                choice: Choice::SettingsLanguage(language.name()),
                enabled: true,
            })
            .collect();
        self.open_picker_with(Kind::SettingsLanguage, rows, String::new());
    }

    /// Shows the settings of the language called `name`.
    pub(super) fn select_settings_language(&mut self, name: &'static str) {
        self.languages.selected = Some(name);
        self.languages.editor = None;
        self.writing = None;
    }

    /// Flips, steps or puts back a setting of the language being set, and writes it down.
    pub(super) fn change_language_setting(
        &mut self,
        change: impl FnOnce(&mut config::Preferences, &str),
    ) {
        let Some(language) = self.settings_language() else {
            return;
        };
        change(&mut self.preferences, language.name());
        self.follow_preferences();
        self.store();
    }

    /// Flips `setting` of the language being set.
    pub(super) fn toggle_language_setting(&mut self, setting: LanguageSetting) {
        self.change_language_setting(|preferences, name| {
            preferences.toggle_language(name, setting)
        });
    }

    /// Moves `setting` of the language being set one `step`.
    pub(super) fn step_language_setting(&mut self, setting: LanguageSetting, step: Step) {
        self.change_language_setting(|preferences, name| {
            preferences.step_language(name, setting, step);
        });
    }

    /// Puts `setting` of the language being set back to the shared preference.
    pub(super) fn reset_language_setting(&mut self, setting: LanguageSetting) {
        self.change_language_setting(|preferences, name| {
            preferences.reset_language(name, setting);
        });
    }

    /// Puts every setting of the language being set back.
    pub(super) fn reset_language_settings(&mut self) {
        self.change_language_setting(|preferences, name| {
            preferences.reset_language_settings(name);
        });
    }

    /// Lays the language being set out with `kind`, asking for the command of an external one.
    pub(super) fn set_language_formatter(&mut self, kind: FormatterKind) {
        let formatter = match kind {
            FormatterKind::LanguageServer => Formatter::LanguageServer,
            FormatterKind::Off => Formatter::Off,
            FormatterKind::External => return self.ask_language_formatter(),
        };
        self.change_language_setting(|preferences, name| {
            preferences.set_formatter(name, formatter);
        });
    }

    /// Asks for the command line the language being set is piped through.
    pub(super) fn ask_language_formatter(&mut self) {
        let Some(language) = self.settings_language() else {
            return;
        };
        let current = match self.preferences.language(Some(language.name())).formatter {
            Formatter::External(command) => command,
            _ => String::new(),
        };
        self.open_picker_with(
            Kind::LanguageFormatter(language.name()),
            Vec::new(),
            current,
        );
    }

    /// Pipes the files of the language called `name` through `command`.
    pub(super) fn set_external_formatter(&mut self, name: &str, command: &str) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }
        self.preferences
            .set_formatter(name, Formatter::External(command.to_owned()));
        self.follow_preferences();
        self.store();
    }

    /// Opens a form for a new or existing server of the language being set,
    /// keeping its installation recipe.
    pub(super) fn edit_language_server(&mut self, at: Option<usize>) {
        let Some(language) = self.settings_language() else {
            return;
        };
        let original = at.and_then(|at| servers(language, &self.language_servers).get(at).copied());
        let mut fields = [
            Input::one_line("Executable"),
            Input::one_line("Arguments"),
            Input::one_line("Initialization options"),
        ];
        fields[0].set(original.map_or("", |server| server.command));
        fields[1].set(&original.map_or_else(
            || "[]".into(),
            |server| serde_json::to_string(server.arguments).unwrap_or_default(),
        ));
        fields[2].set(original.map_or("{}", |server| server.options));
        self.languages.error = None;
        self.languages.editor = Some(ServerEditor {
            language: language.name(),
            index: at,
            fields,
            original,
        });
        self.write_in(Writing::LanguageServerField(0));
    }

    /// Validates and persists the server form without discarding invalid user input.
    pub(super) fn save_language_server(&mut self) {
        if let Err(error) = self.save_language_server_form() {
            self.languages.error = Some(error);
        }
    }

    /// Parses JSON fields and replaces or appends one effective server declaration.
    fn save_language_server_form(&mut self) -> Result<(), String> {
        let Some(editor) = self.languages.editor.as_ref() else {
            return Ok(());
        };
        let language = Language::all()
            .into_iter()
            .find(|language| language.name() == editor.language)
            .ok_or("This language is no longer installed.")?;
        let command = editor.fields[0].value().trim().to_owned();
        let arguments: Vec<String> = serde_json::from_str(&editor.fields[1].value())
            .map_err(|error| format!("Arguments: {error}"))?;
        let options: serde_norway::Value = serde_json::from_str(&editor.fields[2].value())
            .map_err(|error| format!("Initialization options: {error}"))?;
        let install = editor
            .original
            .filter(|server| server.command == command)
            .and_then(|server| server.install)
            .map(config::recipe::StoredRecipe::of);
        let declaration = StoredServer::Invocation {
            command,
            arguments,
            options: Some(options),
            install,
        };
        declaration.validate()?;
        let mut list = servers(language, &self.language_servers);
        let server = declaration.into_server();
        if let Some(index) = editor.index {
            *list
                .get_mut(index)
                .ok_or("This server is no longer configured.")? = server;
        } else {
            list.push(server);
        }
        self.language_servers
            .insert(language.name().into(), ServerList::Replace(list));
        self.languages.editor = None;
        self.languages.error = None;
        self.writing = None;
        self.apply_language_servers();
        self.offered_servers.clear();
        self.offer_missing_servers();
        self.store();
        Ok(())
    }

    /// Removes one effective server entry of the language being set while retaining all others.
    pub(super) fn remove_language_server(&mut self, at: usize) {
        let Some(language) = self.settings_language() else {
            return;
        };
        let mut list = servers(language, &self.language_servers);
        if at >= list.len() {
            return;
        }
        list.remove(at);
        self.language_servers
            .insert(language.name().into(), ServerList::Replace(list));
        self.apply_language_servers();
        self.store();
    }

    /// Restores the declared server list of the language being set and reconciles its open files.
    pub(super) fn reset_language_servers(&mut self) {
        let Some(language) = self.settings_language() else {
            return;
        };
        self.language_servers.remove(language.name());
        self.activate_languages();
    }

    /// Finds a recipe on configured servers before falling back to built-in recipes.
    pub(super) fn configured_server_recipe(
        &self,
        command: &str,
    ) -> Option<pm_text::install::Recipe> {
        pm_text::Language::all()
            .into_iter()
            .flat_map(|language| servers(language, &self.language_servers))
            .find(|server| server.command == command && server.install.is_some())
            .and_then(|server| server.install)
            .or_else(|| pm_text::install::recipe(command))
    }
}
