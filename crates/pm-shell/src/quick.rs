//! The quick input that opens from the command centre: files of the project,
//! `>` the window's commands, `#` the project's symbols, `@` the outline of
//! the open file and `%` the lines of the project holding some text; and
//! the lists of choices it shows in their place: sessions to switch to,
//! appearances, keymaps, settings pages, turns to rewind to and branches.

use crate::model::{Project, Workspace};

/// The most rows the quick input lists.
pub const QUICK_ROWS: usize = 60;

/// The quick input, while it is open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuickInput {
    /// What is typed, prefix included.
    pub query: String,
    /// The row chosen, among the rows listed.
    pub selected: usize,
    /// The list of choices shown instead of what the prefix lists.
    pub choosing: Option<Choosing>,
    /// What the server or the list of choices gave for the query, for the lists that need it.
    pub found: Vec<QuickItem>,
    /// The query the server is asked about, while it has not answered.
    pub waiting: Option<String>,
}

/// What the quick input lists, by the prefix typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickMode {
    /// The files of the project.
    Files,
    /// The window's commands, after `>`.
    Commands,
    /// The project's symbols, after `#`.
    Symbols,
    /// The items of the open file, after `@`.
    Outline,
    /// The lines of the project holding the text, after `%`.
    Text,
}

/// A list of choices the quick input shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choosing {
    /// The sessions to switch to.
    Session,
    /// The appearances: following the desktop, light or dark.
    Theme,
    /// The keymaps the bindings start from.
    Keymap,
    /// The pages of the settings.
    Setting,
    /// The reader's turns of the open session, to take the conversation back to.
    Rewind,
    /// The branches of the open session's repository, to check out.
    Branch,
    /// The places the thing under the caret of the open file is used.
    References,
    /// The changes the language server offers at the caret of the open file.
    Actions,
    /// The states the open file was kept in each time it was saved.
    History,
    /// What the language servers report wrong with the project's files.
    Problems,
}

/// One row of the quick input.
#[derive(Clone, Debug, PartialEq)]
pub struct QuickItem {
    /// What it reads.
    pub title: String,
    /// The muted text after it.
    pub detail: String,
    /// What it is, which picks its icon.
    pub kind: QuickKind,
    /// The keys bound to it, such as `Ctrl+B`.
    pub keys: Option<String>,
    /// The characters of `title` that match what is typed.
    pub matched: Vec<usize>,
    /// What picking it does.
    pub pick: QuickPick,
}

/// What a row of the quick input stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickKind {
    /// A file.
    File,
    /// A command.
    Command,
    /// A symbol of the project or of the file.
    Symbol,
    /// A line holding the text.
    Text,
    /// A session.
    Session,
    /// A choice of a list.
    Choice,
    /// The choice that now holds.
    Current,
}

/// What picking a row does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickPick {
    /// Opens the file, at the line when one is given.
    Open {
        /// The file, from the root of the project.
        path: String,
        /// The line, counted from 1.
        line: Option<usize>,
    },
    /// Runs the command.
    Command(Command),
    /// Opens the session at the index path.
    Session(Vec<usize>),
    /// Picks the choice at the place among the list's options.
    Choose(Choosing, usize),
    /// Puts back the state of the open file kept under this number.
    Version(u64),
}

/// A command of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Shows or hides the sidebar.
    ToggleSidebar,
    /// Shows or hides the dock.
    ToggleDock,
    /// Shows or hides the right panel.
    ToggleRight,
    /// Starts a new chat.
    NewChat,
    /// Opens the settings.
    Settings,
    /// Goes back.
    Back,
    /// Goes forward.
    Forward,
    /// Shows the history.
    History,
    /// Shows the extensions.
    Extensions,
    /// Shows the file tree.
    Files,
    /// Shows the project search.
    Search,
    /// Shows source control.
    Git,
    /// Lists the sessions to switch to.
    SwitchSession,
    /// Lists the appearances: following the desktop, light or dark.
    Theme,
    /// Lists the keymaps.
    Keymap,
    /// Lists the settings pages.
    Setting,
    /// Lists the turns to rewind to.
    Rewind,
    /// Lists the branches to check out.
    Branch,
    /// Lists the symbols of the project.
    Symbols,
    /// Lists the outline of the open file.
    Outline,
    /// Lists the lines holding some text.
    FindText,
    /// Lists the places the thing under the caret is used.
    References,
    /// Lists the changes offered at the caret.
    CodeActions,
    /// Lists the saved states of the open file.
    LocalHistory,
    /// Lists the problems of the project.
    Problems,
}

/// Every command, with what it reads and the keys bound to it.
pub const COMMANDS: [(Command, &str, &str); 25] = [
    (Command::SwitchSession, "Switch session", "Ctrl+Tab"),
    (Command::NewChat, "New chat", "Ctrl+N"),
    (Command::ToggleSidebar, "Toggle sidebar", "Ctrl+B"),
    (Command::ToggleDock, "Toggle dock", "Ctrl+J"),
    (Command::ToggleRight, "Toggle right panel", "Ctrl+Alt+B"),
    (Command::Files, "Show files", "Ctrl+Shift+E"),
    (Command::Search, "Search in project", "Ctrl+Shift+F"),
    (Command::Git, "Show source control", "Ctrl+Shift+G"),
    (Command::Symbols, "Go to symbol in project", "Ctrl+T"),
    (Command::Outline, "Go to symbol in file", "Ctrl+Shift+O"),
    (Command::FindText, "Find text in project", ""),
    (Command::References, "Find all references", "Shift+F12"),
    (Command::CodeActions, "Show code actions", "Ctrl+."),
    (Command::LocalHistory, "Show local history of file", ""),
    (Command::Problems, "List problems", "Ctrl+Shift+M"),
    (Command::Back, "Go back", "Alt+Left"),
    (Command::Forward, "Go forward", "Alt+Right"),
    (Command::History, "Show history", "Ctrl+H"),
    (Command::Extensions, "Show extensions", "Ctrl+Shift+X"),
    (Command::Settings, "Open settings", "Ctrl+,"),
    (Command::Setting, "Open settings page", ""),
    (Command::Theme, "Choose appearance", "Ctrl+K Ctrl+T"),
    (Command::Keymap, "Choose keymap", ""),
    (Command::Rewind, "Rewind conversation", ""),
    (Command::Branch, "Switch branch", ""),
];

/// What the person does in the quick input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickAction {
    /// Opens it with the text typed.
    Open(String),
    /// Opens it on a list of choices.
    Choose(Choosing),
    /// Changes what is typed.
    Query(String),
    /// Moves the row chosen by the rows given, down when positive.
    Move(isize),
    /// Picks the row at the place, or the row chosen.
    Pick(Option<usize>),
    /// Closes it.
    Close,
}

/// What picking a row asks of the window beyond the link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickOutcome {
    /// Opens the file, at the line when one is given.
    OpenFile(String, Option<usize>),
    /// Opens the settings window.
    Settings,
    /// Saves the settings changed.
    SettingsChanged,
}

impl QuickInput {
    /// What the prefix typed asks to list.
    pub fn mode(&self) -> QuickMode {
        match self.query.chars().next() {
            Some('>') => QuickMode::Commands,
            Some('#') => QuickMode::Symbols,
            Some('@') => QuickMode::Outline,
            Some('%') => QuickMode::Text,
            _ => QuickMode::Files,
        }
    }

    /// What is typed after the prefix.
    pub fn needle(&self) -> &str {
        if self.choosing.is_some() {
            return self.query.trim();
        }
        match self.mode() {
            QuickMode::Files => self.query.trim(),
            _ => self.query[1..].trim(),
        }
    }

    /// What the field says while nothing is typed.
    pub fn placeholder(&self) -> &'static str {
        match self.choosing {
            Some(Choosing::Session) => "Switch to a session",
            Some(Choosing::Theme) => "Choose an appearance",
            Some(Choosing::Keymap) => "Choose a keymap",
            Some(Choosing::Setting) => "Open a settings page",
            Some(Choosing::Rewind) => "Take the conversation back to a turn",
            Some(Choosing::Branch) => "Check out a branch",
            Some(Choosing::References) => "Go to a reference",
            Some(Choosing::Actions) => "Take a code action",
            Some(Choosing::History) => "Put back a saved state of the file",
            Some(Choosing::Problems) => "Go to a problem",
            None => "Search files by name, > commands, # symbols, @ outline, % text",
        }
    }
}

/// The rows the quick input lists for `workspace`, best first.
pub fn quick_items(workspace: &Workspace) -> Vec<QuickItem> {
    let Some(quick) = &workspace.quick else {
        return Vec::new();
    };
    let needle = quick.needle();
    let mut items = match quick.choosing {
        Some(Choosing::Session) => ranked(session_items(&workspace.projects, ""), needle),
        Some(_) => ranked(quick.found.clone(), needle),
        None => match quick.mode() {
            QuickMode::Files => ranked(file_items(&workspace.tree.paths), needle),
            QuickMode::Commands => ranked(command_items(), needle),
            QuickMode::Symbols | QuickMode::Outline | QuickMode::Text => quick.found.clone(),
        },
    };
    items.truncate(QUICK_ROWS);
    items
}

/// A row for each file of `paths`.
fn file_items(paths: &[String]) -> Vec<QuickItem> {
    paths
        .iter()
        .map(|path| {
            let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
            QuickItem {
                title: name.to_owned(),
                detail: folder.to_owned(),
                kind: QuickKind::File,
                keys: None,
                matched: Vec::new(),
                pick: QuickPick::Open {
                    path: path.clone(),
                    line: None,
                },
            }
        })
        .collect()
}

/// A row for each command.
fn command_items() -> Vec<QuickItem> {
    COMMANDS
        .iter()
        .map(|(command, title, keys)| QuickItem {
            title: (*title).to_owned(),
            detail: String::new(),
            kind: QuickKind::Command,
            keys: (!keys.is_empty()).then(|| (*keys).to_owned()),
            matched: Vec::new(),
            pick: QuickPick::Command(*command),
        })
        .collect()
}

/// A row for each session of `projects` and the projects inside them, named under `outer`.
fn session_items(projects: &[Project], outer: &str) -> Vec<QuickItem> {
    session_rows(projects, outer, &[])
}

/// The session rows of `projects`, whose index paths start with `prefix`.
fn session_rows(projects: &[Project], outer: &str, prefix: &[usize]) -> Vec<QuickItem> {
    let mut items = Vec::new();
    for (index, project) in projects.iter().enumerate() {
        let mut here = prefix.to_vec();
        here.push(index);
        let name = if outer.is_empty() {
            project.name.clone()
        } else {
            format!("{outer} / {}", project.name)
        };
        for (place, session) in project.sessions.iter().enumerate() {
            let mut at = here.clone();
            at.push(place);
            items.push(QuickItem {
                title: session.title.clone(),
                detail: format!("{name} · {}", session.age),
                kind: QuickKind::Session,
                keys: None,
                matched: Vec::new(),
                pick: QuickPick::Session(at),
            });
        }
        items.extend(session_rows(&project.children, &name, &here));
    }
    items
}

/// The rows of `items` whose titles hold the characters of `needle` in order,
/// best first, each with the characters matched; all of them when `needle` is empty.
fn ranked(items: Vec<QuickItem>, needle: &str) -> Vec<QuickItem> {
    if needle.is_empty() {
        return items;
    }
    let mut scored: Vec<(i64, QuickItem)> = items
        .into_iter()
        .filter_map(|mut item| {
            let (score, matched) = fuzzy(needle, &item.title).or_else(|| {
                fuzzy(needle, &format!("{}/{}", item.detail, item.title))
                    .map(|(score, _)| (score - 50, Vec::new()))
            })?;
            item.matched = matched;
            Some((score, item))
        })
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, item)| item).collect()
}

/// How well `needle` matches `haystack` when its characters appear in it in
/// order, ignoring case, with the places of the characters matched; higher
/// is better, favouring runs and starts of words.
pub fn fuzzy(needle: &str, haystack: &str) -> Option<(i64, Vec<usize>)> {
    let wanted: Vec<char> = needle.to_lowercase().chars().collect();
    let letters: Vec<char> = haystack.chars().collect();
    let mut matched = Vec::with_capacity(wanted.len());
    let mut score = 0_i64;
    let mut next = 0;
    for (place, letter) in letters.iter().enumerate() {
        if next == wanted.len() {
            break;
        }
        if letter.to_lowercase().eq(std::iter::once(wanted[next])) {
            let start = place == 0 || !letters[place - 1].is_alphanumeric();
            let run = matched.last().is_some_and(|last| last + 1 == place);
            score += 1 + if start { 8 } else { 0 } + if run { 5 } else { 0 };
            matched.push(place);
            next += 1;
        }
    }
    (next == wanted.len()).then(|| (score * 100 - letters.len() as i64, matched))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_prefix() {
        let quick = |query: &str| QuickInput {
            query: query.into(),
            ..QuickInput::default()
        };
        assert_eq!(quick(">tog").mode(), QuickMode::Commands);
        assert_eq!(quick(">tog").needle(), "tog");
        assert_eq!(quick("#Link").mode(), QuickMode::Symbols);
        assert_eq!(quick("@main").mode(), QuickMode::Outline);
        assert_eq!(quick("%todo").mode(), QuickMode::Text);
        assert_eq!(quick("main.rs").mode(), QuickMode::Files);
    }

    #[test]
    fn matches_characters_in_order() {
        assert_eq!(
            fuzzy("mrs", "main.rs").map(|(_, at)| at),
            Some(vec![0, 5, 6])
        );
        assert!(fuzzy("srm", "main.rs").is_none());
        let start = fuzzy("d", "dock").unwrap().0;
        let middle = fuzzy("d", "add").unwrap().0;
        assert!(start > middle);
    }

    #[test]
    fn lists_files_and_commands_by_prefix() {
        let mut workspace = Workspace::default();
        workspace.tree.paths = vec!["src/main.rs".into(), "src/link.rs".into()];
        workspace.quick = Some(QuickInput {
            query: "lin".into(),
            ..QuickInput::default()
        });
        let items = quick_items(&workspace);
        assert_eq!(items[0].title, "link.rs");
        workspace.quick = Some(QuickInput {
            query: ">dock".into(),
            ..QuickInput::default()
        });
        assert_eq!(quick_items(&workspace)[0].title, "Toggle dock");
    }
}
