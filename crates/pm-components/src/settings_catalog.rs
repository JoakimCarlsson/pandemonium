//! Every setting the settings window shows as a row: the page it is on, what it is called and does, and who keeps it.

use pm_input::FocusId;

use crate::settings::Section;

/// The first of the controls of the settings; each setting has eight after its own base, its place among them times eight.
const SETTING_BASE: u64 = 20_003_000;

/// How many controls a setting has room for.
const PER_SETTING: u64 = 8;

/// The control of a setting that opens or closes its list of choices.
pub const OPEN_LIST: usize = 6;

/// The control of a setting that puts it back as it comes.
pub(crate) const RESET: usize = 7;

/// The first of the choices of the open list of a setting picked from a long one; the rest follow it.
const PICK_BASE: u64 = 20_004_000;

/// One past the last choice of an open list.
const PICK_END: u64 = 20_005_000;

/// The name of the choice at `index` of the open list.
pub(crate) fn pick_id(index: usize) -> FocusId {
    FocusId(PICK_BASE + index as u64)
}

/// The place in the open list of the choice `id` names.
pub(crate) fn pick_of(id: FocusId) -> Option<usize> {
    (PICK_BASE..PICK_END)
        .contains(&id.0)
        .then(|| (id.0 - PICK_BASE) as usize)
}

/// The box a pattern is written in before it is added to the list of a setting.
pub(crate) const LIST_FIELD: FocusId = FocusId(20_000_065);

/// The button that adds the pattern written in [`LIST_FIELD`] to the list.
pub(crate) const LIST_ADD: FocusId = FocusId(20_000_066);

/// The first of the buttons that remove a line of the list; the rest follow it.
const LIST_REMOVE_BASE: u64 = 20_008_000;

/// The button that removes the line at `index` of the list.
pub(crate) fn list_remove_id(index: usize) -> FocusId {
    FocusId(LIST_REMOVE_BASE + index as u64)
}

/// The line of the list the button `id` removes.
pub(crate) fn list_remove_of(id: FocusId) -> Option<usize> {
    id.0.checked_sub(LIST_REMOVE_BASE)
        .map(|index| index as usize)
}

/// Who keeps a setting between runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Keeper {
    /// The window, for what only changes how it looks or behaves.
    Window,
    /// The server of each machine, for what it acts on, so it holds for a project on any machine.
    Machine,
}

/// One setting of the settings window.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Setting {
    /// What the window shows when it opens.
    Launch,
    /// Whether closing over working agents asks first.
    ConfirmClose,
    /// Whether the window looks for a newer release.
    CheckUpdates,
    /// Which releases it looks for.
    Channel,
    /// Whether a file in a language that is not installed offers to install it.
    OfferLanguages,
    /// Whether the embedded browser is turned on.
    EmbeddedBrowser,
    /// Whether the embedded browser opens on a dev server a shell started.
    OpenDevServers,
    /// Whether links to local pages open in the embedded browser.
    OpenLocalLinks,
    /// Whether agents may open only local addresses and the origins listed.
    RestrictAgentBrowsing,
    /// The origins agents may open besides local addresses.
    AgentOrigins,
    /// Whether the window is light, dark or as the desktop says.
    ThemeMode,
    /// The theme drawn while it is light.
    LightTheme,
    /// The theme drawn while it is dark.
    DarkTheme,
    /// The font the interface is set in.
    InterfaceFont,
    /// How big the interface's text is.
    InterfaceSize,
    /// The font code is set in.
    CodeFont,
    /// How big the text of a file is.
    CodeSize,
    /// How tall a line of code is.
    LineHeight,
    /// How closely things are set.
    Density,
    /// Whether lines have numbers.
    LineNumbers,
    /// Whether the numbers count from the caret.
    RelativeLineNumbers,
    /// Whether the caret's line is marked.
    CurrentLine,
    /// Whether indentation has upright lines.
    IndentGuides,
    /// Whether a line shows where lines should end.
    WrapGuide,
    /// Where lines should end.
    LineLength,
    /// Whether lines are marked by how they differ from the last commit.
    GitGutter,
    /// Whether the caret's line says who changed it.
    InlineBlame,
    /// Whether the uses of the name at the caret are marked.
    Occurrences,
    /// Whether brackets that pair are coloured by their depth.
    BracketColors,
    /// Whether the values a language marks as private are hidden behind blocks.
    RedactPrivateValues,
    /// Whether the newest versions of the dependencies a manifest lists are looked up and offered.
    DependencyHints,
    /// Whether names are coloured by what the language server says they are.
    SemanticHighlighting,
    /// Whether the language server's hints are written inside the lines.
    InlayHints,
    /// Whether the language server's lenses are shown beside the lines.
    CodeLens,
    /// How the caret is drawn.
    CursorShape,
    /// Whether the caret blinks.
    CursorBlink,
    /// Where long lines break.
    SoftWrap,
    /// How many columns a level of indentation is.
    TabSize,
    /// Whether indentation is tabs.
    HardTabs,
    /// Whether brackets and quotes close themselves.
    AutoClose,
    /// Whether a bracket typed over a selection goes around it.
    AutoSurround,
    /// Whether new lines are indented.
    AutoIndent,
    /// Whether a new line after a comment is a comment.
    ExtendComments,
    /// Whether saving formats.
    FormatOnSave,
    /// Whether typing formats what was typed.
    FormatOnType,
    /// Whether typing in a name edits the names that belong with it.
    LinkedEditing,
    /// Whether Tab writes out the snippet a word is the name of.
    SnippetsOnTab,
    /// Whether Tab writes out an Emmet abbreviation in a markup or style file.
    Emmet,
    /// Whether a map of the file shows beside it.
    Minimap,
    /// Whether the lines that open the blocks the view is in stay at its top.
    StickyScroll,
    /// Whether saving takes off trailing spaces.
    TrimWhitespace,
    /// Whether saving ends the file with a line break.
    FinalNewline,
    /// Whether files save themselves.
    Autosave,
    /// Whether completions open as one types.
    CompletionsOnInput,
    /// Whether completions show their documentation.
    CompletionDocs,
    /// Whether resting on a name shows what is known about it.
    Hover,
    /// How long the pointer rests first.
    HoverDelay,
    /// Whether the signature of a call shows.
    SignatureHelp,
    /// How bad something must be to be marked.
    Diagnostics,
    /// Whether finding goes round the end.
    SearchWrap,
    /// Whether finding starts from the selection.
    SeedSearch,
    /// Whether a capital letter makes finding match case.
    Smartcase,
    /// Whether the file scrolls past its end.
    ScrollBeyond,
    /// How many lines are kept around the caret.
    ScrollMargin,
    /// Whether the scrollbar marks problems, changes and matches.
    ScrollbarMarks,
    /// The language whose settings show.
    Language,
    /// How many columns a level of indentation is in the language.
    LanguageTabSize,
    /// Whether the language is indented with tabs.
    LanguageIndent,
    /// Whether saving a file of the language formats it.
    LanguageFormat,
    /// Whether a language server reads the language.
    LanguageServer,
    /// The command started as the language's server.
    LanguageServerCommand,
    /// Which language servers read the language, by id and in order.
    LanguageServers,
    /// The bindings the keymap starts from.
    BaseKeymap,
    /// The program the terminal starts.
    Shell,
    /// What the shell is started with.
    ShellArguments,
    /// How big the text of a shell is.
    TerminalFontSize,
    /// How a shell's cursor is drawn.
    TerminalCursor,
    /// How many lines a shell keeps to scroll back to.
    Scrollback,
    /// Whether selecting in a shell copies.
    CopyOnSelect,
    /// Whether the server fetches by itself.
    AutoFetch,
    /// How often it does.
    FetchInterval,
    /// Whether the server keeps a state of a file each time it is saved.
    LocalHistory,
    /// How many days those states are kept.
    HistoryDays,
    /// How many megabytes of those states a project may take up.
    HistorySize,
    /// What a new session's branch is called.
    BranchTemplate,
    /// Where worktrees are made.
    WorktreeFolder,
    /// The ignored files copied into every new worktree.
    WorktreeInclude,
    /// Whether inline blame follows the editor or is off.
    GitBlame,
    /// Whether the change marks follow the editor or are off.
    GitGutterMarks,
    /// Whether the words that changed inside a changed line of a diff are drawn apart.
    GitWordDiff,
    /// Which layout the addresses of self-hosted git servers use.
    GitHosts,
    /// The agent new work is given to.
    DefaultAgent,
    /// The mode agents start in.
    PermissionMode,
    /// What a session does when its agent finishes.
    OnFinish,
    /// How the reader is told an agent finished.
    Notification,
    /// The server new projects are added on.
    DefaultServer,
    /// Whether files are edited with vim's keys.
    VimMode,
    /// How much vim's unnamed register shares with the system clipboard.
    VimClipboard,
    /// The reader's own vim bindings, laid over the ones that come with it.
    VimBindings,
    /// Whether saving organizes the imports of the file first.
    OrganizeOnSave,
    /// Whether saving applies the fixes of the language server that are safe first.
    FixAllOnSave,
    /// How long a file is before the names in view are asked of the language server first.
    SemanticRangeLines,
    /// How many lines around the view are asked for with it.
    SemanticRangeMargin,
    /// Whether the repositories inside a project and its submodules join the workspace of its language servers.
    WorkspaceRepositories,
    /// The folders besides a project's own that join the workspace of its language servers.
    WorkspaceFolders,
    /// Whether saving a file of the language organizes its imports first.
    LanguageOrganize,
    /// Whether saving a file of the language applies the safe fixes first.
    LanguageFixAll,
    /// Whether typing replayed by vim is typed with pairs and indentation.
    VimReplayTyping,
    /// Whether vim's function, class and comment objects and motions use the syntax tree.
    VimSyntaxObjects,
    /// When finding looks only in the lines that were selected.
    AutoFindInSelection,
    /// Whether selected text is moved by dragging it.
    DragAndDrop,
    /// Where the marker for the changes the language server offers shows.
    Lightbulb,
    /// Whether the code font joins characters into ligatures.
    Ligatures,
    /// Whether the caret follows the arrow keys across right-to-left text.
    VisualCaret,
    /// Whether comparing two texts ignores white space.
    CompareIgnoreWhitespace,
}

impl Setting {
    /// Every setting, in the order the pages show them.
    pub const ALL: [Self; 115] = [
        Self::Launch,
        Self::ConfirmClose,
        Self::CheckUpdates,
        Self::Channel,
        Self::OfferLanguages,
        Self::EmbeddedBrowser,
        Self::OpenDevServers,
        Self::OpenLocalLinks,
        Self::RestrictAgentBrowsing,
        Self::AgentOrigins,
        Self::ThemeMode,
        Self::LightTheme,
        Self::DarkTheme,
        Self::InterfaceFont,
        Self::InterfaceSize,
        Self::CodeFont,
        Self::CodeSize,
        Self::LineHeight,
        Self::Density,
        Self::LineNumbers,
        Self::RelativeLineNumbers,
        Self::CurrentLine,
        Self::IndentGuides,
        Self::WrapGuide,
        Self::LineLength,
        Self::GitGutter,
        Self::InlineBlame,
        Self::Occurrences,
        Self::BracketColors,
        Self::RedactPrivateValues,
        Self::DependencyHints,
        Self::SemanticHighlighting,
        Self::InlayHints,
        Self::CodeLens,
        Self::Minimap,
        Self::StickyScroll,
        Self::CursorShape,
        Self::CursorBlink,
        Self::SoftWrap,
        Self::TabSize,
        Self::HardTabs,
        Self::AutoClose,
        Self::AutoSurround,
        Self::AutoIndent,
        Self::ExtendComments,
        Self::FormatOnType,
        Self::LinkedEditing,
        Self::SnippetsOnTab,
        Self::Emmet,
        Self::FormatOnSave,
        Self::TrimWhitespace,
        Self::FinalNewline,
        Self::Autosave,
        Self::CompletionsOnInput,
        Self::CompletionDocs,
        Self::Hover,
        Self::HoverDelay,
        Self::SignatureHelp,
        Self::Diagnostics,
        Self::SearchWrap,
        Self::SeedSearch,
        Self::Smartcase,
        Self::ScrollBeyond,
        Self::ScrollMargin,
        Self::ScrollbarMarks,
        Self::Language,
        Self::LanguageTabSize,
        Self::LanguageIndent,
        Self::LanguageFormat,
        Self::LanguageServer,
        Self::LanguageServerCommand,
        Self::LanguageServers,
        Self::BaseKeymap,
        Self::Shell,
        Self::ShellArguments,
        Self::TerminalFontSize,
        Self::TerminalCursor,
        Self::Scrollback,
        Self::CopyOnSelect,
        Self::AutoFetch,
        Self::FetchInterval,
        Self::LocalHistory,
        Self::HistoryDays,
        Self::HistorySize,
        Self::BranchTemplate,
        Self::WorktreeFolder,
        Self::WorktreeInclude,
        Self::GitBlame,
        Self::GitGutterMarks,
        Self::GitWordDiff,
        Self::GitHosts,
        Self::DefaultAgent,
        Self::PermissionMode,
        Self::OnFinish,
        Self::Notification,
        Self::DefaultServer,
        Self::VimMode,
        Self::VimClipboard,
        Self::VimBindings,
        Self::OrganizeOnSave,
        Self::FixAllOnSave,
        Self::SemanticRangeLines,
        Self::SemanticRangeMargin,
        Self::WorkspaceRepositories,
        Self::WorkspaceFolders,
        Self::LanguageOrganize,
        Self::LanguageFixAll,
        Self::VimReplayTyping,
        Self::VimSyntaxObjects,
        Self::AutoFindInSelection,
        Self::DragAndDrop,
        Self::Lightbulb,
        Self::Ligatures,
        Self::VisualCaret,
        Self::CompareIgnoreWhitespace,
    ];

    /// The page it is on.
    pub fn page(self) -> Section {
        match self {
            Self::Launch
            | Self::ConfirmClose
            | Self::CheckUpdates
            | Self::Channel
            | Self::OfferLanguages => Section::General,
            Self::EmbeddedBrowser
            | Self::OpenDevServers
            | Self::OpenLocalLinks
            | Self::RestrictAgentBrowsing
            | Self::AgentOrigins => Section::Browser,
            Self::ThemeMode
            | Self::LightTheme
            | Self::DarkTheme
            | Self::InterfaceFont
            | Self::InterfaceSize
            | Self::CodeFont
            | Self::CodeSize
            | Self::LineHeight
            | Self::Density => Section::Appearance,
            Self::LineNumbers
            | Self::RelativeLineNumbers
            | Self::CurrentLine
            | Self::IndentGuides
            | Self::WrapGuide
            | Self::LineLength
            | Self::GitGutter
            | Self::InlineBlame
            | Self::Occurrences
            | Self::BracketColors
            | Self::RedactPrivateValues
            | Self::DependencyHints
            | Self::SemanticHighlighting
            | Self::InlayHints
            | Self::CodeLens
            | Self::Minimap
            | Self::StickyScroll
            | Self::CursorShape
            | Self::CursorBlink
            | Self::Ligatures => Section::EditorAppearance,
            Self::SoftWrap
            | Self::TabSize
            | Self::HardTabs
            | Self::AutoClose
            | Self::AutoSurround
            | Self::AutoIndent
            | Self::ExtendComments
            | Self::FormatOnType
            | Self::LinkedEditing
            | Self::SnippetsOnTab
            | Self::Emmet
            | Self::DragAndDrop
            | Self::VisualCaret => Section::EditorEditing,
            Self::FormatOnSave
            | Self::TrimWhitespace
            | Self::FinalNewline
            | Self::Autosave
            | Self::OrganizeOnSave
            | Self::FixAllOnSave => Section::EditorSaving,
            Self::CompletionsOnInput
            | Self::CompletionDocs
            | Self::Hover
            | Self::HoverDelay
            | Self::SignatureHelp
            | Self::Diagnostics
            | Self::SemanticRangeLines
            | Self::SemanticRangeMargin
            | Self::WorkspaceRepositories
            | Self::WorkspaceFolders
            | Self::Lightbulb => Section::EditorIntelligence,
            Self::SearchWrap
            | Self::SeedSearch
            | Self::Smartcase
            | Self::ScrollBeyond
            | Self::ScrollMargin
            | Self::ScrollbarMarks
            | Self::AutoFindInSelection
            | Self::CompareIgnoreWhitespace => Section::EditorSearch,
            Self::Language
            | Self::LanguageTabSize
            | Self::LanguageIndent
            | Self::LanguageFormat
            | Self::LanguageServer
            | Self::LanguageServerCommand
            | Self::LanguageServers
            | Self::LanguageOrganize
            | Self::LanguageFixAll => Section::Languages,
            Self::BaseKeymap => Section::Keymap,
            Self::Shell
            | Self::ShellArguments
            | Self::TerminalFontSize
            | Self::TerminalCursor
            | Self::Scrollback
            | Self::CopyOnSelect => Section::Terminal,
            Self::AutoFetch
            | Self::FetchInterval
            | Self::LocalHistory
            | Self::HistoryDays
            | Self::HistorySize
            | Self::GitBlame
            | Self::GitGutterMarks
            | Self::GitWordDiff
            | Self::GitHosts => Section::Git,
            Self::BranchTemplate | Self::WorktreeFolder | Self::WorktreeInclude => {
                Section::Worktrees
            }
            Self::DefaultAgent | Self::PermissionMode | Self::OnFinish | Self::Notification => {
                Section::Agents
            }
            Self::DefaultServer => Section::Servers,
            Self::VimMode
            | Self::VimClipboard
            | Self::VimBindings
            | Self::VimReplayTyping
            | Self::VimSyntaxObjects => Section::EditorVim,
        }
    }

    /// Who keeps it between runs.
    pub fn keeper(self) -> Keeper {
        match self {
            Self::LanguageTabSize
            | Self::LanguageIndent
            | Self::LanguageFormat
            | Self::LanguageServer
            | Self::LanguageServerCommand
            | Self::LanguageServers
            | Self::LanguageOrganize
            | Self::LanguageFixAll
            | Self::WorkspaceRepositories
            | Self::WorkspaceFolders
            | Self::Shell
            | Self::ShellArguments
            | Self::AutoFetch
            | Self::FetchInterval
            | Self::LocalHistory
            | Self::HistoryDays
            | Self::HistorySize
            | Self::BranchTemplate
            | Self::WorktreeFolder
            | Self::WorktreeInclude
            | Self::GitHosts
            | Self::DefaultAgent
            | Self::PermissionMode => Keeper::Machine,
            _ => Keeper::Window,
        }
    }

    /// What it is called, and what it does.
    pub fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::Launch => (
                "Open on launch",
                "The last workspace with its open files and layout, or a fresh start.",
            ),
            Self::ConfirmClose => (
                "Confirm before closing",
                "Ask before closing the window while agents are working.",
            ),
            Self::CheckUpdates => (
                "Check for updates",
                "Look for a newer release while the window is open.",
            ),
            Self::Channel => (
                "Update channel",
                "Follow the stable releases, or the pre-releases.",
            ),
            Self::OfferLanguages => (
                "Offer to install languages",
                "Offer to install a language when a file is opened that is written in one that is not installed.",
            ),
            Self::EmbeddedBrowser => (
                "Embedded browser",
                "Show web pages, such as a dev server, in a pane beside the work. Turning it on downloads the browser once, about 120 MB.",
            ),
            Self::OpenDevServers => (
                "Open dev servers in the browser",
                "Open the embedded browser on the web page a program started in a terminal serves, such as a dev server. Needs the embedded browser.",
            ),
            Self::OpenLocalLinks => (
                "Open local links in the browser",
                "Open links to localhost, 127.0.0.1, [::1] and 0.0.0.0 in the embedded browser instead of the system browser. Other links always open in the system browser. Needs the embedded browser.",
            ),
            Self::RestrictAgentBrowsing => (
                "Restrict agent browsing",
                "Let agents open only localhost, 127.0.0.1, [::1] and the origins listed below. Pages the reader opens are not affected. Needs the embedded browser.",
            ),
            Self::AgentOrigins => (
                "Origins agents may open",
                "Besides local addresses, while agent browsing is restricted. Write an origin such as https://example.com; a host alone, such as example.com, means http and https on their usual ports.",
            ),
            Self::ThemeMode => (
                "Theme mode",
                "Light, dark, or whichever the desktop is set to.",
            ),
            Self::LightTheme => ("Light theme", "The theme drawn while the window is light."),
            Self::DarkTheme => ("Dark theme", "The theme drawn while the window is dark."),
            Self::InterfaceFont => (
                "Interface font",
                "The font menus, lists and conversations are set in.",
            ),
            Self::InterfaceSize => (
                "Interface font size",
                "How big the text of menus, lists and conversations is.",
            ),
            Self::CodeFont => ("Code font", "The font files, diffs and shells are set in."),
            Self::CodeSize => ("Code font size", "How big the text of a file is."),
            Self::LineHeight => (
                "Line height",
                "How tall a line of code is, as a multiple of its font size.",
            ),
            Self::Density => (
                "Interface density",
                "How closely rows, panels and gaps are set.",
            ),
            Self::LineNumbers => ("Line numbers", "Show each line's number beside it."),
            Self::RelativeLineNumbers => (
                "Relative line numbers",
                "Count the numbers from the line of the caret.",
            ),
            Self::CurrentLine => (
                "Highlight the current line",
                "Mark the line the caret is on.",
            ),
            Self::IndentGuides => (
                "Indent guides",
                "Show each level of indentation with an upright line.",
            ),
            Self::WrapGuide => (
                "Wrap guide",
                "Show where lines should end with an upright line.",
            ),
            Self::LineLength => (
                "Preferred line length",
                "Where the wrap guide stands and soft wrap breaks, in characters.",
            ),
            Self::GitGutter => (
                "Git changes beside the line numbers",
                "Mark lines added, changed and taken out since the last commit.",
            ),
            Self::InlineBlame => (
                "Inline blame",
                "Show who last changed the line of the caret, and when.",
            ),
            Self::Occurrences => (
                "Highlight uses of the name at the caret",
                "Mark every place the name under the caret is used in the file.",
            ),
            Self::BracketColors => (
                "Colour brackets by depth",
                "Colour each pair of brackets by how many pairs enclose it, so matching ones are easy to see.",
            ),
            Self::RedactPrivateValues => (
                "Hide private values",
                "Cover the values a language marks as private, such as the secrets of an environment file, with blocks, so sharing the screen does not give them away.",
            ),
            Self::DependencyHints => (
                "Dependency updates",
                "Look up the newest version of each dependency a manifest lists, such as in package.json, Cargo.toml or go.mod, and offer a link that updates it. Turn it off to stop looking versions up on the web.",
            ),
            Self::SemanticHighlighting => (
                "Semantic highlighting",
                "Colour names by what the language server says they are, over what the syntax says.",
            ),
            Self::InlayHints => (
                "Inlay hints",
                "Write what the language server infers inside the lines, such as the type of a name or the name of a parameter.",
            ),
            Self::CodeLens => (
                "Code lens",
                "Show what the language server says about a function or type at the end of its line, such as how many times it is used.",
            ),
            Self::Minimap => (
                "Minimap",
                "Draw a map of the whole file beside it; press or drag on it to scroll.",
            ),
            Self::StickyScroll => (
                "Sticky scroll",
                "Keep the lines that open the blocks the top line is in at the top of the view.",
            ),
            Self::FormatOnType => (
                "Format on type",
                "Format what was just typed as its language says, when its language server can.",
            ),
            Self::LinkedEditing => (
                "Linked editing",
                "Typing in a name, such as a tag's, types in the names that belong with it too.",
            ),
            Self::SnippetsOnTab => (
                "Snippets on Tab",
                "Write out the snippet a word is the name of when Tab is pressed after it; Tab then moves between its blanks.",
            ),
            Self::Emmet => (
                "Emmet abbreviations",
                "Write out an abbreviation such as ul>li*3 in a markup or style file when Tab is pressed after it.",
            ),
            Self::CursorShape => (
                "Cursor shape",
                "A bar between characters, a block over one, or a line under it.",
            ),
            Self::CursorBlink => ("Blinking cursor", "Make the caret blink while it rests."),
            Self::SoftWrap => (
                "Soft wrap",
                "Break long lines onto the next row at the window edge or the preferred line length.",
            ),
            Self::TabSize => (
                "Tab size",
                "How many columns a level of indentation is; Language keeps each language's own.",
            ),
            Self::HardTabs => (
                "Indent with",
                "Spaces or tabs; Language keeps each language's own.",
            ),
            Self::AutoClose => (
                "Close brackets and quotes",
                "Write the closing one as the opening one is typed.",
            ),
            Self::AutoSurround => (
                "Surround the selection",
                "A bracket or quote typed over a selection goes around it.",
            ),
            Self::AutoIndent => (
                "Indent new lines",
                "Indent a new line as the language says.",
            ),
            Self::ExtendComments => (
                "Continue comments",
                "A new line after a line comment starts with the comment too.",
            ),
            Self::FormatOnSave => (
                "Format on save",
                "Format the file as its language says when it is saved.",
            ),
            Self::OrganizeOnSave => (
                "Organize imports on save",
                "Sort and prune the imports of the file as its language server says, before it is saved.",
            ),
            Self::FixAllOnSave => (
                "Fix all on save",
                "Apply every fix the language server marks safe to apply at once, before the file is saved.",
            ),
            Self::SemanticRangeLines => (
                "Names in view first, lines",
                "In a file with at least this many lines, the language server is asked for the names in view first and for the rest after; 0 always asks for the whole file at once.",
            ),
            Self::SemanticRangeMargin => (
                "Names around the view",
                "How many lines above and below the view are asked for with the names in view, counted in lines.",
            ),
            Self::WorkspaceRepositories => (
                "Attach nested repositories",
                "Have each language server of a project also work on the repositories kept inside the project and on the submodules of its repository.",
            ),
            Self::WorkspaceFolders => (
                "Extra workspace folders",
                "Folders added to the workspace of a project's language servers, written as a folder or as project=folder, separated by commas; a folder that is not a full path is taken from the root of the project, and a project is named by its folder or the name of that folder.",
            ),
            Self::LanguageOrganize => (
                "Organize imports on save",
                "Organize the imports of its files before they are saved; Editor follows the editor's setting.",
            ),
            Self::LanguageFixAll => (
                "Fix all on save",
                "Apply the safe fixes of the language server to its files before they are saved; Editor follows the editor's setting.",
            ),
            Self::TrimWhitespace => (
                "Remove trailing whitespace",
                "Take spaces and tabs off the ends of lines when saving.",
            ),
            Self::FinalNewline => (
                "End with a line break",
                "Make the file end with a line break when saving.",
            ),
            Self::Autosave => (
                "Autosave",
                "Save a file a moment after it was last changed.",
            ),
            Self::CompletionsOnInput => (
                "Show completions while typing",
                "Open the list of completions as you type; Ctrl+Space always opens it.",
            ),
            Self::CompletionDocs => (
                "Completion documentation",
                "Show the documentation of the completion picked beside the list.",
            ),
            Self::Hover => (
                "Hover",
                "Show what the language server says about the name the pointer rests on.",
            ),
            Self::HoverDelay => (
                "Hover delay",
                "How long the pointer rests before that, in milliseconds.",
            ),
            Self::SignatureHelp => (
                "Signature help",
                "Show the signature of the call the caret is in as you type.",
            ),
            Self::Diagnostics => (
                "Show problems",
                "How bad something the language server finds must be for it to be marked.",
            ),
            Self::SearchWrap => ("Wrap around", "Go round to the first match after the last."),
            Self::SeedSearch => (
                "Search the selection",
                "Start finding with what is selected.",
            ),
            Self::Smartcase => (
                "Smart case",
                "Match case when what you look for has a capital letter.",
            ),
            Self::ScrollBeyond => (
                "Scroll beyond the last line",
                "Scroll on until the last line is near the top.",
            ),
            Self::ScrollMargin => (
                "Scroll margin",
                "How many lines are kept between the caret and the edges of the view.",
            ),
            Self::ScrollbarMarks => (
                "Scrollbar markers",
                "Mark problems, changed lines and search matches along the scrollbar.",
            ),
            Self::Language => ("Language", "The language whose settings show below."),
            Self::LanguageTabSize => (
                "Tab size",
                "How many columns a level of indentation is; Editor follows the editor's setting.",
            ),
            Self::LanguageIndent => (
                "Indent with",
                "Spaces or tabs; Editor follows the editor's setting.",
            ),
            Self::LanguageFormat => (
                "Format on save",
                "Format its files when they are saved; Editor follows the editor's setting.",
            ),
            Self::LanguageServer => (
                "Language server",
                "Have a language server read its files for completions, hovers and problems.",
            ),
            Self::LanguageServerCommand => (
                "Language server command",
                "Started in place of the server the language comes with; empty keeps that one.",
            ),
            Self::LanguageServers => (
                "Language servers",
                "The servers that read its files, by id and in order; `...` stands for the ones that start by themselves, and a server that waits to be named starts only when it is listed.",
            ),
            Self::BaseKeymap => (
                "Base keymap",
                "The keys start as another editor has them; your own keys go over them.",
            ),
            Self::Shell => (
                "Shell",
                "The program the terminal starts; empty for your login shell.",
            ),
            Self::ShellArguments => (
                "Shell arguments",
                "What the shell is started with, written as on a command line.",
            ),
            Self::TerminalFontSize => (
                "Terminal font size",
                "How big the text of a shell is; follows the code font unless set.",
            ),
            Self::TerminalCursor => ("Cursor style", "How the cursor of a shell is drawn."),
            Self::Scrollback => (
                "Scrollback",
                "How many lines a shell keeps to scroll back to.",
            ),
            Self::CopyOnSelect => (
                "Copy on select",
                "Copy text in a shell as soon as it is selected.",
            ),
            Self::AutoFetch => (
                "Fetch automatically",
                "Fetch every remote of each project in the background.",
            ),
            Self::FetchInterval => (
                "Fetch every",
                "How many minutes pass between one fetch and the next.",
            ),
            Self::LocalHistory => (
                "Local history",
                "Keep a state of each file every time it is saved, to compare with and restore in the timeline of the file.",
            ),
            Self::HistoryDays => (
                "Keep local history for",
                "How many days a saved state is kept before it is removed.",
            ),
            Self::HistorySize => (
                "Local history size",
                "How many megabytes of saved states one project may take up; the oldest are removed past it.",
            ),
            Self::BranchTemplate => (
                "Session branch name",
                "What a new session's branch is called; {title} stands for its title. Empty works on no branch.",
            ),
            Self::WorktreeFolder => (
                "Worktree folder",
                "Where session worktrees are made, a folder per project; empty keeps them inside each project.",
            ),
            Self::WorktreeInclude => (
                "Files copied into new worktrees",
                "Ignored files and folders copied from the project into every new session worktree, so it starts with its .env and build output. Paths are from the project root; * stands for part of one name and a folder ends in /.",
            ),
            Self::GitBlame => (
                "Inline blame",
                "Follow the editor's inline blame setting, or turn it off here.",
            ),
            Self::GitGutterMarks => (
                "Change marks",
                "Follow the editor's setting for marks beside changed lines, or turn them off here.",
            ),
            Self::GitWordDiff => (
                "Highlight changed words",
                "Mark the words that changed inside a changed line of a diff, not only the whole line.",
            ),
            Self::GitHosts => (
                "Self-hosted git servers",
                "The layout of permalinks for a server whose name does not give it away, as host=layout separated by commas; layouts are github, gitlab, bitbucket, gitea, azure, sourcehut and gitee.",
            ),
            Self::DefaultAgent => ("Default agent", "The agent new sessions start with."),
            Self::PermissionMode => (
                "Default permission mode",
                "The mode an agent starts in, when it offers that mode.",
            ),
            Self::OnFinish => (
                "When an agent finishes",
                "Do nothing, tell you, or open the session on what it changed.",
            ),
            Self::Notification => (
                "Notifications",
                "How you are told an agent finished: a sound, a desktop notification, or neither.",
            ),
            Self::DefaultServer => ("Default server", "The server new projects are added on."),
            Self::VimMode => (
                "Vim mode",
                "Edit files with vim's keys: normal, insert, visual and replace modes, motions, operators, text objects, registers, macros and the command line.",
            ),
            Self::VimClipboard => (
                "Share the clipboard",
                "Whether vim's unnamed register is the system clipboard: for every yank and delete, for yanks only, or never.",
            ),
            Self::VimBindings => (
                "Your bindings",
                "Keys written as keys => Action, with @ and a context after to hold only there, such as gh => Hover or jk => NormalBefore @ insert. They come after vim's own, so yours win; a line that cannot be read is not added.",
            ),
            Self::VimReplayTyping => (
                "Replay typing with pairs and indentation",
                "Text vim types again, for a count such as 3ihello, for . and for macros, closes brackets and quotes, types over a closer and indents new lines as typing it yourself does; off, it is put in as plain text. Follows Auto-close and Auto-indent.",
            ),
            Self::VimSyntaxObjects => (
                "Syntax objects and motions",
                "Let vim's function, class and comment objects (af, if, ac, ic) and motions (]], [[, ]m, [m) go by the file's syntax tree; off, they do nothing where only the tree knows the answer.",
            ),
            Self::AutoFindInSelection => (
                "Find in selection",
                "Limit finding and replacing to the selected lines as the bar opens: never, whenever something is selected, or when the selection runs over several lines.",
            ),
            Self::DragAndDrop => (
                "Drag and drop",
                "Move selected text by pressing on it and dragging it; hold Ctrl as you let go to copy it instead; Ctrl held as you press goes to the definition instead.",
            ),
            Self::Lightbulb => (
                "Code action marker",
                "Show a marker beside the line number of the caret's line where the language server offers changes: on that line always, or only while it has a problem. Needs line numbers.",
            ),
            Self::Ligatures => (
                "Font ligatures",
                "Join characters such as -> and != into one glyph where the code font has ligatures for them.",
            ),
            Self::VisualCaret => (
                "Caret follows the arrow keys in right-to-left text",
                "In a line with right-to-left text the arrow keys move the caret the way they point on screen, not through the text in reading order.",
            ),
            Self::CompareIgnoreWhitespace => (
                "Compare ignores white space",
                "Leave out lines that differ only in white space when comparing a file with the clipboard, with what was saved or with another file.",
            ),
        }
    }

    /// The name of its control at `option`.
    pub fn id(self, option: usize) -> FocusId {
        let place = Self::ALL
            .iter()
            .position(|held| *held == self)
            .unwrap_or_default() as u64;
        FocusId(SETTING_BASE + place * PER_SETTING + option as u64)
    }

    /// The setting, and the option of its control, that `id` names.
    pub(crate) fn of(id: FocusId) -> Option<(Self, usize)> {
        let offset = id.0.checked_sub(SETTING_BASE)?;
        let setting = Self::ALL.get((offset / PER_SETTING) as usize)?;
        Some((*setting, (offset % PER_SETTING) as usize))
    }

    /// The setting written as text whose box `id` is.
    pub(crate) fn text_of(id: FocusId) -> Option<Self> {
        Self::of(id)
            .filter(|(setting, option)| *option == 0 && setting.is_text())
            .map(|(setting, _)| setting)
    }

    /// Whether its title or what it does holds every word of `needle`, lowercased.
    pub fn matches(self, needle: &str) -> bool {
        let (title, about) = self.words();
        let haystack = format!(
            "{} {} {}",
            title.to_lowercase(),
            about.to_lowercase(),
            self.page().title().to_lowercase()
        );
        needle
            .split_whitespace()
            .all(|word| haystack.contains(word))
    }
}

/// The settings on the page `section`, in the order it shows them.
pub fn on_page(section: Section) -> impl Iterator<Item = Setting> {
    Setting::ALL
        .into_iter()
        .filter(move |setting| setting.page() == section)
}

/// The settings whose words hold every word of `needle`, in the order the pages show them.
pub fn found(needle: &str) -> Vec<Setting> {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    Setting::ALL
        .into_iter()
        .filter(|setting| setting.matches(&needle))
        .collect()
}
