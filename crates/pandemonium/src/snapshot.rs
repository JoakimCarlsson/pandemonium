//! Drawing one frame to a PNG without opening a window.

use std::path::Path;

use pm_acp::{
    Alternative, Background, Choice, Diff, Discovery, Event, Inquiry, Kind, Permission, Pick,
    PlanStep, Progress, Question, Setting, Slash, Tool, ToolState, Verdict,
};
use pm_components::Project;
use pm_components::adder::{ADDER_BROWSE, ADDER_CLONE, ADDER_ROW_BASE, Found, Kind as Row};
use pm_components::diff::gutter_id;
use pm_components::git_controls::{BRANCH_BUTTON, BRANCH_FILTER, Pick as Choosing};
use pm_components::git_menus::MenuKind;
use pm_components::git_prompts::Prompt;
use pm_components::open_files::EDITOR_FIELD;
use pm_components::settings::{SETTINGS_ADD, SETTINGS_SECTION_BASE};
use pm_components::state::SETTINGS;
use pm_components::state::{
    AGENT_BASE, CHAT_TEXT, COMMENT_ADD, COMMENT_FIELD, COMMIT_MESSAGE, DETAILS_BASE, FILE_BASE,
    RESTORE_BASE, REVIEW_FILES, STAGE_BASE, SUBAGENTS_CHIP,
};
use pm_components::tree_actions::{TREE_NEW_FOLDER, TreeTarget};
use pm_input::{FocusId, PointerState};
use pm_pages::{
    ADD_PROJECT, HISTORY, OnboardingState, PROMPT, SEND, Screen, Step, TAB_DIFF, TERMINAL,
    WorkspaceState,
};
use pm_renderer::{DrawList, Renderer};
use pm_text::Edit;
use pm_theme::{Appearance, default_family};
use pm_ui::{Input, TextPoint, TextSelection};

use crate::scene;

/// Logical width of the snapshot.
const WIDTH: u32 = 1280;

/// Logical height of the snapshot.
const HEIGHT: u32 = 800;

/// Which screen a snapshot shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shot {
    /// A step of onboarding.
    Onboarding(Step),
    /// The workspace on its first session.
    Workspace,
    /// The workspace on the diff of its first session.
    Diff,
    /// The workspace on the diff of its first session with a comment kept and another being written.
    Commenting,
    /// The workspace on the diff of its first session with the project's files listed in the review panel.
    Files,
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
    /// The workspace of the projects onboarding added, with no sessions.
    New,
    /// The workspace with no project at all.
    Empty,
    /// The workspace on the history page.
    History,
    /// The workspace with the menu of a project open.
    ProjectMenu,
    /// The workspace with a project dragged over another.
    Dragging,
    /// The page for starting work in this repository, with the agent's settings known.
    Welcome,
    /// The page for starting work with a long prompt that wraps and has a line break.
    Wrapped,
    /// The page for starting work after a command was picked from the list behind a slash.
    Slashed,
    /// The page for starting work with Codex and a dollar sign being typed.
    Dollar,
    /// The page for starting work with the list of models scrolled to its end.
    Scrolling,
    /// The page for starting work with the menu of where it is done open.
    Placing,
    /// A conversation fed the events an agent produces, finished.
    Live,
    /// A conversation whose agent waits for permission.
    Asking,
    /// A conversation whose agent waits for the answers to a form of three questions.
    Inquiring,
    /// A conversation whose reader answered a form of questions.
    Answered,
    /// A conversation whose turn ended while two subagents and a command were still running.
    Background,
    /// The same conversation with the subagents above the prompt box open.
    Agents,
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
    /// The same selection with the menu over a reply open.
    MessageMenu,
    /// The page for starting work with the panel for adding a project open on its choices.
    Adding,
    /// The workspace with the settings panel open.
    Settings,
    /// The settings panel on Agents, which opens the sections inside it.
    SettingsAgents,
    /// The settings panel on the MCP servers inside Agents, with one installed, the registry's offers and the menu of the installed one open.
    SettingsMcp,
    /// The settings panel on the Agent Servers inside Agents, with the form for adding one open.
    SettingsForm,
    /// The same panel waiting for the accounts and organizations to clone from.
    Loading,
    /// The same panel on the accounts and organizations to clone from.
    Owning,
    /// The same panel on the repositories of one of them.
    Cloning,
    /// The same panel on the folders of the machine, to open one as a project.
    Folders,
}

/// The workspace of made-up sessions.
fn workspace_sample() -> WorkspaceState {
    WorkspaceState::sample()
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
        "origin/main",
        "origin/feat/editor",
    ];
    workspace_sample()
        .after(TAB_DIFF)
        .after(BRANCH_BUTTON)
        .with_session_branches(0, names.map(str::to_owned).to_vec())
        .edited(BRANCH_FILTER, Edit::Paste("feat".to_owned()))
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
        Ok((Some(text.to_owned()), false)),
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
        Ok((Some(text.to_owned()), false)),
    );
    let mut state = state.edited(EDITOR_FIELD, Edit::Paste("// edited\n".to_owned()));
    state.open_viewer("crates/pandemonium/src/lib.rs".to_owned(), false);
    state.with_file(
        0,
        "crates/pandemonium/src/lib.rs",
        false,
        Ok((Some("pub mod session;\n".to_owned()), false)),
    )
}

/// The page for starting work with a file of the project open beside it.
fn new_file() -> WorkspaceState {
    let mut state = workspace_sample().after(FocusId(pm_components::state::PROJECT_BASE));
    state.open_viewer("src/main.rs".to_owned(), false);
    let session = state.review_index().unwrap_or_default();
    state.with_file(
        session,
        "src/main.rs",
        false,
        Ok((
            Some("fn main() {\n    println!(\"hi\");\n}\n".to_owned()),
            false,
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
    ];
    state.with_files(
        &key,
        paths.map(str::to_owned).to_vec(),
        vec!["crates/pandemonium/src/empty".to_owned()],
    )
}

/// The workspace with a shell open that has run a few commands, as the shell would have drawn them.
fn terminal() -> WorkspaceState {
    let mut state = workspace_sample().after(TERMINAL);
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

/// The projects a first launch has after onboarding.
fn projects() -> Vec<Project> {
    ["~/code/pandemonium", "~/code/voxel"]
        .into_iter()
        .filter_map(Project::from_source)
        .collect()
}

impl Shot {
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
            Self::Onboarding(step) => Screen::Onboarding(OnboardingState {
                step,
                projects: if step == Step::Project {
                    ["pandemonium", "voxel"]
                        .into_iter()
                        .filter_map(Project::from_source)
                        .collect()
                } else {
                    Vec::new()
                },
                ..OnboardingState::default()
            }),
            Self::Workspace | Self::Selecting => Screen::Workspace(workspace()),
            Self::MessageMenu => {
                Screen::Workspace(workspace().with_message_menu(3, (520.0, 230.0), true))
            }
            Self::Unfolded => Screen::Workspace(workspace().after(FocusId(DETAILS_BASE + 3000))),
            Self::Diff => Screen::Workspace(workspace().after(TAB_DIFF)),
            Self::Commenting => Screen::Workspace(commenting()),
            Self::Files => Screen::Workspace(files()),
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
            Self::Naming => Screen::Workspace(files().after(TREE_NEW_FOLDER)),
            Self::Discarding => Screen::Workspace(picked().asked(Prompt::Discard(vec![0, 2]))),
            Self::Branching => Screen::Workspace(branching()),
            Self::Reading => Screen::Workspace(reading()),
            Self::Editing => Screen::Workspace(editing()),
            Self::NewFile => Screen::Workspace(new_file()),
            Self::Terminal => Screen::Workspace(terminal()),
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
            Self::Empty => Screen::Workspace(WorkspaceState::new(&[])),
            Self::History => Screen::Workspace(workspace().after(HISTORY)),
            Self::ProjectMenu => {
                Screen::Workspace(workspace().with_project_menu(1, (236.0, 250.0)))
            }
            Self::Dragging => Screen::Workspace(workspace().dragged_over(0, 2, (150.0, 470.0))),
            Self::Welcome => Screen::Workspace(welcome()),
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
            Self::Dollar => Screen::Workspace(
                welcome()
                    .after(FocusId(AGENT_BASE + 1))
                    .edited(PROMPT, Edit::Paste("$r".to_owned())),
            ),
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
            Self::Background => Screen::Workspace(live(Turn::Background)),
            Self::Agents => {
                Screen::Workspace(live(Turn::Background).after(FocusId(SUBAGENTS_CHIP.0)))
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
            Self::Expanded => Screen::Workspace(
                [201_000, 201_004, 201_005]
                    .into_iter()
                    .fold(live(Turn::Finished), |state, id| state.after(FocusId(id))),
            ),
            Self::Attached => Screen::Workspace(
                welcome()
                    .with_attachments(&["/home/me/shot.png".to_owned(), "/home/me/notes.md".to_owned()])
                    .with_preview("/home/me/shot.png", gradient()),
            ),
            Self::Missing => {
                Screen::Workspace(welcome().edited(PROMPT, Edit::Paste("@zzzzqq".to_owned())))
            }
            Self::Settings => Screen::Workspace(welcome().after(SETTINGS)),
            Self::SettingsAgents => Screen::Workspace(
                welcome()
                    .after(SETTINGS)
                    .after(FocusId(SETTINGS_SECTION_BASE + 1)),
            ),
            Self::SettingsMcp => Screen::Workspace(
                welcome()
                    .after(SETTINGS)
                    .after(FocusId(SETTINGS_SECTION_BASE + 3))
                    .with_mcp_servers(vec![pm_protocol::McpServer {
                        name: "context7".to_owned(),
                        target: "https://mcp.context7.com/mcp".to_owned(),
                        env: vec![("API_KEY".to_owned(), "abc".to_owned())],
                        description: "Up-to-date code docs for any prompt".to_owned(),
                        website: "https://context7.com".to_owned(),
                        enabled: true,
                    }])
                    .with_tool_listings(
                        "",
                        Ok(vec![
                            tool_listing("Context7", "context7", "https://mcp.context7.com/mcp"),
                            tool_listing("GitHub", "github", "https://api.githubcopilot.com/mcp/"),
                        ]),
                    )
                    .with_settings_menu(0, (800.0, 380.0)),
            ),
            Self::SettingsForm => Screen::Workspace(
                welcome()
                    .after(SETTINGS)
                    .after(FocusId(SETTINGS_SECTION_BASE + 2))
                    .after(SETTINGS_ADD),
            ),
            Self::Adding => Screen::Workspace(welcome().after(ADD_PROJECT)),
            Self::Loading => Screen::Workspace(welcome().after(ADD_PROJECT).after(ADDER_CLONE)),
            Self::Owning => Screen::Workspace(
                welcome()
                    .after(ADD_PROJECT)
                    .after(ADDER_CLONE)
                    .with_owners(Ok(owners())),
            ),
            Self::Folders => Screen::Workspace(welcome().after(ADD_PROJECT).after(ADDER_BROWSE).with_folders(
                "~",
                Ok((
                    "/home/me/code".to_owned(),
                    ["pandemonium", "voxel", "wasa-cli", "world"]
                        .map(str::to_owned)
                        .to_vec(),
                )),
            )),
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
    [
        waiting(1, "Then add a test for the clamp", &[]),
        waiting(
            2,
            "Use the layout in this screenshot for the empty state\nand keep the icon",
            &["empty.png"],
        ),
    ]
    .into_iter()
    .fold(live(Turn::Working), |state, event| state.heard(0, event))
    .edited(
        PROMPT,
        Edit::Paste("Stop, the clamp lives in pm-ui".to_owned()),
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
    } else if turn == Turn::Inquiring {
        let alternative = |label: &str, description: &str| Alternative {
            label: label.to_owned(),
            description: description.to_owned(),
        };
        events.push(Event::Inquire(Inquiry {
            ticket: 1,
            questions: vec![
                Question {
                    header: "Themes".to_owned(),
                    text: "Which themes should the system setting apply to?".to_owned(),
                    multiple: false,
                    alternatives: vec![
                        alternative(
                            "Light and dark only",
                            "Follow the operating system between the two built-in themes.",
                        ),
                        alternative(
                            "Every theme",
                            "Let each custom theme declare a light and a dark variant.",
                        ),
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
    for event in events {
        state = state.heard(0, event);
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
    let mut renderer = Renderer::headless(WIDTH, HEIGHT, 1.0).map_err(|error| error.to_string())?;
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
    };
    let family = default_family();
    let theme = family.variant(appearance);
    let screen = shot.screen();
    let resting = Input {
        elapsed_ms: 0.0,
        ..input
    };
    scene::paint(
        WIDTH as f32,
        HEIGHT as f32,
        &screen,
        &resting,
        theme,
        &mut shaper,
        &mut DrawList::new(),
    );
    scene::paint(
        WIDTH as f32,
        HEIGHT as f32,
        &screen,
        &input,
        theme,
        &mut shaper,
        &mut list,
    );
    let pixels = renderer
        .snapshot(&list, &mut shaper, scene::ground(theme))
        .ok_or("the renderer could not read its frame back")?;
    image::save_buffer(path, &pixels, WIDTH, HEIGHT, image::ColorType::Rgba8)
        .map_err(|error| error.to_string())
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
