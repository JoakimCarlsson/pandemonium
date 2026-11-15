//! `--snapshot`: draws the window over the sample workspace, or a section
//! of the settings window over sample settings, into a PNG, without a
//! window or a server, on the page asked for: a session, the welcome page
//! of a new session or chat, the history, the extensions or a project's
//! overview; with the right panel's Files or Search tab, the quick input,
//! the project menu, the adder, the made-up projects' notice, or the folder
//! picker's window, shown when asked for.

use std::path::Path;

use pm_pages::{
    Actions, PICKER_SIZE, PickerActions, SETTINGS_SIZE, SettingsActions, folder_picker_page,
    settings_page, settings_section, workspace_page,
};

/// The size drawn when none is given.
const DEFAULT_SIZE: (u32, u32) = (1636, 720);

/// The size of a snapshot of the settings window at its narrowest.
const NARROW_SIZE: (u32, u32) = (640, 420);

/// How long the live snapshot waits for the server at each stage.
const LIVE_WAIT: std::time::Duration = std::time::Duration::from_secs(4);

/// How long the live snapshot waits for the server to end the shells it started.
const CLOSE_WAIT: std::time::Duration = std::time::Duration::from_millis(300);

/// The words a snapshot understands after its size.
const WORDS: &[&str] = &[
    "dark",
    "light",
    "live",
    "welcome",
    "chat",
    "history",
    "extensions",
    "overview",
    "approve",
    "inquiry",
    "trust",
    "continue",
    "signed-out",
    "update",
    "sign-in",
    "status",
    "session-menu",
    "rename",
    "discard",
    "quitting",
    "problems",
    "tasks",
    "debug",
    "selection",
    "git-menu",
    "git-source",
    "git-confirm",
    "branch-menu",
    "git-rebase",
    "git-stashes",
    "git-submodules",
    "diff",
    "code",
    "code-find",
    "code-replace",
    "code-goto",
    "code-vim",
    "code-problems",
    "code-folded",
    "code-unsaved",
    "code-debug",
    "code-scrolled",
    "code-markdown",
    "code-picture",
    "code-completions",
    "code-hover",
    "code-signature",
    "code-annotations",
    "code-actions",
    "code-rename",
    "code-drag",
    "code-colour",
    "code-rtl",
    "code-language",
    "code-compare",
    "code-file-menu",
    "tab-menu",
    "tab-steps",
    "split",
    "split-wide",
    "tab-pinned",
    "tab-many",
    "files-picked",
    "quick-help",
    "quick-line",
    "excerpts",
    "update-tab",
    "files",
    "search",
    "quick",
    "quick-commands",
    "quick-sessions",
    "quick-text",
    "quick-theme",
    "quick-symbols",
    "quick-outline",
    "quick-keymap",
    "quick-settings",
    "quick-rewind",
    "quick-branch",
    "quick-references",
    "quick-actions",
    "quick-history",
    "quick-problems",
    "diff-notes",
    "conflict",
    "revision",
    "changes",
    "search-edit",
    "file-menu",
    "file-delete",
    "browser",
    "browser-find",
    "browser-menu",
    "browser-tools",
    "browser-ask",
    "browser-cert",
    "browser-clear",
    "browser-notice",
    "browser-pick",
    "project-menu",
    "chat-menu",
    "adder",
    "clone",
    "clone-failed",
    "sample",
    "remote",
    "picker",
    "picker-remote",
    "picker-failed",
    "form",
    "composer",
    "composer-model",
    "composer-slash",
    "composer-queued",
    "composer-attached",
    "composer-field",
    "composer-mention",
    "welcome-place",
    "welcome-branch",
    "settings-<section>[-narrow]",
];

/// Runs `--snapshot <path> [WIDTHxHEIGHT] [dark|light] [live]
/// [welcome|chat|history|extensions|overview|settings-<section>[-narrow]]
/// [approve|inquiry|trust|continue] [update|sign-in]
/// [problems|tasks|debug] [selection]
/// [git-menu|git-source|git-confirm|branch-menu|diff|diff-notes|conflict|revision|changes]
/// [code[-find|-replace|-goto|-vim|-debug|-scrolled|-markdown|-picture]]
/// [code-completions|code-hover|code-signature|code-annotations|code-actions]
/// [code-rename|code-drag|code-colour|code-rtl|code-language|code-compare|code-file-menu]
/// [tab-menu|tab-steps|split|excerpts|update-tab]
/// [files|search|search-edit|quick[-<list>]|file-menu|file-delete]
/// [browser[-find|-menu|-tools|-ask|-cert|-clear|-notice|-pick]]
/// [project-menu|session-menu|chat-menu|adder|clone|clone-failed|sample|remote]
/// [picker|picker-remote|picker-failed] [form]
/// [composer[-model|-slash|-queued|-attached|-field|-mention]|welcome-place|welcome-branch]
/// [signed-out]
/// [compact] [interface-<px>] [codesize-<px>] [line-<times>] [terminal-<px>]
/// [font-<family>] [mono-<family>]`
/// with `arguments` (those after the flag) and returns the process's exit
/// code: 0 when the PNG was written, 1 when drawing failed, 2 when the
/// arguments are wrong.
pub fn run(arguments: &[String]) -> i32 {
    let Some(path) = arguments.first() else {
        eprintln!("pandemonium: --snapshot needs a path");
        return 2;
    };
    let given = arguments.get(1).and_then(|text| parse_size(text));
    let skipped = if given.is_some() { 2 } else { 1 };
    let words: Vec<&str> = arguments.iter().skip(skipped).map(String::as_str).collect();
    if let Some(unknown) = words.iter().find(|word| !known(word)) {
        eprintln!(
            "pandemonium: --snapshot does not know {unknown}; the size must look like 1636x720 and the words are {}",
            WORDS.join(", ")
        );
        return 2;
    }
    let light = words.contains(&"light");
    if let Some(shot) = words.iter().find_map(|word| word.strip_prefix("settings-")) {
        return settings_shot(path, shot, given, light, &words);
    }
    let live = words.contains(&"live");
    let mut appearance = pm_shell::SettingsModel::default();
    crate::snapshot_appearance::apply(&mut appearance, &words);
    let theme = pm_pages::window_theme(&appearance, !light);
    if let Some(picker) = picker_named(&words) {
        let actions = PickerActions::inert();
        return draw(
            move || folder_picker_page(&picker, theme, &actions),
            given.unwrap_or((PICKER_SIZE.0 as u32, PICKER_SIZE.1 as u32)),
            light,
            path,
        );
    }
    let mut workspace = if live {
        live_workspace(&words)
    } else {
        pm_shell::sample_workspace()
    };
    if let (false, Some(page)) = (live, page_named(&words, &workspace)) {
        let first = workspace.page.clone();
        workspace.trail.visit(first);
        workspace.trail.visit(page.clone());
        workspace.page = page;
    }
    if !live && let Some(ask) = ask_named(&words) {
        pm_shell::sample_ask(&mut workspace.extras, ask);
    }
    if !live && let Some(sample) = composer_named(&words) {
        if sample == pm_shell::SampleComposer::Mention {
            workspace.tree.paths = pm_shell::sample_mentions();
        }
        pm_shell::sample_composer(&mut workspace.extras, sample);
    }
    let start = if ["welcome", "welcome-place", "welcome-branch"]
        .iter()
        .any(|word| words.contains(word))
    {
        pm_shell::start_in(&workspace.projects, &[0])
    } else if words.contains(&"chat") {
        Some(pm_shell::start_chat())
    } else {
        None
    };
    if let Some(mut start) = start {
        start.place_menu = words.contains(&"welcome-place");
        if words.contains(&"welcome-branch") {
            start.worktree = true;
            start.branches = vec!["main".to_owned(), "feature/search".to_owned()];
            start.base = Some("feature/search".to_owned());
            start.settings = workspace.extras.composer.settings.clone();
        }
        pm_shell::show_start(&mut workspace, start);
    }
    if !live {
        show_panels(&mut workspace, &words);
        show_projects(&mut workspace, &words);
    }
    show_git_state(&mut workspace.git, &words);
    if !live {
        show_review(&mut workspace, &words);
    }
    if workspace.git.diff.is_some()
        || workspace.git.revision.is_some()
        || words.contains(&"changes")
    {
        workspace.open_files.tab = pm_shell::SessionTab::Diff;
    }
    if words.iter().any(|word| {
        [
            "code",
            "tab-menu",
            "tab-steps",
            "split",
            "split-wide",
            "tab-pinned",
            "tab-many",
        ]
        .contains(word)
            || word.starts_with("code-")
    }) {
        open_sample_files(&mut workspace.open_files);
        show_code_state(&mut workspace, &words);
    }
    if let Some(tab) = dock_tab_named(&words) {
        workspace.dock_tab = tab;
    }
    if words.contains(&"selection") {
        workspace.terminal.selection = Some(((0, 1), (0, 22)));
    }
    if words.contains(&"update") || words.contains(&"update-tab") {
        workspace.update = Some(pm_shell::sample_update());
    }
    if words.contains(&"update-tab") {
        if let Some(offer) = &mut workspace.update {
            offer.open = false;
        }
        workspace
            .open_files
            .act(pm_shell::TabAction::Open(pm_shell::SessionTab::Update));
    }
    if words.contains(&"excerpts") {
        pm_shell::sample_search_edit(&mut workspace.search);
        workspace
            .open_files
            .act(pm_shell::TabAction::Open(pm_shell::SessionTab::Excerpts));
    }
    if words.contains(&"sign-in") {
        workspace.sign_in = Some(pm_shell::sample_sign_in());
    }
    show_session_state(&mut workspace, &words);
    if let Some(sample) = browser_named(&words) {
        workspace.browser = Some(pm_shell::sample_browser(sample));
        workspace.open_files.tab = pm_shell::SessionTab::Browser;
    }
    let actions = Actions::inert();
    draw(
        move || workspace_page(&workspace, theme, &actions),
        given.unwrap_or(DEFAULT_SIZE),
        light,
        path,
    )
}

/// Shows in `workspace` the bars and questions about sessions `words` name: `status` a failure in the status bar, `session-menu` the open session's menu, `rename` its rename dialog, `discard` the question before its changes are thrown away, and `quitting` the question before the window closes on working agents.
fn show_session_state(workspace: &mut pm_shell::Workspace, words: &[&str]) {
    let open = workspace
        .open
        .as_ref()
        .and_then(|at| pm_shell::session_at(&workspace.projects, at))
        .map(|session| (session.id, session.title.clone()));
    if words.contains(&"status") {
        workspace.status = Some("Could not save notes.md: permission denied".to_owned());
    }
    if let Some((id, title)) = open {
        if words.contains(&"session-menu") {
            workspace.session_menu = Some(id);
        }
        if words.contains(&"rename") {
            workspace.renaming = Some(pm_shell::Renaming { session: id, title });
        }
        if words.contains(&"discard") {
            workspace.closing = Some(id);
        }
    }
    if words.contains(&"quitting") {
        workspace.quitting = Some(2);
    }
}

/// Draws `content` at `size` into the PNG at `path`, in the light theme when
/// `light`, and returns the process's exit code: 0 when it was written, 1 when
/// drawing failed.
fn draw(content: impl FnMut() + 'static, size: (u32, u32), light: bool, path: &str) -> i32 {
    match rustle::preview(content, size, !light, Path::new(path)) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("pandemonium: {error}");
            1
        }
    }
}

/// The folder picker a word among `words` names: `picker` on this machine,
/// `picker-remote` on another one, `picker-failed` on one that cannot be reached.
fn picker_named(words: &[&str]) -> Option<pm_shell::FolderPicker> {
    words.iter().find_map(|word| match *word {
        "picker" => Some(pm_shell::sample_picker(None, false)),
        "picker-remote" => Some(pm_shell::sample_picker(Some("devbox"), false)),
        "picker-failed" => Some(pm_shell::sample_picker(Some("devbox"), true)),
        _ => None,
    })
}

/// Shows on the sample `workspace` what `words` name of the sidebar's
/// projects: the menu of a project, the adder choosing or cloning, a clone
/// that failed, the made-up projects' notice, or projects on other machines.
fn show_projects(workspace: &mut pm_shell::Workspace, words: &[&str]) {
    if words.contains(&"project-menu") {
        workspace.project_menu = workspace
            .projects
            .get(2)
            .map(|project| project.path.clone());
    }
    if words.contains(&"session-menu") && workspace.session_menu.is_none() {
        workspace.session_menu = workspace
            .projects
            .iter()
            .flat_map(|project| &project.sessions)
            .next()
            .map(|session| session.id);
    }
    if words.contains(&"chat-menu") {
        workspace.session_menu = workspace.chats.first().map(|chat| chat.id);
    }
    if words.contains(&"adder") {
        workspace.adder = Some(pm_shell::Adder::default());
    }
    if words.contains(&"clone") || words.contains(&"clone-failed") {
        workspace.adder = Some(pm_shell::Adder {
            step: pm_shell::AddStep::Clone,
            url: "JoakimCarlsson/rustle".to_owned(),
            error: words.contains(&"clone-failed").then(|| {
                "fatal: repository 'https://github.com/JoakimCarlsson/rustle/' not found".to_owned()
            }),
            machines: vec![
                pm_shell::Machine {
                    name: "This machine".to_owned(),
                    host: None,
                },
                pm_shell::Machine {
                    name: "devbox".to_owned(),
                    host: Some("devbox".to_owned()),
                },
            ],
            ..pm_shell::Adder::default()
        });
    }
    if words.contains(&"sample") {
        workspace.sample = true;
    }
    if words.contains(&"remote") {
        workspace
            .projects
            .extend(pm_shell::sample_remote_projects());
    }
}

/// Opens in `git` the Git panel's menu, question or Diff tab named among
/// `words`: `git-menu` for the menu on a changed file, `git-source` for the
/// panel's own menu, `git-confirm` for the question before discarding,
/// `branch-menu` for the branch list, `diff` for the Diff tab, `git-rebase`
/// for a rebase stopped with a partly staged and a conflicted file, and
/// `git-stashes` for the menu of stashes, and `git-submodules` for the
/// menu picking a submodule to work on.
fn show_git_state(git: &mut pm_shell::GitSummary, words: &[&str]) {
    let first = git.listed().first().map(|change| change.path.clone());
    let second = git.listed().get(1).map(|change| change.path.clone());
    for word in words {
        match *word {
            "git-menu" => git.menu = second.clone().map(pm_shell::GitMenu::Change),
            "git-source" => git.menu = Some(pm_shell::GitMenu::Source),
            "git-confirm" => {
                git.prompt = second
                    .clone()
                    .map(|path| pm_shell::GitPrompt::Discard(vec![path]))
            }
            "branch-menu" => git.branch_menu = Some(pm_shell::BranchMenu::default()),
            "diff" => git.diff = first.clone(),
            "git-rebase" => {
                git.operation = Some("Rebasing onto main (2/5)".to_owned());
                git.skippable = true;
                if let Some(change) = git.changes.first_mut() {
                    change.partly = true;
                    change.staged = false;
                }
                if let Some(change) = git.changes.last_mut() {
                    change.conflicted = true;
                    change.untracked = false;
                    change.status = 'U';
                }
            }
            "git-submodules" => {
                git.root = "~/Documents/repos/void".to_owned();
                git.submodules = vec![
                    ("vendor/tree-sitter".to_owned(), "master".to_owned()),
                    ("docs/site".to_owned(), "@ 4f2a9c1".to_owned()),
                ];
                git.menu = Some(pm_shell::GitMenu::Repositories);
            }
            "git-stashes" => {
                git.stashes = vec![
                    (0, "WIP on main: Keep the reader's position".to_owned()),
                    (1, "On main: try a smaller ranking window".to_owned()),
                ];
                git.menu = Some(pm_shell::GitMenu::Stashes);
            }
            _ => {}
        }
    }
}

/// Shows on the sample `workspace` the review states `words` name: notes
/// on the diff, a conflicted file, a commit's files, or search results as
/// editable excerpts, with the Rust and TOML grammars installed on this
/// machine loaded to colour them.
fn show_review(workspace: &mut pm_shell::Workspace, words: &[&str]) {
    let _ = pm_shell::installed_grammar("rust");
    let _ = pm_shell::installed_grammar("toml");
    for word in words {
        pm_shell::sample_review(&mut workspace.git, word);
    }
    if words.contains(&"search-edit") {
        workspace.right_tab = pm_shell::RightTab::Search;
        pm_shell::sample_search_edit(&mut workspace.search);
    }
}

/// Shows on the sample `workspace` what `words` name: the Files tab with
/// changes marked, its menu open on a file or the question before deleting
/// one, the Search tab, or the quick input on files, commands, sessions,
/// lines or themes.
fn show_panels(workspace: &mut pm_shell::Workspace, words: &[&str]) {
    let marked = ["files", "file-menu", "file-delete", "files-picked"];
    if words.iter().any(|word| marked.contains(word)) {
        workspace.right_tab = pm_shell::RightTab::Files;
        workspace.git.changes = pm_shell::sample_changes();
    }
    if words.contains(&"file-menu") {
        workspace.tree.menu = Some("crates/pm-shell/src/link.rs".to_owned());
    }
    if words.contains(&"files-picked") {
        workspace.tree.picked = [
            "crates/pm-shell/src/search.rs".to_owned(),
            "docs/protocol.md".to_owned(),
        ]
        .into();
    }
    if words.contains(&"file-delete") {
        workspace.tree.removing = Some("crates/pm-shell/src/link.rs".to_owned());
    }
    if words.contains(&"search") {
        workspace.right_tab = pm_shell::RightTab::Search;
    }
    let quick = words.iter().find_map(|word| match *word {
        "quick" => Some("files"),
        _ => word.strip_prefix("quick-"),
    });
    if let Some(kind) = quick {
        workspace.quick = Some(pm_shell::sample_quick(kind));
    }
}

/// Whether `word` is one a snapshot understands after its size.
fn known(word: &str) -> bool {
    word.starts_with("settings-")
        || WORDS.contains(&word)
        || crate::snapshot_appearance::known(word)
}

/// Draws the settings window on the section `shot` names, such as `editor-vim`, or
/// `editor-vim-narrow` for the narrowest window, into `path` at `size` or the
/// size the shot implies, in the light theme when `light`, with the section's
/// form open and filled in when `words` hold `form`, and the appearance
/// settings changed as the appearance words among `words` say.
fn settings_shot(
    path: &str,
    shot: &str,
    size: Option<(u32, u32)>,
    light: bool,
    words: &[&str],
) -> i32 {
    let (slug, narrow) = match shot.strip_suffix("-narrow") {
        Some(slug) => (slug, true),
        None => (shot, false),
    };
    let Some(section) = settings_section(slug) else {
        eprintln!("pandemonium: no settings section is called {slug}");
        return 2;
    };
    let size = size.unwrap_or(if narrow {
        NARROW_SIZE
    } else {
        (SETTINGS_SIZE.0 as u32, SETTINGS_SIZE.1 as u32)
    });
    let mut model = pm_shell::sample_settings(section);
    crate::snapshot_appearance::apply(&mut model, words);
    let theme = pm_pages::window_theme(&model, !light);
    if words.contains(&"form") {
        pm_shell::sample_forms(&mut model);
    }
    let actions = SettingsActions::inert();
    draw(
        move || settings_page(&model, theme, &actions),
        size,
        light,
        path,
    )
}

/// The workspace of the local server: its projects and sessions, with the
/// newest session opened and its conversation loaded, then the page named
/// among `words` shown with what the server lists for it, and the Files or
/// Search tab filled from the server when `words` name it.
fn live_workspace(words: &[&str]) -> pm_shell::Workspace {
    let mut link = pm_shell::Link::connect(std::sync::Arc::new(|| {}));
    pump_for(&mut link, LIVE_WAIT);
    if let Some(at) = newest_session(&link.workspace().projects, Vec::new()) {
        link.open_session(at);
        pump_for(&mut link, LIVE_WAIT);
    }
    if let Some(page) = page_named(words, link.workspace()) {
        link.navigate(page);
        pump_for(&mut link, LIVE_WAIT);
    }
    if words.contains(&"files") {
        link.choose_right_tab(pm_shell::RightTab::Files);
        pump_for(&mut link, LIVE_WAIT);
    }
    if words.contains(&"search") {
        link.choose_right_tab(pm_shell::RightTab::Search);
        link.search(pm_shell::SearchAction::Query("fn main".to_owned()));
        link.search(pm_shell::SearchAction::Run);
        pump_for(&mut link, LIVE_WAIT);
    }
    let workspace = link.workspace().clone();
    link.close_terminals();
    pump_for(&mut link, CLOSE_WAIT);
    workspace
}

/// Handles what the server sends for `duration`.
fn pump_for(link: &mut pm_shell::Link, duration: std::time::Duration) {
    let end = std::time::Instant::now() + duration;
    while std::time::Instant::now() < end {
        link.pump();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// The index path of the first session listed among `projects`, under `prefix`.
fn newest_session(projects: &[pm_shell::Project], prefix: Vec<usize>) -> Option<Vec<usize>> {
    projects.iter().enumerate().find_map(|(index, project)| {
        let mut here = prefix.clone();
        here.push(index);
        if project.sessions.is_empty() {
            newest_session(&project.children, here)
        } else {
            here.push(0);
            Some(here)
        }
    })
}

/// The page named among `words`, if one is: `history`, `extensions`, or
/// `overview` for the overview of the first project of `workspace` with a
/// session.
fn page_named(words: &[&str], workspace: &pm_shell::Workspace) -> Option<pm_shell::Page> {
    words.iter().find_map(|word| match *word {
        "history" => Some(pm_shell::Page::History),
        "extensions" => Some(pm_shell::Page::Extensions),
        "overview" => newest_session(&workspace.projects, Vec::new())
            .and_then(|at| workspace.projects.get(at[0]))
            .or(workspace.projects.first())
            .map(|project| pm_shell::Page::Overview(project.path.clone())),
        _ => None,
    })
}

/// What the sample agent waits on as a word among `words` names it: the
/// approve band, the inquiry card, the trust card, or the bar continuing a chat.
fn ask_named(words: &[&str]) -> Option<pm_shell::SampleAsk> {
    words.iter().find_map(|word| match *word {
        "approve" => Some(pm_shell::SampleAsk::Plan),
        "inquiry" => Some(pm_shell::SampleAsk::Inquiry),
        "trust" => Some(pm_shell::SampleAsk::Trust),
        "continue" => Some(pm_shell::SampleAsk::Continue),
        "signed-out" => Some(pm_shell::SampleAsk::SignIn),
        _ => None,
    })
}

/// The composer state named among `words`, if any.
fn composer_named(words: &[&str]) -> Option<pm_shell::SampleComposer> {
    words.iter().find_map(|word| match *word {
        "composer" => Some(pm_shell::SampleComposer::Plain),
        "composer-model" => Some(pm_shell::SampleComposer::ModelMenu),
        "composer-slash" => Some(pm_shell::SampleComposer::Slash),
        "composer-queued" => Some(pm_shell::SampleComposer::Queued),
        "composer-attached" => Some(pm_shell::SampleComposer::Attached),
        "composer-mention" => Some(pm_shell::SampleComposer::Mention),
        "composer-field" => Some(pm_shell::SampleComposer::Field),
        _ => None,
    })
}

/// The state of the browser named among `words`, if one is: `browser` for a painted page, or `browser-` and `find`, `menu`, `tools`, `ask`, `cert`, `clear`, `notice` or `pick`.
fn browser_named(words: &[&str]) -> Option<pm_shell::BrowserSample> {
    words.iter().find_map(|word| match *word {
        "browser" => Some(pm_shell::BrowserSample::Page),
        "browser-find" => Some(pm_shell::BrowserSample::Find),
        "browser-menu" => Some(pm_shell::BrowserSample::Menu),
        "browser-tools" => Some(pm_shell::BrowserSample::Tools),
        "browser-ask" => Some(pm_shell::BrowserSample::Ask),
        "browser-cert" => Some(pm_shell::BrowserSample::Certificate),
        "browser-clear" => Some(pm_shell::BrowserSample::Clear),
        "browser-notice" => Some(pm_shell::BrowserSample::Notice),
        "browser-pick" => Some(pm_shell::BrowserSample::Pick),
        _ => None,
    })
}

/// The dock tab named among `words`, if one is: `problems`, `tasks` or `debug`.
fn dock_tab_named(words: &[&str]) -> Option<pm_shell::DockTab> {
    words.iter().find_map(|word| match *word {
        "problems" => Some(pm_shell::DockTab::Problems),
        "tasks" => Some(pm_shell::DockTab::Tasks),
        "debug" => Some(pm_shell::DockTab::Debug),
        _ => None,
    })
}

/// The Rust file the `code` snapshot shows open.
const CODE_SAMPLE: &str = include_str!("window.rs");

/// The manifest the `code` snapshot shows in the preview tab.
const MANIFEST_SAMPLE: &str = include_str!("../Cargo.toml");

/// Opens a Rust file in a tab of its own, highlighted with the Rust grammar
/// installed on this machine when there is one, with a manifest in the
/// preview tab beside it, and shows the Rust file.
fn open_sample_files(files: &mut pm_shell::OpenFiles) {
    let _ = pm_shell::installed_grammar("rust");
    let _ = pm_shell::installed_grammar("toml");
    let folder = "~/Documents/repos/pandemonium";
    files.open(folder, "crates/pandemonium/src/window.rs", true);
    files.files[0].take_text(CODE_SAMPLE, false, Some("rust".to_owned()));
    files.open(folder, "crates/pandemonium/Cargo.toml", false);
    files.files[1].take_text(MANIFEST_SAMPLE, false, Some("toml".to_owned()));
    files.open(folder, "crates/pandemonium/src/window.rs", false);
    let caret = CODE_SAMPLE.find("fn root").unwrap_or(0);
    files.files[0]
        .document
        .0
        .borrow_mut()
        .edit(pm_text::Edit::Select(caret + 3, caret + 7));
}

/// The README the `code-markdown` snapshot shows rendered.
const README_SAMPLE: &str = include_str!("../../../README.md");

/// The picture the `code-picture` snapshot shows.
const PICTURE_SAMPLE: &[u8] = include_bytes!("../../../assets/brand/png/pandemonium-256.png");

/// Shows on the sample files of `workspace` what `words` name: the find
/// bar, the find bar with its replace row, the go-to-line bar, vim's keys,
/// breakpoints with inline values and lines changed since the last commit,
/// the view scrolled into a block so its opening lines stick, the README
/// rendered, or a picture.
fn show_code_state(workspace: &mut pm_shell::Workspace, words: &[&str]) {
    let files = &mut workspace.open_files;
    for word in words {
        match *word {
            "code-find" => {
                files.files[0].bar = Some(pm_shell::EditorBar::Find("palette".to_owned()))
            }
            "code-replace" => {
                files.files[0].bar = Some(pm_shell::EditorBar::Replace("palette".to_owned()))
            }
            "code-goto" => files.files[0].bar = Some(pm_shell::EditorBar::GoTo("120".to_owned())),
            "code-vim" => files.vim = true,
            "code-problems" => show_problems(&files.files[0]),
            "code-folded" => {
                files.files[0].document.0.borrow_mut().fold_at_caret();
            }
            "code-unsaved" => {
                files.files[0].document.0.borrow_mut().mark_unsaved();
                files.act(pm_shell::TabAction::Close(0));
            }
            "code-scrolled" | "code-debug" => {
                let line = CODE_SAMPLE[..CODE_SAMPLE.find("drag_resize_window(edge)").unwrap_or(0)]
                    .lines()
                    .count();
                files.files[0].go_to(line + 1);
            }
            _ => {}
        }
    }
    if words.contains(&"code-debug") {
        let base = CODE_SAMPLE
            .replacen(
                "let modifier = match height",
                "let modifier = match size",
                1,
            )
            .replacen("                .cursor(edge.cursor())\n", "", 1)
            .replacen(
                "/// How the window changes",
                "// removed\n/// How the window changes",
                1,
            );
        files.files[0].take_base(Some(base));
        let caret = files.files[0].document.0.borrow().field.caret();
        let line = files.files[0].document.0.borrow().line_of(caret);
        let path = files.files[0].path.clone();
        workspace.debug.breakpoints = [line.saturating_sub(4), line]
            .into_iter()
            .map(|line| pm_shell::DebugBreakpoint {
                path: path.clone(),
                line,
                verified: Some(true),
            })
            .collect();
        if let Some(session) = &mut workspace.debug.session {
            session.inline_values = vec![pm_shell::InlineValue {
                path: format!("/home/me/pandemonium/{path}"),
                line,
                text: "event = KeyEvent { key: Enter }, handled = false".to_owned(),
            }];
        }
    }
    let files = &mut workspace.open_files;
    let folder = "~/Documents/repos/pandemonium";
    if words.contains(&"code-markdown")
        && let Some(index) = files.open(folder, "README.md", true)
    {
        files.files[index].take_text(README_SAMPLE, false, Some("markdown".to_owned()));
        files.files[index].rendered = true;
        files.tab = pm_shell::SessionTab::File(index);
    }
    show_language_samples(workspace, words);
    let files = &mut workspace.open_files;
    if words.contains(&"code-picture")
        && let Some(index) = files.open(folder, "assets/brand/png/pandemonium-256.png", true)
    {
        files.files[index].body =
            pm_shell::FileBody::Picture(std::rc::Rc::new(PICTURE_SAMPLE.to_vec()));
        files.tab = pm_shell::SessionTab::File(index);
    }
}

/// Marks problems on the sample `file` as a language server would: an
/// error, a warning and a hint on the lines around its caret.
fn show_problems(file: &pm_shell::OpenFile) {
    let mut document = file.document.0.borrow_mut();
    let caret = document.field.caret();
    let line = document.line_of(caret) as u32;
    let found = |line: u32, from: u32, to: u32, severity, message: &str| pm_editor::Diagnostic {
        span: pm_editor::Span::new(
            pm_editor::Position {
                line,
                character: from,
            },
            pm_editor::Position {
                line,
                character: to,
            },
        ),
        severity,
        message: message.to_owned(),
        source: Some("rustc".to_owned()),
    };
    document.diagnostics = vec![
        found(
            line,
            3,
            7,
            pm_editor::Severity::Error,
            "cannot find value `root`",
        ),
        found(
            line + 2,
            8,
            20,
            pm_editor::Severity::Warning,
            "unused variable",
        ),
        found(
            line + 4,
            4,
            9,
            pm_editor::Severity::Hint,
            "consider borrowing",
        ),
    ];
}

/// Shows on the sample files of `workspace` the language features, panes
/// and menus `words` name: completions, a hover card, a signature, names
/// coloured with hints and lenses, the lightbulb's menu, renaming, text
/// dragged, a colour picker, right-to-left text, the offer of a missing
/// language, a comparison, the file's menu, a tab's menu, the tabs stepped
/// through forward twice and back once, and split panes.
fn show_language_samples(workspace: &mut pm_shell::Workspace, words: &[&str]) {
    let folder = "~/Documents/repos/pandemonium";
    let samples = [
        ("code-completions", pm_shell::CodeSample::Completions),
        ("code-hover", pm_shell::CodeSample::Hover),
        ("code-signature", pm_shell::CodeSample::Signature),
        ("code-annotations", pm_shell::CodeSample::Annotations),
        ("code-actions", pm_shell::CodeSample::Actions),
        ("code-rename", pm_shell::CodeSample::Rename),
        ("code-drag", pm_shell::CodeSample::Drag),
    ];
    for (word, sample) in samples {
        if words.contains(&word) {
            pm_shell::sample_code(&mut workspace.open_files.files[0], sample);
        }
    }
    let files = &mut workspace.open_files;
    if words.contains(&"code-colour")
        && let Some(index) = files.open(folder, "site/theme.css", true)
    {
        let _ = pm_shell::installed_grammar("css");
        files.files[index].take_text(pm_shell::COLOUR_SAMPLE, false, Some("css".to_owned()));
        pm_shell::sample_code(&mut files.files[index], pm_shell::CodeSample::Colour);
    }
    if words.contains(&"code-rtl")
        && let Some(index) = files.open(folder, "locales/greetings.toml", true)
    {
        files.files[index].take_text(
            pm_shell::RIGHT_TO_LEFT_SAMPLE,
            false,
            Some("toml".to_owned()),
        );
        let caret = pm_shell::RIGHT_TO_LEFT_SAMPLE.find("עולם").unwrap_or(0);
        files.files[index]
            .document
            .0
            .borrow_mut()
            .edit(pm_text::Edit::Select(caret, caret + "עולם".len()));
    }
    if words.contains(&"code-language")
        && let Some(index) = files.open(folder, "legacy/telemetry.adb", true)
    {
        files.files[index].take_text(
            "with Ada.Text_IO; use Ada.Text_IO;\n\nprocedure Telemetry is\nbegin\n   Put_Line (\"Sending the session's figures\");\nend Telemetry;\n",
            false,
            None,
        );
        workspace.extensions = Some(pm_shell::sample_workspace().extensions.unwrap_or_default());
    }
    let files = &mut workspace.open_files;
    if words.contains(&"code-compare") {
        let text = files.files[0].document.0.borrow().field.text().to_owned();
        let older = text
            .replacen(
                "let installed = remember(|| false);",
                "let installed = remember(|| true);",
                1,
            )
            .replacen("    let _ = revision.get();\n", "", 1);
        files.files[0].compare = Some(pm_shell::Comparison::between("Saved", &older, &text));
    }
    if words.contains(&"code-file-menu") {
        files.menu = Some(pm_shell::TabMenu::File(0));
    }
    if words.contains(&"tab-menu") {
        files.menu = Some(pm_shell::TabMenu::Tab(0));
    }
    if words.contains(&"tab-steps") {
        for step in [
            pm_shell::TabAction::Next,
            pm_shell::TabAction::Next,
            pm_shell::TabAction::Previous,
        ] {
            files.act(step);
        }
    }
    if words.contains(&"split") {
        files.act(pm_shell::TabAction::MoveTo(
            pm_shell::SessionTab::File(1),
            pm_shell::Pane::Right,
        ));
    }
    if words.contains(&"split-wide") {
        files.act(pm_shell::TabAction::Resize(650));
    }
    if words.contains(&"tab-pinned") {
        let last = files.files.len().saturating_sub(1);
        files.act(pm_shell::TabAction::TogglePin(last));
    }
    if words.contains(&"tab-many") {
        for index in 0..12 {
            files.open("~/sample", &format!("src/module_{index}.rs"), true);
        }
    }
}

/// The width and height in `text`, written as `WIDTHxHEIGHT`.
fn parse_size(text: &str) -> Option<(u32, u32)> {
    let (width, height) = text.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}
