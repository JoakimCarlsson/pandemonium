//! Made-up projects, sessions and changes, for showing the workspace before real ones exist.

use std::time::Instant;

use pm_acp::{Diff, Kind as ToolKind, Permission, PlanStep, Setting, Slash, ToolState};
use pm_icon::IconName;

use crate::project::Project;

/// An agent a workspace can be started with, in the same order as [`pm_acp::AGENTS`].
#[derive(Clone, Copy, Debug)]
pub struct AgentChoice {
    /// The glyph on its chip.
    pub icon: IconName,
    /// What it is called.
    pub name: &'static str,
}

/// The agents offered when starting work, in the order the chips show them.
pub const AGENTS: [AgentChoice; 3] = [
    AgentChoice {
        icon: IconName::Sparkles,
        name: "Claude Code",
    },
    AgentChoice {
        icon: IconName::Terminal,
        name: "Codex",
    },
    AgentChoice {
        icon: IconName::Zap,
        name: "Grok Build",
    },
];

/// Where a session stands in the agent, review, code flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// The agent is still at it.
    Working,
    /// The agent is done and its changes wait to be read.
    Review,
    /// Nothing is left to do.
    Done,
}

/// What a line of a diff does to the file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// The start of a stretch of changes.
    Hunk,
    /// A line left as it was.
    Context,
    /// A line put in.
    Add,
    /// A line taken out.
    Remove,
}

/// One line of a unified diff.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffLine {
    /// What it does to the file.
    pub kind: Kind,
    /// The line itself, without its marker.
    pub text: String,
}

/// A file an agent changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// Where the file is, from the root of the project.
    pub path: String,
    /// The lines of its diff.
    pub diff: Vec<DiffLine>,
    /// Whether the reader has read it.
    pub viewed: bool,
    /// Whether it goes into the next commit.
    pub staged: bool,
}

impl Change {
    /// A change to `path` read out of `source`, one line per diff line, each
    /// starting with `@`, `+`, `-` or a space.
    pub fn new(path: &str, source: &str) -> Self {
        let diff = source
            .lines()
            .map(|line| {
                let (kind, text) = match line.chars().next() {
                    Some('@') => (Kind::Hunk, line),
                    Some('+') => (Kind::Add, &line[1..]),
                    Some('-') => (Kind::Remove, &line[1..]),
                    Some(_) => (Kind::Context, &line[1..]),
                    None => (Kind::Context, ""),
                };
                DiffLine {
                    kind,
                    text: text.to_owned(),
                }
            })
            .collect();
        Self {
            path: path.to_owned(),
            diff,
            viewed: false,
            staged: false,
        }
    }

    /// How many lines the change puts in.
    pub fn added(&self) -> usize {
        self.count(Kind::Add)
    }

    /// How many lines the change takes out.
    pub fn removed(&self) -> usize {
        self.count(Kind::Remove)
    }

    /// The directory the file is in, or nothing when it is at the root.
    pub fn directory(&self) -> &str {
        self.path
            .rsplit_once('/')
            .map_or("", |(directory, _)| directory)
    }

    /// The file's name.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// How many lines of `kind` the diff has.
    fn count(&self, kind: Kind) -> usize {
        self.diff.iter().filter(|line| line.kind == kind).count()
    }
}

/// One turn of a conversation.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// Whether the reader wrote it, rather than the agent.
    pub from_user: bool,
    /// What it says.
    pub text: String,
    /// The points an agent reply lists under its text.
    pub bullets: Vec<String>,
    /// How long the agent worked on it, if it is a reply.
    pub duration: Option<String>,
    /// What the agent did on the way to the reply, oldest first.
    pub steps: Vec<Step>,
    /// Whether the steps are shown, when the reader has chosen; otherwise only the latest turn's are.
    pub open: Option<bool>,
    /// How many steps the reply had when its text last grew, to tell when new words follow a step.
    pub said_after: usize,
    /// The files the reader sent with it, by path.
    pub attachments: Vec<String>,
}

/// One thing the agent did on the way to a reply.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// What the agent thought.
    Thought(Thought),
    /// A tool it used.
    Tool(ToolUse),
    /// The plan it keeps.
    Plan(Vec<PlanStep>),
}

/// What the agent thought, and for how long.
#[derive(Clone, Debug, PartialEq)]
pub struct Thought {
    /// What it thought.
    pub text: String,
    /// When it began.
    pub started: Instant,
    /// How many seconds it went on, once it has stopped.
    pub seconds: Option<u64>,
}

/// A tool the agent used, shown as a line of the conversation.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolUse {
    /// What the agent calls this use of the tool.
    pub id: String,
    /// What it did.
    pub title: String,
    /// What kind of tool it is.
    pub kind: ToolKind,
    /// How far it got.
    pub state: ToolState,
    /// The file, command or query it worked on.
    pub target: Option<String>,
    /// What it changed in a file.
    pub diff: Option<Diff>,
    /// What it printed.
    pub output: Option<String>,
    /// When it began.
    pub started: Instant,
    /// How many seconds it took, once it has ended.
    pub seconds: Option<u64>,
}

impl Message {
    /// A message the reader wrote.
    pub fn user(text: &str) -> Self {
        Self::new(true, text, Vec::new(), None)
    }

    /// A reply from the agent that says `text` so far.
    pub fn reply(text: &str) -> Self {
        Self::new(false, text, Vec::new(), None)
    }

    /// An agent reply that took `duration`, listing `bullets` under `text`.
    fn agent(text: &str, bullets: &[&str], duration: &str) -> Self {
        Self::new(
            false,
            text,
            bullets.iter().map(|bullet| (*bullet).to_owned()).collect(),
            Some(duration.to_owned()),
        )
    }

    /// A message with nothing done on the way to it.
    fn new(from_user: bool, text: &str, bullets: Vec<String>, duration: Option<String>) -> Self {
        Self {
            from_user,
            text: text.to_owned(),
            bullets,
            duration,
            steps: Vec::new(),
            open: None,
            said_after: 0,
            attachments: Vec::new(),
        }
    }
}

/// One agent's work on one task: its conversation and what it changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    /// What the task is called.
    pub title: String,
    /// Which project, by position, it belongs to.
    pub project: usize,
    /// The branch its worktree is on.
    pub branch: String,
    /// The folder the agent works in.
    pub directory: String,
    /// The branch its worktree was made from, when it has one of its own.
    pub base: Option<String>,
    /// Which agent, by position in [`AGENTS`], runs it.
    pub agent: usize,
    /// Where it stands.
    pub status: Status,
    /// How long ago it last changed.
    pub updated: String,
    /// The conversation so far.
    pub messages: Vec<Message>,
    /// The files it changed that are not committed.
    pub changes: Vec<Change>,
    /// The messages of the commits made from it, oldest first.
    pub commits: Vec<String>,
    /// What can be changed about the conversation, once the agent has said.
    pub settings: Vec<Setting>,
    /// The commands the agent offers behind a slash.
    pub commands: Vec<Slash>,
    /// The skills the agent offers behind a dollar sign.
    pub skills: Vec<Slash>,
    /// The question the agent waits on the reader to answer, if it waits on one.
    pub ask: Option<Permission>,
    /// When the turn that is running began, if one is.
    pub turn_started: Option<Instant>,
    /// How many of the prompts this window sent the server has yet to repeat back, so that they are not added twice.
    pub echoes: usize,
    /// Whether the reader closed it, which takes it out of the sidebar but not out of the history.
    pub closed: bool,
}

impl Session {
    /// A session without changes or commits.
    fn new(
        title: &str,
        project: usize,
        branch: &str,
        status: Status,
        updated: &str,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            title: title.to_owned(),
            project,
            branch: branch.to_owned(),
            directory: String::new(),
            base: None,
            agent: 0,
            status,
            updated: updated.to_owned(),
            messages,
            changes: Vec::new(),
            commits: Vec::new(),
            settings: Vec::new(),
            commands: Vec::new(),
            skills: Vec::new(),
            ask: None,
            turn_started: None,
            echoes: 0,
            closed: false,
        }
    }

    /// What a session's checkout is called: its branch, or, on a detached head, the commit and the branch it was made from.
    pub fn checkout_label(branch: Option<&str>, head: Option<&str>, base: Option<&str>) -> String {
        match (branch, head, base) {
            (Some(branch), _, _) => branch.to_owned(),
            (None, Some(head), Some(base)) => format!("{base} @ {head}"),
            (None, Some(head), None) => format!("@ {head}"),
            (None, None, _) => String::new(),
        }
    }

    /// A session the server held already: it has no messages until its events are replayed, and nothing is under way.
    pub fn restored(
        title: &str,
        project: usize,
        branch: &str,
        directory: &str,
        base: Option<String>,
        agent: usize,
        closed: bool,
    ) -> Self {
        let mut session = Self::new(title, project, branch, Status::Done, "", Vec::new());
        session.closed = closed;
        session.directory = directory.to_owned();
        session.base = base;
        session.agent = agent;
        session
    }

    /// A session just handed `prompt`, working on it, called `title`, in `project`.
    pub fn started(title: &str, project: usize, prompt: &str) -> Self {
        let mut session = Self::new(
            title,
            project,
            "task/new",
            Status::Working,
            "now",
            vec![Message::user(prompt)],
        );
        session.echoes = 1;
        session
    }

    /// The position of the latest reply that has steps, which is the one whose steps show unasked.
    pub fn latest_steps(&self) -> Option<usize> {
        self.messages
            .iter()
            .rposition(|message| !message.steps.is_empty())
    }

    /// How many lines all its changes put in.
    pub fn added(&self) -> usize {
        self.changes.iter().map(Change::added).sum()
    }

    /// How many lines all its changes take out.
    pub fn removed(&self) -> usize {
        self.changes.iter().map(Change::removed).sum()
    }
}

/// The projects the sample sessions belong to.
pub fn projects() -> Vec<Project> {
    ["pandemonium", "wasa-cli", "voxel", "world"]
        .into_iter()
        .map(|name| Project {
            name: name.to_owned(),
            path: format!("~/{name}"),
        })
        .collect()
}

/// The sessions the sidebar opens with.
pub fn sessions() -> Vec<Session> {
    let mut checkpoints = Session::new(
        "Restore sessions from checkpoints",
        0,
        "task/227-checkpoints",
        Status::Review,
        "now",
        vec![
            Message::user(
                "Sessions lose their place when the app restarts. Add checkpoints so I can return to any point in a session.",
            ),
            Message::agent(
                "Checkpoints are in. Each one records the session root and its id, and a session can now be restored from any of them.",
                &[
                    "Added a Checkpoint type that stores the session root and an id.",
                    "Sessions keep an ordered list of checkpoints and can restore the latest one.",
                    "Restoring a checkpoint that no longer exists returns an error instead of panicking.",
                    "Formatting, clippy and the test suite are clean.",
                ],
                "Worked for 4m 12s",
            ),
            Message::user("Can restoring be undone?"),
            Message::agent(
                "Yes. **Restoring** now takes a checkpoint of the *current* state first, so the previous position is always one step back:\n\n1. `Session::restore` calls `checkpoint()` before it moves the root.\n2. The new checkpoint is pushed onto the list, so ~~nothing is lost~~ the old state stays reachable.\n\n```rust\nlet before = session.checkpoint().id;\nsession.restore(target)?;\n```\n\n### Undo\n\n- Restoring `before` again undoes it, and the checkpoint taken on the way keeps the position the reader undid from.\n  - Nothing is ever dropped.\n\n> Undoing is restoring `before` again.\n\n| Action | Checkpoints |\n| --- | --- |\n| Restore | +1 |\n| Undo | +1 |\n\nReady for review, see [the diff](#).\n\n<details>\n<summary>Why it matters</summary>\n\nThe first version only needs to be <b>fun</b>.<br>Press <kbd>Ctrl</kbd> to go. <!-- hidden -->\n\n</details>",
                &[],
                "Worked for 1m 03s",
            ),
        ],
    );
    checkpoints.changes = vec![
        Change::new(
            "crates/pandemonium/src/checkpoint.rs",
            "@@ -0,0 +1,18 @@\n+use std::path::PathBuf;\n+\n+/// A saved point in a session that can be returned to.\n+pub struct Checkpoint {\n+    pub id: u64,\n+    pub root: PathBuf,\n+}\n+\n+impl Checkpoint {\n+    pub fn new(id: u64, root: PathBuf) -> Self {\n+        Self { id, root }\n+    }\n+\n+    pub fn exists(&self) -> bool {\n+        self.root.exists()\n+    }\n+}",
        ),
        Change::new(
            "crates/pandemonium/src/session.rs",
            "@@ -12,9 +12,22 @@ pub struct Session {\n pub struct Session {\n     pub id: SessionId,\n     pub root: PathBuf,\n+    checkpoints: Vec<Checkpoint>,\n }\n \n impl Session {\n-    pub fn new(id: SessionId, root: PathBuf) -> Self {\n-        Self { id, root }\n+    pub fn new(id: SessionId, root: PathBuf) -> Self {\n+        Self { id, root, checkpoints: Vec::new() }\n+    }\n+\n+    pub fn checkpoint(&mut self) -> &Checkpoint {\n+        let id = self.checkpoints.len() as u64;\n+        self.checkpoints.push(Checkpoint::new(id, self.root.clone()));\n+        self.checkpoints.last().unwrap()\n+    }\n+\n+    pub fn restore(&mut self, id: u64) -> Result<(), RestoreError> {\n+        let target = self.checkpoints.get(id as usize).ok_or(RestoreError::Missing(id))?;\n+        self.root = target.root.clone();\n+        Ok(())\n     }\n }",
        ),
        Change::new(
            "crates/pandemonium-ui/src/restore.rs",
            "@@ -0,0 +1,10 @@\n+use pandemonium::Session;\n+\n+/// Restores a session and reports whether anything changed.\n+pub fn restore_latest(session: &mut Session) -> bool {\n+    match session.latest_checkpoint() {\n+        Some(id) => session.restore(id).is_ok(),\n+        None => false,\n+    }\n+}",
        ),
        Change::new(
            "docs/checkpoints.md",
            "@@ -0,0 +1,5 @@\n+# Checkpoints\n+\n+A checkpoint saves the root of a session.\n+Restoring one first saves the current state,\n+so a restore can always be undone.",
        ),
        Change::new(
            "crates/pandemonium/src/lib.rs",
            "@@ -1,5 +1,6 @@\n mod agent;\n+mod checkpoint;\n mod session;\n \n pub use agent::Agent;\n+pub use checkpoint::Checkpoint;\n pub use session::Session;",
        ),
    ];

    let mut titles = Session::new(
        "Add automatic session titles",
        0,
        "task/231-titles",
        Status::Review,
        "12m",
        vec![
            Message::user(
                "New sessions are all called \"Untitled\". Name them from the first prompt.",
            ),
            Message::agent(
                "Sessions now take their title from the first prompt, trimmed to a short phrase. Ready for review.",
                &[],
                "Worked for 2m 40s",
            ),
        ],
    );
    titles.changes = vec![Change::new(
        "crates/pandemonium/src/title.rs",
        "@@ -0,0 +1,8 @@\n+/// The short phrase a session is named by, taken from its first prompt.\n+pub fn title_of(prompt: &str) -> String {\n+    prompt\n+        .split_whitespace()\n+        .take(6)\n+        .collect::<Vec<_>>()\n+        .join(\" \")\n+}",
    )];

    vec![
        checkpoints,
        titles,
        Session::new(
            "Fix worktree cleanup on exit",
            0,
            "task/233-worktrees",
            Status::Working,
            "4m",
            vec![
                Message::user(
                    "Worktrees pile up in .pandemonium after sessions end. Clean them up when a session is closed.",
                ),
                Message::agent(
                    "Adding cleanup to the shutdown path. I need your approval to remove the leftover worktrees before the test can run.",
                    &[],
                    "Working for 3m 20s",
                ),
            ],
        ),
        Session::new(
            "Improve editor typing latency",
            0,
            "main",
            Status::Working,
            "18m",
            vec![
                Message::user("Typing in large files feels laggy. Find out why and fix it."),
                Message::agent(
                    "Profiling a 20k line file now. The first hot spot is re-highlighting the whole buffer on each keystroke.",
                    &[],
                    "Working for 6m 02s",
                ),
            ],
        ),
        Session::new(
            "Release v0.7.0-beta.2",
            0,
            "main",
            Status::Done,
            "2h",
            vec![
                Message::user("Cut the beta release."),
                Message::agent(
                    "Tagged v0.7.0-beta.2, built the installers and published the release notes.",
                    &[],
                    "Worked for 8m 30s",
                ),
            ],
        ),
        Session::new(
            "Greet user and await task",
            1,
            "main",
            Status::Done,
            "6m",
            vec![
                Message::user("Hi"),
                Message::agent(
                    "Hello. What would you like to work on in wasa-cli?",
                    &[],
                    "Worked for 2s",
                ),
            ],
        ),
        Session::new(
            "Add a --json output flag",
            1,
            "task/12-json-flag",
            Status::Review,
            "1h",
            vec![
                Message::user("Every command should be able to print machine-readable output."),
                Message::agent(
                    "Added a global --json flag. Commands now print one JSON object instead of formatted text. Ready for review.",
                    &[],
                    "Worked for 3m 10s",
                ),
            ],
        ),
        Session::new(
            "Mesh chunks off the main thread",
            2,
            "task/41-chunk-meshing",
            Status::Working,
            "9m",
            vec![
                Message::user("Chunk meshing stalls the frame. Move it to a worker pool."),
                Message::agent(
                    "Sketching a job queue for chunk meshing and measuring the current stall.",
                    &[],
                    "Working for 2m 15s",
                ),
            ],
        ),
    ]
}
