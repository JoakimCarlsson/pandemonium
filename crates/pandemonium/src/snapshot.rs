//! Drawing one frame to a PNG without opening a window.

use std::path::Path;

use pm_acp::{
    Background, Choice, Diff, Discovery, Event, Kind, Permission, Pick, PlanStep, Progress,
    Setting, Slash, Tool, ToolState, Verdict,
};
use pm_components::Project;
use pm_components::state::{DETAILS_BASE, RESTORE_BASE, SUBAGENTS_CHIP};
use pm_input::{FocusId, PointerState};
use pm_pages::{
    HISTORY, OnboardingState, PROMPT, SEND, Screen, Step, TAB_DIFF, TERMINAL, WorkspaceState,
};
use pm_renderer::{DrawList, Renderer};
use pm_text::Edit;
use pm_theme::{Appearance, default_family};
use pm_ui::Input;

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
    /// The workspace with every project folded, showing how many sessions each has.
    Folded,
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
    /// A conversation whose turn ended while two subagents and a command were still running.
    Background,
    /// The same conversation with the subagents above the prompt box open.
    Agents,
    /// A conversation whose agent is on a turn with prompts queued behind it and another being written.
    Queued,
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
}

/// The workspace of made-up sessions.
fn workspace_sample() -> WorkspaceState {
    WorkspaceState::sample()
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
            Self::Workspace => Screen::Workspace(workspace()),
            Self::Unfolded => Screen::Workspace(workspace().after(FocusId(DETAILS_BASE + 3000))),
            Self::Diff => Screen::Workspace(workspace().after(TAB_DIFF)),
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
            Self::Folded => Screen::Workspace(
                (1000..1004).fold(workspace(), |state, id| state.after(FocusId(id))),
            ),
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
                    .after(FocusId(331))
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
            Self::Background => Screen::Workspace(live(Turn::Background)),
            Self::Agents => {
                Screen::Workspace(live(Turn::Background).after(FocusId(SUBAGENTS_CHIP.0)))
            }
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
            Self::Slashing => {
                Screen::Workspace(welcome().edited(PROMPT, Edit::Paste("/c".to_owned())))
            }
        }
    }
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
