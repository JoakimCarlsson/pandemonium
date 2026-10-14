//! Drawing one frame to a PNG without opening a window.

use std::path::Path;

use pm_acp::{
    Choice, Diff, Discovery, Event, Kind, Permission, Pick, PlanStep, Progress, Setting, Slash,
    Tool, ToolState, Verdict,
};
use pm_components::Project;
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
    /// A conversation with the model menu open.
    Picking,
    /// A conversation with the details of its steps open.
    Expanded,
    /// A conversation with a slash being typed.
    Slashing,
    /// The page for starting work with an `@` being typed that no file fits.
    Missing,
    /// The page for starting work with files attached.
    Attached,
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
        let workspace = WorkspaceState::sample;
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
            Self::Diff => Screen::Workspace(workspace().after(TAB_DIFF)),
            Self::Terminal => Screen::Workspace(workspace().after(TERMINAL)),
            Self::New => Screen::Workspace(WorkspaceState::new(&projects())),
            Self::Empty => Screen::Workspace(WorkspaceState::new(&[])),
            Self::History => Screen::Workspace(workspace().after(HISTORY)),
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
            Self::Live => Screen::Workspace(live(false)),
            Self::Asking => Screen::Workspace(live(true)),
            Self::Picking => Screen::Workspace(welcome().after(FocusId(4200))),
            Self::Expanded => Screen::Workspace(
                [201_000, 201_004, 201_005]
                    .into_iter()
                    .fold(live(false), |state, id| state.after(FocusId(id))),
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

/// A conversation that went through what an agent says in a turn, with `asking` for the last of it left open.
fn live(asking: bool) -> WorkspaceState {
    let tool = |id: &str, title: &str, kind, target: &str| Tool {
        id: id.to_owned(),
        title: Some(title.to_owned()),
        kind: Some(kind),
        state: Some(ToolState::Done),
        target: Some(target.to_owned()),
        diff: None,
        output: None,
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
    if asking {
        events.push(Event::Ask(Permission {
            ticket: 1,
            title: "Run cargo clippy".to_owned(),
            command: Some("cargo clippy --workspace".to_owned()),
            choices: vec![
                Choice {
                    id: "a".to_owned(),
                    name: "Allow once".to_owned(),
                    verdict: Verdict::Allow,
                },
                Choice {
                    id: "b".to_owned(),
                    name: "Always allow".to_owned(),
                    verdict: Verdict::Always,
                },
                Choice {
                    id: "c".to_owned(),
                    name: "Deny".to_owned(),
                    verdict: Verdict::Reject,
                },
            ],
        }));
    } else {
        events.push(Event::Finished(None));
    }
    for event in events {
        state = state.heard(0, event);
    }
    state
}

/// Draws the screen as it looks `elapsed_ms` into the animations, with the
/// pointer at `pointer` and focus on `focused` if given, in `appearance`, and
/// writes it to `path` as a PNG.
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
    scene::paint(
        WIDTH as f32,
        HEIGHT as f32,
        &shot.screen(),
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
