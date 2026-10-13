//! Made-up projects, sessions and changes, for showing the workspace before real ones exist.

use pm_icon::IconName;

use crate::project::Project;

/// An agent a workspace can be started with.
#[derive(Clone, Copy, Debug)]
pub struct AgentChoice {
    /// The glyph on its chip.
    pub icon: IconName,
    /// What it is called.
    pub name: &'static str,
    /// The model it starts with.
    pub model: &'static str,
    /// The mode it starts in.
    pub mode: &'static str,
}

/// The agents offered when starting work, in the order the chips show them.
pub const AGENTS: [AgentChoice; 4] = [
    AgentChoice {
        icon: IconName::Sparkles,
        name: "Claude Code",
        model: "Default (recommended)",
        mode: "Manual",
    },
    AgentChoice {
        icon: IconName::Terminal,
        name: "Codex",
        model: "GPT-5.6",
        mode: "Read-only",
    },
    AgentChoice {
        icon: IconName::FileCode,
        name: "Cursor",
        model: "Auto",
        mode: "Ask",
    },
    AgentChoice {
        icon: IconName::Zap,
        name: "Gemini CLI",
        model: "Gemini 3 Pro",
        mode: "Default",
    },
];

/// What a new workspace can start from, one click each.
pub const SUGGESTIONS: [(IconName, &str); 4] = [
    (IconName::Sparkles, "Explain how this project is laid out"),
    (IconName::Git, "Fix the failing tests on this branch"),
    (IconName::GitCompare, "Review my uncommitted changes"),
    (IconName::Zap, "Find and remove dead code"),
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
    fn new(path: &str, source: &str) -> Self {
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
}

impl Message {
    /// A message the reader wrote.
    pub fn user(text: &str) -> Self {
        Self {
            from_user: true,
            text: text.to_owned(),
            bullets: Vec::new(),
            duration: None,
        }
    }

    /// An agent reply that took `duration`, listing `bullets` under `text`.
    fn agent(text: &str, bullets: &[&str], duration: &str) -> Self {
        Self {
            from_user: false,
            text: text.to_owned(),
            bullets: bullets.iter().map(|bullet| (*bullet).to_owned()).collect(),
            duration: Some(duration.to_owned()),
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
            agent: 0,
            status,
            updated: updated.to_owned(),
            messages,
            changes: Vec::new(),
            commits: Vec::new(),
        }
    }

    /// A session just handed `prompt`, working on it, called `title`, in `project`.
    pub fn started(title: &str, project: usize, prompt: &str) -> Self {
        Self::new(
            title,
            project,
            "task/new",
            Status::Working,
            "now",
            vec![Message::user(prompt)],
        )
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
                "Yes. Restoring now takes a checkpoint of the current state first, so the previous position is always one step back. Ready for review.",
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
