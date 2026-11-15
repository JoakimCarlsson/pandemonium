//! The workspace the snapshots render: the projects, conversation and panels
//! of the reference screenshot, without a server.

use crate::dialog::{SignInCode, UpdateOffer, UpdateRun};
use crate::dock_sample::{sample_debug, sample_problems, sample_tasks, sample_terminals};
use crate::extensions::{Extension, ExtensionAction, Tone};
use crate::git_sample::sample_git;
use crate::history::{PastSession, Span};
use crate::model::{
    Commit, DockTab, PanelSizes, Project, RightTab, Session, SessionStatus, Workspace,
};
use crate::open_files::OpenFiles;
use crate::page::{Page, Trail};
use crate::sample_conversation::{conversation, extras};
use crate::start::offered_agents;
use crate::terminal::TerminalScreen;

/// A project named `name` at `~/repos/<name>` with `sessions` and `children`.
fn project(name: &str, sessions: Vec<Session>, children: Vec<Project>) -> Project {
    Project {
        name: name.to_owned(),
        path: format!("~/repos/{name}"),
        children,
        sessions,
        collapsed: false,
        reach: None,
    }
}

/// An idle session titled `title`, last changed `age`.
fn session(id: u64, title: &str, age: &str) -> Session {
    Session {
        id,
        title: title.to_owned(),
        age: age.to_owned(),
        status: SessionStatus::Idle,
        branch: "main".to_owned(),
        directory: format!("~/repos/sample-{id}"),
    }
}

/// The sample workspace: five projects, one open session with a long agent
/// answer, the git panel on a clean `main`, and a terminal in the dock.
pub fn sample_workspace() -> Workspace {
    Workspace {
        projects: vec![
            project("vault", vec![], vec![]),
            project("minmux", vec![], vec![]),
            project(
                "harness",
                vec![],
                vec![
                    project("oh-my-crab", vec![], vec![]),
                    project("oh-my-pi", vec![], vec![]),
                    project(
                        "pandemonium",
                        vec![session(7, "Make install and uninstall t…", "4d ago")],
                        vec![],
                    ),
                    project("pi", vec![], vec![]),
                    project("thinkrail", vec![], vec![]),
                ],
            ),
            project(
                "void",
                vec![session(9, "Reader session continuation", "4d ago")],
                vec![],
            ),
        ],
        open: Some(vec![3, 0]),
        title: "Reader session continuation".to_owned(),
        branch: "main".to_owned(),
        agent: "Claude (MinavilkorxAlster) · Opus 5.5".to_owned(),
        model: "Opus 5.5".to_owned(),
        permissions: "Bypass permissions".to_owned(),
        account: "MinavilkorxAlster".to_owned(),
        conversation: conversation(),
        extras: extras(),
        draft: String::new(),
        git: sample_git(),
        sidebar_open: true,
        right_tab: RightTab::Git,
        right_open: true,
        dock_tab: DockTab::Terminal,
        dock_open: true,
        terminal: TerminalScreen::printed(
            b"\x1b[1;32m[jdaddy@jdaddy-systemproductname void]$\x1b[0m ",
            120,
            8,
        ),
        terminals: sample_terminals(),
        terminal_shown: Some(1),
        problems: sample_problems(),
        tasks: sample_tasks(),
        debug: sample_debug(),
        status: None,
        starting: None,
        agents: offered_agents(),
        last_agent: None,
        page: Page::Session(9),
        trail: Trail::default(),
        history: history(),
        extensions: Some(extensions()),
        commits: commits(),
        sizes: PanelSizes::default(),
        update: None,
        sign_in: None,
        open_files: OpenFiles::default(),
        tree: crate::sample_files::sample_files(),
        search: crate::sample_files::sample_search(),
        quick: None,
        browser: None,
        shelf: Default::default(),
        chats: vec![
            session(14, "What does a Wayland compositor do?", "1h ago"),
            session(13, "Compare tokio and smol", "2d ago"),
        ],
        project_menu: None,
        session_menu: None,
        renaming: None,
        adder: None,
        picker: None,
        sample: false,
    }
}

/// The made-up projects the sidebar shows before the reader adds a real one.
pub fn sample_projects() -> Vec<Project> {
    sample_workspace().projects
}

/// A history row for session `id` titled `title` in `project`.
fn past(id: u64, title: &str, project: &str, branch: &str, age: &str, span: Span) -> PastSession {
    PastSession {
        id,
        title: title.to_owned(),
        agent: "claude".to_owned(),
        project: project.to_owned(),
        machine: "local".to_owned(),
        branch: branch.to_owned(),
        age: age.to_owned(),
        span,
        closed: id != 7 && id != 9,
    }
}

/// The sample history: the open sessions and a few closed ones.
fn history() -> Vec<PastSession> {
    vec![
        past(
            12,
            "Port the history page to Rustle",
            "pandemonium",
            "history",
            "2h ago",
            Span::Today,
        ),
        past(
            11,
            "Fix the dock's terminal scrollback",
            "pandemonium",
            "dock-scroll",
            "5h ago",
            Span::Today,
        ),
        past(
            10,
            "Sketch the evidence contract",
            "void",
            "contract",
            "1d ago",
            Span::Yesterday,
        ),
        past(
            9,
            "Reader session continuation",
            "void",
            "main",
            "4d ago",
            Span::Earlier,
        ),
        past(
            7,
            "Make install and uninstall t…",
            "pandemonium",
            "main",
            "4d ago",
            Span::Earlier,
        ),
        past(
            5,
            "Benchmark the vault index",
            "vault",
            "bench",
            "9d ago",
            Span::Earlier,
        ),
    ]
}

/// An extension row named `name`.
fn extension(name: &str, about: &str, installed: bool, standing: &str, tone: Tone) -> Extension {
    Extension {
        id: name.to_lowercase(),
        name: name.to_owned(),
        version: if installed {
            "0.3.1".to_owned()
        } else {
            String::new()
        },
        about: about.to_owned(),
        provides: Vec::new(),
        owner: None,
        standing: standing.to_owned(),
        tone,
        installed,
        package: false,
        local: false,
        action: if installed {
            ExtensionAction::Uninstall
        } else {
            ExtensionAction::Install
        },
        suffixes: about
            .split_whitespace()
            .filter_map(|word| word.strip_prefix('.'))
            .map(str::to_owned)
            .collect(),
        names: Vec::new(),
    }
}

/// The sample extensions: a few installed, a few available.
fn extensions() -> Vec<Extension> {
    let mut hello = extension(
        "Hello",
        "A sample extension package",
        true,
        "",
        Tone::Subtle,
    );
    hello.package = true;
    hello.local = true;
    hello.provides =
        vec!["theme: Hello Dusk · icon theme: Hello Icons · command: /hello".to_owned()];
    let mut go = extension(
        "Go",
        "Go  .go",
        true,
        "Go update: gopls v0.18.1",
        Tone::Accent,
    );
    go.action = ExtensionAction::Update;
    vec![
        go,
        hello,
        extension(
            "Rust",
            "Rust  .rs",
            true,
            "rust-analyzer 2026-11-10",
            Tone::Muted,
        ),
        extension("Ada", "Ada  .adb .ads", false, "", Tone::Subtle),
        extension("Elixir", "Elixir  .ex .exs", false, "", Tone::Subtle),
        extension("Zig", "Zig  .zig .zon", false, "", Tone::Subtle),
    ]
}

/// The sample overview's recent commits.
fn commits() -> Vec<Commit> {
    [
        (
            "29b70e6",
            "Add a Models settings page for per-provider model slots",
        ),
        (
            "bb9ef9e",
            "Add a Model Servers settings page and setup hints",
        ),
        (
            "97c8327",
            "Start agent chats without a project, in a scratch folder",
        ),
        (
            "191645c",
            "Scroll overflowing history sessions and reset on search changes",
        ),
    ]
    .into_iter()
    .map(|(id, summary)| Commit {
        id: id.to_owned(),
        summary: summary.to_owned(),
    })
    .collect()
}

/// A newer release offered with its dialog open, for snapshots.
pub fn sample_update() -> UpdateOffer {
    UpdateOffer {
        name: "Pandemonium 0.16.0".to_owned(),
        version: "0.16.0".to_owned(),
        url: "https://github.com/JoakimCarlsson/pandemonium/releases/tag/v0.16.0".to_owned(),
        published: Some("2026-11-12".to_owned()),
        notes: Some(
            "## What's new\n\n- Resizable panels that remember their sizes\n- The window reopens on the page it was left on\n- Light themes follow the desktop".to_owned(),
        ),
        run: UpdateRun::Idle,
        open: true,
    }
}

/// A sign-in waiting for its code, for snapshots.
pub fn sample_sign_in() -> SignInCode {
    SignInCode {
        agent: "copilot".to_owned(),
        url: "https://github.com/login/device".to_owned(),
        code: "ABCD-1234".to_owned(),
    }
}

/// Projects on other machines: one whose server answers with a session, one
/// being reached, and one whose server cannot be reached.
pub fn sample_remote_projects() -> Vec<Project> {
    let mut reached = project(
        "api",
        vec![session(21, "Rate limit the search endpoint", "3h ago")],
        vec![],
    );
    reached.path = "ssh://devbox/home/dev/api".to_owned();
    reached.reach = Some(crate::model::ProjectReach::Reached);
    let mut reaching = project("models", vec![], vec![]);
    reaching.path = "wsl://Ubuntu/home/me/models".to_owned();
    reaching.reach = Some(crate::model::ProjectReach::Reaching);
    let mut lost = project("cluster", vec![], vec![]);
    lost.path = "ssh://gpu-box/srv/cluster".to_owned();
    lost.reach = Some(crate::model::ProjectReach::Unreachable(
        "ssh: connect to host gpu-box port 22: No route to host".to_owned(),
    ));
    vec![reached, reaching, lost]
}

/// A folder picker looking inside a home folder, on the machine `host` or this
/// one, or showing why the machine could not be reached when `failed`.
pub fn sample_picker(host: Option<&str>, failed: bool) -> crate::projects::FolderPicker {
    let known = [pm_config::KnownServer {
        name: "devbox".to_owned(),
        reach: pm_config::Reach::Ssh,
        address: "devbox".to_owned(),
        ..pm_config::KnownServer::default()
    }];
    let mut picker = crate::projects::FolderPicker::open(host.map(str::to_owned), &known);
    let home = if host.is_some() {
        "/home/dev"
    } else {
        "/home/jdaddy"
    };
    picker.listed(
        home.to_owned(),
        [
            "Documents",
            "Downloads",
            "Music",
            "Pictures",
            "repos",
            "scratch",
        ]
        .map(str::to_owned)
        .to_vec(),
    );
    picker.step(&crate::projects::PickerAction::Into("repos".to_owned()));
    picker.listed(
        format!("{home}/repos"),
        ["harness", "minmux", "rustle", "vault", "void"]
            .map(str::to_owned)
            .to_vec(),
    );
    if failed {
        picker.reach = crate::projects::Reach::Failed(
            "ssh: connect to host devbox port 22: Connection refused".to_owned(),
        );
    }
    picker
}
