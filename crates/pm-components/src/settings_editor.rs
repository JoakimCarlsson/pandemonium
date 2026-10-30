//! The editor's pages of the settings window: how files look, how they are edited and saved, what the language server adds, and searching and scrolling; and putting the editor's settings to work in the open files.

use crate::settings::Section;
use crate::state::WorkspaceState;

/// The pages of the editor's settings, in the order the list of sections shows them.
pub(crate) const PAGES: [Section; 6] = [
    Section::EditorAppearance,
    Section::EditorEditing,
    Section::EditorSaving,
    Section::EditorIntelligence,
    Section::EditorSearch,
    Section::EditorVim,
];

impl WorkspaceState {
    /// The state with the editor's settings `settings`, read from where they were kept, put to work.
    pub fn with_editor_settings(mut self, settings: pm_editor::EditorSettings) -> Self {
        self.editor = settings;
        self.editor_loaded = true;
        self.apply_indentation();
        self.apply_vim();
        self
    }

    /// Indents every open file as its language's settings say, then as the editor's do, over the language's own.
    pub(crate) fn apply_indentation(&mut self) {
        let (columns, tabs) = (self.editor.tab_size, self.editor.hard_tabs);
        let machine = self.machine.clone();
        for (_, viewer) in self.every_viewer() {
            if let crate::open_files::Contents::Text(document) = &mut viewer.contents {
                let language = document
                    .language
                    .as_deref()
                    .map(|id| machine.language(id))
                    .unwrap_or_default();
                document
                    .set_indentation(language.tab_size.or(columns), language.hard_tabs.or(tabs));
            }
        }
    }

    /// Whether saving the file written in `language` formats it, as the language's settings say, then as the editor's do.
    pub(crate) fn formats_on_save(&self, language: Option<&str>) -> bool {
        language
            .and_then(|id| self.machine.language(id).format_on_save)
            .unwrap_or(self.editor.format_on_save)
    }
}

impl WorkspaceState {
    /// Whether saving the file written in `language` organizes its imports first, as the language's settings say, then as the editor's do.
    pub(crate) fn organizes_on_save(&self, language: Option<&str>) -> bool {
        language
            .and_then(|id| self.machine.language(id).organize_imports_on_save)
            .unwrap_or(self.editor.organize_imports_on_save)
    }

    /// Whether saving the file written in `language` applies the safe fixes of its language server first, as the language's settings say, then as the editor's do.
    pub(crate) fn fixes_on_save(&self, language: Option<&str>) -> bool {
        language
            .and_then(|id| self.machine.language(id).fix_all_on_save)
            .unwrap_or(self.editor.fix_all_on_save)
    }
}

/// What a page of the editor's settings says about itself under its heading.
pub(crate) fn summary(section: Section) -> &'static str {
    match section {
        Section::Editor => {
            "How files look, how they are edited and saved, and what the language server adds as you write."
        }
        Section::EditorAppearance => {
            "The line numbers, what is marked beside and over the lines, and the caret."
        }
        Section::EditorEditing => {
            "Wrapping, indentation, brackets and quotes, and what happens when a line is broken."
        }
        Section::EditorSaving => "What is done to a file as it is saved, and when it saves itself.",
        Section::EditorIntelligence => {
            "Completions, hovers, signatures and the problems the language server finds."
        }
        Section::EditorSearch => "Finding in a file, and how the view and its scrollbar follow it.",
        Section::EditorVim => {
            "Modal editing with vim's keys: whether it is on, what it shares with the clipboard, the bindings you add, and how replayed typing and syntax objects behave."
        }
        _ => "",
    }
}
