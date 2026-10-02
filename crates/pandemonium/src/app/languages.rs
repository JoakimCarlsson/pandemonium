//! Applies language catalogue operations and server edits through existing seams.

use crate::app::{App, Wake, Writing};
use crate::config::{self, ServerList, StoredServer};
use crate::input::Input;
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

    /// Opens a form for a new or existing server while retaining its installation recipe.
    pub(super) fn edit_language_server(&mut self, index: usize, at: Option<usize>) {
        let Some(language) = pm_text::Language::all().get(index).copied() else {
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
        self.languages.expanded = Some(language.name().into());
        self.languages.installed_open = true;
        self.languages.editor = Some(ServerEditor {
            language: language.name().into(),
            index: at,
            fields,
            original,
        });
        self.write_in(Writing::LanguageField(1));
    }

    /// Validates and persists a server form without discarding invalid user input.
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
        let language = pm_text::Language::called(&editor.language)
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

    /// Removes one effective server entry while retaining all others.
    pub(super) fn remove_language_server(&mut self, index: usize, at: usize) {
        let Some(language) = pm_text::Language::all().get(index).copied() else {
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

    /// Restores a language's declared server list and reconciles its open files.
    pub(super) fn reset_language_servers(&mut self, index: usize) {
        let Some(language) = pm_text::Language::all().get(index).copied() else {
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
