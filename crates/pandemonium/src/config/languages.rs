//! What one language changes about how a file is written, over the
//! preferences every language shares.
//!
//! The settings file keeps them under `languages`; a language
//! that says nothing about a setting is written by the shared preference.

use std::collections::BTreeMap;

use pm_text::Indent;

use crate::config::preferences::{Preferences, Step, TAB_SIZES, stepped};

/// What lays a file out when it is saved.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Formatter {
    /// The language servers behind the file.
    #[default]
    LanguageServer,
    /// A program that is given the file on its standard input and answers
    /// with the laid out file on its standard output, as a command line.
    External(String),
    /// Nothing: the file is written as it is.
    Off,
}

/// Which of the formatters a language is laid out by, without the command line of an external one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatterKind {
    /// The language servers behind the file.
    LanguageServer,
    /// A program the file is piped through.
    External,
    /// Nothing.
    Off,
}

impl FormatterKind {
    /// Every kind, in the order they are offered.
    pub const ALL: [Self; 3] = [Self::LanguageServer, Self::External, Self::Off];

    /// What the choice is called.
    pub const fn label(self) -> &'static str {
        match self {
            Self::LanguageServer => "Language Server",
            Self::External => "External",
            Self::Off => "Off",
        }
    }
}

impl Formatter {
    /// Which kind of formatter this is.
    pub const fn kind(&self) -> FormatterKind {
        match self {
            Self::LanguageServer => FormatterKind::LanguageServer,
            Self::External(_) => FormatterKind::External,
            Self::Off => FormatterKind::Off,
        }
    }
}

/// One setting a language can override.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanguageSetting {
    /// How wide a step of indentation and a tab are.
    TabSize,
    /// Whether indentation is written as tabs.
    HardTabs,
    /// Whether the file is laid out when it is saved.
    FormatOnSave,
    /// Whether the imports are put in order when it is saved.
    OrganizeImportsOnSave,
    /// Whether the fixes a server can make on its own are made when it is saved.
    FixOnSave,
    /// Whether the space at the ends of lines goes when it is saved.
    TrimWhitespace,
    /// Whether it always ends in a line break when it is saved.
    FinalNewline,
    /// What lays the file out.
    Formatter,
}

/// The settings one language overrides; what is `None` follows the shared preference.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LanguageOverrides {
    /// How wide a step of indentation and a tab are.
    pub tab_size: Option<usize>,
    /// Whether indentation is written as tabs.
    pub hard_tabs: Option<bool>,
    /// Whether the file is laid out when it is saved.
    pub format_on_save: Option<bool>,
    /// Whether the imports are put in order when it is saved.
    pub organize_imports_on_save: Option<bool>,
    /// Whether the fixes a server can make on its own are made when it is saved.
    pub fix_on_save: Option<bool>,
    /// Whether the space at the ends of lines goes when it is saved.
    pub trim_whitespace: Option<bool>,
    /// Whether it always ends in a line break when it is saved.
    pub final_newline: Option<bool>,
    /// What lays the file out.
    pub formatter: Option<Formatter>,
}

/// Every setting of a language, with the overrides applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageSettings {
    /// How a file in the language is indented, and how wide a tab is.
    pub indent: Indent,
    /// Whether the language says how its files are indented, over what their lines show.
    pub indent_fixed: bool,
    /// Whether the file is laid out when it is saved.
    pub format_on_save: bool,
    /// Whether the imports are put in order when it is saved.
    pub organize_imports_on_save: bool,
    /// Whether the fixes a server can make on its own are made when it is saved.
    pub fix_on_save: bool,
    /// Whether the space at the ends of lines goes when it is saved.
    pub trim_whitespace: bool,
    /// Whether it always ends in a line break when it is saved.
    pub final_newline: bool,
    /// What lays the file out.
    pub formatter: Formatter,
}

impl LanguageOverrides {
    /// Whether nothing is overridden.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Whether `setting` is overridden.
    pub fn sets(&self, setting: LanguageSetting) -> bool {
        match setting {
            LanguageSetting::TabSize => self.tab_size.is_some(),
            LanguageSetting::HardTabs => self.hard_tabs.is_some(),
            LanguageSetting::FormatOnSave => self.format_on_save.is_some(),
            LanguageSetting::OrganizeImportsOnSave => self.organize_imports_on_save.is_some(),
            LanguageSetting::FixOnSave => self.fix_on_save.is_some(),
            LanguageSetting::TrimWhitespace => self.trim_whitespace.is_some(),
            LanguageSetting::FinalNewline => self.final_newline.is_some(),
            LanguageSetting::Formatter => self.formatter.is_some(),
        }
    }

    /// Puts `setting` back to following the shared preference.
    pub fn clear(&mut self, setting: LanguageSetting) {
        match setting {
            LanguageSetting::TabSize => self.tab_size = None,
            LanguageSetting::HardTabs => self.hard_tabs = None,
            LanguageSetting::FormatOnSave => self.format_on_save = None,
            LanguageSetting::OrganizeImportsOnSave => self.organize_imports_on_save = None,
            LanguageSetting::FixOnSave => self.fix_on_save = None,
            LanguageSetting::TrimWhitespace => self.trim_whitespace = None,
            LanguageSetting::FinalNewline => self.final_newline = None,
            LanguageSetting::Formatter => self.formatter = None,
        }
    }
}

impl Preferences {
    /// The settings of the language called `name`, or of a file in no language.
    pub fn language(&self, name: Option<&str>) -> LanguageSettings {
        let overrides = name
            .and_then(|name| self.languages.get(name))
            .cloned()
            .unwrap_or_default();
        LanguageSettings {
            indent: Indent {
                width: overrides.tab_size.unwrap_or(self.tab_size),
                tabs: overrides.hard_tabs.unwrap_or(self.hard_tabs),
            },
            indent_fixed: overrides.tab_size.is_some() || overrides.hard_tabs.is_some(),
            format_on_save: overrides.format_on_save.unwrap_or(self.format_on_save),
            organize_imports_on_save: overrides
                .organize_imports_on_save
                .unwrap_or(self.organize_imports_on_save),
            fix_on_save: overrides.fix_on_save.unwrap_or(self.fix_on_save),
            trim_whitespace: overrides.trim_whitespace.unwrap_or(self.trim_whitespace),
            final_newline: overrides.final_newline.unwrap_or(self.final_newline),
            formatter: overrides.formatter.unwrap_or_default(),
        }
    }

    /// Whether the language called `name` overrides `setting`.
    pub fn overrides(&self, name: &str, setting: LanguageSetting) -> bool {
        self.languages
            .get(name)
            .is_some_and(|overrides| overrides.sets(setting))
    }

    /// Whether the language called `name` overrides anything.
    pub fn customized(&self, name: &str) -> bool {
        self.languages
            .get(name)
            .is_some_and(|overrides| !overrides.is_empty())
    }

    /// Flips `setting` for the language called `name`, when it is a switch.
    pub fn toggle_language(&mut self, name: &str, setting: LanguageSetting) {
        let current = self.language(Some(name));
        let overrides = self.languages.entry(name.to_owned()).or_default();
        match setting {
            LanguageSetting::HardTabs => overrides.hard_tabs = Some(!current.indent.tabs),
            LanguageSetting::FormatOnSave => {
                overrides.format_on_save = Some(!current.format_on_save);
            }
            LanguageSetting::OrganizeImportsOnSave => {
                overrides.organize_imports_on_save = Some(!current.organize_imports_on_save);
            }
            LanguageSetting::FixOnSave => overrides.fix_on_save = Some(!current.fix_on_save),
            LanguageSetting::TrimWhitespace => {
                overrides.trim_whitespace = Some(!current.trim_whitespace);
            }
            LanguageSetting::FinalNewline => {
                overrides.final_newline = Some(!current.final_newline);
            }
            LanguageSetting::TabSize | LanguageSetting::Formatter => {}
        }
        self.forget_empty(name);
    }

    /// Moves a number setting of the language called `name` one `step`.
    pub fn step_language(&mut self, name: &str, setting: LanguageSetting, step: Step) {
        if setting != LanguageSetting::TabSize {
            return;
        }
        let width = self.language(Some(name)).indent.width;
        let moved = stepped(width as f32, step, 1.0, TAB_SIZES) as usize;
        self.languages.entry(name.to_owned()).or_default().tab_size = Some(moved);
    }

    /// Lays the language called `name`'s files out with `formatter`.
    pub fn set_formatter(&mut self, name: &str, formatter: Formatter) {
        self.languages.entry(name.to_owned()).or_default().formatter = Some(formatter);
    }

    /// Puts `setting` of the language called `name` back to following the shared preference.
    pub fn reset_language(&mut self, name: &str, setting: LanguageSetting) {
        if let Some(overrides) = self.languages.get_mut(name) {
            overrides.clear(setting);
        }
        self.forget_empty(name);
    }

    /// Puts every setting of the language called `name` back.
    pub fn reset_language_settings(&mut self, name: &str) {
        self.languages.remove(name);
    }

    /// Drops the entry of `name` once it overrides nothing.
    fn forget_empty(&mut self, name: &str) {
        if self
            .languages
            .get(name)
            .is_some_and(LanguageOverrides::is_empty)
        {
            self.languages.remove(name);
        }
    }

    /// The overrides of every language that has any.
    pub fn language_overrides(&self) -> &BTreeMap<String, LanguageOverrides> {
        &self.languages
    }
}
