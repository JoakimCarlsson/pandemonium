//! The keys that work the editor: what each action is, the presets that bind keys to them as other editors do, the reader's own bindings over a preset, and finding the action a press of keys asks for.
//!
//! A binding is written as the chords it takes, one after another, each its
//! modifiers and its key joined by `+`: `ctrl+shift+k`, or `ctrl+k ctrl+i`
//! for two in a row. Keys are written as they are on a keyboard laid out in
//! US English, so `ctrl+shift+[` is the binding a press of Control, Shift
//! and the key that types `[` makes, whatever that key types with Shift.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Something the keys can ask the editor to do.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Action {
    /// Goes to where the name at the caret is defined.
    GoToDefinition,
    /// Goes to where it is declared.
    GoToDeclaration,
    /// Goes to where its type is defined.
    GoToTypeDefinition,
    /// Goes to where it is implemented.
    GoToImplementation,
    /// Lists where it is used.
    FindReferences,
    /// Lists the functions that call it.
    IncomingCalls,
    /// Gives it a new name across the project.
    Rename,
    /// Formats the file.
    Format,
    /// Lists the changes the language server offers.
    CodeActions,
    /// Shows what the language server says about it.
    Hover,
    /// Lists what could be written at the caret.
    Complete,
    /// Shows the signature of the call the caret is in.
    SignatureHelp,
    /// Goes back to where the caret was before a jump.
    GoBack,
    /// Goes forward again.
    GoForward,
    /// Comments the lines out, or back in.
    ToggleComment,
    /// Indents the lines.
    Indent,
    /// Outdents the lines.
    Outdent,
    /// Moves the lines up.
    MoveLineUp,
    /// Moves the lines down.
    MoveLineDown,
    /// Writes the lines again above.
    DuplicateLineUp,
    /// Writes the lines again below.
    DuplicateLineDown,
    /// Deletes the lines.
    DeleteLine,
    /// Grows the selection along the syntax tree.
    ExpandSelection,
    /// Shrinks it back.
    ShrinkSelection,
    /// Goes to the next problem.
    NextProblem,
    /// Goes to the problem before.
    PreviousProblem,
    /// Selects the next place the selection comes as well.
    AddNextOccurrence,
    /// Selects every place it comes.
    SelectAllOccurrences,
    /// Adds a caret on the line above.
    AddCaretAbove,
    /// Adds a caret on the line below.
    AddCaretBelow,
    /// Selects the line.
    SelectLine,
    /// Joins the line with the next.
    JoinLines,
    /// Goes to the matching bracket.
    MatchingBracket,
    /// Starts a new line below.
    NewlineBelow,
    /// Starts a new line above.
    NewlineAbove,
    /// Folds the block around the caret.
    Fold,
    /// Unfolds it.
    Unfold,
    /// Finds in the file.
    Find,
    /// Finds and replaces in the file.
    Replace,
    /// Selects the next match.
    FindNext,
    /// Selects the match before.
    FindPrevious,
    /// Goes to a line by its number.
    GoToLine,
    /// Lists the items of the file.
    Outline,
    /// Lists the symbols of the project.
    ProjectSymbols,
    /// Finds a file of the project.
    FileFinder,
    /// Searches the files of the project.
    SearchProject,
    /// Lists the problems of the project.
    Problems,
    /// Shows the file in the file manager.
    RevealInFileManager,
    /// Saves the file.
    Save,
}

impl Action {
    /// Every action, in the order the keymap lists them.
    pub const ALL: [Self; 49] = [
        Self::GoToDefinition,
        Self::GoToDeclaration,
        Self::GoToTypeDefinition,
        Self::GoToImplementation,
        Self::FindReferences,
        Self::IncomingCalls,
        Self::GoBack,
        Self::GoForward,
        Self::Rename,
        Self::Format,
        Self::CodeActions,
        Self::Hover,
        Self::Complete,
        Self::SignatureHelp,
        Self::ToggleComment,
        Self::Indent,
        Self::Outdent,
        Self::MoveLineUp,
        Self::MoveLineDown,
        Self::DuplicateLineUp,
        Self::DuplicateLineDown,
        Self::DeleteLine,
        Self::JoinLines,
        Self::NewlineBelow,
        Self::NewlineAbove,
        Self::ExpandSelection,
        Self::ShrinkSelection,
        Self::SelectLine,
        Self::AddNextOccurrence,
        Self::SelectAllOccurrences,
        Self::AddCaretAbove,
        Self::AddCaretBelow,
        Self::MatchingBracket,
        Self::Fold,
        Self::Unfold,
        Self::NextProblem,
        Self::PreviousProblem,
        Self::Find,
        Self::Replace,
        Self::FindNext,
        Self::FindPrevious,
        Self::GoToLine,
        Self::Outline,
        Self::ProjectSymbols,
        Self::FileFinder,
        Self::SearchProject,
        Self::Problems,
        Self::RevealInFileManager,
        Self::Save,
    ];

    /// The name it is kept under in the settings.
    pub fn key(self) -> &'static str {
        match self {
            Self::GoToDefinition => "go_to_definition",
            Self::GoToDeclaration => "go_to_declaration",
            Self::GoToTypeDefinition => "go_to_type_definition",
            Self::GoToImplementation => "go_to_implementation",
            Self::FindReferences => "find_references",
            Self::IncomingCalls => "incoming_calls",
            Self::Rename => "rename",
            Self::Format => "format",
            Self::CodeActions => "code_actions",
            Self::Hover => "hover",
            Self::Complete => "complete",
            Self::SignatureHelp => "signature_help",
            Self::GoBack => "go_back",
            Self::GoForward => "go_forward",
            Self::ToggleComment => "toggle_comment",
            Self::Indent => "indent",
            Self::Outdent => "outdent",
            Self::MoveLineUp => "move_line_up",
            Self::MoveLineDown => "move_line_down",
            Self::DuplicateLineUp => "duplicate_line_up",
            Self::DuplicateLineDown => "duplicate_line_down",
            Self::DeleteLine => "delete_line",
            Self::ExpandSelection => "expand_selection",
            Self::ShrinkSelection => "shrink_selection",
            Self::NextProblem => "next_problem",
            Self::PreviousProblem => "previous_problem",
            Self::AddNextOccurrence => "add_next_occurrence",
            Self::SelectAllOccurrences => "select_all_occurrences",
            Self::AddCaretAbove => "add_caret_above",
            Self::AddCaretBelow => "add_caret_below",
            Self::SelectLine => "select_line",
            Self::JoinLines => "join_lines",
            Self::MatchingBracket => "matching_bracket",
            Self::NewlineBelow => "newline_below",
            Self::NewlineAbove => "newline_above",
            Self::Fold => "fold",
            Self::Unfold => "unfold",
            Self::Find => "find",
            Self::Replace => "replace",
            Self::FindNext => "find_next",
            Self::FindPrevious => "find_previous",
            Self::GoToLine => "go_to_line",
            Self::Outline => "outline",
            Self::ProjectSymbols => "project_symbols",
            Self::FileFinder => "file_finder",
            Self::SearchProject => "search_project",
            Self::Problems => "problems",
            Self::RevealInFileManager => "reveal_in_file_manager",
            Self::Save => "save",
        }
    }

    /// What the keymap calls it.
    pub fn title(self) -> &'static str {
        match self {
            Self::GoToDefinition => "Go to Definition",
            Self::GoToDeclaration => "Go to Declaration",
            Self::GoToTypeDefinition => "Go to Type Definition",
            Self::GoToImplementation => "Go to Implementation",
            Self::FindReferences => "Find All References",
            Self::IncomingCalls => "Show Incoming Calls",
            Self::Rename => "Rename Symbol",
            Self::Format => "Format Buffer",
            Self::CodeActions => "Show Code Actions",
            Self::Hover => "Show Hover",
            Self::Complete => "Show Completions",
            Self::SignatureHelp => "Show Signature Help",
            Self::GoBack => "Go Back",
            Self::GoForward => "Go Forward",
            Self::ToggleComment => "Toggle Comment",
            Self::Indent => "Indent",
            Self::Outdent => "Outdent",
            Self::MoveLineUp => "Move Line Up",
            Self::MoveLineDown => "Move Line Down",
            Self::DuplicateLineUp => "Duplicate Line Up",
            Self::DuplicateLineDown => "Duplicate Line Down",
            Self::DeleteLine => "Delete Line",
            Self::ExpandSelection => "Expand Selection",
            Self::ShrinkSelection => "Shrink Selection",
            Self::NextProblem => "Next Problem",
            Self::PreviousProblem => "Previous Problem",
            Self::AddNextOccurrence => "Add Next Occurrence",
            Self::SelectAllOccurrences => "Select All Occurrences",
            Self::AddCaretAbove => "Add Caret Above",
            Self::AddCaretBelow => "Add Caret Below",
            Self::SelectLine => "Select Line",
            Self::JoinLines => "Join Lines",
            Self::MatchingBracket => "Go to Matching Bracket",
            Self::NewlineBelow => "New Line Below",
            Self::NewlineAbove => "New Line Above",
            Self::Fold => "Fold",
            Self::Unfold => "Unfold",
            Self::Find => "Find",
            Self::Replace => "Replace",
            Self::FindNext => "Find Next",
            Self::FindPrevious => "Find Previous",
            Self::GoToLine => "Go to Line",
            Self::Outline => "Go to Symbol in File",
            Self::ProjectSymbols => "Go to Symbol in Project",
            Self::FileFinder => "Go to File",
            Self::SearchProject => "Search in Project",
            Self::Problems => "Show Problems",
            Self::RevealInFileManager => "Reveal in File Manager",
            Self::Save => "Save",
        }
    }

    /// The group the keymap lists it under.
    pub fn group(self) -> &'static str {
        match self {
            Self::GoToDefinition
            | Self::GoToDeclaration
            | Self::GoToTypeDefinition
            | Self::GoToImplementation
            | Self::FindReferences
            | Self::IncomingCalls
            | Self::GoBack
            | Self::GoForward
            | Self::NextProblem
            | Self::PreviousProblem
            | Self::MatchingBracket
            | Self::GoToLine
            | Self::Outline
            | Self::ProjectSymbols
            | Self::FileFinder
            | Self::Problems => "Navigation",
            Self::Rename
            | Self::Format
            | Self::CodeActions
            | Self::Hover
            | Self::Complete
            | Self::SignatureHelp => "Code Intelligence",
            Self::AddNextOccurrence
            | Self::SelectAllOccurrences
            | Self::AddCaretAbove
            | Self::AddCaretBelow
            | Self::SelectLine
            | Self::ExpandSelection
            | Self::ShrinkSelection => "Selection",
            Self::Find
            | Self::Replace
            | Self::FindNext
            | Self::FindPrevious
            | Self::SearchProject => "Search",
            Self::RevealInFileManager | Self::Save => "File",
            _ => "Editing",
        }
    }

    /// Whether its keys work anywhere in the window, not only while a file has the keyboard.
    pub fn everywhere(self) -> bool {
        matches!(
            self,
            Self::FileFinder | Self::SearchProject | Self::Problems
        )
    }

    /// The action kept under the name `key`.
    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.key() == key)
    }
}

/// A set of bindings as another editor has them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Pandemonium's own.
    #[default]
    Pandemonium,
    /// As Visual Studio Code has them.
    VsCode,
    /// As the JetBrains editors have them.
    JetBrains,
    /// As Sublime Text has them.
    Sublime,
}

impl Preset {
    /// Every preset, in the order they are offered.
    pub const ALL: [Self; 4] = [
        Self::Pandemonium,
        Self::VsCode,
        Self::JetBrains,
        Self::Sublime,
    ];

    /// What it is called.
    pub fn title(self) -> &'static str {
        match self {
            Self::Pandemonium => "Pandemonium",
            Self::VsCode => "VS Code",
            Self::JetBrains => "JetBrains",
            Self::Sublime => "Sublime Text",
        }
    }

    /// The bindings it puts over Pandemonium's own; an action with none listed is left without keys.
    fn changes(self) -> &'static [(Action, &'static [&'static str])] {
        use Action::*;
        match self {
            Self::Pandemonium => &[],
            Self::VsCode => &[
                (GoToTypeDefinition, &[]),
                (GoToImplementation, &["ctrl+f12"]),
                (FindReferences, &["shift+f12", "alt+shift+f12"]),
                (GoBack, &["ctrl+alt+-"]),
                (DuplicateLineUp, &["ctrl+alt+shift+up"]),
                (DuplicateLineDown, &["ctrl+alt+shift+down"]),
                (AddCaretAbove, &["alt+shift+up"]),
                (AddCaretBelow, &["alt+shift+down"]),
                (JoinLines, &[]),
                (MatchingBracket, &["ctrl+shift+\\"]),
            ],
            Self::JetBrains => &[
                (GoToDefinition, &["ctrl+b"]),
                (GoToTypeDefinition, &["ctrl+shift+b"]),
                (GoToImplementation, &["ctrl+alt+b"]),
                (FindReferences, &["alt+f7"]),
                (IncomingCalls, &["ctrl+alt+h"]),
                (Rename, &["shift+f6"]),
                (Format, &["ctrl+alt+l"]),
                (CodeActions, &["alt+enter"]),
                (Hover, &["ctrl+q"]),
                (SignatureHelp, &["ctrl+p"]),
                (GoBack, &["ctrl+alt+left"]),
                (GoForward, &["ctrl+alt+right"]),
                (MoveLineUp, &["alt+shift+up"]),
                (MoveLineDown, &["alt+shift+down"]),
                (DuplicateLineUp, &[]),
                (DuplicateLineDown, &["ctrl+d"]),
                (DeleteLine, &["ctrl+y"]),
                (ExpandSelection, &["ctrl+w"]),
                (ShrinkSelection, &["ctrl+shift+w"]),
                (NextProblem, &["f2"]),
                (PreviousProblem, &["shift+f2"]),
                (AddNextOccurrence, &["alt+j"]),
                (SelectAllOccurrences, &["ctrl+alt+shift+j"]),
                (SelectLine, &[]),
                (JoinLines, &["ctrl+shift+j"]),
                (MatchingBracket, &["ctrl+shift+m"]),
                (NewlineBelow, &["shift+enter"]),
                (NewlineAbove, &["ctrl+alt+enter"]),
                (Fold, &["ctrl+-"]),
                (Unfold, &["ctrl+="]),
                (Replace, &["ctrl+r"]),
                (Outline, &["ctrl+f12"]),
                (ProjectSymbols, &["ctrl+alt+shift+n"]),
                (FileFinder, &["ctrl+shift+n"]),
                (Problems, &["alt+6"]),
            ],
            Self::Sublime => &[
                (GoToImplementation, &[]),
                (FindReferences, &["shift+f12"]),
                (MoveLineUp, &["ctrl+shift+up"]),
                (MoveLineDown, &["ctrl+shift+down"]),
                (DuplicateLineUp, &[]),
                (DuplicateLineDown, &["ctrl+shift+d"]),
                (ExpandSelection, &["ctrl+shift+a"]),
                (SelectAllOccurrences, &["alt+f3"]),
                (Outline, &["ctrl+r"]),
                (ProjectSymbols, &["ctrl+shift+r"]),
            ],
        }
    }
}

/// Pandemonium's own bindings.
fn own() -> &'static [(Action, &'static [&'static str])] {
    use Action::*;
    &[
        (GoToDefinition, &["f12"]),
        (GoToDeclaration, &[]),
        (GoToTypeDefinition, &["ctrl+f12"]),
        (GoToImplementation, &["shift+f12"]),
        (FindReferences, &["alt+shift+f12"]),
        (IncomingCalls, &["alt+shift+h"]),
        (Rename, &["f2"]),
        (Format, &["ctrl+shift+i", "alt+shift+f"]),
        (CodeActions, &["ctrl+."]),
        (Hover, &["ctrl+k ctrl+i"]),
        (Complete, &["ctrl+space"]),
        (SignatureHelp, &["ctrl+shift+space"]),
        (GoBack, &["ctrl+-"]),
        (GoForward, &["ctrl+shift+-"]),
        (ToggleComment, &["ctrl+/"]),
        (Indent, &["ctrl+]"]),
        (Outdent, &["ctrl+["]),
        (MoveLineUp, &["alt+up"]),
        (MoveLineDown, &["alt+down"]),
        (DuplicateLineUp, &["alt+shift+up"]),
        (DuplicateLineDown, &["alt+shift+down"]),
        (DeleteLine, &["ctrl+shift+k"]),
        (ExpandSelection, &["alt+shift+right"]),
        (ShrinkSelection, &["alt+shift+left"]),
        (NextProblem, &["f8"]),
        (PreviousProblem, &["shift+f8"]),
        (AddNextOccurrence, &["ctrl+d"]),
        (SelectAllOccurrences, &["ctrl+shift+l"]),
        (AddCaretAbove, &["ctrl+alt+up"]),
        (AddCaretBelow, &["ctrl+alt+down"]),
        (SelectLine, &["ctrl+l"]),
        (JoinLines, &["ctrl+j"]),
        (MatchingBracket, &["ctrl+m", "ctrl+shift+\\"]),
        (NewlineBelow, &["ctrl+enter"]),
        (NewlineAbove, &["ctrl+shift+enter"]),
        (Fold, &["ctrl+shift+["]),
        (Unfold, &["ctrl+shift+]"]),
        (Find, &["ctrl+f"]),
        (Replace, &["ctrl+h"]),
        (FindNext, &["f3"]),
        (FindPrevious, &["shift+f3"]),
        (GoToLine, &["ctrl+g"]),
        (Outline, &["ctrl+shift+o"]),
        (ProjectSymbols, &["ctrl+t"]),
        (FileFinder, &["ctrl+p"]),
        (SearchProject, &["ctrl+shift+f"]),
        (Problems, &["ctrl+shift+m"]),
        (RevealInFileManager, &["ctrl+k r"]),
        (Save, &["ctrl+s"]),
    ]
}

/// The key a press of Shift and the key that types `shifted` makes, on a keyboard laid out in US English, written as the key types without Shift; any other key as it is.
fn unshifted(shifted: char) -> char {
    let pairs = [
        ('{', '['),
        ('}', ']'),
        ('|', '\\'),
        ('_', '-'),
        ('+', '='),
        (':', ';'),
        ('"', '\''),
        ('<', ','),
        ('>', '.'),
        ('?', '/'),
        ('~', '`'),
        ('!', '1'),
        ('@', '2'),
        ('#', '3'),
        ('$', '4'),
        ('%', '5'),
        ('^', '6'),
        ('&', '7'),
        ('*', '8'),
        ('(', '9'),
        (')', '0'),
    ];
    pairs
        .iter()
        .find(|(held, _)| *held == shifted)
        .map_or(shifted.to_ascii_lowercase(), |(_, plain)| *plain)
}

/// One press of keys: the modifiers held and the key.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Chord {
    /// Control, or Command.
    pub control: bool,
    /// Alt, or Option.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// The key, as its name is written: a letter, digit or sign as the key types it without Shift, or `enter`, `f12`, `up` and the like.
    pub key: String,
}

impl Chord {
    /// The chord of the key named `key`, typing `typed` when it is one that types, pressed with the modifiers given.
    pub fn new(key: &str, control: bool, alt: bool, shift: bool) -> Self {
        let mut characters = key.chars();
        let key = match (characters.next(), characters.next()) {
            (Some(single), None) => unshifted(single).to_string(),
            _ => key.to_ascii_lowercase(),
        };
        Self {
            control,
            alt,
            shift,
            key,
        }
    }

    /// The chord written as `text`, such as `ctrl+shift+k`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().to_ascii_lowercase();
        let (modifiers, key) = match text.rsplit_once('+') {
            Some((modifiers, "")) => (modifiers.strip_suffix('+').unwrap_or(modifiers), "+"),
            Some((modifiers, key)) => (modifiers, key),
            None => ("", text.as_str()),
        };
        if key.is_empty() {
            return None;
        }
        let held = |name: &str| modifiers.split('+').any(|part| part == name);
        Some(Self::new(
            key,
            held("ctrl") || held("cmd"),
            held("alt"),
            held("shift"),
        ))
    }

    /// The chord as it is written in the settings, such as `ctrl+shift+k`.
    pub fn written(&self) -> String {
        let mut parts = Vec::new();
        if self.control {
            parts.push("ctrl".to_owned());
        }
        if self.alt {
            parts.push("alt".to_owned());
        }
        if self.shift {
            parts.push("shift".to_owned());
        }
        parts.push(self.key.clone());
        parts.join("+")
    }

    /// The chord as it is shown, such as `Ctrl+Shift+K`.
    pub fn shown(&self) -> String {
        let mut parts = Vec::new();
        if self.control {
            parts.push("Ctrl".to_owned());
        }
        if self.alt {
            parts.push("Alt".to_owned());
        }
        if self.shift {
            parts.push("Shift".to_owned());
        }
        let key = match self.key.as_str() {
            "up" => "Up".to_owned(),
            "down" => "Down".to_owned(),
            "left" => "Left".to_owned(),
            "right" => "Right".to_owned(),
            "enter" => "Enter".to_owned(),
            "space" => "Space".to_owned(),
            "tab" => "Tab".to_owned(),
            "escape" => "Escape".to_owned(),
            "backspace" => "Backspace".to_owned(),
            "delete" => "Delete".to_owned(),
            "home" => "Home".to_owned(),
            "end" => "End".to_owned(),
            "pageup" => "Page Up".to_owned(),
            "pagedown" => "Page Down".to_owned(),
            key if key.len() > 1 && key.starts_with('f') => key.to_ascii_uppercase(),
            key => key.to_ascii_uppercase(),
        };
        parts.push(key);
        parts.join("+")
    }
}

/// A binding as it is shown, its chords one after another, such as `Ctrl+K Ctrl+I`.
pub fn shown(binding: &str) -> String {
    binding
        .split_whitespace()
        .filter_map(Chord::parse)
        .map(|chord| chord.shown())
        .collect::<Vec<_>>()
        .join(" ")
}

/// What pressing a chord after the ones pressed before it comes to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pressed {
    /// The action the chords are bound to.
    Action(Action),
    /// The start of a binding of more chords; the next press finishes it.
    Waiting,
    /// No binding.
    Nothing,
}

/// The keys bound to each action: a preset with the reader's own bindings over it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keymap {
    /// Each binding, as its chords, with the action it is bound to.
    bindings: Vec<(Vec<Chord>, Action)>,
}

impl Keymap {
    /// The keymap of `preset` with `own`, the reader's own bindings by the name each action is kept under, over it.
    pub fn new(preset: Preset, own: &BTreeMap<String, Vec<String>>) -> Self {
        let mut by_action: BTreeMap<Action, Vec<String>> = BTreeMap::new();
        for (action, keys) in self::own().iter().chain(preset.changes()) {
            by_action.insert(*action, keys.iter().map(|key| (*key).to_owned()).collect());
        }
        for (name, keys) in own {
            if let Some(action) = Action::of_key(name) {
                by_action.insert(action, keys.clone());
            }
        }
        let bindings = by_action
            .into_iter()
            .flat_map(|(action, keys)| {
                keys.into_iter().filter_map(move |key| {
                    let chords: Option<Vec<Chord>> =
                        key.split_whitespace().map(Chord::parse).collect();
                    chords
                        .filter(|chords| !chords.is_empty())
                        .map(|chords| (chords, action))
                })
            })
            .collect();
        Self { bindings }
    }

    /// The bindings of `action`, as they are written.
    pub fn keys_of(&self, action: Action) -> Vec<String> {
        self.bindings
            .iter()
            .filter(|(_, held)| *held == action)
            .map(|(chords, _)| {
                chords
                    .iter()
                    .map(Chord::written)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    /// The first binding of `action` as it is shown, when it has one.
    pub fn label(&self, action: Action) -> Option<String> {
        self.keys_of(action).first().map(|binding| shown(binding))
    }

    /// What pressing `chord` after `before` comes to.
    pub fn press(&self, before: &[Chord], chord: &Chord) -> Pressed {
        let mut sequence = before.to_vec();
        sequence.push(chord.clone());
        if let Some((_, action)) = self.bindings.iter().find(|(chords, _)| *chords == sequence) {
            return Pressed::Action(*action);
        }
        let waiting = self
            .bindings
            .iter()
            .any(|(chords, _)| chords.len() > sequence.len() && chords.starts_with(&sequence));
        if waiting {
            Pressed::Waiting
        } else {
            Pressed::Nothing
        }
    }

    /// The actions other than `action` bound to `binding` too.
    pub fn clashes(&self, action: Action, binding: &str) -> Vec<Action> {
        let chords: Vec<Chord> = binding
            .split_whitespace()
            .filter_map(Chord::parse)
            .collect();
        self.bindings
            .iter()
            .filter(|(held, other)| *held == chords && *other != action)
            .map(|(_, other)| *other)
            .collect()
    }
}
