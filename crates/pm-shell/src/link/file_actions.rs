//! The link's part in what can be done with an open file besides editing
//! it; reading what the folder holds goes through the server, so it works
//! the same for remote projects.

use pm_editor::{FIX_ALL, ORGANIZE_IMPORTS, ServerAsk};
use pm_protocol::Request;

use super::code::CodeAsk;
use super::{Ask, Link};
use crate::file_actions::FileAction;
use crate::model::RightTab;
use crate::open_files::{Comparison, SessionTab};
use crate::quick::Choosing;

impl Link {
    /// Carries out `action` on the open file at `index`.
    pub fn file_action(&mut self, index: usize, action: FileAction) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        let (directory, path) = (file.directory.clone(), file.path.clone());
        let text = file.document.0.borrow().field.text().to_owned();
        let whole = file.document.0.borrow().whole_span();
        self.workspace.open_files.menu = None;
        match action {
            FileAction::Reveal => {
                self.workspace.tree.selected = Some(path.clone());
                let mut folder = String::new();
                for part in path
                    .split('/')
                    .collect::<Vec<_>>()
                    .split_last()
                    .map_or(&[][..], |(_, rest)| rest)
                {
                    if !folder.is_empty() {
                        folder.push('/');
                    }
                    folder.push_str(part);
                    self.workspace.tree.expanded.insert(folder.clone());
                }
                self.workspace.right_open = true;
                self.choose_right_tab(RightTab::Files);
            }
            FileAction::History => {
                self.workspace.open_files.tab = SessionTab::File(index);
                self.open_choices(Choosing::History);
            }
            FileAction::CompareClipboard(clipboard) => {
                self.show_comparison(index, self.comparison("Clipboard", &clipboard, &text));
            }
            FileAction::CompareSaved => self.ask(
                Request::ReadFile {
                    directory: directory.clone(),
                    path: path.clone(),
                    head: false,
                    revision: None,
                },
                Ask::Code(CodeAsk::Compare(directory, path)),
            ),
            FileAction::CompareCommitted => match file.base.clone() {
                Some(base) => {
                    self.show_comparison(index, self.comparison("Last commit", &base, &text))
                }
                None => self.workspace.status = Some("No commit holds this file yet.".to_owned()),
            },
            FileAction::ChooseCompare => {
                self.workspace.open_files.tab = SessionTab::File(index);
                self.open_choices(Choosing::CompareFile);
            }
            FileAction::CompareWith(other) => {
                let Some(other) = self.workspace.open_files.files.get(other) else {
                    return;
                };
                let label = other.path.clone();
                let held = other.document.0.borrow().field.text().to_owned();
                self.show_comparison(index, self.comparison(label, &held, &text));
            }
            FileAction::OrganizeImports => self.code(
                index,
                ServerAsk::CodeActions(whole, Some(vec![ORGANIZE_IMPORTS.to_owned()])),
            ),
            FileAction::FixAll => self.code(
                index,
                ServerAsk::CodeActions(whole, Some(vec![FIX_ALL.to_owned()])),
            ),
            FileAction::Format => self.code(index, ServerAsk::Format(None)),
            FileAction::InstallLanguage(id) => self.press_extension(&id),
            FileAction::DeclineLanguage(id) => self.decline_language(id),
        }
    }

    /// Stops offering to install the language `id`, now and in later runs, by keeping it in the window's settings.
    fn decline_language(&mut self, id: String) {
        self.workspace.open_files.decline(&id);
        let declined = &mut self.settings.window.general.declined_languages;
        if !declined.contains(&id) {
            declined.push(id);
            self.settings_changed = true;
        }
    }

    /// How `text` differs from `other`, called `label`, leaving out white space as the editor settings say.
    pub(super) fn comparison(
        &self,
        label: impl Into<String>,
        other: &str,
        text: &str,
    ) -> Comparison {
        Comparison::between_with(
            label,
            other,
            text,
            self.settings.editor.compare_ignore_whitespace,
        )
    }

    /// Reads the file at `other` of the shown file's folder from the server, to compare the shown file with it.
    pub(super) fn compare_with_file(&mut self, other: String) {
        let Some(file) = self.workspace.open_files.shown() else {
            return;
        };
        let directory = file.directory.clone();
        let path = file.path.clone();
        self.ask(
            Request::ReadFile {
                directory: directory.clone(),
                path: other.clone(),
                head: false,
                revision: None,
            },
            Ask::Code(CodeAsk::CompareFile(directory, path, other)),
        );
    }

    /// Shows `comparison` in place of the text of the open file at `index`,
    /// or says the two are the same.
    pub(super) fn show_comparison(&mut self, index: usize, comparison: Comparison) {
        if comparison.is_same() {
            self.workspace.status = Some(format!("The file is the same as {}.", comparison.label));
            return;
        }
        if let Some(file) = self.workspace.open_files.files.get_mut(index) {
            file.compare = Some(comparison);
            file.preview = false;
        }
        self.workspace.open_files.tab = SessionTab::File(index);
    }
}
