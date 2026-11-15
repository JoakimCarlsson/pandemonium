//! The quick input that opens from the command centre: files of the project,
//! `>` the window's commands, `#` the project's symbols, `@` the outline of
//! the open file and `%` the lines of the project holding some text; and
//! the lists of choices it shows in their place: sessions to switch to,
//! appearances, keymaps, settings pages, turns to rewind to, branches,
//! tasks, debug scenarios and toolchains. `:` goes to a line of the open
//! file and `?` lists the prefixes.

use pm_editor::{Action, Keymap};

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
    /// The window's commands, with the keys the keymap binds them to, the recently used first.
    pub commands: Vec<QuickItem>,
    /// The files opened lately from the quick input, the latest first.
    pub recent_files: Vec<String>,
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
    /// A line of the open file to go to, after `:`.
    Line,
    /// The prefixes that can be typed first, after `?`.
    Help,
}

/// Each prefix the quick input understands, with what it lists.
pub const PREFIXES: [(&str, &str); 6] = [
    (">", "Run a command"),
    ("#", "Go to a symbol in the project"),
    ("@", "Go to a symbol in the open file"),
    ("%", "Find text in the project"),
    (":", "Go to a line of the open file"),
    ("?", "Show what can be typed first"),
];

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
    /// The reader's turns of the open session, to branch a new session off before.
    Fork,
    /// The reader's turns of the open session, to put its files back as they were before.
    Restore,
    /// The tasks of the project, to run one.
    Task,
    /// What the project can be debugged as, to start one.
    Debug,
    /// The toolchains the project's languages can be started with, to choose one.
    Toolchain,
    /// The stashes of the open session's repository, to apply one.
    StashApply,
    /// The stashes of the open session's repository, to apply one and drop it.
    StashPop,
    /// The stashes of the open session's repository, to drop one.
    StashDrop,
    /// The places a language server found related to the thing under the caret of the open file, as this seek looks for them, such as the functions calling it.
    Related(pm_lsp::Seek),
    /// The commits that changed the open file, to compare it with the file as one of them left it.
    Commits,
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
    /// Does the keymap action.
    Action(Action),
    /// Types the prefix into the field.
    Prefix(String),
    /// Chooses the toolchain a language of the open project starts with.
    Toolchain {
        /// The language, by family.
        language: String,
        /// The toolchain, by the id the server gave it.
        id: String,
    },
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

/// Every command, with what it reads and the keymap action whose keys it shows.
pub const COMMANDS: [(Command, &str, Option<Action>); 25] = [
    (
        Command::SwitchSession,
        "Switch session",
        Some(Action::GoToSession),
    ),
    (Command::NewChat, "New chat", Some(Action::NewChat)),
    (Command::ToggleSidebar, "Toggle sidebar", None),
    (
        Command::ToggleDock,
        "Toggle dock",
        Some(Action::ToggleTerminal),
    ),
    (Command::ToggleRight, "Toggle right panel", None),
    (Command::Files, "Show files", None),
    (
        Command::Search,
        "Search in project",
        Some(Action::SearchProject),
    ),
    (
        Command::Git,
        "Show source control",
        Some(Action::ToggleSourceControl),
    ),
    (
        Command::Symbols,
        "Go to symbol in project",
        Some(Action::ProjectSymbols),
    ),
    (
        Command::Outline,
        "Go to symbol in file",
        Some(Action::Outline),
    ),
    (Command::FindText, "Find text in project", None),
    (
        Command::References,
        "Find all references",
        Some(Action::FindReferences),
    ),
    (
        Command::CodeActions,
        "Show code actions",
        Some(Action::CodeActions),
    ),
    (
        Command::LocalHistory,
        "Show local history of file",
        Some(Action::FileHistory),
    ),
    (Command::Problems, "List problems", Some(Action::Problems)),
    (Command::Back, "Go back", Some(Action::WorkspaceBack)),
    (
        Command::Forward,
        "Go forward",
        Some(Action::WorkspaceForward),
    ),
    (Command::History, "Show history", Some(Action::ShowHistory)),
    (
        Command::Extensions,
        "Show extensions",
        Some(Action::ShowExtensions),
    ),
    (
        Command::Settings,
        "Open settings",
        Some(Action::OpenSettings),
    ),
    (Command::Setting, "Open settings page", None),
    (
        Command::Theme,
        "Choose appearance",
        Some(Action::ChangeTheme),
    ),
    (Command::Keymap, "Choose keymap", Some(Action::ChangeKeymap)),
    (
        Command::Rewind,
        "Rewind conversation",
        Some(Action::RewindConversation),
    ),
    (
        Command::Branch,
        "Switch branch",
        Some(Action::CheckoutBranch),
    ),
];

/// How many files and commands the quick input remembers as used lately.
pub const RECENT_LIMIT: usize = 8;

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
    /// Puts this text on the system clipboard.
    Copy(String),
    /// Shows this folder of this machine in the desktop's file manager.
    OpenFolder(String),
    /// Closes the window, asking first when the settings say so.
    Quit,
    /// Switches between the light and the dark appearance, from the one showing.
    ToggleTheme,
    /// Opens this address in the system's browser.
    OpenAddress(String),
}

impl QuickInput {
    /// What the prefix typed asks to list.
    pub fn mode(&self) -> QuickMode {
        match self.query.chars().next() {
            Some('>') => QuickMode::Commands,
            Some('#') => QuickMode::Symbols,
            Some('@') => QuickMode::Outline,
            Some('%') => QuickMode::Text,
            Some(':') => QuickMode::Line,
            Some('?') => QuickMode::Help,
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
            Some(Choosing::Fork) => "Branch a new session off before a turn",
            Some(Choosing::Restore) => "Put the files back as they were before a turn",
            Some(Choosing::Task) => "Run a task",
            Some(Choosing::Debug) => "Start debugging",
            Some(Choosing::Toolchain) => "Choose a toolchain",
            Some(Choosing::StashApply) => "Apply a stash",
            Some(Choosing::StashPop) => "Pop a stash",
            Some(Choosing::StashDrop) => "Drop a stash",
            Some(Choosing::Related(pm_lsp::Seek::IncomingCalls)) => "Go to a caller",
            Some(Choosing::Related(pm_lsp::Seek::OutgoingCalls)) => "Go to a called function",
            Some(Choosing::Related(pm_lsp::Seek::Supertypes)) => "Go to a supertype",
            Some(Choosing::Related(pm_lsp::Seek::Subtypes)) => "Go to a subtype",
            Some(Choosing::Related(_)) => "Go to a place found",
            Some(Choosing::Commits) => "Compare with the file as a commit left it",
            None => match self.mode() {
                QuickMode::Line => "Go to a line by its number",
                QuickMode::Help => "Type a prefix, or select one",
                _ => "Search files by name (type ? to see what else can be typed first)",
            },
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
            QuickMode::Files => ranked(
                file_items(&workspace.tree.paths, &quick.recent_files),
                needle,
            ),
            QuickMode::Commands => ranked(quick.commands.clone(), needle),
            QuickMode::Line => line_items(workspace, needle),
            QuickMode::Help => ranked(help_items(), needle),
            QuickMode::Symbols | QuickMode::Outline | QuickMode::Text => quick.found.clone(),
        },
    };
    items.truncate(QUICK_ROWS);
    items
}

/// A row for each file of `paths`, those of `recent` that are among them first, the latest first.
fn file_items(paths: &[String], recent: &[String]) -> Vec<QuickItem> {
    let lately = recent.iter().filter(|path| paths.contains(path));
    let rest = paths.iter().filter(|path| !recent.contains(path));
    lately
        .chain(rest)
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

/// A row for each command of the window and each keymap action that works everywhere, with the keys `keymap` binds them to, those titled in `recent` first, the latest first.
pub fn command_items(keymap: &Keymap, recent: &[String]) -> Vec<QuickItem> {
    let covered: Vec<Action> = COMMANDS
        .iter()
        .filter_map(|(_, _, action)| *action)
        .collect();
    let commands = COMMANDS.iter().map(|(command, title, action)| QuickItem {
        title: (*title).to_owned(),
        detail: String::new(),
        kind: QuickKind::Command,
        keys: action.and_then(|action| keymap.label(action)),
        matched: Vec::new(),
        pick: QuickPick::Command(*command),
    });
    let actions = Action::ALL
        .into_iter()
        .filter(|action| {
            action.everywhere() && *action != Action::CommandPalette && !covered.contains(action)
        })
        .map(|action| QuickItem {
            title: action.title().to_owned(),
            detail: String::new(),
            kind: QuickKind::Command,
            keys: keymap.label(action),
            matched: Vec::new(),
            pick: QuickPick::Action(action),
        });
    let mut items: Vec<QuickItem> = commands.chain(actions).collect();
    let lead = |item: &QuickItem| {
        recent
            .iter()
            .position(|title| *title == item.title)
            .unwrap_or(usize::MAX)
    };
    items.sort_by_key(lead);
    items
}

/// The row that goes to the line `needle` names in the open file, or a row saying what to type.
fn line_items(workspace: &Workspace, needle: &str) -> Vec<QuickItem> {
    let Some(file) = workspace.open_files.shown() else {
        return Vec::new();
    };
    let lines = file.document.0.borrow().field.text().lines().count().max(1);
    let line = needle
        .parse::<usize>()
        .ok()
        .filter(|line| (1..=lines).contains(line));
    let title = match line {
        Some(line) => format!("Go to line {line}"),
        None => format!("Type a line number between 1 and {lines}"),
    };
    vec![QuickItem {
        title,
        detail: file.path.clone(),
        kind: QuickKind::Text,
        keys: None,
        matched: Vec::new(),
        pick: match line {
            Some(line) => QuickPick::Open {
                path: file.path.clone(),
                line: Some(line),
            },
            None => QuickPick::Prefix(":".to_owned()),
        },
    }]
}

/// A row for each prefix the quick input understands.
fn help_items() -> Vec<QuickItem> {
    PREFIXES
        .iter()
        .map(|(prefix, what)| QuickItem {
            title: (*what).to_owned(),
            detail: String::new(),
            kind: QuickKind::Command,
            keys: Some((*prefix).to_owned()),
            matched: Vec::new(),
            pick: QuickPick::Prefix((*prefix).to_owned()),
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
        assert_eq!(quick(":12").mode(), QuickMode::Line);
        assert_eq!(quick(":12").needle(), "12");
        assert_eq!(quick("?").mode(), QuickMode::Help);
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
            commands: command_items(&Keymap::default(), &[]),
            ..QuickInput::default()
        });
        assert_eq!(quick_items(&workspace)[0].title, "Toggle dock");
        workspace.quick = Some(QuickInput {
            query: "?".into(),
            ..QuickInput::default()
        });
        assert_eq!(quick_items(&workspace).len(), PREFIXES.len());
    }

    /// The command list holds every action that works everywhere, with the keys the keymap binds, and the recently used lead.
    #[test]
    fn commands_come_from_the_keymap() {
        let keymap = Keymap::new(pm_editor::Preset::default(), &Default::default());
        let items = command_items(&keymap, &["Push".to_owned()]);
        assert_eq!(items[0].title, "Push");
        assert!(
            items
                .iter()
                .any(|item| item.pick == QuickPick::Action(Action::StartDebugging))
        );
        let palette = items
            .iter()
            .find(|item| item.pick == QuickPick::Command(Command::Settings))
            .unwrap();
        assert!(palette.keys.is_some());
        assert_eq!(palette.keys, keymap.label(Action::OpenSettings));
    }

    /// Recently opened files lead the file list.
    #[test]
    fn recent_files_lead() {
        let paths = vec!["a.rs".to_owned(), "b.rs".to_owned()];
        let items = file_items(&paths, &["b.rs".to_owned(), "gone.rs".to_owned()]);
        assert_eq!(items[0].title, "b.rs");
        assert_eq!(items.len(), 2);
    }
}
