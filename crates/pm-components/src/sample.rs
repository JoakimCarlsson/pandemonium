//! Made-up projects, sessions and changes, for showing the workspace before real ones exist.

use std::time::Instant;

use pm_acp::{Diff, Kind as ToolKind, Permission, PlanStep, Setting, Slash, ToolState};

use crate::dock::TerminalTab;
use crate::draft::Draft;
use crate::project::Project;

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

/// How a changed file came to differ from the last commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fate {
    /// The file is new.
    Added,
    /// The file is gone.
    Deleted,
    /// The file has other contents.
    Modified,
}

/// Lines of one side of a diff that a comment is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Range {
    /// Whether the lines are numbered in the new file, and in the old one when not.
    pub new: bool,
    /// The first line.
    pub start: usize,
    /// The last line.
    pub end: usize,
}

impl Range {
    /// The lines as a pull request names them: `line R12`, or `lines L12 to L14` for the old file.
    pub fn label(&self) -> String {
        let side = if self.new { 'R' } else { 'L' };
        if self.start == self.end {
            format!("line {side}{}", self.start)
        } else {
            format!("lines {side}{} to {side}{}", self.start, self.end)
        }
    }

    /// The lines as a sentence for the agent: `line 12`, or `lines 12–14 (before the change)`.
    pub fn describe(&self) -> String {
        let lines = if self.start == self.end {
            format!("line {}", self.start)
        } else {
            format!("lines {}–{}", self.start, self.end)
        };
        if self.new {
            lines
        } else {
            format!("{lines} (before the change)")
        }
    }
}

/// A note on a changed file, or on lines of it, waiting to be sent to the agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Comment {
    /// The file it is about.
    pub path: String,
    /// The lines it is about, or the whole file when there are none.
    pub range: Option<Range>,
    /// What it says.
    pub text: String,
}

/// A file an agent changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// Where the file is, from the root of the project.
    pub path: String,
    /// The lines of its diff.
    pub diff: Vec<DiffLine>,
    /// The words that changed inside the removed and added lines of its diff.
    pub words: Vec<pm_git::Words>,
    /// Whether the reader has read it.
    pub viewed: bool,
    /// Whether some of it is in the index, which is what the next commit takes.
    pub staged: bool,
    /// Whether some of it is not: it has changes the index does not, or git does not track it yet.
    pub unstaged: bool,
    /// Whether git does not track it yet.
    pub untracked: bool,
    /// Whether it is changed on both sides of a merge.
    pub conflicted: bool,
    /// What the index holds about it, as the letter git writes it with.
    pub index: Option<char>,
    /// What the working folder holds about it that the index does not, as the letter git writes it with.
    pub worktree: Option<char>,
    /// Where it was before it was renamed, when it was.
    pub from: Option<String>,
}

/// What a file's status comes to once its letter is read: how loudly it asks to be noticed, and the letter it is written with.
fn status_of(letter: char) -> Option<(u8, &'static str)> {
    match letter {
        'M' | 'T' => Some((0, "M")),
        'R' | 'C' => Some((1, "R")),
        'A' => Some((2, "A")),
        'D' => Some((3, "D")),
        '?' => Some((4, "U")),
        'U' => Some((5, "C")),
        _ => None,
    }
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
            words: Vec::new(),
            viewed: false,
            staged: false,
            unstaged: true,
            untracked: false,
            conflicted: false,
            index: None,
            worktree: Some('M'),
            from: None,
        }
    }

    /// Whether all of it is in the index.
    pub fn fully_staged(&self) -> bool {
        self.staged && !self.unstaged
    }

    /// Whether the last commit has no such file: it is new, staged or not, and putting it back means taking it off the disk.
    pub fn is_created(&self) -> bool {
        self.untracked || self.index == Some('A')
    }

    /// The one letter to write the file down as, and whether it is a new file, a deleted one or a changed one, for its colour.
    pub fn mark(&self) -> &'static str {
        let index = self.index.and_then(status_of);
        let worktree = self.worktree.and_then(status_of);
        match (index, worktree) {
            _ if self.conflicted => "C",
            (Some(index), Some(worktree)) if worktree.0 > index.0 => worktree.1,
            (Some(index), _) => index.1,
            (None, Some(worktree)) => worktree.1,
            (None, None) => "M",
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

    /// Whether the file is new, gone or has other contents, as its first hunk says.
    pub fn fate(&self) -> Fate {
        let header = self
            .diff
            .first()
            .filter(|line| line.kind == Kind::Hunk)
            .map_or("", |line| line.text.as_str());
        if header.starts_with("@@ -0,0 ") {
            Fate::Added
        } else if header.contains(" +0,0 @@") {
            Fate::Deleted
        } else {
            Fate::Modified
        }
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
    /// The forms of questions the reader answered during the reply, each with how many steps came before it: every question and its answer, or nothing when skipped.
    pub answers: Vec<(usize, Vec<Answered>)>,
    /// Whether the steps are shown, when the reader has chosen; otherwise only the latest turn's are.
    pub open: Option<bool>,
    /// How many steps the reply had when its text last grew, to tell when new words follow a step.
    pub said_after: usize,
    /// The pieces of the text as the agent said them, each with how many steps came before it.
    pub said: Vec<(usize, String)>,
    /// The files the reader sent with it, by path.
    pub attachments: Vec<String>,
    /// Whether the server kept the files as they were before the reader's turn, so they can be put back.
    pub checkpoint: bool,
}

/// A question of a form the reader answered, with the answer, or nothing when it was skipped.
pub type Answered = (String, Option<String>);

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
    /// The subagent call whose work this is, when it is one of a subagent's.
    pub parent: Option<String>,
    /// The kind of subagent it starts, when it starts one.
    pub agent: Option<String>,
}

impl ToolUse {
    /// Whether it starts a subagent.
    pub fn is_subagent(&self) -> bool {
        self.agent.is_some()
    }

    /// Whether it is a subagent still at work.
    pub fn is_working_agent(&self) -> bool {
        self.is_subagent() && self.state == ToolState::Running
    }
}

/// A command or task that goes on running beyond the call that started it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shell {
    /// The handle the agent knows it by.
    pub id: String,
    /// The command or task that was started.
    pub label: String,
    /// The call that started it, which stays running as long as it does.
    pub call: Option<String>,
    /// When it was first heard of.
    pub started: Instant,
}

impl Message {
    /// The message as it was written: the markdown of a reply, with the points it lists as a list under it.
    pub fn markdown_text(&self) -> String {
        let mut text = self.text.clone();
        for bullet in &self.bullets {
            text.push_str(&format!("\n- {bullet}"));
        }
        text
    }

    /// The message as it reads: a reply's markdown without its markup, with the points it lists as a list under it.
    pub fn plain_text(&self) -> String {
        if self.from_user {
            return self.text.clone();
        }
        let mut text = pm_markdown::plain(&self.text);
        for bullet in &self.bullets {
            text.push_str(&format!("\n- {bullet}"));
        }
        text
    }

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
            answers: Vec::new(),
            said: Vec::new(),
            attachments: Vec::new(),
            checkpoint: false,
        }
    }

    /// The pieces of the text in the order the agent said them among its steps, each with how many steps came before it; the whole text after every step when it was not told piece by piece.
    pub fn chunks(&self) -> Vec<(usize, &str)> {
        if self.said.is_empty() {
            return if self.text.is_empty() {
                Vec::new()
            } else {
                vec![(self.steps.len(), self.text.as_str())]
            };
        }
        self.said
            .iter()
            .map(|(before, piece)| (*before, piece.as_str()))
            .collect()
    }
}

/// A prompt waiting on the server for the agent's turn to end.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Queued {
    /// The number the server gave it within its session.
    pub id: u64,
    /// What the reader wrote.
    pub text: String,
    /// The names of the files attached.
    pub files: Vec<String>,
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
    /// Whether the worktree is on a branch rather than at a bare commit.
    pub on_branch: bool,
    /// What the branch follows and how far apart they are.
    pub tracking: pm_git::Tracking,
    /// The folder the agent works in.
    pub directory: String,
    /// The branch its worktree was made from, when it has one of its own.
    pub base: Option<String>,
    /// Which agent, by position in the list of agents, runs it.
    pub agent: usize,
    /// Where it stands.
    pub status: Status,
    /// When it last changed, in milliseconds since the epoch.
    pub updated_at: u64,
    /// The conversation so far.
    pub messages: Vec<Message>,
    /// The files it changed that are not committed.
    pub changes: Vec<Change>,
    /// The notes on those files that have not been sent to the agent.
    pub comments: Vec<Comment>,
    /// The messages of the commits made from it, oldest first.
    pub commits: Vec<String>,
    /// What can be changed about the conversation, once the agent has said.
    pub settings: Vec<Setting>,
    /// The commands the agent offers behind a slash.
    pub commands: Vec<Slash>,
    /// The skills the agent offers behind a dollar sign.
    pub skills: Vec<Slash>,
    /// The id of the login its agent runs as, or nothing for the agent's own.
    pub login: Option<String>,
    /// The question the agent waits on the reader to answer, if it waits on one.
    pub ask: Option<Permission>,
    /// The form of questions the agent waits on the reader to answer, if it waits on one.
    pub inquiry: Option<Draft>,
    /// When the turn that is running began, if one is.
    pub turn_started: Option<Instant>,
    /// How many of the prompts this window sent the server has yet to repeat back, so that they are not added twice.
    pub echoes: usize,
    /// The prompts waiting for the agent's turn to end, in the order they were queued.
    pub queued: Vec<Queued>,
    /// Whether the reader closed it, which takes it out of the sidebar but not out of the history.
    pub closed: bool,
    /// Whether it stands for the project's own folder rather than for work an agent does: it is never listed, and holds what the git panel shows while no session is open.
    pub home: bool,
    /// The shells in its terminal dock.
    pub terminals: Vec<TerminalTab>,
    /// Which of the shells shows, by position.
    pub terminal_active: usize,
    /// The commands and tasks the agent left running beyond its turn.
    pub shells: Vec<Shell>,
    /// How full the context is, once the agent has said.
    pub context: Option<pm_acp::Usage>,
}

impl Session {
    /// A session without changes or commits.
    fn new(
        title: &str,
        project: usize,
        branch: &str,
        status: Status,
        minutes_ago: u64,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            title: title.to_owned(),
            project,
            branch: branch.to_owned(),
            on_branch: !branch.is_empty(),
            tracking: pm_git::Tracking::default(),
            directory: String::new(),
            base: None,
            agent: 0,
            status,
            updated_at: crate::age::now().saturating_sub(minutes_ago * 60_000),
            messages,
            changes: Vec::new(),
            comments: Vec::new(),
            commits: Vec::new(),
            settings: Vec::new(),
            context: None,
            commands: Vec::new(),
            skills: Vec::new(),
            login: None,
            ask: None,
            inquiry: None,
            turn_started: None,
            echoes: 0,
            queued: Vec::new(),
            closed: false,
            home: false,
            terminals: Vec::new(),
            terminal_active: 0,
            shells: Vec::new(),
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
        let mut session = Self::new(title, project, branch, Status::Done, 0, Vec::new());
        session.closed = closed;
        session.directory = directory.to_owned();
        session.base = base;
        session.agent = agent;
        session
    }

    /// The stand-in for the folder of the project at `project`, kept out of every list, that the git panel reads while no session is open.
    pub fn home(project: usize, directory: &str) -> Self {
        let mut session = Self::new("", project, "", Status::Done, 0, Vec::new());
        session.closed = true;
        session.home = true;
        session.directory = directory.to_owned();
        session
    }

    /// How long ago it last changed, in the short form the lists show.
    pub fn age(&self) -> String {
        crate::age::label(self.updated_at, crate::age::now())
    }

    /// A session just handed `prompt`, working on it, called `title`, in `project`.
    pub fn started(title: &str, project: usize, prompt: &str) -> Self {
        let mut session = Self::new(
            title,
            project,
            "task/new",
            Status::Working,
            0,
            vec![Message::user(prompt)],
        );
        session.echoes = 1;
        session
    }

    /// The position among the reader's turns of the message at `index`, counted from nought.
    pub fn turn_of(&self, index: usize) -> u64 {
        self.messages[..index.min(self.messages.len())]
            .iter()
            .filter(|message| message.from_user)
            .count() as u64
    }

    /// The position of the latest reply that has steps, which is the one whose steps show unasked.
    pub fn latest_steps(&self) -> Option<usize> {
        self.messages
            .iter()
            .rposition(|message| !message.steps.is_empty())
    }

    /// The tools of every reply that start subagents, oldest first, each with the position of its reply and of its step.
    pub fn subagents(&self) -> Vec<(usize, usize, &ToolUse)> {
        self.messages
            .iter()
            .enumerate()
            .flat_map(|(reply, message)| {
                message
                    .steps
                    .iter()
                    .enumerate()
                    .filter_map(move |(position, step)| match step {
                        Step::Tool(tool) if tool.is_subagent() => Some((reply, position, tool)),
                        _ => None,
                    })
            })
            .collect()
    }

    /// Whether something the agent started is still running, although its turn may be over.
    pub fn background_running(&self) -> bool {
        !self.shells.is_empty()
            || self.messages.iter().any(|message| {
                message
                    .steps
                    .iter()
                    .any(|step| matches!(step, Step::Tool(tool) if tool.is_working_agent()))
            })
    }

    /// Where it stands as the reader should see it: at work while the agent has anything running.
    pub fn shown(&self) -> Status {
        if self.status != Status::Working && self.background_running() {
            Status::Working
        } else {
            self.status
        }
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
        0,
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
        12,
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
            4,
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
            18,
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
            120,
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
            6,
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
            60,
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
            9,
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
