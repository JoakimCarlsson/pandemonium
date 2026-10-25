//! Drawing one frame to a PNG without opening a window.

use std::path::Path;

use pm_acp::{
    Alternative, Background, Choice, Diff, Discovery, Event, Hook, Inquiry, Kind, Permission, Pick,
    PlanStep, Progress, Question, Setting, Slash, Tool, ToolState, Verdict,
};
use pm_components::Project;
use pm_components::adder::{ADDER_BROWSE, ADDER_CLONE, ADDER_ROW_BASE, Found, Kind as Row};
use pm_components::diff::gutter_id;
use pm_components::git_controls::{BRANCH_BUTTON, BRANCH_FILTER, Pick as Choosing};
use pm_components::git_menus::MenuKind;
use pm_components::git_prompts::Prompt;
use pm_components::hooks_prompt::HOOKS_REVIEW;
use pm_components::open_files::EDITOR_FIELD;
use pm_components::panes::{Item, SPLIT_RIGHT, Side};
use pm_components::settings::{SETTINGS_ADD, SETTINGS_FIND, Section};
use pm_components::settings_catalog::{OPEN_LIST, Setting as SettingsCatalogue};
use pm_components::settings_machines::{MACHINE_ADD, MACHINE_ADDRESS, MACHINE_NAME};
use pm_components::state::SETTINGS;
use pm_components::state::{
    AGENT_BASE, CHAT_TEXT, COMMENT_ADD, COMMENT_FIELD, COMMIT_MESSAGE, DETAILS_BASE, FILE_BASE,
    RESTORE_BASE, REVIEW_FILES, STAGE_BASE, SUBAGENTS_CHIP, TAB_CHAT,
};
use pm_components::tree_actions::{TREE_NEW_FOLDER, TreeTarget};
use pm_input::{FocusId, PointerState};
use pm_pages::{ADD_PROJECT, HISTORY, PROMPT, SEND, Screen, TAB_DIFF, TERMINAL, WorkspaceState};
use pm_renderer::{DrawList, Renderer};
use pm_text::Edit;
use pm_theme::{Appearance, default_family};
use pm_ui::{Input, TextPoint, TextSelection};

use crate::scene;

/// Logical width of the snapshot.
const WIDTH: u32 = 1280;

/// Logical height of the snapshot.
const HEIGHT: u32 = 800;

/// Logical width of a snapshot of the settings window.
const SETTINGS_WIDTH: u32 = 960;

/// Logical height of a snapshot of the settings window.
const SETTINGS_HEIGHT: u32 = 720;

/// Logical width of a snapshot of the folder picker's window.
const PICKER_WIDTH: u32 = 880;

/// Logical height of a snapshot of the folder picker's window.
const PICKER_HEIGHT: u32 = 560;

/// Logical width of a snapshot of the settings window at its narrowest.
const NARROW_WIDTH: u32 = 640;

/// Logical height of a snapshot of the settings window at its smallest.
const NARROW_HEIGHT: u32 = 420;

/// The name of the shot of each section of the settings window, after `settings-`.
pub const SETTINGS_SHOTS: [(&str, Section); 23] = [
    ("general", Section::General),
    ("appearance", Section::Appearance),
    ("editor", Section::Editor),
    ("editor-appearance", Section::EditorAppearance),
    ("editor-editing", Section::EditorEditing),
    ("editor-saving", Section::EditorSaving),
    ("editor-intelligence", Section::EditorIntelligence),
    ("editor-search", Section::EditorSearch),
    ("languages", Section::Languages),
    ("keymap", Section::Keymap),
    ("terminal", Section::Terminal),
    ("git", Section::Git),
    ("worktrees", Section::Worktrees),
    ("agents", Section::Agents),
    ("agent-servers", Section::AgentServers),
    ("mcp-servers", Section::McpServers),
    ("customizations", Section::Customizations),
    ("skills", Section::Skills),
    ("instructions", Section::Instructions),
    ("agent-definitions", Section::AgentDefinitions),
    ("hooks", Section::Hooks),
    ("tools", Section::Tools),
    ("servers", Section::Servers),
];

/// Which screen a snapshot shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shot {
    /// The workspace on its first session.
    Workspace,
    /// Every workspace attention state and the session menu, on an unread error when true.
    SessionStates(bool),
    /// A forty-line prompt scrolled part way through its contents.
    ComposerLong,
    /// A session whose account needs signing in again.
    SignedOut,
    /// The workspace asking whether to close the window while agents are working.
    Quitting,
    /// The workspace telling of a newer release in its title bar.
    Update,
    /// The workspace on the diff of its first session.
    Diff,
    /// The workspace on the diff of its first session with the words that changed inside changed lines marked.
    WordDiff,
    /// The workspace on the diff of its first session with a comment kept and another being written.
    Commenting,
    /// The workspace on the diff of its first session with the project's files listed in the review panel.
    Files,
    /// Ten thousand files in expanded folders, for tree rendering and latency checks.
    FilesLarge,
    /// The large file tree scrolled into the middle of an expanded folder.
    FilesScrolled,
    /// A long Unicode prompt below a large conversation, at the caret.
    InputStress,
    /// The same long prompt manually scrolled away from its caret.
    InputStressScrolled,
    /// A long prompt with an IME composition at its caret.
    InputStressComposing,
    /// Expanded ignored folders with nested files dimmed.
    IgnoredFiles,
    /// The workspace on the diff with the source control panel: staged and untracked files, a message, the graph.
    Source,
    /// The source control panel with git's refusal of a branch switch said in it.
    Refused,
    /// The source control panel with the menu over its actions open.
    SourceMenu,
    /// The source control panel with the menu on a commit of the graph open.
    HistoryMenu,
    /// The source control panel asking what to call a stash.
    Stashing,
    /// The workspace on the diff with two changed files picked and the menu on them open.
    ChangeMenu,
    TreeMenu,
    /// The project's files with three rows picked, two as a range, and the menu on them open.
    TreePicked,
    Naming,
    /// The workspace on the diff with the question of whether to discard two changed files open.
    Discarding,
    /// The workspace with the list of branches open and a name being typed.
    Branching,
    /// The workspace reading a file as it is in the last commit.
    Reading,
    Editing,
    NewFile,
    /// The workspace with the terminal open.
    Terminal,
    /// The workspace with the terminal open and a web address in it underlined as what a press opens.
    TerminalLink,
    /// The workspace with text of the terminal marked and the menu over it open.
    TerminalMenu,
    /// The overview of a project.
    Overview,
    /// The sidebar with the submodules of a project under it, a session working in one, and work being started in another.
    Submodules,
    /// A folded change opened from the project Changes list.
    OverviewDiff,
    /// The overview of a project with the menu on a changed file of its folder open.
    OverviewChangeMenu,
    /// The workspace with the terminal open and the sidebar, the review panel and the terminal dragged to other sizes.
    Resized,
    /// A workspace with projects and no sessions.
    New,
    /// The workspace with no project at all.
    Empty,
    /// The workspace on the history page with overflowing session groups.
    History,
    /// The history page scrolled to its oldest session.
    HistoryScrolled,
    /// The workspace with the menu of a project open.
    ProjectMenu,
    /// The workspace with a project dragged over another.
    Dragging,
    /// The page for starting work in this repository, with the agent's settings known.
    Welcome,
    /// The page for starting work in this repository with the terminal open under it.
    WelcomeTerminal,
    /// A session cut into three panes: the conversation above the diff, beside a file.
    Panes,
    /// A session with the tab of a file being dragged to the right edge of its pane.
    PanesDragging,
    /// A session with the menu of panes open.
    PaneMenu,
    /// A session whose pane was split, an empty pane on its right.
    PaneSplit,
    /// A session whose pane was split, a file then opened in the empty pane on its right.
    PaneSplitFile,
    /// The page for starting work with a long prompt that wraps and has a line break.
    Wrapped,
    /// The page for starting work after a command was picked from the list behind a slash.
    Slashed,
    /// The page for starting work with Codex and a dollar sign being typed.
    Dollar,
    /// A Codex conversation with messages and its skills menu open.
    DollarSession,
    /// The page for starting work with the list of models scrolled to its end.
    Scrolling,
    /// The page for starting work with the menu of where it is done open.
    Placing,
    /// A conversation fed the events an agent produces, finished.
    Live,
    /// A conversation whose agent waits for permission.
    Asking,
    /// A conversation whose agent waits for permission to run a long command.
    AskingCommand,
    /// A conversation whose agent waits for the answers to a form of three questions.
    Inquiring,
    /// A conversation whose reader answered a form of questions.
    Answered,
    /// A conversation whose Codex holds back hooks until the reader trusts them, asking what to do.
    Hooks,
    /// The same conversation with the hooks held back listed for review.
    HooksReview,
    /// A conversation whose turn ended while two subagents and a command were still running.
    Background,
    /// The same conversation with the subagents above the prompt box open.
    Agents,
    /// The open panel with dozens of expanded tool runs.
    AgentsExpanded,
    /// The expanded panel scrolled to its last rows.
    AgentsScrolled,
    /// A conversation whose agent is on a turn with prompts queued behind it and another being written.
    Queued,
    /// A conversation with the context ring in the prompt box.
    Usage,
    /// A conversation with the model menu open.
    Picking,
    /// A finished conversation scrolled back to its first message, whose files were kept before it.
    Kept,
    /// A finished conversation with the offer to put the files back as they were before its first message open.
    Restoring,
    /// A conversation with the details of its steps open.
    Expanded,
    /// A conversation with a slash being typed.
    Slashing,
    /// The page for starting work with an `@` being typed that no file fits.
    Missing,
    /// The page for starting work with files attached.
    Attached,
    /// The workspace on its first session with the fold-out of its last reply open.
    Unfolded,
    /// The page for starting work with a dead key's accent waiting for its letter in the prompt.
    Composing,
    /// The workspace with the terminal open and a dead key's accent waiting for its letter at the cursor.
    TerminalComposing,
    /// The workspace on its first session with a stretch of the conversation's text selected.
    Selecting,
    /// The workspace on its first session, which runs as an account the reader added.
    SessionAccount,
    /// The same selection with the menu over a reply open.
    MessageMenu,
    /// The page for starting work with the panel for adding a project open on its choices.
    Adding,
    /// A file with its long lines wrapped at a line length of 36 and a block caret that does not blink.
    SoftWrap,
    /// The settings window on a section, at its narrowest when the flag is set.
    SettingsPage(Section, bool),
    /// The settings window showing what a search for fonts found, at its narrowest when the flag is set.
    SettingsSearch(bool),
    /// The settings window on Appearance with the list of code fonts open, at its narrowest when the flag is set.
    SettingsPicker(bool),
    /// The settings window on Servers with the form for adding one open, at its narrowest when the flag is set.
    SettingsMachine(bool),
    /// The settings window on Appearance set compact with interface text at 15 pixels, at its narrowest when the flag is set.
    SettingsCompact(bool),
    /// The settings panel on the MCP servers inside Agents, with one installed, the registry's offers and the menu of the installed one open.
    SettingsMcp,
    /// The settings panel on the Agent Servers inside Agents, with the form for adding one open.
    SettingsForm,
    /// The settings panel on the Agent Servers inside Agents, with two accounts added to the first agent.
    SettingsAccounts,
    /// The same panel waiting for the accounts and organizations to clone from.
    Loading,
    /// The same panel on the accounts and organizations to clone from.
    Owning,
    /// The same panel on the repositories of one of them.
    Cloning,
    /// The folder picker's window on the folders of the machine, to open one as a project.
    Folders,
    /// A Rust file being written: highlighted, with what the language server finds wrong under it and the brackets at the caret paired.
    Code,
    /// A file written in a language that is not installed, with the offer to install it above its lines.
    LanguageOffer,
    /// A markdown file shown rendered as a page, with the switch to its source.
    MarkdownPreview,
    /// A vector picture shown drawn, with the switch to its source.
    VectorPreview,
    /// A picture file shown whole, with how many pixels it has.
    PicturePreview,
    /// The same file with the list of completions open at the caret.
    Completing,
    /// The same file with the card about a place that has something wrong with it.
    Hovering,
    /// The same file with breakpoints in its gutter and a program paused on one, the debug tab open under it.
    Debugging,
    /// The same file with the dock open on the list of what is wrong with the project.
    Problems,
    /// The same file with the sidebar hidden.
    SidebarHidden,
    /// The same file with its menu open.
    EditorMenu,
    /// The same file with the places a name is used listed under the caret.
    References,
    /// The same file with the box for a new name of the thing at the caret open.
    Renaming,
    /// The same file with the changes the language server offers at the caret listed.
    CodeActions,
    /// The same file with the commits that changed it, and the states it was saved in, listed as one timeline.
    FileHistory,
    /// The same file set against a state it was saved in, its changed lines edited in place.
    Timeline,
    /// A file a merge stopped on, with both sides of each conflict and the result under them.
    Conflict,
    /// The diff of a session that edited files inside a submodule and moved it to another commit.
    DiffSubmodule,
    /// The source control panel of a folder with submodules: the list of repositories with the folder's own picked, or, `true`, a submodule picked with its branches open.
    BranchesSubmodule(bool),
    /// The same file with the caret in a call, its signature over it and the uses of a name marked.
    Signing,
    /// The same file with brackets coloured by depth, names coloured by what the language server says they are, hints inside lines and lenses at the ends of lines.
    Annotated,
    /// The review panel on its search tab, with the hits of a search for a word, grouped by file.
    ProjectSearch,
    /// The hits of a search as excerpts of three files, one under another, edited in place.
    Excerpts,
    /// The same file with the bar for finding and replacing open on a word it holds several times.
    Finding,
    /// The same file with the bar for finding opened again over a query typed before, the query selected whole.
    FindReopened,
    /// The same file with its outline listed, narrowed by a letter typed.
    Outline,
    /// The same file with every use of a name selected, a caret at each.
    Cursors,
    /// The same file with the body of a function folded away.
    Folded,
    /// The same file scrolled into a method, the lines that open its blocks kept at the top.
    Sticky,
    /// The finder for the files of the project, narrowed by what is typed.
    Finder,
    /// The same file with the list of every command open, narrowed by what is typed.
    Commands,
    /// The same file with the list of every command open over the tasks the project's manifests imply.
    Tasks,
    /// A file with a `main` and tests, with the run button of each in the gutter.
    RunButtons,
    /// The same file with the list of toolchains installed open.
    Toolchains,
    /// The same file with the list of the other sessions open.
    Sessions,
    /// The same file with the quick input listing what can be typed first.
    QuickHelp,
    /// The same file with the quick input listing the themes.
    Themes,
    /// The same file with the quick input going to a line.
    GoLine,
    /// The same file with the menu on its tab open.
    TabMenu,
    /// The extensions page, some installed, one being installed, one that failed and one of the reader's own.
    Extensions,
    /// The extensions page asking whether to install an extension, with what it asks to be allowed.
    ExtensionInstall,
}

/// Loads the grammar of the language `id` installed on this machine, if it is, and learns the endings of the files written in it, so a snapshot can highlight with it.
fn load_installed_grammar(id: &str) {
    let Some(manifest) = pm_lsp::installed_manifest(id) else {
        return;
    };
    let (Ok(folder), Ok(grammar)) = (pm_lsp::language_folder(id), pm_lsp::grammar_file(id)) else {
        return;
    };
    let read = |file: &str| std::fs::read_to_string(folder.join(file)).unwrap_or_default();
    let Ok(wasm) = std::fs::read(grammar) else {
        return;
    };
    let queries = pm_treesitter::Queries {
        highlights: read("highlights.scm"),
        indents: read("indents.scm"),
        outline: read("outline.scm"),
        injections: read("injections.scm"),
        brackets: read("brackets.scm"),
        runnables: read("runnables.scm"),
    };
    let editing = pm_treesitter::Editing {
        indent: manifest.indent,
        tabs: manifest.tabs,
        line_comment: manifest.line_comment.clone(),
        pairs: pm_treesitter::Editing::pairs_of(&manifest.pairs),
    };
    if let Ok(loaded) =
        pm_treesitter::Grammar::load(id, &manifest.grammar, &wasm, &queries, editing)
    {
        let mut aliases = manifest.aliases.clone();
        aliases.push(manifest.grammar.clone());
        pm_treesitter::registry::add(loaded, &aliases);
        pm_components::file_icon::learn_languages(
            &None,
            &[pm_protocol::ExtensionInfo {
                id: id.to_owned(),
                suffixes: manifest.suffixes.clone(),
                installed: true,
                ..Default::default()
            }],
        );
    }
}

/// The path of the Rust file the code snapshots write in.
const CODE_PATH: &str = "crates/pandemonium/src/session.rs";

/// The tab of the file the workspace with a file open shows.
fn code_item() -> Item {
    Item::File {
        path: CODE_PATH.to_owned(),
        head: false,
    }
}

/// A Rust file of the first session open for writing, its grammar loaded when Rust is installed on this machine, with the caret after the brace that opens `new` and what the language server finds wrong in it.
fn code() -> WorkspaceState {
    for id in ["rust", "markdown", "markdown-inline"] {
        load_installed_grammar(id);
    }
    let mut state = workspace_sample().after(TAB_DIFF);
    state.open_viewer(CODE_PATH.to_owned(), false);
    let text = "use std::collections::HashMap;\nuse std::path::PathBuf;\n\n/// A session of an agent, with its **own** worktree.\n#[derive(Clone, Debug)]\npub struct Session {\n    pub id: u64,\n    pub root: PathBuf,\n    names: HashMap<String, usize>,\n}\n\nconst LIMIT: usize = 64;\n\nimpl Session {\n    /// Makes a session working in `root`.\n    pub fn new(id: u64, root: PathBuf) -> Self {\n        let names = HashMap::with_capacity(LIMIT);\n        let label: u32 = \"main\";\n        Self { id, root, names }\n    }\n\n    /// How many names the session knows.\n    pub fn count(&self) -> usize {\n        self.names.len()\n    }\n}\n";
    let state = state.with_file(
        0,
        CODE_PATH,
        false,
        Ok((Some(text.to_owned()), false, Some("rust".to_owned()))),
    );
    let base = text
        .replace("        let label: u32 = \"main\";\n", "")
        .replace("HashMap::with_capacity(LIMIT)", "HashMap::new()")
        .replace(
            "const LIMIT: usize = 64;\n\n",
            "const LIMIT: usize = 64;\nconst SPARE: usize = 8;\n\n",
        );
    let state = state.with_base(0, CODE_PATH, Some(base));
    let directory = state
        .sessions
        .first()
        .map(|session| session.directory.clone())
        .unwrap_or_default();
    let span = |line: u32, from: u32, to: u32| {
        pm_protocol::Span::new(
            pm_protocol::Position::new(line, from),
            pm_protocol::Position::new(line, to),
        )
    };
    let diagnostics = vec![
        pm_protocol::Diagnostic {
            span: span(17, 26, 32),
            severity: pm_protocol::Severity::Error,
            message: "mismatched types\nexpected `u32`, found `&str`".to_owned(),
            source: Some("rustc".to_owned()),
        },
        pm_protocol::Diagnostic {
            span: span(17, 12, 17),
            severity: pm_protocol::Severity::Warning,
            message: "unused variable: `label`".to_owned(),
            source: Some("rustc".to_owned()),
        },
    ];
    let state = state
        .with_opened(
            0,
            CODE_PATH,
            Ok(pm_protocol::Triggers {
                completion: vec![".".to_owned(), ":".to_owned()],
                signature: vec!["(".to_owned(), ",".to_owned()],
                ..Default::default()
            }),
        )
        .with_diagnostics(&None, &directory, CODE_PATH, diagnostics);
    let caret = text.find("-> Self {").map_or(0, |at| at + "-> Self ".len());
    state
        .edited(EDITOR_FIELD, Edit::Place(caret, false))
        .with_blame(
            0,
            CODE_PATH,
            16,
            pm_git::Blame {
                id: "514332d8".to_owned(),
                author: "Joakim Carlsson".to_owned(),
                when: (pm_components::age::now() / 1000) as i64 - 3 * 86_400,
                summary: "Reopen files browsed on a project page".to_owned(),
            },
        )
}

/// The review panel on its search tab, after a search for `names` that found lines in three files.
fn project_search() -> WorkspaceState {
    let hit = |path: &str, line: u32, column: u32, text: &str| pm_protocol::Location {
        path: path.to_owned(),
        span: pm_protocol::Span::new(
            pm_protocol::Position::new(line, column),
            pm_protocol::Position::new(line, column),
        ),
        inside: true,
        line: Some(text.to_owned()),
        ..pm_protocol::Location::default()
    };
    let mut state = code().project_search_opened();
    for character in "names".chars() {
        state = state.project_search_typed(pm_text::Edit::Insert(character));
    }
    let session = state.review_index().unwrap_or_default();
    state.with_project_search(
        session,
        "names",
        false,
        false,
        Ok(vec![
            hit(
                CODE_PATH,
                8,
                4,
                "    names: HashMap<String, usize>,",
            ),
            hit(
                CODE_PATH,
                16,
                12,
                "        let names = HashMap::with_capacity(LIMIT);",
            ),
            hit(
                CODE_PATH,
                23,
                13,
                "        self.names.len()",
            ),
            hit(
                "crates/pm-lsp/src/hub.rs",
                120,
                30,
                "    /// The names of the servers the reader chose, kept for the length of the session and written out again when the project is opened.",
            ),
            hit("docs/protocol.md", 41, 9, "- **`names`:** every name"),
        ]),
    )
}

/// The hits of the search for `names` opened as excerpts, the files they come from read.
fn excerpts() -> WorkspaceState {
    let long = |line: usize, text: &str| {
        (0..140)
            .map(|row| {
                if row == line {
                    text.to_owned()
                } else {
                    format!("    let value_{row} = {row};")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let state = project_search().excerpts_opened();
    let session = state.review_index().unwrap_or_default();
    state
        .with_file(
            session,
            "crates/pm-lsp/src/hub.rs",
            false,
            Ok((
                Some(long(
                    120,
                    "    /// The names of the servers the reader chose.",
                )),
                false,
                Some("rust".to_owned()),
            )),
        )
        .with_file(
            session,
            "docs/protocol.md",
            false,
            Ok((Some(long(41, "- **`names`:** every name")), false, None)),
        )
}

/// The Rust file with what its language server says to show in and beside its lines: what each name is, hints inside lines and lenses at the ends of lines.
fn annotated() -> WorkspaceState {
    let state = code();
    let (text, version) = state
        .shown_document()
        .map(|document| (document.field.text().to_owned(), document.version))
        .unwrap_or_default();
    let place = |line: &str, word: &str| {
        let row = text
            .lines()
            .position(|held| held.contains(line))
            .unwrap_or(0);
        let column = text
            .lines()
            .nth(row)
            .and_then(|held| held.find(word))
            .unwrap_or(0);
        (row as u32, column as u32, column as u32 + word.len() as u32)
    };
    let token = |line: &str, word: &str, kind: pm_protocol::TokenKind| {
        let (line, start, end) = place(line, word);
        pm_protocol::SemanticToken {
            line,
            start,
            length: end - start,
            kind,
        }
    };
    let hint = |line: &str, word: &str, label: &str, kind: pm_protocol::HintKind, after: bool| {
        let (row, start, end) = place(line, word);
        pm_protocol::InlayHint {
            position: pm_protocol::Position::new(row, if after { end } else { start }),
            label: label.to_owned(),
            kind,
            pad_left: after,
            pad_right: !after,
        }
    };
    let lens = |line: &str, word: &str, title: &str, pressable: bool| {
        let (row, start, end) = place(line, word);
        pm_protocol::CodeLens {
            span: pm_protocol::Span::new(
                pm_protocol::Position::new(row, start),
                pm_protocol::Position::new(row, end),
            ),
            title: title.to_owned(),
            pressable,
        }
    };
    state
        .with_opened(
            0,
            CODE_PATH,
            Ok(pm_protocol::Triggers {
                semantic_tokens: true,
                inlay_hints: true,
                code_lens: true,
                ..Default::default()
            }),
        )
        .with_semantic_tokens(
            0,
            CODE_PATH,
            version,
            vec![
                token("with_capacity", "HashMap", pm_protocol::TokenKind::Type),
                token("with_capacity", "LIMIT", pm_protocol::TokenKind::Constant),
                token("let names", "names", pm_protocol::TokenKind::Variable),
                token("Self { id", "root", pm_protocol::TokenKind::Variable),
                token("self.names.len", "len", pm_protocol::TokenKind::Function),
            ],
        )
        .with_inlay_hints(
            0,
            CODE_PATH,
            version,
            vec![
                hint(
                    "let names",
                    "names",
                    ": HashMap<String, usize>",
                    pm_protocol::HintKind::Type,
                    true,
                ),
                hint(
                    "with_capacity",
                    "LIMIT",
                    "capacity:",
                    pm_protocol::HintKind::Parameter,
                    false,
                ),
            ],
        )
        .with_code_lenses(
            0,
            CODE_PATH,
            version,
            vec![
                lens("pub struct Session", "Session", "3 implementations", true),
                lens("pub fn count", "count", "2 references", true),
                lens("pub fn new", "new", "Run", false),
            ],
        )
}

/// The Rust file with `self.names.` typed into `count` and the language server's completions open under it.
fn completing() -> WorkspaceState {
    let state = code();
    let anchor = "        self.names.len()";
    let at = state
        .shown_document()
        .and_then(|document| document.field.text().find(anchor))
        .map_or(0, |at| at + "        self.names.".len());
    let state = state
        .edited(EDITOR_FIELD, Edit::Place(at, false))
        .edited(EDITOR_FIELD, Edit::Paste("i".to_owned()))
        .completion_requested((236.0, 548.0));
    let version = state
        .shown_document()
        .map_or(0, |document| document.version);
    let item = |label: &str, detail: &str, kind: pm_protocol::CompletionKind| {
        pm_protocol::Completion {
            label: label.to_owned(),
            detail: Some(detail.to_owned()),
            kind,
            text: label.to_owned(),
            span: None,
            filter: label.to_owned(),
            sort: label.to_owned(),
            documentation: (label == "insert").then(|| {
                "Inserts a key-value pair into the map.\n\nIf the map did not have this key present, `None` is returned.\n\n```rust\nlet mut map = HashMap::new();\nassert_eq!(map.insert(37, \"a\"), None);\n```".to_owned()
            }),
        }
    };
    use pm_protocol::CompletionKind::{Function, Type};
    state.with_completions(
        0,
        CODE_PATH,
        version,
        vec![
            item("insert", "fn(&mut self, k: K, v: V) -> Option<V>", Function),
            item("into_iter", "fn(self) -> IntoIter<K, V>", Function),
            item("into_keys", "fn(self) -> IntoKeys<K, V>", Function),
            item("into_values", "fn(self) -> IntoValues<K, V>", Function),
            item("is_empty", "fn(&self) -> bool", Function),
            item("iter", "fn(&self) -> Iter<'_, K, V>", Function),
            item("iter_mut", "fn(&mut self) -> IterMut<'_, K, V>", Function),
            item("IntoIter", "struct IntoIter<K, V>", Type),
        ],
    )
}

/// The Rust file with two breakpoints set and the program paused on the first, its stack, variables, watch and console in the dock.
/// What the manifests of a project with a Cargo manifest, a Makefile and npm scripts imply, and a task it writes itself.
fn sample_tasks() -> Vec<pm_protocol::Task> {
    let task = |name: &str, command: &str, group: &str, source: &str| pm_protocol::Task {
        name: name.to_owned(),
        command: command.to_owned(),
        group: group.to_owned(),
        source: source.to_owned(),
        ..pm_protocol::Task::default()
    };
    vec![
        task(
            "release notes",
            "scripts/notes.sh",
            "",
            ".pandemonium.tasks.json",
        ),
        task(
            "cargo build",
            "cargo build --workspace",
            "build",
            "Cargo.toml",
        ),
        task(
            "cargo check",
            "cargo check --workspace",
            "build",
            "Cargo.toml",
        ),
        task("cargo test", "cargo test --workspace", "test", "Cargo.toml"),
        task("make: lint", "make lint", "", "Makefile"),
        task("make: run", "make run", "run", "Makefile"),
        task("npm: build", "npm run build", "build", "package.json"),
        task("npm: dev", "npm run dev", "run", "package.json"),
    ]
}

/// The toolchains a machine with rustup, two Node.js versions and a virtual environment has.
fn sample_toolchains() -> Vec<pm_protocol::ToolchainInfo> {
    let toolchain =
        |language: &str, id: &str, detail: &str, selected: bool| pm_protocol::ToolchainInfo {
            language: language.to_owned(),
            id: id.to_owned(),
            name: id.to_owned(),
            detail: detail.to_owned(),
            selected,
        };
    vec![
        toolchain("rust", "stable-x86_64-unknown-linux-gnu", "default", true),
        toolchain("rust", "nightly-x86_64-unknown-linux-gnu", "", false),
        toolchain("javascript", "system", "v22.11.0", true),
        toolchain("javascript", "nvm v20.18.0", "v20.18.0", false),
        toolchain("python", "system", "Python 3.13.1", true),
        toolchain("python", "venv .venv", "Python 3.12.7", false),
    ]
}

/// A file with a `main`, a test and a module of tests, so that the gutter shows a run button beside each.
fn runnable() -> WorkspaceState {
    let state = code();
    let text = "use std::path::PathBuf;\n\n/// Where the session works.\npub fn root() -> PathBuf {\n    PathBuf::from(\".\")\n}\n\nfn main() {\n    println!(\"{}\", root().display());\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn finds_the_root() {\n        assert_eq!(root(), PathBuf::from(\".\"));\n    }\n\n    #[test]\n    fn shows_the_root() {\n        assert!(!root().display().to_string().is_empty());\n    }\n}\n";
    state.with_file(
        0,
        CODE_PATH,
        false,
        Ok((Some(text.to_owned()), false, Some("rust".to_owned()))),
    )
}

fn debugging() -> WorkspaceState {
    use pm_protocol::{
        Breakpoint, DebugCategory, DebugFrame, DebugLine, DebugScope, DebugThread, DebugView,
        DebugWatched, Placed, Standing, Variable,
    };
    let mut state = code();
    let root = state
        .sessions
        .first()
        .map(|session| session.directory.clone())
        .unwrap_or_default();
    let path = std::path::PathBuf::from(&root).join(CODE_PATH);
    let frame = |id: i64, name: &str, line: usize| DebugFrame {
        id,
        name: name.to_owned(),
        path: Some(path.clone()),
        line,
        column: 0,
    };
    let variable = |name: &str, value: &str, kind: &str, reference: i64| Variable {
        name: name.to_owned(),
        value: value.to_owned(),
        kind: Some(kind.to_owned()),
        reference,
    };
    let line = |category: DebugCategory, text: &str| DebugLine {
        category,
        text: text.to_owned(),
    };
    state.debug.breakpoints.insert(
        (0, CODE_PATH.to_owned()),
        vec![
            Breakpoint {
                line: 17,
                ..Breakpoint::default()
            },
            Breakpoint {
                line: 24,
                condition: Some("id > 3".to_owned()),
                ..Breakpoint::default()
            },
        ],
    );
    state.debug.watches = vec!["root".to_owned(), "names.len()".to_owned()];
    state.debug.unfolded.insert(7);
    state.debug.run = Some(pm_components::debug::Run {
        session: 0,
        debug: 1,
        view: DebugView {
            label: "Run pandemonium".to_owned(),
            standing: Standing::Stopped,
            reason: Some("breakpoint 1".to_owned()),
            threads: vec![DebugThread {
                id: 1,
                name: "main".to_owned(),
            }],
            thread: Some(1),
            frames: vec![
                frame(1, "Session::new", 17),
                frame(2, "start", 211),
                frame(3, "main", 31),
            ],
            frame: Some(1),
            scopes: vec![DebugScope {
                name: "Locals".to_owned(),
                reference: 5,
                expensive: false,
            }],
            variables: vec![
                (
                    5,
                    vec![
                        variable("id", "4", "u64", 0),
                        variable("root", "\"/home/me/code\"", "PathBuf", 0),
                        variable("names", "HashMap { len: 2 }", "HashMap<String, usize>", 7),
                    ],
                ),
                (
                    7,
                    vec![
                        variable("main", "0", "usize", 0),
                        variable("lib", "1", "usize", 0),
                    ],
                ),
            ],
            watches: vec!["root".to_owned(), "names.len()".to_owned()],
            watched: vec![
                DebugWatched {
                    expression: "root".to_owned(),
                    value: Ok(variable("root", "\"/home/me/code\"", "PathBuf", 0)),
                },
                DebugWatched {
                    expression: "names.len()".to_owned(),
                    value: Err("no method named `len`".to_owned()),
                },
            ],
            lines: vec![
                line(DebugCategory::Console, "Launched target/debug/pandemonium"),
                line(DebugCategory::Stdout, "listening on 6868"),
                line(DebugCategory::Stderr, "warning: token is sent in the clear"),
                line(DebugCategory::Asked, "id + 1"),
                line(DebugCategory::Answer, "5"),
            ],
            line_count: 5,
            placed: vec![(
                path.clone(),
                vec![Placed {
                    line: 24,
                    verified: false,
                    message: None,
                }],
            )],
        },
    });
    state.bottom = pm_components::dock::BottomTab::Debug;
    state.terminal = true;
    state
}

/// The Rust file with the dock open on the list of what is wrong with the project.
fn problems() -> WorkspaceState {
    let mut state = code();
    let at = |line: u32, character: u32| pm_protocol::Position { line, character };
    let problem = |path: &str, line: u32, severity, message: &str| pm_protocol::Problem {
        path: path.to_owned(),
        diagnostic: pm_protocol::Diagnostic {
            span: pm_protocol::Span {
                start: at(line, 4),
                end: at(line, 9),
            },
            severity,
            message: message.to_owned(),
            source: Some("rustc".to_owned()),
        },
    };
    let session = state.review_index().unwrap_or_default();
    state = state.with_problems(
        session,
        vec![
            problem(
                CODE_PATH,
                12,
                pm_protocol::Severity::Error,
                "mismatched types: expected `u32`, found `&str`",
            ),
            problem(
                CODE_PATH,
                17,
                pm_protocol::Severity::Warning,
                "unused variable: `label`",
            ),
            problem(
                "src/main.rs",
                3,
                pm_protocol::Severity::Warning,
                "unused import: `std::fmt`",
            ),
        ],
    );
    state.bottom = pm_components::dock::BottomTab::Problems;
    state.terminal = true;
    state
}

/// The Rust file with the card about the string that does not fit its type open over it.
fn hovering() -> WorkspaceState {
    let state = code();
    let offset = state
        .shown_document()
        .and_then(|document| document.field.text().find("main\";"))
        .unwrap_or_default();
    let state = state.hover_requested(offset, (330.0, 440.0));
    let position = state
        .shown_document()
        .map(|document| document.position_of(offset))
        .unwrap_or_default();
    state.with_hover(
        0,
        CODE_PATH,
        position,
        Some(pm_protocol::Hover {
            text: "\n```rust\npandemonium::session\n```\n\n```rust\npub fn new(id: u64, root: PathBuf) -> Self\n```\n\n---\n\nMakes a session working in `root`, with no names yet. See [the session docs](https://example.com/session).\n\n```rust\npub fn with_a_very_long_signature(first_argument: HashMap<String, Vec<PathBuf>>, second_argument: Option<&mut Session>) -> Result<Self, std::io::Error>\n```\n\nParagraph 1 of the documentation keeps going so the card has more to say than fits, which is what makes it scroll instead of running off the bottom of the window.\n\nParagraph 2 of the documentation keeps going so the card has more to say than fits, which is what makes it scroll instead of running off the bottom of the window.\n\nParagraph 3 of the documentation keeps going so the card has more to say than fits, which is what makes it scroll instead of running off the bottom of the window.\n\nParagraph 4 of the documentation keeps going so the card has more to say than fits, which is what makes it scroll instead of running off the bottom of the window.\n\nParagraph 5 of the documentation keeps going so the card has more to say than fits, which is what makes it scroll instead of running off the bottom of the window.\n\nParagraph 6 of the documentation keeps going so the card has more to say than fits, which is what makes it scroll instead of running off the bottom of the window.\n\nParagraph 7 of the documentation keeps going so the card has more to say than fits, which is what makes it scroll instead of running off the bottom of the window.\n\nParagraph 8 of the documentation keeps going so the card has more to say than fits, which is what makes it scroll instead of running off the bottom of the window.".to_owned(),
            span: None,
        }),
    )
}

/// The Rust file with the places `Session::new` is used listed under the caret.
fn references() -> WorkspaceState {
    let state = code().seek_requested(0, pm_protocol::Seek::References, (330.0, 470.0));
    let place = |path: &str, line: u32, text: &str| pm_protocol::Location {
        path: path.to_owned(),
        span: pm_protocol::Span::new(
            pm_protocol::Position::new(line, 8),
            pm_protocol::Position::new(line, 11),
        ),
        inside: true,
        line: Some(text.to_owned()),
        ..pm_protocol::Location::default()
    };
    state.with_places(
        0,
        pm_protocol::Seek::References,
        vec![
            place(
                CODE_PATH,
                14,
                "    pub fn new(id: u64, root: PathBuf) -> Self {",
            ),
            place(
                "crates/pandemonium/src/app.rs",
                211,
                "        let session = Session::new(next_id, root.clone());",
            ),
            place(
                "crates/pandemonium/src/work.rs",
                88,
                "            sessions.push(Session::new(id, folder));",
            ),
            place(
                "crates/pm-server/src/core.rs",
                301,
                "    let session = Session::new(request.id, place(&request.directory));",
            ),
            place(
                "tests/sessions.rs",
                12,
                "    let made = Session::new(1, PathBuf::from(\"/tmp/a\"));",
            ),
        ],
    )
}

/// The Rust file with the caret in the call to `with_capacity`, its signature over it, and the uses of `names` marked.
fn signing() -> WorkspaceState {
    let state = code();
    let text = state
        .shown_document()
        .map(|document| document.field.text().to_owned())
        .unwrap_or_default();
    let caret = text.find("(LIMIT)").map_or(0, |at| at + 1);
    let mut state = state
        .edited(EDITOR_FIELD, Edit::Place(caret, false))
        .with_signature_at((470.0, 362.0));
    let version = state
        .shown_document()
        .map_or(0, |document| document.version);
    let uses: Vec<pm_protocol::Span> = text
        .match_indices("names")
        .filter_map(|(at, _)| {
            let document = state.shown_document()?;
            Some(pm_protocol::Span::new(
                document.position_of(at),
                document.position_of(at + "names".len()),
            ))
        })
        .collect();
    state = state.with_highlights(0, CODE_PATH, version, uses);
    state.with_signature(
        0,
        CODE_PATH,
        version,
        Some(pm_protocol::Signature {
            label: "pub fn with_capacity(capacity: usize) -> HashMap<K, V, RandomState>".to_owned(),
            active: Some((21, 36)),
            documentation: Some(
                "Creates an empty `HashMap` with at least the specified capacity.".to_owned(),
            ),
            others: 0,
        }),
    )
}

/// The first session with a file edited inside the submodule `vendor/inner`, the submodule moved two commits on, and its diff open.
fn diff_submodule() -> WorkspaceState {
    let mut state = workspace_sample();
    if let Some(session) = state.sessions.first_mut() {
        let mut edited = pm_components::sample::Change::new(
            "vendor/inner/src/parse.rs",
            "@@ -3,3 +3,4 @@\n fn parse(text: &str) -> Tree {\n-    Tree::empty()\n+    let tokens = lex(text);\n+    Tree::from(tokens)\n }",
        );
        edited.submodule = Some("vendor/inner".to_owned());
        let mut moved = pm_components::sample::Change::new("vendor/inner", "");
        moved.moved = Some(pm_git::Moved {
            from: Some("a1b2c3d".to_owned()),
            to: "e4f5a6b".to_owned(),
            commits: vec![
                "Lex before parsing".to_owned(),
                "Build trees from tokens".to_owned(),
            ],
            back: false,
        });
        session.changes.splice(0..0, [moved, edited]);
    }
    state.after(TAB_DIFF)
}

/// The session of [`diff_submodule`] with its submodules listed as repositories, the folder's own picked, or, when `inside`, the submodule `vendor/inner` picked with its branches open.
fn branches_submodule(inside: bool) -> WorkspaceState {
    let mut state = diff_submodule();
    state.git.submodule_heads = vec![
        ("vendor/inner".to_owned(), "main".to_owned()),
        ("vendor/inner/nested".to_owned(), "@ 3f2a1c9".to_owned()),
        ("deps/router".to_owned(), "feat/router".to_owned()),
    ];
    if inside {
        state.git.branch_menu = true;
        state.git.repository = Some("vendor/inner".to_owned());
        state.git.branches = vec![
            "main".to_owned(),
            "fix/lexer".to_owned(),
            "origin/main".to_owned(),
        ];
    } else {
        state.git.branches = vec!["task/227-checkpoints".to_owned(), "main".to_owned()];
    }
    state
}

/// A JavaScript file open while JavaScript is not installed, so the offer to install it is above its lines.
fn language_offer() -> WorkspaceState {
    let path = "scripts/build.js";
    let mut state = workspace_sample().after(TAB_DIFF);
    state.extensions = sample_languages();
    state.open_viewer(path.to_owned(), false);
    state.with_file(
        0,
        path,
        false,
        Ok((
            Some("const fs = require(\"fs\");\n\nfunction build(files) {\n  return files.map((file) => fs.readFileSync(file, \"utf8\"));\n}\n\nmodule.exports = { build };\n".to_owned()),
            false,
            None,
        )),
    )
}

/// The Rust file set against a state it was saved in a while ago, its changed lines edited in place.
fn timeline() -> WorkspaceState {
    let state = code();
    let older = state
        .shown_document()
        .map(|document| document.field.text().to_owned())
        .unwrap_or_default()
        .replace("const LIMIT: usize = 64;", "const LIMIT: usize = 32;")
        .replace("HashMap::with_capacity(LIMIT)", "HashMap::new()")
        .replace(
            "    /// How many names the session knows.\n    pub fn count(&self) -> usize {\n        self.names.len()\n    }\n",
            "    pub fn len(&self) -> usize {\n        self.names.len()\n    }\n",
        );
    state.timeline_opened(0, CODE_PATH, "25m ago".to_owned(), older)
}

/// A file a merge stopped on, with two places where the sides disagree, and the result under them.
fn conflict() -> WorkspaceState {
    load_installed_grammar("rust");
    let path = "crates/pandemonium/src/session.rs";
    let ours = "pub struct Session {\n    pub id: u64,\n    pub root: PathBuf,\n    checkpoints: Vec<Checkpoint>,\n}\n";
    let theirs = "pub struct Session {\n    pub id: u64,\n    pub root: PathBuf,\n    titles: Vec<String>,\n}\n";
    let merged = "use std::path::PathBuf;\n\n/// A session of an agent.\n<<<<<<< HEAD\npub struct Session {\n    pub id: u64,\n    pub root: PathBuf,\n    checkpoints: Vec<Checkpoint>,\n}\n=======\npub struct Session {\n    pub id: u64,\n    pub root: PathBuf,\n    titles: Vec<String>,\n}\n>>>>>>> origin/main\n\nimpl Session {\n    pub fn new(id: u64, root: PathBuf) -> Self {\n<<<<<<< HEAD\n        Self { id, root, checkpoints: Vec::new() }\n=======\n        Self { id, root, titles: Vec::new() }\n>>>>>>> origin/main\n    }\n}\n";
    workspace_sample()
        .after(TAB_DIFF)
        .with_conflict(
            0,
            path,
            Some(ours.to_owned()),
            Some(theirs.to_owned()),
            None,
        )
        .with_file(
            0,
            path,
            false,
            Ok((Some(merged.to_owned()), false, Some("rust".to_owned()))),
        )
}

/// The Rust file with the changes the language server offers at the caret listed.
fn code_actions() -> WorkspaceState {
    let state = code().code_actions_requested((330.0, 470.0));
    let action = |title: &str, kind: &str, preferred: bool| pm_protocol::CodeAction {
        title: title.to_owned(),
        kind: Some(kind.to_owned()),
        preferred,
        disabled: None,
    };
    state.with_code_actions(
        0,
        CODE_PATH,
        vec![
            action("Change type of `count` to `&str`", "quickfix", true),
            action("Convert to `parse::<u32>()`", "quickfix", false),
            action("Extract into variable", "refactor.extract", false),
            action("Inline `label`", "refactor.inline", false),
            pm_protocol::CodeAction {
                title: "Generate documentation template".to_owned(),
                kind: Some("refactor.rewrite".to_owned()),
                preferred: false,
                disabled: Some("not on a function".to_owned()),
            },
        ],
    )
}

/// The Rust file with the commits that changed it listed.
fn file_history() -> WorkspaceState {
    let state = code().file_history_requested((330.0, 470.0));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64);
    let commit = |id: &str, author: &str, when: &str, ago: i64, summary: &str| pm_git::FileCommit {
        object: format!("{id}0000000000000000000000000000000"),
        id: id.to_owned(),
        author: author.to_owned(),
        when: when.to_owned(),
        time: now - ago,
        summary: summary.to_owned(),
        path: CODE_PATH.to_owned(),
    };
    let saved = |ago: i64, size: u64| pm_protocol::LocalVersion {
        id: ((now - ago) * 1000) as u64,
        size,
    };
    let state = state.with_local_history(
        0,
        CODE_PATH,
        vec![
            saved(90, 1_204),
            saved(25 * 60, 1_187),
            saved(70 * 60, 1_102),
        ],
    );
    state.with_file_history(
        0,
        CODE_PATH,
        vec![
            commit(
                "54149c90",
                "Joakim Carlsson",
                "10 minutes ago",
                600,
                "Add go to definition, formatting and jumps",
            ),
            commit(
                "514332d8",
                "Joakim Carlsson",
                "2 hours ago",
                7_200,
                "Reopen files browsed on a project page",
            ),
            commit(
                "e3e796f4",
                "Joakim Carlsson",
                "3 days ago",
                259_200,
                "Add language support: tree-sitter highlighting",
            ),
            commit(
                "81f70099",
                "Joakim Carlsson",
                "2 weeks ago",
                1_209_600,
                "Add the preview channel for pre-releases",
            ),
        ],
    )
}

/// The sample extension package: it brings a language, a theme, a debug adapter, a tool server and a command, and asks to run programs and download files, of which only running programs is allowed once it is `installed`.
fn sample_extension(installed: bool) -> pm_protocol::ExtensionInfo {
    pm_protocol::ExtensionInfo {
        id: "hello".to_owned(),
        name: "Hello".to_owned(),
        kind: "extension".to_owned(),
        version: "0.1.0".to_owned(),
        description:
            "A sample extension: a language, a theme, a debug adapter, a tool server and a command."
                .to_owned(),
        installed,
        local: true,
        provides: [
            "language: Hello Config",
            "theme: Hello Dusk",
            "debug adapter: Hello",
            "tool server: hello-tools",
            "command: Say hello",
        ]
        .map(str::to_owned)
        .to_vec(),
        requests: ["process", "download"].map(str::to_owned).to_vec(),
        granted: if installed {
            vec!["process".to_owned()]
        } else {
            Vec::new()
        },
        ..Default::default()
    }
}

/// The languages a server on this machine could list: some installed, one being installed and one that failed.
fn sample_languages() -> Vec<pm_protocol::ExtensionInfo> {
    let language =
        |id: &str, name: &str, suffix: &str, installed: bool| pm_protocol::ExtensionInfo {
            id: id.to_owned(),
            name: name.to_owned(),
            kind: "language".to_owned(),
            version: "0.1.0".to_owned(),
            description: format!("{name} support."),
            icon: std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../languages")
                    .join(id)
                    .join("icon.svg"),
            )
            .ok(),
            color: Some("#8fb8e8".to_owned()),
            hidden: false,
            suffixes: vec![suffix.to_owned()],
            installed,
            busy: None,
            problem: None,
            server: None,
            local: false,
            ..Default::default()
        };
    vec![
        sample_extension(true),
        pm_protocol::ExtensionInfo {
            server: Some("gopls v0.24.0".to_owned()),
            ..language("go", "Go", "go", true)
        },
        language("javascript", "JavaScript", "js", false),
        language("json", "JSON", "json", false),
        pm_protocol::ExtensionInfo {
            busy: Some("Installing the language server".to_owned()),
            ..language("markdown", "Markdown", "md", false)
        },
        pm_protocol::ExtensionInfo {
            problem: Some("Python: installing this language server needs Node.js and npm, which is not on this machine".to_owned()),
            ..language("python", "Python", "py", true)
        },
        pm_protocol::ExtensionInfo {
            server: Some("Uses /usr/bin/rust-analyzer".to_owned()),
            ..language("rust", "Rust", "rs", true)
        },
        language("toml", "TOML", "toml", false),
        pm_protocol::ExtensionInfo {
            local: true,
            server: Some("zls 0.14.0".to_owned()),
            ..language("zig", "Zig", "zig", true)
        },
        language("typescript", "TypeScript", "ts", false),
    ]
}

/// The workspace of made-up sessions.
fn workspace_sample() -> WorkspaceState {
    WorkspaceState::sample()
}

/// The workspace on the diff of its first session, the words that changed inside its changed lines marked as the server tells them.
fn word_diff() -> WorkspaceState {
    let mut state = workspace_sample().after(TAB_DIFF);
    for change in &mut state.sessions[0].changes {
        let source = change
            .diff
            .iter()
            .map(|line| {
                let marker = match line.kind {
                    pm_components::sample::Kind::Add => "+",
                    pm_components::sample::Kind::Remove => "-",
                    pm_components::sample::Kind::Context => " ",
                    pm_components::sample::Kind::Hunk => "",
                };
                format!("{marker}{}", line.text)
            })
            .collect::<Vec<_>>()
            .join("\n");
        change.words = pm_git::words(&source);
    }
    state
}

/// The workspace on the diff with a comment kept on some lines and a box open to write another on a file.
fn commenting() -> WorkspaceState {
    let state = workspace_sample()
        .after(TAB_DIFF)
        .comment_drag_begun(gutter_id(1, true, 23))
        .comment_drag_extended(gutter_id(1, true, 26))
        .comment_drag_ended()
        .edited(
            COMMENT_FIELD,
            Edit::Paste("Return the id from checkpoint() and drop the unwrap.".to_owned()),
        )
        .after(COMMENT_ADD);
    state
        .comment_drag_begun(gutter_id(1, false, 18))
        .comment_drag_extended(gutter_id(1, false, 19))
        .comment_drag_ended()
        .scrolled_diff(560.0, 5000.0)
}

/// The commits of a graph with a branch that forks off and is merged back.
fn commits() -> Vec<pm_git::Commit> {
    use pm_git::{Commit, Edge, Half, Lanes};
    let edge = |half, from, to, color| Edge {
        half,
        from,
        to,
        color,
    };
    let line = |lane: usize, color: usize, width: usize, edges: Vec<Edge>| Lanes {
        lane,
        color,
        edges,
        width,
    };
    let commit = |id: &str, refs: &[&str], summary: &str, lanes: Lanes| Commit {
        id: id.to_owned(),
        refs: refs.iter().map(|name| (*name).to_owned()).collect(),
        summary: summary.to_owned(),
        lanes,
    };
    vec![
        commit(
            "f180f7b",
            &["HEAD -> task/rewrite", "origin/task/rewrite"],
            "Build only the visible diff rows, stop copying the state on every event",
            line(0, 0, 1, vec![edge(Half::Lower, 0, 0, 0)]),
        ),
        commit(
            "0fdd820",
            &[],
            "Show the diff side by side with file icons and line comments",
            line(
                0,
                0,
                2,
                vec![
                    edge(Half::Upper, 0, 0, 0),
                    edge(Half::Lower, 0, 0, 0),
                    edge(Half::Lower, 0, 1, 1),
                ],
            ),
        ),
        commit(
            "d94cadd",
            &["origin/feat/modal"],
            "Add projects from a modal that browses server folders",
            line(
                1,
                1,
                2,
                vec![
                    edge(Half::Upper, 0, 0, 0),
                    edge(Half::Upper, 1, 1, 1),
                    edge(Half::Lower, 0, 0, 0),
                    edge(Half::Lower, 1, 1, 1),
                ],
            ),
        ),
        commit(
            "9a625ad",
            &["main"],
            "Select the conversation's text with the pointer",
            line(
                0,
                0,
                2,
                vec![
                    edge(Half::Upper, 0, 0, 0),
                    edge(Half::Upper, 1, 0, 1),
                    edge(Half::Lower, 0, 0, 0),
                ],
            ),
        ),
        commit(
            "b0fcc99",
            &["tag: v0.12.0"],
            "Ask the agent's questions in a card above the prompt",
            line(0, 0, 1, vec![edge(Half::Upper, 0, 0, 0)]),
        ),
    ]
}

/// The workspace on the diff with the source control panel filled in: a staged file, an untracked one, a message waiting and the graph.
fn source() -> WorkspaceState {
    let mut state = workspace_sample()
        .after(TAB_DIFF)
        .after(FocusId(STAGE_BASE))
        .after(FocusId(FILE_BASE + 1))
        .edited(
            COMMIT_MESSAGE,
            Edit::Paste("Restore sessions from checkpoints".to_owned()),
        )
        .with_history(0, commits());
    if let Some(session) = state.sessions.get_mut(0) {
        session.changes[3].untracked = true;
        session.changes[3].worktree = Some('?');
    }
    state
}

/// The workspace on the diff with the first and third changed files picked.
fn picked() -> WorkspaceState {
    workspace_sample()
        .after(TAB_DIFF)
        .after(FocusId(FILE_BASE))
        .change_picked(2, Choosing::Toggle)
}

/// The workspace with the list of branches open and `feat` typed in its box.
fn branching() -> WorkspaceState {
    let names = [
        "task/227-checkpoints",
        "main",
        "feat/session-titles",
        "feat/worktree-cleanup",
        "task/271-add-per-project-tasks-run-buttons-and-a-toolchain-selector",
        "origin/main",
        "origin/feat/editor",
    ];
    workspace_sample()
        .after(TAB_DIFF)
        .after(BRANCH_BUTTON)
        .with_session_branches(0, names.map(str::to_owned).to_vec())
        .edited(BRANCH_FILTER, Edit::Paste("t".to_owned()))
}

/// The workspace reading a file of the first session as it is in the last commit.
fn reading() -> WorkspaceState {
    let mut state = workspace_sample().after(TAB_DIFF);
    state.open_viewer("crates/pandemonium/src/session.rs".to_owned(), true);
    let text = "use std::path::PathBuf;\n\n/// A session of an agent.\npub struct Session {\n    pub id: SessionId,\n    pub root: PathBuf,\n}\n\nimpl Session {\n    pub fn new(id: SessionId, root: PathBuf) -> Self {\n        Self { id, root }\n    }\n}\n";
    state.with_file(
        0,
        "crates/pandemonium/src/session.rs",
        true,
        Ok((Some(text.to_owned()), false, None)),
    )
}

/// The workspace with one file kept open after an edit and another being looked at.
fn editing() -> WorkspaceState {
    let mut state = workspace_sample().after(TAB_DIFF);
    state.open_viewer("crates/pandemonium/src/session.rs".to_owned(), false);
    let text = "use std::path::PathBuf;\n\n/// A session of an agent.\npub struct Session {\n    pub id: SessionId,\n    pub root: PathBuf,\n}\n\nimpl Session {\n    pub fn new(id: SessionId) -> Self {\n        let name = \"main\"; // 42\n\tlet tabbed = 1;\n        Self { id, root: PathBuf::from(name) }\n    }\n}\n";
    let state = state.with_file(
        0,
        "crates/pandemonium/src/session.rs",
        false,
        Ok((Some(text.to_owned()), false, None)),
    );
    let mut state = state.edited(EDITOR_FIELD, Edit::Paste("// edited\n".to_owned()));
    state.open_viewer("crates/pandemonium/src/lib.rs".to_owned(), false);
    state.with_file(
        0,
        "crates/pandemonium/src/lib.rs",
        false,
        Ok((Some("pub mod session;\n".to_owned()), false, None)),
    )
}

/// The overview of the first project: its sessions, its own folder on a branch ahead of what it follows with a change, and its latest commits.
fn overview() -> WorkspaceState {
    let mut state = workspace_sample().after(FocusId(pm_components::state::PROJECT_BASE));
    let home = state.review_index().unwrap_or_default();
    if let Some(session) = state.sessions.get_mut(home) {
        session.branch = "main".to_owned();
        session.on_branch = true;
        session.tracking = pm_git::Tracking {
            upstream: Some("origin/main".to_owned()),
            ahead: 2,
            behind: 0,
        };
    }
    let changes = state.sessions[0].changes[..1].to_vec();
    state.sessions[home].changes = changes;
    state.with_history(home, commits())
}

/// The first project with two submodules, one nested in the other, its second session working in the outer one, and new work being started in the nested one.
fn submodules() -> WorkspaceState {
    let mut state = workspace_sample();
    state.projects[0].submodules =
        vec!["vendor/inner".to_owned(), "vendor/inner/nested".to_owned()];
    if let Some(session) = state
        .sessions
        .iter_mut()
        .filter(|session| session.project == 0 && !session.home)
        .nth(1)
    {
        session.submodule = Some("vendor/inner".to_owned());
    }
    state.after(FocusId(pm_components::state::NEW_IN_SUBMODULE_BASE + 1))
}

/// The page for starting work with a file of the project open beside it.
fn new_file() -> WorkspaceState {
    let mut state = workspace_sample().after(FocusId(pm_components::state::NEW_IN_BASE));
    state.open_viewer("src/main.rs".to_owned(), false);
    let session = state.review_index().unwrap_or_default();
    state.with_file(
        session,
        "src/main.rs",
        false,
        Ok((
            Some("fn main() {\n    println!(\"hi\");\n}\n".to_owned()),
            false,
            None,
        )),
    )
}

/// The workspace with the project's files listed in the review panel.
fn files() -> WorkspaceState {
    let state = workspace_sample().after(REVIEW_FILES);
    let key = state.files_of.clone().unwrap_or_default();
    let paths = [
        "Cargo.toml",
        "README.md",
        "docs/protocol.md",
        "crates/pandemonium/src/main.rs",
        "crates/pandemonium/src/session.rs",
        "crates/pandemonium/src/checkpoint.rs",
        "crates/pandemonium/src/lib.rs",
        "crates/pandemonium/Cargo.toml",
        "crates/pm-ui/src/lib.rs",
        "scripts/install.sh",
        ".env",
    ];
    state.with_files(
        &key,
        paths.map(str::to_owned).to_vec(),
        vec![
            "crates/pandemonium/src/empty".to_owned(),
            "target".to_owned(),
        ],
        vec![".env".to_owned(), "target".to_owned()],
    )
}

/// The independent prompt and conversation sizes used by typing benchmarks and render fixtures.
fn input_stress(long_prompt: bool, long_chat: bool) -> WorkspaceState {
    let mut state = workspace_sample();
    state.review = false;
    let prompt = if long_prompt {
        "A paragraph with Unicode café 🦀 and ordinary text to wrap. ".repeat(12) + "\n"
    } else {
        "hello".to_owned()
    };
    state.prompt = pm_components::state::prompt_field(&if long_prompt {
        prompt.repeat(120)
    } else {
        prompt
    });
    if long_chat {
        let mut message = state.sessions[0]
            .messages
            .iter()
            .find(|message| !message.from_user)
            .unwrap()
            .clone();
        message.text =
            "A paragraph of agent output with **formatting** and `code`.\n\n".repeat(100);
        message.said.clear();
        message.steps.clear();
        message.answers.clear();
        state.sessions[0].messages = vec![message; 40];
    }
    state
}

/// A large expanded tree with stable names, optionally scrolled halfway down.
fn large_files(scrolled: bool) -> WorkspaceState {
    let mut state = files();
    state.files = (0..10_000)
        .map(|index| format!("src/module-{:03}/file-{:05}.rs", index / 100, index))
        .collect();
    state.tree_folders.clear();
    state.tree_ignored.clear();
    state.open_tree = std::iter::once("src".to_owned())
        .chain((0..100).map(|index| format!("src/module-{index:03}")))
        .collect();
    if scrolled {
        state.tree_scroll = 120_000.0;
    }
    state
}

/// Expanded ignored folders with every nested file and folder dimmed.
fn ignored_files() -> WorkspaceState {
    let mut state = files();
    let paths = [
        "server/prod/connect.sh",
        "server/prod/key.pem",
        "server/stage/connect.sh",
        "server/stage/clip.mp4",
    ];
    let folders = ["server", "server/prod", "server/stage"];
    state.files.extend(paths.map(str::to_owned));
    state.tree_folders.extend(folders.map(str::to_owned));
    state
        .tree_ignored
        .extend(paths.into_iter().chain(folders).map(str::to_owned));
    state.open_tree.extend(folders.map(str::to_owned));
    state
}

/// The workspace with a shell open that has run a few commands, as the shell would have drawn them.
fn terminal() -> WorkspaceState {
    with_shell(workspace_sample())
}

/// `state` with the terminal opened and a few commands run in its shell, as the shell would have drawn them.
fn with_shell(state: WorkspaceState) -> WorkspaceState {
    let mut state = state.after(TERMINAL);
    state.fit_terminal(75, 11);
    let printed = "\x1b[1;32mpandemonium\x1b[0m \x1b[34m~/code/pandemonium\x1b[0m $ git status\r\n\
On branch \x1b[36mtask/227-checkpoints\x1b[0m\r\n\
Changes not staged for commit:\r\n\
\t\x1b[31mmodified:   crates/pandemonium/src/session.rs\x1b[0m\r\n\
\x1b[1;32mpandemonium\x1b[0m \x1b[34m~/code/pandemonium\x1b[0m $ cargo test\r\n\
\x1b[1;32m   Compiling\x1b[0m pandemonium v0.12.0\r\n\
\x1b[1;32m    Finished\x1b[0m test profile in 6.21s\r\n\
test result: \x1b[32mok\x1b[0m. 9 passed; 0 failed\r\n\
\x1b[1;32mpandemonium\x1b[0m \x1b[34m~/code/pandemonium\x1b[0m $ ";
    state.terminal_output(1, printed.as_bytes());
    state
}

/// The projects shown in the sample workspace.
fn projects() -> Vec<Project> {
    ["~/code/pandemonium", "~/code/voxel"]
        .into_iter()
        .filter_map(Project::from_source)
        .collect()
}

impl Shot {
    /// Whether this snapshot shows the folder picker's window instead of the main one.
    fn in_picker_window(self) -> bool {
        matches!(self, Self::Folders)
    }

    /// Whether this snapshot shows the settings window instead of the main one.
    fn in_settings_window(self) -> bool {
        matches!(
            self,
            Self::SettingsPage(..)
                | Self::SettingsSearch(_)
                | Self::SettingsPicker(_)
                | Self::SettingsMachine(_)
                | Self::SettingsCompact(_)
                | Self::SettingsMcp
                | Self::SettingsForm
                | Self::SettingsAccounts
        )
    }

    /// Whether this snapshot shows the settings window at its smallest.
    fn narrow(self) -> bool {
        matches!(
            self,
            Self::SettingsPage(_, true)
                | Self::SettingsSearch(true)
                | Self::SettingsPicker(true)
                | Self::SettingsMachine(true)
                | Self::SettingsCompact(true)
        )
    }

    /// The settings shot `name` names: `settings` for the first section, `settings-` and a section's name for another, `settings-search`, `settings-picker`, `settings-machine` or `settings-compact`, each with `-narrow` after it for the window at its smallest.
    pub fn settings_named(name: &str) -> Option<Self> {
        let rest = name.strip_prefix("settings")?;
        let (rest, narrow) = match rest.strip_suffix("-narrow") {
            Some(rest) => (rest, true),
            None => (rest, false),
        };
        let rest = rest.strip_prefix('-').unwrap_or(rest);
        match rest {
            "" => Some(Self::SettingsPage(Section::General, narrow)),
            "search" => Some(Self::SettingsSearch(narrow)),
            "picker" => Some(Self::SettingsPicker(narrow)),
            "machine" => Some(Self::SettingsMachine(narrow)),
            "compact" => Some(Self::SettingsCompact(narrow)),
            "form" => Some(Self::SettingsForm),
            "accounts" => Some(Self::SettingsAccounts),
            "mcp" => Some(Self::SettingsMcp),
            slug => SETTINGS_SHOTS
                .iter()
                .find(|(held, _)| *held == slug)
                .map(|(_, section)| Self::SettingsPage(*section, narrow)),
        }
    }

    /// The stretch of selectable text this snapshot shows selected, if any.
    fn selection(self) -> Option<TextSelection> {
        matches!(self, Self::Selecting | Self::MessageMenu).then_some(TextSelection {
            id: CHAT_TEXT,
            anchor: TextPoint { piece: 27, byte: 3 },
            head: TextPoint { piece: 38, byte: 2 },
        })
    }

    /// The screen this snapshot is of.
    fn screen(self) -> Screen {
        let workspace = workspace_sample;
        match self {
            Self::Code => Screen::Workspace(code()),
            Self::LanguageOffer => Screen::Workspace(language_offer()),
            Self::MarkdownPreview => Screen::Workspace(previewed("README.md")),
            Self::VectorPreview => Screen::Workspace(previewed("assets/logo.svg")),
            Self::PicturePreview => Screen::Workspace(previewed("assets/screenshot.png")),
            Self::SoftWrap => {
                let editor = pm_editor::EditorSettings {
                    soft_wrap: pm_editor::SoftWrap::LineLength,
                    preferred_line_length: 36,
                    wrap_guide: true,
                    cursor_shape: pm_editor::CursorShape::Block,
                    cursor_blink: false,
                    ..pm_editor::EditorSettings::default()
                };
                Screen::Workspace(
                    code()
                        .with_editor_settings(editor)
                        .with_code_view(560.0, 7.2)
                        .edited(EDITOR_FIELD, Edit::Place(600, false)),
                )
            }
            Self::Completing => Screen::Workspace(completing()),
            Self::Hovering => Screen::Workspace(hovering()),
            Self::Debugging => Screen::Workspace(debugging()),
            Self::Problems => Screen::Workspace(problems()),
            Self::SidebarHidden => {
                let mut state = code();
                state.sidebar_open = false;
                Screen::Workspace(state)
            }
            Self::EditorMenu => Screen::Workspace(code().with_menu(EDITOR_FIELD, (420.0, 200.0))),
            Self::References => Screen::Workspace(references()),
            Self::Renaming => {
                let state = code();
                let caret = state
                    .shown_document()
                    .map_or(0, |document| document.field.caret());
                Screen::Workspace(state.rename_requested(caret.saturating_sub(12), (330.0, 470.0)))
            }
            Self::CodeActions => Screen::Workspace(code_actions()),
            Self::FileHistory => Screen::Workspace(file_history()),
            Self::Timeline => Screen::Workspace(timeline()),
            Self::Conflict => Screen::Workspace(conflict()),
            Self::DiffSubmodule => Screen::Workspace(diff_submodule()),
            Self::BranchesSubmodule(inside) => Screen::Workspace(branches_submodule(inside)),
            Self::Signing => Screen::Workspace(signing()),
            Self::Annotated => Screen::Workspace(annotated()),
            Self::ProjectSearch => Screen::Workspace(project_search()),
            Self::Excerpts => Screen::Workspace(excerpts()),
            Self::TabMenu => Screen::Workspace(code().with_tab_menu(0, (500.0, 52.0))),
            Self::Finder => {
                let mut state = code();
                state.files = [
                    "Cargo.toml",
                    "README.md",
                    "crates/pandemonium/src/main.rs",
                    "crates/pandemonium/src/session.rs",
                    "crates/pandemonium/src/snapshot.rs",
                    "crates/pm-server/src/sessions.rs",
                    "crates/pm-protocol/src/response.rs",
                    "docs/protocol.md",
                ]
                .map(str::to_owned)
                .to_vec();
                Screen::Workspace(state.file_finder_requested((340.0, 0.0)).edited(
                    pm_components::listing::LISTING_FILTER,
                    Edit::Paste("ses".to_owned()),
                ))
            }
            Self::QuickHelp => Screen::Workspace(code().file_finder_requested((340.0, 0.0)).edited(
                pm_components::listing::LISTING_FILTER,
                Edit::Paste("?".to_owned()),
            )),
            Self::Themes => Screen::Workspace(code().choices_requested(
                pm_components::quick_choices::Choosing::Theme,
                (340.0, 0.0),
            )),
            Self::GoLine => Screen::Workspace(code().file_finder_requested((340.0, 0.0)).edited(
                pm_components::listing::LISTING_FILTER,
                Edit::Paste(":12".to_owned()),
            )),
            Self::Tasks => Screen::Workspace(
                code()
                    .with_tasks(0, sample_tasks(), None)
                    .commands_requested((340.0, 0.0))
                    .edited(
                        pm_components::listing::LISTING_FILTER,
                        Edit::Paste("task".to_owned()),
                    ),
            ),
            Self::RunButtons => Screen::Workspace(runnable()),
            Self::Toolchains => Screen::Workspace(
                code()
                    .toolchains_requested((340.0, 0.0))
                    .with_toolchains(0, sample_toolchains()),
            ),
            Self::Sessions => Screen::Workspace(code().sessions_requested((340.0, 0.0))),
            Self::Commands => Screen::Workspace(
                code()
                    .command_used(pm_editor::Action::FileFinder)
                    .command_used(pm_editor::Action::GoToLine)
                    .commands_requested((340.0, 0.0))
                    .edited(
                        pm_components::listing::LISTING_FILTER,
                        Edit::Paste("go".to_owned()),
                    ),
            ),
            Self::Folded => Screen::Workspace(
                code().lines_edited(pm_components::line_edits::LineEdit::Fold),
            ),
            Self::Sticky => Screen::Workspace(code().scrolled_viewer(360.0, 10_000.0)),
            Self::Cursors => {
                let state = code();
                let at = state
                    .shown_document()
                    .and_then(|document| document.field.text().find("names"))
                    .unwrap_or_default();
                Screen::Workspace(
                    state
                        .edited(EDITOR_FIELD, Edit::Place(at + 2, false))
                        .lines_edited(pm_components::line_edits::LineEdit::SelectAllOccurrences),
                )
            }
            Self::Outline => Screen::Workspace(
                code()
                    .outline_requested((340.0, 0.0))
                    .edited(pm_components::listing::LISTING_FILTER, Edit::Paste("n".to_owned())),
            ),
            Self::Finding => Screen::Workspace(
                code()
                    .edited(EDITOR_FIELD, Edit::Place(0, false))
                    .find_opened(true)
                    .find_typed(Edit::Paste("names".to_owned()))
                    .find_stepped(false),
            ),
            Self::FindReopened => Screen::Workspace(
                code()
                    .find_opened(false)
                    .find_typed(Edit::Paste("names".to_owned()))
                    .find_opened(false),
            ),
            Self::Extensions => Screen::Workspace(
                welcome()
                    .after(pm_components::extensions::EXTENSIONS)
                    .with_extensions(sample_languages()),
            ),
            Self::ExtensionInstall => Screen::Workspace(
                welcome()
                    .after(pm_components::extensions::EXTENSIONS)
                    .with_extensions(sample_languages())
                    .with_extension_preview(
                        "~/code/pandemonium/extensions/hello".to_owned(),
                        sample_extension(false),
                    ),
            ),
            Self::ComposerLong => {
                let prompt = (1..=40).map(|line| format!("Prompt line {line}\n")).collect();
                Screen::Workspace(
                    workspace()
                        .edited(PROMPT, Edit::Paste(prompt))
                        .edited(PROMPT, Edit::ScrollTo(220.0)),
                )
            }
            Self::SignedOut => {
                let mut state = workspace();
                state.sessions[0].login = Some("a1".to_owned());
                Screen::Workspace(state.heard(0, Event::SignedOut {
                    reason: "Failed to authenticate: OAuth session expired and could not be refreshed".to_owned(),
                    account: Some("work@example.com".to_owned()),
                }))
            }
            Self::SessionStates(unread) => {
                let mut state = workspace();
                for session in &mut state.sessions {
                    session.project = 0;
                    session.status = pm_components::sample::Status::Idle;
                    session.mark_read();
                }
                state.sessions[0].title = "Idle session".into();
                state.sessions[1].title = "Agent working".into();
                state.sessions[1].status = pm_components::sample::Status::Working;
                state.sessions[2].title = "Unread response".into();
                state.sessions[2].marked_unread = true;
                state.sessions[3].title = "Unread error".into();
                state = state.heard(3, Event::Failed("Connection lost".into()));
                state.sessions[4].title = "Needs input".into();
                state = state.heard(4, Event::Ask(Permission {
                    ticket: 1, title: "Allow command?".into(), command: None, plan: None, choices: Vec::new(),
                }));
                let selected = if unread { 3 } else { 0 };
                state = state.after(FocusId(pm_components::state::SESSION_BASE + selected as u64));
                Screen::Workspace(state.with_session_menu(selected, (220.0, 160.0)))
            }
            Self::Workspace | Self::Selecting => Screen::Workspace(workspace()),
            Self::SessionAccount => {
                let mut state = workspace();
                let agent = state.agent_id_at(state.sessions[0].agent).unwrap_or_default();
                state.sessions[0].login = Some("a1".to_owned());
                Screen::Workspace(state.with_accounts(
                    vec![(agent, "a1".to_owned(), "work@example.com".to_owned())],
                    Vec::new(),
                ))
            }
            Self::Quitting => Screen::Workspace(workspace().close_asked()),
            Self::Update => Screen::Workspace(workspace().with_release(
                pm_components::update::Update::Newer {
                name: "Pandemonium 0.14.0".to_owned(),
                url: "https://github.com".to_owned(),
            },
            )),
            Self::MessageMenu => {
                Screen::Workspace(workspace().with_message_menu(3, (520.0, 230.0), true))
            }
            Self::Unfolded => Screen::Workspace(workspace().after(FocusId(DETAILS_BASE + 3000))),
            Self::Diff => {
                load_installed_grammar("rust");
                Screen::Workspace(workspace().after(TAB_DIFF))
            }
            Self::WordDiff => {
                load_installed_grammar("rust");
                Screen::Workspace(word_diff())
            }
            Self::Commenting => Screen::Workspace(commenting()),
            Self::Files => Screen::Workspace(files()),
            Self::FilesLarge => Screen::Workspace(large_files(false)),
            Self::FilesScrolled => Screen::Workspace(large_files(true)),
            Self::InputStress => Screen::Workspace(input_stress(true, true)),
            Self::InputStressScrolled => {
                let mut state = input_stress(true, true);
                state.prompt.apply(pm_text::Edit::ScrollTo(700.0));
                Screen::Workspace(state)
            }
            Self::InputStressComposing => {
                let mut state = input_stress(true, true);
                state.prompt.apply(pm_text::Edit::Compose("字".to_owned()));
                Screen::Workspace(state)
            }
            Self::IgnoredFiles => Screen::Workspace(ignored_files()),
            Self::Source => Screen::Workspace(source()),
            Self::SourceMenu => {
                Screen::Workspace(source().with_git_menu(MenuKind::Source, (700.0, 110.0)))
            }
            Self::HistoryMenu => {
                let mut state = source();
                state.git.graph_all = true;
                Screen::Workspace(state.with_git_menu(MenuKind::History(2), (1000.0, 600.0)))
            }
            Self::Stashing => Screen::Workspace(source().asked(Prompt::StashMessage)),
            Self::Refused => Screen::Workspace(
                source().git_failed("Could not switch branch: fatal: a branch named 'main' already exists"),
            ),
            Self::ChangeMenu => Screen::Workspace(picked().with_change_menu(0, (1000.0, 250.0))),
            Self::TreeMenu => Screen::Workspace(files().with_tree_menu(
                TreeTarget {
                    path: "crates/pandemonium/src/session.rs".to_owned(),
                    folder: false,
                },
                (1000.0, 250.0),
            )),
            Self::TreePicked => {
                let file = |path: &str| TreeTarget {
                    path: path.to_owned(),
                    folder: false,
                };
                Screen::Workspace(
                    files()
                        .tree_row_picked(file(".env"), Choosing::Toggle)
                        .tree_row_picked(file("Cargo.toml"), Choosing::Range)
                        .tree_row_picked(file("README.md"), Choosing::Toggle)
                        .with_tree_menu(file("README.md"), (1000.0, 250.0)),
                )
            }
            Self::Naming => Screen::Workspace(files().after(TREE_NEW_FOLDER)),
            Self::Discarding => Screen::Workspace(picked().asked(Prompt::Discard(vec![0, 2]))),
            Self::Branching => Screen::Workspace(branching()),
            Self::Reading => Screen::Workspace(reading()),
            Self::Editing => Screen::Workspace(editing()),
            Self::NewFile => Screen::Workspace(new_file()),
            Self::Terminal => Screen::Workspace(terminal()),
            Self::TerminalLink => {
                let mut state = terminal();
                state.terminal_output(
                    1,
                    b"gh pr view --web\r\nOpening https://github.com/alster/pandemonium/pull/250 or \x1b]8;;https://docs.rs/vte\x1b\\the vte docs\x1b]8;;\x1b\\\r\n",
                );
                state.reach_terminal_link(Some((9, 20)));
                Screen::Workspace(state)
            }
            Self::Overview => Screen::Workspace(overview()),
            Self::Submodules => Screen::Workspace(submodules()),
            Self::OverviewDiff => {
                let mut state = overview();
                let path = state.review_session().unwrap().changes[0].path.clone();
                state.closed_diffs.insert(path);
                Screen::Workspace(state.after(FocusId(pm_components::state::FILE_BASE)))
            }
            Self::OverviewChangeMenu => {
                Screen::Workspace(overview().with_change_menu(0, (1000.0, 250.0)))
            }
            Self::Resized => {
                let mut state = terminal();
                state.sidebar_width = 360.0;
                state.review_width = 300.0;
                state.terminal_height = 400.0;
                Screen::Workspace(state)
            }
            Self::TerminalMenu => {
                let mut state = terminal();
                state.select_in_terminal((1, 0), (2, 20));
                Screen::Workspace(state.with_terminal_menu((420.0, 560.0)))
            }
            Self::TerminalComposing => {
                let mut state = terminal();
                state.compose_in_terminal("¨");
                Screen::Workspace(state)
            }
            Self::Composing => Screen::Workspace(
                welcome()
                    .edited(PROMPT, Edit::Paste("Varför försvinner sm".to_owned()))
                    .edited(PROMPT, Edit::Compose("¨".to_owned())),
            ),
            Self::New => Screen::Workspace(WorkspaceState::new(&projects())),
            Self::Empty => Screen::default(),
            Self::History | Self::HistoryScrolled => {
                let mut state = workspace().after(HISTORY);
                let template = state.sessions[0].clone();
                let now = pm_components::age::now();
                for index in 0..36 {
                    let mut session = template.clone();
                    session.home = false;
                    session.title = format!("History session {}", index + 1);
                    session.updated_at = now.saturating_sub(index * 6 * 60 * 60 * 1000);
                    state.sessions.push(session);
                }
                if self == Self::HistoryScrolled {
                    state.history_scroll = f32::MAX;
                }
                Screen::Workspace(state)
            }
            Self::ProjectMenu => {
                Screen::Workspace(workspace().with_project_menu(1, (236.0, 250.0)))
            }
            Self::Dragging => Screen::Workspace(workspace().dragged_over(0, 2, (150.0, 470.0))),
            Self::Welcome => Screen::Workspace(welcome()),
            Self::WelcomeTerminal => Screen::Workspace(with_shell(welcome())),
            Self::Panes => Screen::Workspace(
                code()
                    .pane_dragged(code_item(), Some((0, Side::Right)), (0.0, 0.0))
                    .pane_dropped()
                    .pane_dragged(Item::Diff, Some((0, Side::Bottom)), (0.0, 0.0))
                    .pane_dropped()
                    .after(TAB_CHAT),
            ),
            Self::PaneSplit => Screen::Workspace(code().after(SPLIT_RIGHT)),
            Self::PaneSplitFile => {
                let mut state = code().after(SPLIT_RIGHT);
                state.open_viewer("README.md".to_owned(), false);
                Screen::Workspace(state)
            }
            Self::PaneMenu => Screen::Workspace(code().with_pane_menu((690.0, 44.0))),
            Self::PanesDragging => Screen::Workspace(code().pane_dragged(
                code_item(),
                Some((0, Side::Right)),
                (900.0, 420.0),
            )),
            Self::Wrapped => Screen::Workspace(welcome().edited(
                PROMPT,
                Edit::Paste(
                    "Why does the sidebar lose its scroll position whenever a project is added, and what would it take to keep it where the reader left it? Start with the state that stores the offset.\nThen write the fix.".to_owned(),
                ),
            )),
            Self::Slashed => Screen::Workspace(
                welcome()
                    .edited(PROMPT, Edit::Paste("/c".to_owned()))
                    .after(FocusId(4401)),
            ),
            Self::Dollar | Self::DollarSession => {
                let mut state = if matches!(self, Self::DollarSession) {
                    let mut state = WorkspaceState::sample();
                    state.sessions[0].agent = 1;
                    state
                } else {
                    welcome().after(FocusId(AGENT_BASE + 1))
                };
                state.sync_context();
                state = state.with_skills(1, 0, vec![
                    Slash { name: "review".to_owned(), description: "Review the changes in this project".to_owned() },
                    Slash { name: "release-notes".to_owned(), description: "Write release notes from recent commits".to_owned() },
                ], Default::default());
                Screen::Workspace(state.after(pm_components::state::SKILL_BUTTON))
            }
            Self::Scrolling => {
                Screen::Workspace(welcome().after(FocusId(4200)).scrolled_menu(1000.0, 1000.0))
            }
            Self::Placing => {
                let here = std::env::current_dir().unwrap_or_default();
                let projects: Vec<Project> = Project::from_source(&here.to_string_lossy())
                    .into_iter()
                    .collect();
                Screen::Workspace(
                    WorkspaceState::new(&projects)
                        .with_discovery(0, claude_discovery())
                        .after(FocusId(4250)),
                )
            }
            Self::Live => Screen::Workspace(live(Turn::Finished)),
            Self::Asking => Screen::Workspace(live(Turn::Asking)),
            Self::AskingCommand => Screen::Workspace(live(Turn::Command)),
            Self::Answered => Screen::Workspace(live(Turn::Working).heard(
                0,
                Event::Answered(vec![
                    (
                        "If a duck wore pants, how would it wear them?".to_owned(),
                        Some("Something else".to_owned()),
                    ),
                    ("Which would win in a fight?".to_owned(), None),
                ]),
            )),
            Self::Inquiring => Screen::Workspace(live(Turn::Inquiring)),
            Self::Hooks => Screen::Workspace(hooks_held()),
            Self::HooksReview => Screen::Workspace(hooks_held().after(HOOKS_REVIEW)),
            Self::Background => Screen::Workspace(live(Turn::Background)),
            Self::Agents => {
                Screen::Workspace(live(Turn::Background).after(FocusId(SUBAGENTS_CHIP.0)))
            }
            Self::AgentsExpanded | Self::AgentsScrolled => {
                let mut state = live(Turn::Background);
                let session = state.open_index().unwrap();
                for index in 0..40 {
                    state = state.heard(session, Event::Tool(Tool {
                        parent: Some("a1".to_owned()),
                        id: format!("many-{index}"),
                        title: Some("Grep".to_owned()),
                        kind: Some(Kind::Search),
                        state: Some(ToolState::Done),
                        target: Some(format!("caller_{index}")),
                        diff: None,
                        output: None,
                        agent: None,
                    }));
                }
                let (reply, position, _) = state.sessions[session].subagents().into_iter()
                    .find(|(_, _, tool)| tool.id == "a1").unwrap();
                state = state.after(SUBAGENTS_CHIP).after(FocusId(pm_components::state::STEP_BASE + (reply * 1000 + position) as u64));
                if matches!(self, Self::AgentsScrolled) {
                    state.subagents_scroll = 10000.0;
                }
                Screen::Workspace(state)
            }
            Self::Usage => Screen::Workspace(
                live(Turn::Finished)
                    .heard(
                        0,
                        Event::Used(pm_acp::Usage {
                            used: 84_000,
                            size: 200_000,
                        }),
                    )
            ),
            Self::Queued => Screen::Workspace(queued()),
            Self::Kept => Screen::Workspace(kept()),
            Self::Restoring => Screen::Workspace(kept().after(FocusId(RESTORE_BASE))),
            Self::Picking => Screen::Workspace(welcome().after(FocusId(4200))),
            Self::Expanded => {
                load_installed_grammar("rust");
                Screen::Workspace(
                    [201_000, 201_004, 201_005]
                        .into_iter()
                        .fold(live(Turn::Finished), |state, id| state.after(FocusId(id))),
                )
            }
            Self::Attached => Screen::Workspace(
                welcome()
                    .with_attachments(&["/home/me/shot.png".to_owned(), "/home/me/notes.md".to_owned()])
                    .with_preview("/home/me/shot.png", gradient()),
            ),
            Self::Missing => {
                Screen::Workspace(welcome().edited(PROMPT, Edit::Paste("@zzzzqq".to_owned())))
            }
            Self::SettingsPage(section, _) => {
                let mut state = settings_sample().after(section.id());
                let mut window = state.window.clone();
                window.general.embedded_browser = true;
                state = state.with_window_settings(window);
                state.set_browser_status(Some(
                    "Downloading the embedded browser: 42 of about 110 MB.".to_owned(),
                ));
                Screen::Workspace(state)
            }
            Self::SettingsSearch(_) => Screen::Workspace(
                settings_sample().edited(SETTINGS_FIND, Edit::Paste("font".to_owned())),
            ),
            Self::SettingsPicker(narrow) => Screen::Workspace(
                settings_sample()
                    .after(Section::Appearance.id())
                    .after(SettingsCatalogue::CodeFont.id(OPEN_LIST))
                    .scrolled_settings(if narrow { 560.0 } else { 0.0 }, 2000.0),
            ),
            Self::SettingsCompact(_) => {
                let state = settings_sample();
                let mut window = state.window.clone();
                window.appearance.density = pm_config::Density::Compact;
                window.appearance.interface_size = 15;
                Screen::Workspace(
                    state
                        .with_window_settings(window)
                        .after(Section::Appearance.id()),
                )
            }
            Self::SettingsMachine(narrow) => Screen::Workspace(
                settings_sample()
                    .after(Section::Servers.id())
                    .after(MACHINE_ADD)
                    .edited(MACHINE_NAME, Edit::Paste("Lab".to_owned()))
                    .edited(MACHINE_ADDRESS, Edit::Paste("100.64.0.9:6868".to_owned()))
                    .scrolled_settings(if narrow { 300.0 } else { 0.0 }, 2000.0),
            ),
            Self::SettingsMcp => Screen::Workspace(
                welcome()
                    .after(SETTINGS)
                    .after(Section::McpServers.id())
                    .with_mcp_servers(vec![pm_protocol::McpServer {
                        name: "context7".to_owned(),
                        target: "https://mcp.context7.com/mcp".to_owned(),
                        env: vec![("API_KEY".to_owned(), "abc".to_owned())],
                        description: "Up-to-date code docs for any prompt".to_owned(),
                        website: "https://context7.com".to_owned(),
                        enabled: true,
                        extension: None,
                    }])
                    .with_tool_listings(
                        "",
                        Ok(vec![
                            tool_listing("Context7", "context7", "https://mcp.context7.com/mcp"),
                            tool_listing("GitHub", "github", "https://api.githubcopilot.com/mcp/"),
                        ]),
                    )
                    .with_settings_menu(0, (711.0, 357.0)),
            ),
            Self::SettingsForm => Screen::Workspace(
                welcome()
                    .after(SETTINGS)
                    .after(Section::AgentServers.id())
                    .after(SETTINGS_ADD),
            ),
            Self::SettingsAccounts => {
                let agent = welcome().agent_id_at(0).unwrap_or_default();
                Screen::Workspace(
                    welcome()
                        .after(SETTINGS)
                        .after(Section::AgentServers.id())
                        .with_accounts(
                            vec![
                                (agent.clone(), "a1".to_owned(), "work@example.com".to_owned()),
                                (agent.clone(), "a2".to_owned(), "Account 2".to_owned()),
                            ],
                            vec![(agent, "a1".to_owned())],
                        ),
                )
            }
            Self::Adding => Screen::Workspace(welcome().after(ADD_PROJECT)),
            Self::Loading => Screen::Workspace(welcome().after(ADD_PROJECT).after(ADDER_CLONE)),
            Self::Owning => Screen::Workspace(
                welcome()
                    .after(ADD_PROJECT)
                    .after(ADDER_CLONE)
                    .with_owners(Ok(owners())),
            ),
            Self::Folders => Screen::Workspace(
                welcome()
                    .after(ADD_PROJECT)
                    .after(ADDER_BROWSE)
                    .with_folders("~", sample_folders()),
            ),
            Self::Cloning => Screen::Workspace(
                welcome()
                    .after(ADD_PROJECT)
                    .after(ADDER_CLONE)
                    .with_owners(Ok(owners()))
                    .after(FocusId(ADDER_ROW_BASE))
                    .with_repositories("JoakimCarlsson", Ok(repositories())),
            ),
            Self::Slashing => {
                Screen::Workspace(welcome().edited(PROMPT, Edit::Paste("/c".to_owned())))
            }
        }
    }
}

/// The folders the server would list for the home folder.
fn sample_folders() -> Result<(String, Vec<String>), String> {
    Ok((
        "/home/me/code".to_owned(),
        ["pandemonium", "voxel", "wasa-cli", "world"]
            .map(str::to_owned)
            .to_vec(),
    ))
}

/// Accounts and organizations as the server would list them for the account.
fn owners() -> Vec<Found> {
    [
        ("JoakimCarlsson", "Personal repositories", false),
        ("alsterverse", "Organization", true),
    ]
    .into_iter()
    .map(|(name, detail, organization)| Found {
        name: name.to_owned(),
        detail: detail.to_owned(),
        value: name.to_owned(),
        kind: Row::Owner(organization),
    })
    .collect()
}

/// Repositories as the server would list them for the account.
fn repositories() -> Vec<Found> {
    [
        (
            "JoakimCarlsson/pandemonium",
            "Agent-first desktop workspace",
        ),
        ("JoakimCarlsson/wasa-cli", "Command line client"),
        ("JoakimCarlsson/voxel", "Voxel engine experiments"),
        ("JoakimCarlsson/world", "Procedural world generator"),
        ("JoakimCarlsson/dotfiles", "Shell and editor configuration"),
    ]
    .into_iter()
    .map(|(name, detail)| Found {
        name: name.to_owned(),
        detail: detail.to_owned(),
        value: format!("git@github.com:{name}.git"),
        kind: Row::Repository,
    })
    .collect()
}

/// The page for starting work in this repository, once Claude Code has said what it offers.
/// The workspace with the settings window open, knowing the languages, fonts and servers a machine has, and with some settings changed from how they come.
fn settings_sample() -> WorkspaceState {
    let mut machine = pm_protocol::MachineSettings::default();
    machine.git.branch_template = "pm/{title}".to_owned();
    machine.set_language(
        "go",
        pm_protocol::LanguagePreferences {
            tab_size: Some(4),
            ..pm_protocol::LanguagePreferences::default()
        },
    );
    let mut window = pm_config::WindowSettings::default();
    window.appearance.theme_mode = pm_config::ThemeMode::Dark;
    window.servers.known = vec![
        pm_config::KnownServer {
            name: "Studio".to_owned(),
            reach: pm_config::Reach::Ssh,
            address: "me@studio.local".to_owned(),
            token: String::new(),
        },
        pm_config::KnownServer {
            name: "Build box".to_owned(),
            reach: pm_config::Reach::Direct,
            address: "100.64.0.7:6868".to_owned(),
            token: "secret".to_owned(),
        },
        pm_config::KnownServer {
            name: "Ubuntu".to_owned(),
            reach: pm_config::Reach::Wsl,
            address: "Ubuntu".to_owned(),
            token: String::new(),
        },
        pm_config::KnownServer {
            name: "App container".to_owned(),
            reach: pm_config::Reach::Container,
            address: "/home/me/app".to_owned(),
            token: String::new(),
        },
        pm_config::KnownServer {
            name: "Home server".to_owned(),
            reach: pm_config::Reach::Tunnel,
            address: "home.example.com".to_owned(),
            token: "secret".to_owned(),
        },
    ];
    let fonts = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect();
    let state = welcome()
        .with_extensions(sample_languages())
        .with_fonts(
            fonts(&[
                "DejaVu Sans",
                "DejaVu Sans Mono",
                "Fira Code",
                "Inter",
                "JetBrains Mono",
                "Noto Sans",
                "Noto Sans Mono",
                "Source Code Pro",
                "Ubuntu",
                "Ubuntu Mono",
                "Hack",
                "IBM Plex Mono",
                "Iosevka",
                "Cascadia Code",
            ]),
            fonts(&[
                "DejaVu Sans Mono",
                "Fira Code",
                "JetBrains Mono",
                "Noto Sans Mono",
                "Source Code Pro",
                "Ubuntu Mono",
                "Hack",
                "IBM Plex Mono",
                "Iosevka",
                "Cascadia Code",
            ]),
        )
        .with_window_settings(window)
        .with_machine_settings(machine)
        .with_release(pm_components::update::Update::Newer {
            name: "Pandemonium 0.14.0".to_owned(),
            url: "https://github.com".to_owned(),
        })
        .after(SETTINGS);
    with_library_sample(state)
}

/// `state` with what Claude Code is given in the project read, a skill and two tools of it turned off, and the GitHub tool server's set unfolded.
fn with_library_sample(state: WorkspaceState) -> WorkspaceState {
    use pm_protocol::library::{Item, Kind, Product, Scope, Source, ToolEntry, ToolSet};
    let here = std::env::current_dir().unwrap_or_default();
    let inside = |path: &str| here.join(path).to_string_lossy().into_owned();
    let item = |kind, scope, name: &str, description: &str, path: String| Item {
        kind,
        scope,
        name: name.to_owned(),
        description: description.to_owned(),
        path,
        ..Item::default()
    };
    let items = vec![
        item(Kind::Skill, Scope::Workspace, "release-notes", "Writes the notes of a release from the commits since the last tag", inside(".claude/skills/release-notes/SKILL.md")),
        item(Kind::Skill, Scope::User, "commit", "Commit the changes made in this session on the current branch", "/home/me/.claude/skills/commit/SKILL.md".to_owned()),
        item(Kind::Skill, Scope::BuiltIn, "frontend-design:frontend-design", "Guidance for distinctive, intentional visual design", "/home/me/.claude/plugins/cache/official/frontend-design/skills/frontend-design/SKILL.md".to_owned()),
        Item {
            applies: vec!["the whole project".to_owned()],
            ..item(Kind::Instruction, Scope::Workspace, "CLAUDE.md", "", inside("CLAUDE.md"))
        },
        Item {
            applies: vec!["crates/pm-ui/".to_owned()],
            ..item(Kind::Instruction, Scope::Workspace, "crates/pm-ui/CLAUDE.md", "", inside("crates/pm-ui/CLAUDE.md"))
        },
        Item {
            applies: vec!["every project".to_owned()],
            ..item(Kind::Instruction, Scope::User, "~/.claude/CLAUDE.md", "", "/home/me/.claude/CLAUDE.md".to_owned())
        },
        Item {
            model: "sonnet".to_owned(),
            tools: vec!["Read".to_owned(), "Grep".to_owned(), "Glob".to_owned()],
            ..item(Kind::Agent, Scope::Workspace, "reviewer", "Reviews a change for bugs before it is committed", inside(".claude/agents/reviewer.md"))
        },
        item(Kind::Agent, Scope::User, "researcher", "Finds out how a library is used across the code", "/home/me/.claude/agents/researcher.md".to_owned()),
        Item {
            command: "~/.claude/hooks/guard.sh".to_owned(),
            ..item(Kind::Hook, Scope::User, "PreToolUse", "Bash", "/home/me/.claude/settings.json".to_owned())
        },
        Item {
            command: "make fmt".to_owned(),
            ..item(Kind::Hook, Scope::Workspace, "Stop", "", inside(".claude/settings.json"))
        },
    ];
    let tool = |name: &str, description: &str| ToolEntry {
        name: name.to_owned(),
        description: description.to_owned(),
    };
    let sets = ToolSet::built_in(Some(Product::Claude))
        .into_iter()
        .chain([
            ToolSet {
                id: "github".to_owned(),
                name: "github".to_owned(),
                source: Source::Server,
                plugin: None,
                tools: vec![
                    tool("create_issue", "Open a new issue in a repository"),
                    tool("get_issue", "Read an issue and its comments"),
                    tool(
                        "list_pull_requests",
                        "List the pull requests of a repository",
                    ),
                    tool("search_code", "Search code across repositories"),
                    tool("add_issue_comment", "Comment on an issue"),
                ],
                error: None,
            },
            ToolSet {
                id: "context7".to_owned(),
                name: "context7".to_owned(),
                source: Source::Plugin,
                plugin: Some("docs".to_owned()),
                tools: vec![
                    tool("resolve-library-id", "Find the id of a library"),
                    tool("get-library-docs", "Read the documentation of a library"),
                ],
                error: None,
            },
        ])
        .collect();
    let withheld = vec![
        "skill/user/commit".to_owned(),
        "tool/github/create_issue".to_owned(),
        "tool/github/add_issue_comment".to_owned(),
        "tool/built-in/WebSearch".to_owned(),
    ];
    let agent = state.agent;
    let mut state = state
        .with_library(0, agent, Some(Product::Claude), items, withheld.clone())
        .with_tool_sets(0, agent, Some(Product::Claude), sets, withheld);
    state.settings_page.library.open_sets = vec!["github".to_owned()];
    state
}

fn welcome() -> WorkspaceState {
    let here = std::env::current_dir().unwrap_or_default();
    let projects: Vec<Project> = Project::from_source(&here.to_string_lossy())
        .into_iter()
        .collect();
    WorkspaceState::new(&projects).with_discovery(0, claude_discovery())
}

/// What Claude Code says about itself: its settings and commands.
fn claude_discovery() -> Discovery {
    Discovery {
        settings: claude_settings(),
        commands: ["compact", "clear", "model", "review", "init"]
            .into_iter()
            .map(|name| Slash {
                name: name.to_owned(),
                description: format!("Run {name}"),
            })
            .collect(),
    }
}

/// What Claude Code lets the reader change, as it reports it.
fn claude_settings() -> Vec<Setting> {
    let pick = |value: &str, name: &str, description: &str| Pick {
        value: value.to_owned(),
        name: name.to_owned(),
        description: description.to_owned(),
    };
    let setting = |id: &str, category: &str, name: &str, current: &str, picks| Setting {
        id: id.to_owned(),
        category: category.to_owned(),
        name: name.to_owned(),
        current: current.to_owned(),
        picks,
    };
    vec![
        setting(
            "mode",
            "mode",
            "Mode",
            "default",
            vec![
                pick("default", "Manual", "Always ask before making changes"),
                pick("plan", "Plan", "Create a plan before making changes"),
            ],
        ),
        setting(
            "model",
            "model",
            "Model",
            "default",
            vec![
                pick("default", "Default (recommended)", "Sonnet 5.5"),
                pick("opus", "Opus 5.5", "For complex work and everyday tasks"),
                pick("haiku", "Haiku 5.5", "Fastest for quick answers"),
                pick("a", "Haiku 4.5", "Fastest for quick answers"),
                pick("b", "Sonnet 5", "Efficient for routine tasks"),
                pick("c", "Opus 5", "Best for everyday, complex tasks"),
                pick("d", "Fable 5", "Most capable for your hardest tasks"),
                pick("e", "Opus 4.8", "Best for everyday, complex tasks"),
                pick("f", "Opus 4.7", "Best for everyday, complex tasks"),
                pick("g", "Sonnet 4.6", "Efficient for routine tasks"),
            ],
        ),
        setting(
            "effort",
            "thought_level",
            "Effort",
            "default",
            vec![
                pick("default", "Default", ""),
                pick("low", "Low", ""),
                pick("high", "High", ""),
            ],
        ),
    ]
}

/// How the turn of a made-up conversation stands at the end of it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Turn {
    /// The agent finished it.
    Finished,
    /// The agent waits for permission.
    Asking,
    /// The agent waits for permission to run a long command.
    Command,
    /// The agent waits for the answers to a form of questions.
    Inquiring,
    /// The agent is still on it.
    Working,
    /// The agent finished it with subagents and a command left running.
    Background,
}

/// A conversation on a turn, with two prompts waiting behind it and a third being written.
fn queued() -> WorkspaceState {
    let waiting = |id: u64, text: &str, files: &[&str]| Event::Queued {
        id,
        text: text.to_owned(),
        files: files.iter().map(|name| (*name).to_owned()).collect(),
    };
    let state = live(Turn::Working);
    let session = state.sessions.len() - 1;
    [
        waiting(1, "Then add a test for the clamp", &[]),
        waiting(
            2,
            "Use the layout in this screenshot for the empty state\nand keep the icon",
            &["empty.png"],
        ),
    ]
    .into_iter()
    .fold(state, |state, event| state.heard(session, event))
    .edited(
        PROMPT,
        Edit::Paste("Stop, the clamp lives in pm-ui".to_owned()),
    )
}

/// A finished conversation whose Codex holds back a new hook and a changed one until the reader trusts them.
fn hooks_held() -> WorkspaceState {
    let hook = |key: &str, event: &str, matcher: Option<&str>, command: &str, changed| Hook {
        key: format!("/home/reader/.codex/hooks.json:{key}"),
        event: event.to_owned(),
        matcher: matcher.map(str::to_owned),
        command: command.to_owned(),
        source: "/home/reader/.codex/hooks.json".to_owned(),
        hash: format!("sha256:{key}"),
        changed,
    };
    let state = live(Turn::Finished);
    let session = state.sessions.len() - 1;
    state.heard(
        session,
        Event::Hooks(vec![
            hook(
                "pre_tool_use:0:0",
                "preToolUse",
                Some("Bash"),
                "sh -c 'cat >> ~/.cache/scratch/pandemonium/pretool.log'",
                false,
            ),
            hook(
                "session_start:0:0",
                "sessionStart",
                None,
                "/home/reader/.local/bin/muffle hook codex --terse",
                true,
            ),
        ]),
    )
}

/// A finished conversation whose files were kept before its first message, scrolled back to it.
fn kept() -> WorkspaceState {
    live(Turn::Finished)
        .heard(
            0,
            Event::Prompted {
                text: "Why does the sidebar lose its scroll position?".to_owned(),
                files: Vec::new(),
                checkpoint: true,
            },
        )
        .scrolled_chat(-400.0, 400.0)
}

/// A conversation that went through what an agent says in a turn, ending as `turn` says.
fn live(turn: Turn) -> WorkspaceState {
    let tool = |id: &str, title: &str, kind, target: &str| Tool {
        id: id.to_owned(),
        title: Some(title.to_owned()),
        kind: Some(kind),
        state: Some(ToolState::Done),
        target: Some(target.to_owned()),
        diff: None,
        output: None,
        parent: None,
        agent: None,
    };
    let mut state = WorkspaceState::new(&projects())
        .edited(
            PROMPT,
            Edit::Paste("Why does the sidebar lose its scroll position?".to_owned()),
        )
        .after(SEND);
    let mut events = vec![
        Event::Settings(claude_settings()),
        Event::Commands(vec![
            Slash {
                name: "compact".to_owned(),
                description: "Summarise the conversation to free up context".to_owned(),
            },
            Slash {
                name: "clear".to_owned(),
                description: "Clear the conversation".to_owned(),
            },
            Slash {
                name: "model".to_owned(),
                description: "Choose the model for this session".to_owned(),
            },
        ]),
        Event::Thought(
            "The scroll offset lives in the workspace state, so a rebuilt list should keep it."
                .to_owned(),
        ),
        Event::Plan(vec![
            PlanStep {
                text: "Find where the offset is stored".to_owned(),
                progress: Progress::Done,
            },
            PlanStep {
                text: "Check how it is clamped".to_owned(),
                progress: Progress::Doing,
            },
            PlanStep {
                text: "Write the fix".to_owned(),
                progress: Progress::Pending,
            },
        ]),
        Event::Tool(tool(
            "1",
            "Read",
            Kind::Read,
            "~/code/pandemonium/crates/pm-components/src/workspace/state.rs",
        )),
        Event::Tool(tool("2", "Grep", Kind::Search, "sidebar_scroll")),
        Event::Tool(Tool {
            diff: Some(Diff {
                path: "~/code/pandemonium/src/lib.rs".to_owned(),
                old: Some("fn a() {}\nfn b() {}\nfn c() {}\n".to_owned()),
                new: "fn a() {}\nfn d() {}\nfn c() {}\n".to_owned(),
            }),
            ..tool("3", "Edit", Kind::Edit, "~/code/pandemonium/src/lib.rs")
        }),
        Event::Tool(Tool {
            output: Some("running 12 tests\ntest result: ok. 12 passed".to_owned()),
            ..tool("4", "Terminal", Kind::Execute, "cargo test")
        }),
        Event::Said(
            "The offset is clamped against a height measured before the list is rebuilt."
                .to_owned(),
        ),
        Event::Tool(tool(
            "5",
            "Read",
            Kind::Read,
            "~/code/pandemonium/README.md",
        )),
        Event::Said("I changed the clamp to use the new height.".to_owned()),
    ];
    if turn == Turn::Asking {
        events.push(Event::Ask(Permission {
            ticket: 1,
            title: "Plan ready for review".to_owned(),
            command: None,
            plan: Some(
                "## Export a session as Markdown\n\nAdd an export that writes the conversation to a Markdown file. Nothing is edited until you approve.\n\n1. Render the transcript\n   - Add a Markdown writer that streams messages to any `io::Write`.\n2. Wire it to the UI\n   - Add an export item to the session menu.\n\n### Files to change\n\n- `crates/pandemonium/src/export.rs`\n- `crates/pandemonium/src/session.rs`"
                    .to_owned(),
            ),
            choices: vec![
                Choice {
                    id: "a".to_owned(),
                    name: "Approve and ask before each edit".to_owned(),
                    verdict: Verdict::Allow,
                },
                Choice {
                    id: "b".to_owned(),
                    name: "Approve and start".to_owned(),
                    verdict: Verdict::Always,
                },
                Choice {
                    id: "c".to_owned(),
                    name: "Keep planning".to_owned(),
                    verdict: Verdict::Reject,
                },
            ],
        }));
    } else if turn == Turn::Command {
        let choice = |id: &str, name: &str, verdict| Choice {
            id: id.to_owned(),
            name: name.to_owned(),
            verdict,
        };
        events.push(Event::Ask(Permission {
            ticket: 1,
            title: "Allow Claude to run Run a long read-only demo command?".to_owned(),
            command: Some("printf '%s\\n' \"permission prompt demo\" \"line two of a deliberately long command\" && ls -la /home/jdaddy/Documents/repos/vault | head -n 5 && git -C /home/jdaddy/Documents/repos/vault log --oneline --decorate --max-count=3 && wc -l /home/jdaddy/Documents/repos/vault/AGENTS.md /home/jdaddy/Documents/repos/vault/Makefile 2>/dev/null && date --iso-8601=seconds".to_owned()),
            plan: None,
            choices: vec![
                choice("a", "Allow once", Verdict::Allow),
                choice("b", "Always allow", Verdict::Always),
                choice("c", "Deny", Verdict::Reject),
            ],
        }));
    } else if turn == Turn::Inquiring {
        let alternative = |label: &str, description: &str| Alternative {
            label: label.to_owned(),
            description: description.to_owned(),
        };
        events.push(Event::Inquire(Inquiry {
            ticket: 1,
            questions: vec![
                Question {
                    header: "Direction".to_owned(),
                    text: "Imagine vault is three months from now and you open it every evening after a long day: the assistant can finally start jobs, the curating path is rewired, sources download straight into the library, and the whole thing feels like a private streaming service that knows your taste. When you picture the single most important thing that should have changed in how you interact with it compared to today, which of these is closest to the core of that change?".to_owned(),
                    multiple: false,
                    alternatives: vec![
                        alternative(
                            "Assistant acts for me",
                            "I ask in plain language to find new scenes like the last five I liked and download the best ones. The assistant queues a scan, identifies and downloads candidates, then generates jobs itself, with me only confirming the result.",
                        ),
                        alternative(
                            "Library curates itself",
                            "Rewire internal ingest and bring back a curate-style path so new material is discovered, fetched, identified and filed with minimal input, while still respecting library paths and avoiding time-triggered jobs.",
                        ),
                        alternative("Browsing and taste first", "Focus on the grid, search, taste profile, recommendations and playback so finding something to watch is instant and personal. Automation can stay manual for now."),
                        alternative("Metadata quality first", "Fix identify, populate visual descriptions, improve matching and scraper definitions so every scene has correct performers, studios and tags. Everything else builds on clean data."),
                    ],
                },
                Question {
                    header: "Where".to_owned(),
                    text: "Where should the setting live?".to_owned(),
                    multiple: true,
                    alternatives: vec![
                        alternative("Settings page", ""),
                        alternative("Menu bar", ""),
                    ],
                },
                Question {
                    header: "Name".to_owned(),
                    text: "What should it be called?".to_owned(),
                    multiple: false,
                    alternatives: vec![alternative("Appearance", ""), alternative("Theme", "")],
                },
            ],
        }));
    } else if turn == Turn::Finished {
        events.push(Event::Finished(None));
    } else if turn == Turn::Background {
        let agent = |id: &str, title: &str| Tool {
            state: Some(ToolState::Running),
            agent: Some("explore".to_owned()),
            ..tool(id, title, Kind::Other, title)
        };
        let child = |id: &str, parent: &str, target: &str| Tool {
            parent: Some(parent.to_owned()),
            ..tool(id, "Grep", Kind::Search, target)
        };
        events.extend([
            Event::Tool(agent("a1", "Find every caller of Session::new")),
            Event::Tool(agent(
                "a2",
                "Check how worktrees are located from a session root",
            )),
            Event::Tool(child("c1", "a1", "Session::new")),
            Event::Tool(child("c2", "a1", "Session::default")),
            Event::Tool(child("c3", "a2", "worktree_root")),
            Event::Tool(Tool {
                state: Some(ToolState::Running),
                ..tool("s1", "Terminal", Kind::Execute, "cargo watch -x test")
            }),
            Event::Background(Background {
                id: "s1".to_owned(),
                label: "cargo watch -x test".to_owned(),
                running: true,
                call: Some("s1".to_owned()),
            }),
            Event::Finished(None),
        ]);
    }
    let asked = state.sessions.len() - 1;
    for event in events {
        state = state.heard(asked, event);
    }
    state
}

/// Draws the screen as it looks `elapsed_ms` into the animations, with the
/// pointer at `pointer` and focus on `focused` if given, in `appearance`, and
/// writes it to `path` as a PNG. The pointer is taken to have rested where it
/// is since the start, so a tooltip under it shows once `elapsed_ms` is long
/// enough.
pub fn write(
    path: &Path,
    elapsed_ms: f32,
    pointer: Option<(f32, f32)>,
    focused: Option<FocusId>,
    appearance: Appearance,
    shot: Shot,
) -> Result<(), String> {
    let (width, height) = if shot.narrow() {
        (NARROW_WIDTH, NARROW_HEIGHT)
    } else if shot.in_settings_window() {
        (SETTINGS_WIDTH, SETTINGS_HEIGHT)
    } else if matches!(shot, Shot::Inquiring) {
        (WIDTH, 1000)
    } else if shot.in_picker_window() {
        (PICKER_WIDTH, PICKER_HEIGHT)
    } else {
        (WIDTH, HEIGHT)
    };
    let paint = if shot.in_settings_window() {
        scene::paint_settings
    } else if shot.in_picker_window() {
        scene::paint_picker
    } else {
        scene::paint
    };
    let mut renderer = Renderer::headless(width, height, 1.0).map_err(|error| error.to_string())?;
    let mut shaper = scene::shaper();
    let mut list = DrawList::new();
    let input = Input {
        pointer: PointerState {
            position: pointer,
            ..PointerState::default()
        },
        focused,
        elapsed_ms,
        selection: shot.selection(),
        held_bar: None,
    };
    let family = default_family();
    let theme = family.variant(appearance);
    let screen = shot.screen();
    let resting = Input {
        elapsed_ms: 0.0,
        ..input
    };
    paint(
        width as f32,
        height as f32,
        &screen,
        &resting,
        theme,
        &mut shaper,
        &mut DrawList::new(),
    );
    paint(
        width as f32,
        height as f32,
        &screen,
        &input,
        theme,
        &mut shaper,
        &mut list,
    );
    let pixels = renderer
        .snapshot(&list, &mut shaper, scene::ground(theme))
        .ok_or("the renderer could not read its frame back")?;
    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)
        .map_err(|error| error.to_string())
}

/// What a markdown file shown rendered in a snapshot says.
const PREVIEW_MARKDOWN: &str = "<p align=\"center\"><img src=\"assets/banner.png\" width=\"300\" alt=\"pandemonium\"></p>\n\n[![CI](https://img.shields.io/badge/ci-passing-green.svg)](https://github.com) ![License](https://img.shields.io/badge/license-MIT-blue.svg)\n\n# pandemonium\n\nA GPU-native editor for conducting agents. Several **projects** live in one window; a *session* is an agent working in its own git worktree of one of them.\n\nEach turn costs $t = n \\cdot c$ tokens:\n\n$$\n\\sum_{i=1}^{n} \\frac{c_i}{\\sqrt{2}} \\le T\n$$\n\n## Commands\n\n```sh\nmake run     # cargo run -p pandemonium\nmake lint    # cargo clippy --workspace\n```\n\n```mermaid\nflowchart LR\n    Window -->|requests| Server\n    Server --> Agents\n    Server --> Git\n```\n\n- Read [the protocol](docs/protocol.md) before changing `pm-protocol`.\n- Layers point one way.\n- An agent outlives its client.\n\n> A session is a worktree you review, not a terminal you supervise.\n\n| Crate | Job |\n| --- | --- |\n| `pm-ui` | the element tree |\n| `pm-renderer` | talks to wgpu |\n";

/// A badge of the kind a README shows from the web, for a snapshot.
const PREVIEW_BADGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="88" height="20"><rect width="88" height="20" rx="3" fill="#555"/><rect x="34" width="54" height="20" rx="3" fill="#4c1"/><text x="6" y="14" fill="#fff" font-family="sans-serif" font-size="11">ci</text><text x="40" y="14" fill="#fff" font-family="sans-serif" font-size="11">passing</text></svg>"##;

/// What a vector picture shown drawn in a snapshot holds.
const PREVIEW_VECTOR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="160" viewBox="0 0 240 160"><rect width="240" height="160" rx="24" fill="#1f2937"/><circle cx="80" cy="80" r="44" fill="#f97316"/><path d="M140 40 L204 120 L140 120 Z" fill="#38bdf8"/></svg>"##;

/// The workspace with the file at `path` open and shown rendered, as what it ends in says it is.
fn previewed(path: &str) -> WorkspaceState {
    load_installed_grammar("markdown");
    let mut state = workspace_sample().after(TAB_DIFF);
    state.open_viewer(path.to_owned(), false);
    match pm_components::preview::Rendering::of(path) {
        Some(pm_components::preview::Rendering::Picture) => {
            state.with_picture(0, path, false, Ok(gradient()))
        }
        Some(pm_components::preview::Rendering::Vector) => state
            .with_file(
                0,
                path,
                false,
                Ok((Some(PREVIEW_VECTOR.to_owned()), false, None)),
            )
            .with_drawn(
                0,
                path,
                false,
                crate::work::picture::draw(path, PREVIEW_VECTOR),
            ),
        _ => {
            let palette = crate::work::picture::Palette::of(
                pm_theme::default_family().variant(pm_theme::Appearance::Dark),
                true,
            );
            pm_components::figures::figures_of(PREVIEW_MARKDOWN, Some(path))
                .iter()
                .fold(
                    state.with_file(
                        0,
                        path,
                        false,
                        Ok((
                            Some(PREVIEW_MARKDOWN.to_owned()),
                            false,
                            Some("markdown".to_owned()),
                        )),
                    ),
                    |state, (key, wanted)| match wanted {
                        pm_components::figures::Wanted::Diagram(source) => {
                            state.with_figure(*key, crate::work::picture::diagram(source, &palette))
                        }
                        pm_components::figures::Wanted::Formula(source) => {
                            state.with_figure(*key, crate::work::picture::formula(source, &palette))
                        }
                        pm_components::figures::Wanted::Address(_) => state.with_figure(
                            *key,
                            crate::work::picture::figure(PREVIEW_BADGE.as_bytes(), false),
                        ),
                        pm_components::figures::Wanted::File(_) => state.with_figure(
                            *key,
                            Ok(pm_components::figures::Drawing {
                                picture: gradient(),
                                density: 1.0,
                            }),
                        ),
                    },
                )
        }
    }
}

/// A picture that fades from red to blue, for a snapshot of a preview.
fn gradient() -> pm_renderer::Picture {
    let (width, height) = (200u32, 120u32);
    let pixels = (0..height)
        .flat_map(|y| {
            (0..width)
                .flat_map(move |x| [(255 * x / width) as u8, (255 * y / height) as u8, 200, 255])
        })
        .collect();
    pm_renderer::Picture {
        key: 1,
        width,
        height,
        pixels: std::sync::Arc::new(pixels),
    }
}

/// A listing of the registry of tool servers, for a snapshot.
fn tool_listing(title: &str, name: &str, target: &str) -> pm_protocol::ToolListing {
    pm_protocol::ToolListing {
        id: name.to_owned(),
        title: title.to_owned(),
        description: "Up-to-date code docs for any prompt".to_owned(),
        name: name.to_owned(),
        target: target.to_owned(),
        env: Vec::new(),
        inputs: Vec::new(),
        asks: false,
        website: String::new(),
    }
}

#[cfg(test)]
mod offer_tests {
    use super::*;
    use pm_components::language_offer::{OFFER_DECLINE, OFFER_INSTALL};
    use pm_components::state::Outgoing;

    /// Pressing Install asks for the language, and once the server lists it installed the open file takes it.
    #[test]
    fn install_asks_and_the_file_takes_the_language() {
        let mut state = language_offer().after(OFFER_INSTALL);
        let asked = state.take_outbox().into_iter().any(|outgoing| {
            matches!(outgoing, Outgoing::InstallExtensions { ids } if ids == ["javascript"])
        });
        assert!(asked);
        assert_eq!(state.shown_document().unwrap().language, None);
        let installed = sample_languages()
            .into_iter()
            .map(|language| pm_protocol::ExtensionInfo {
                installed: language.installed || language.id == "javascript",
                ..language
            })
            .collect::<Vec<_>>();
        let state = state.with_host_extensions(&None, &installed);
        assert_eq!(
            state.shown_document().unwrap().language.as_deref(),
            Some("javascript")
        );
    }

    /// Pressing Not now remembers the language, and it is not offered or installed from the bar after.
    #[test]
    fn declining_is_remembered() {
        let mut state = language_offer().after(OFFER_DECLINE);
        assert_eq!(state.window.general.declined_languages, ["javascript"]);
        state.take_outbox();
        let mut state = state.after(OFFER_INSTALL);
        assert!(
            !state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::InstallExtensions { .. }))
        );
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    /// Measures rebuilding the same screen against repainting retained geometry, without GPU submission.
    #[test]
    #[ignore = "manual CPU benchmark: cargo test -p pandemonium benchmark_retained_screen -- --ignored --nocapture"]
    fn benchmark_retained_screen() {
        let screen = Shot::FilesScrolled.screen();
        let mut shaper = scene::shaper();
        let mut list = DrawList::new();
        let theme = default_family().dark;
        let mut cached = scene::Cached::default();
        for retained in [false, true] {
            let mut samples = Vec::new();
            for frame in 0..61 {
                let input = Input {
                    pointer: PointerState {
                        position: Some((1200.0, 220.0 + (frame % 20) as f32 * 24.0)),
                        ..PointerState::default()
                    },
                    elapsed_ms: frame as f32 * 16.67,
                    ..Input::default()
                };
                let started = std::time::Instant::now();
                let output = if retained {
                    cached.paint(
                        0,
                        WIDTH as f32,
                        HEIGHT as f32,
                        &screen,
                        &input,
                        &theme,
                        &mut shaper,
                        &mut list,
                    )
                } else {
                    scene::paint(
                        WIDTH as f32,
                        HEIGHT as f32,
                        &screen,
                        &input,
                        &theme,
                        &mut shaper,
                        &mut list,
                    )
                };
                std::hint::black_box(output);
                if frame > 0 {
                    samples.push(started.elapsed().as_secs_f64() * 1000.0);
                }
            }
            samples.sort_by(f64::total_cmp);
            eprintln!(
                "screen build+layout+paint retained={retained}: p50={:.2} p95={:.2} p99={:.2} ms",
                samples[29], samples[56], samples[59]
            );
        }
    }

    /// Model updates and viewport changes rebuild retained geometry and font changes can clear it.
    #[test]
    fn retained_screen_invalidates_for_content_and_viewport() {
        let mut cached = scene::Cached::default();
        let mut shaper = scene::shaper();
        let mut list = DrawList::new();
        let mut state = files();
        let theme = default_family().dark;
        let input = Input::default();
        let first = cached.paint(
            0,
            WIDTH as f32,
            HEIGHT as f32,
            &Screen::Workspace(state.clone()),
            &input,
            &theme,
            &mut shaper,
            &mut list,
        );
        state.open_tree.insert("crates".to_owned());
        let screen = Screen::Workspace(state);
        let changed = cached.paint(
            1,
            WIDTH as f32,
            HEIGHT as f32,
            &screen,
            &input,
            &theme,
            &mut shaper,
            &mut list,
        );
        assert!(changed.regions.len() > first.regions.len());
        let resized = cached.paint(
            1,
            1000.0,
            HEIGHT as f32,
            &screen,
            &input,
            &theme,
            &mut shaper,
            &mut list,
        );
        assert_ne!(changed.regions, resized.regions);
        cached.clear();
        let rebuilt = cached.paint(
            1,
            1000.0,
            HEIGHT as f32,
            &screen,
            &input,
            &theme,
            &mut shaper,
            &mut list,
        );
        assert_eq!(resized.regions, rebuilt.regions);
    }

    /// Measures editing and drawing independently for prompt size and conversation size.
    #[test]
    #[ignore = "manual CPU benchmark: cargo test -p pandemonium benchmark_typing -- --ignored --nocapture"]
    fn benchmark_typing() {
        for (long_prompt, long_chat) in [(false, false), (true, false), (false, true), (true, true)]
        {
            let state = input_stress(long_prompt, long_chat);
            let mut screen = Screen::Workspace(state);
            let mut shaper = scene::shaper();
            let mut cached = scene::Cached::default();
            let theme = default_family().dark;
            let mut list = DrawList::new();
            let mut samples = Vec::new();
            for version in 0..21 {
                let started = std::time::Instant::now();
                screen = screen.edited(PROMPT, pm_text::Edit::Insert('x'));
                let input = Input {
                    focused: Some(PROMPT),
                    ..Input::default()
                };
                std::hint::black_box(cached.paint(
                    version,
                    WIDTH as f32,
                    HEIGHT as f32,
                    &screen,
                    &input,
                    &theme,
                    &mut shaper,
                    &mut list,
                ));
                if version > 0 {
                    samples.push(started.elapsed().as_secs_f64() * 1000.0);
                }
            }
            samples.sort_by(f64::total_cmp);
            eprintln!(
                "typing CPU long_prompt={long_prompt} long_chat={long_chat}: p50={:.2} p95={:.2} ms",
                samples[9], samples[18]
            );
        }
    }
}

/// Writes one frame of the window with a live page open in the browser to a PNG, after sending a few events into the page when `interact`, and with the developer tools open when `tools`; prints the title the page ends with.
pub fn write_browser(path: &Path, url: &str, tools: bool, interact: bool) -> Result<(), String> {
    use pm_browser::{Button as Mouse, Engine, Key as Typed, Modifiers as Held, View};
    use pm_components::browser::{
        BROWSER_BODY, BROWSER_TOOLS_BODY, PAGE_SURFACE, Pointer, TOOLS_SURFACE,
    };
    use pm_renderer::Picture;
    use std::time::Duration;

    let (width, height) = (WIDTH, HEIGHT);
    let theme_family = default_family();
    let theme = theme_family.variant(Appearance::Dark);
    let mut shaper = scene::shaper();
    let mut renderer = Renderer::headless(width, height, 1.0).map_err(|error| error.to_string())?;
    let mut list = DrawList::new();
    let input = Input {
        pointer: PointerState::default(),
        focused: None,
        elapsed_ms: 0.0,
        selection: None,
        held_bar: None,
    };
    let mut workspace = workspace_sample();
    workspace.open_browser(url);
    if tools {
        workspace = workspace.browser_tools_shown();
    }
    let mut screen = Screen::Workspace(workspace);
    let region = |output: &pm_ui::Output, id: FocusId| {
        output
            .regions
            .iter()
            .find(|region| region.id == id)
            .map(|region| region.rect)
    };
    let output = scene::paint(
        width as f32,
        height as f32,
        &screen,
        &input,
        theme,
        &mut shaper,
        &mut list,
    );
    let body = region(&output, BROWSER_BODY).ok_or("the page has no window")?;
    let engine = Engine::start().map_err(|error| error.to_string())?;
    let page = View::open(&engine, url, body.width as u32, body.height as u32, 1.0)
        .map_err(|error| error.to_string())?;
    let pump = |seconds: f64| std::thread::sleep(Duration::from_secs_f64(seconds));
    let tools_view = if tools {
        let room = region(&output, BROWSER_TOOLS_BODY).ok_or("the tools have no window")?;
        pump(2.0);
        Some(page.open_devtools(room.width as u32, room.height as u32, 1.0))
    } else {
        None
    };
    pump(3.0);
    if tools_view.is_some() {
        pump(3.0);
    }
    if interact {
        let none = Held::default();
        page.focus(true);
        page.pointer_moved(40, 40, none, false);
        page.pointer_button(40, 40, Mouse::Left, true, 1, none);
        page.pointer_button(40, 40, Mouse::Left, false, 1, none);
        pump(0.3);
        for letter in ['h', 'i'] {
            for pressed in [true, false] {
                page.key(&Typed {
                    pressed,
                    code: letter.to_ascii_uppercase() as i32,
                    text: Some(letter.to_string()),
                    modifiers: none,
                });
            }
        }
        pump(0.3);
        page.pointer_moved(60, 130, none, false);
        page.pointer_button(60, 130, Mouse::Left, true, 1, none);
        page.pointer_button(60, 130, Mouse::Left, false, 1, none);
        pump(0.3);
        page.scrolled(200, 300, 0, -300, none);
        pump(1.0);
    }
    let picture = |view: &View, surface: u32| {
        view.frame().map(|frame| Picture {
            key: Picture::live_key(surface, frame.generation),
            width: frame.width,
            height: frame.height,
            pixels: frame.rgba,
        })
    };
    let title = page.title();
    let Screen::Workspace(state) = &mut screen;
    {
        state.browser_painted(
            picture(&page, PAGE_SURFACE),
            tools_view
                .as_ref()
                .and_then(|view| picture(view, TOOLS_SURFACE)),
            &title,
            &page.address(),
            Pointer::Arrow,
            false,
        );
    }
    println!("title: {title}");
    scene::paint(
        width as f32,
        height as f32,
        &screen,
        &input,
        theme,
        &mut shaper,
        &mut list,
    );
    let pixels = renderer
        .snapshot(&list, &mut shaper, scene::ground(theme))
        .ok_or("the renderer could not read its frame back")?;
    drop(tools_view);
    drop(page);
    drop(engine);
    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)
        .map_err(|error| error.to_string())
}
