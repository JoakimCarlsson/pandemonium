//! The plain data of an open session's conversation and of the cards, strips
//! and bars around it: messages made of markdown, agent steps and notices,
//! activity, the plan, questions waiting on the reader, spend and the pull
//! request.

/// The parsed markdown a message block holds.
pub type Markdown = Vec<pm_markdown::Block>;

/// Who wrote a message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Speaker {
    /// The person at the keyboard.
    #[default]
    User,
    /// The agent.
    Agent,
}

/// One message of a conversation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message {
    /// Who wrote it.
    pub speaker: Speaker,
    /// Its blocks, in order.
    pub blocks: Vec<Block>,
    /// The line after the message, such as `Worked for 6s`.
    pub footer: Option<String>,
    /// For the reader's messages, the turn it began, counted from nought.
    pub turn: Option<u64>,
    /// For the reader's messages, whether the files were kept before the turn, so they can be put back.
    pub checkpoint: bool,
    /// For the reader's messages, the names of the files sent with it.
    pub files: Vec<String>,
}

impl Message {
    /// The text of the message as the reader would copy it: its markdown as
    /// plain text, its thoughts and notices, and its steps by their titles.
    pub fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(Block::plain_text)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// The message as markdown: its text as written, thoughts quoted, steps as list items and notices in bold.
    pub fn markdown(&self) -> String {
        self.blocks
            .iter()
            .map(|block| match block {
                Block::Markdown { source, .. } => source.clone(),
                Block::Thought(text) => text
                    .lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                Block::Tool(step) => format!("- {}", step.title),
                Block::Notice(notice) => format!("**{}** {}", notice.title, notice.detail),
            })
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// A block of a message.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    /// Text the speaker wrote, as markdown, with the source it was parsed from.
    Markdown {
        /// The source as written.
        source: String,
        /// Its parsed blocks.
        blocks: Markdown,
    },
    /// What the agent thought while working.
    Thought(String),
    /// A tool the agent used.
    Tool(ToolStep),
    /// A notice set into the conversation, such as a backup model taking over.
    Notice(Notice),
}

impl Block {
    /// The block holding the markdown `source`, parsed.
    pub fn markdown(source: impl Into<String>) -> Self {
        let source = source.into();
        let blocks = pm_markdown::parse(&source);
        Block::Markdown { source, blocks }
    }

    /// The block's text as the reader would copy it.
    pub fn plain_text(&self) -> String {
        match self {
            Block::Markdown { source, .. } => pm_markdown::plain(source),
            Block::Thought(text) => text.clone(),
            Block::Tool(step) => step.title.clone(),
            Block::Notice(notice) => format!("{}: {}", notice.title, notice.detail),
        }
    }
}

/// A tool call of the agent, as one row of its steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolStep {
    /// The id the agent gave the call.
    pub id: String,
    /// What the call does, such as `Read src/main.rs`.
    pub title: String,
    /// What kind of tool it is, which picks its glyph.
    pub kind: ToolKind,
    /// How it stands.
    pub state: StepState,
    /// The file or address it works on, when it names one.
    pub target: Option<String>,
    /// What it printed or returned, when it said.
    pub output: Option<String>,
    /// Lines added and removed, for a call that edits a file.
    pub lines: Option<(usize, usize)>,
    /// What an edit changed, as the lines of a unified diff.
    pub diff: Option<String>,
    /// The kind of subagent the call starts, when it starts one.
    pub agent: Option<String>,
    /// The subagent the call started, by the id it can be stopped by, while it runs.
    pub subagent: Option<String>,
    /// What the chip of the subagent's worktree says, such as `Merged 3 files`.
    pub worktree: Option<String>,
    /// The calls the subagent this call started made, in order.
    pub children: Vec<ToolStep>,
}

impl ToolStep {
    /// The steps under this one, all the way down, counted.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(ToolStep::count).sum::<usize>()
    }

    /// The step `id` among this one and those under it.
    pub fn find_mut(&mut self, id: &str) -> Option<&mut ToolStep> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }

    /// Marks this step and those under it that still run as `state`, and forgets the subagent it ran.
    pub fn close(&mut self, state: StepState) {
        if self.state == StepState::Running {
            self.state = state;
        }
        self.subagent = None;
        for child in &mut self.children {
            child.close(state);
        }
    }
}

/// The kind of a tool call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolKind {
    /// Reading files.
    Read,
    /// Changing files.
    Edit,
    /// Searching.
    Search,
    /// Running commands.
    Execute,
    /// Fetching from the web.
    Fetch,
    /// Anything else.
    #[default]
    Other,
}

/// How a step stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepState {
    /// Still running.
    #[default]
    Running,
    /// Finished.
    Done,
    /// Failed.
    Failed,
}

/// A notice set into the conversation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notice {
    /// What kind of notice it is, which picks its glyph and colour.
    pub kind: NoticeKind,
    /// Its first line.
    pub title: String,
    /// What it says beyond its title.
    pub detail: String,
}

/// What a notice tells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoticeKind {
    /// A backup model answers for the rest of the turn.
    #[default]
    Fallback,
    /// An answer to a question asked on the side.
    Aside,
    /// The work was handed over to a new conversation.
    HandedOff,
    /// The turn or the agent failed.
    Failure,
    /// Files were put back or a turn's changes taken out of them.
    Files,
    /// The reader answered the agent's questions.
    Answered,
}

/// Something running beside the turn, shown as a chip above the prompt.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    /// The id the agent gave it.
    pub id: String,
    /// What it is.
    pub kind: ActivityKind,
    /// What it is doing, as the agent named it.
    pub label: String,
}

/// What kind of thing an activity is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActivityKind {
    /// A command or task left running beyond its call.
    #[default]
    Background,
    /// A subagent.
    Subagent,
}

/// One step of the agent's plan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanItem {
    /// What the step is.
    pub text: String,
    /// How far along it is.
    pub progress: PlanProgress,
}

/// How far along a plan step is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlanProgress {
    /// Not started.
    #[default]
    Pending,
    /// Being worked on.
    Doing,
    /// Done.
    Done,
}

/// A permission the agent waits on, such as approving its plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Approval {
    /// The number the agent put the question under.
    pub ticket: u64,
    /// What is asked.
    pub title: String,
    /// The command it would run, when it asks to run one.
    pub command: Option<String>,
    /// The plan written out, as markdown, when it asks to leave planning.
    pub plan: Option<Markdown>,
    /// The answers offered: the id sent back and the name shown, and whether
    /// it refuses.
    pub choices: Vec<ApprovalChoice>,
}

/// One answer offered to an [`Approval`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ApprovalChoice {
    /// The id sent back.
    pub id: String,
    /// The name shown.
    pub name: String,
    /// Whether choosing it refuses.
    pub refuses: bool,
}

/// A form of questions the agent waits on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InquiryForm {
    /// The number the agent put the form under.
    pub ticket: u64,
    /// The questions, in order.
    pub questions: Vec<InquiryQuestion>,
}

/// One question of an [`InquiryForm`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InquiryQuestion {
    /// A short tag naming it.
    pub header: String,
    /// The question.
    pub text: String,
    /// Whether more than one answer can be picked.
    pub multiple: bool,
    /// The answers offered: each a label and what it means.
    pub options: Vec<(String, String)>,
}

/// What the reader answered to one question: the labels picked, and what they
/// wrote in their own words.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InquiryReply {
    /// The labels picked.
    pub picks: Vec<String>,
    /// What was written instead, or beside them.
    pub other: String,
}

/// The agent's request to trust the project before using what its folder brings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustAsk {
    /// The number the agent put the request under.
    pub ticket: u64,
    /// The folder asked about.
    pub root: String,
    /// The card's heading, as the trust of the project stands.
    pub title: String,
    /// Why it asks and what the agent goes without, warning of hooks, which run commands.
    pub reason: String,
    /// What was left out: each kind, such as `Hook`, and name.
    pub skipped: Vec<(String, String)>,
}

/// The reader's answer to a [`TrustAsk`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustVerdict {
    /// Trust it for this conversation.
    Once,
    /// Trust it from now on.
    Always,
    /// Do not trust it.
    Reject,
}

/// What the conversation has cost so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpendSummary {
    /// The cost in dollars, when the price of every model asked is known.
    pub cost: Option<f64>,
    /// Whether some models' prices are unknown, so the cost is a floor.
    pub partial: bool,
    /// Tokens sent and received.
    pub tokens: u64,
    /// Requests made.
    pub requests: u64,
    /// Each model asked: its name, cost in dollars when known, and tokens.
    pub models: Vec<(String, Option<f64>, u64)>,
}

/// The pull request of the open session's branch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PullRequestInfo {
    /// Its number on the code host, or nought when the branch has none yet.
    pub number: u64,
    /// Its title.
    pub title: String,
    /// How it stands: `open`, `merged` or `closed`.
    pub state: String,
    /// Whether it is a draft.
    pub draft: bool,
    /// The review decision, such as `Approved`, or empty.
    pub review: String,
    /// Checks passed, failed and pending.
    pub checks: (usize, usize, usize),
    /// Its page on the code host.
    pub url: String,
    /// What the code host calls a review, in lower case, such as `merge request`.
    pub noun: String,
    /// Whether the code host's reviews can be read and acted on; when not, only pushing is offered.
    pub supported: bool,
    /// Whether the branch has commits its remote does not, or no remote branch yet.
    pub ahead: bool,
    /// What is being done to it, such as `Pushing…`, while the server is at it.
    pub busy: Option<String>,
    /// Why the last thing done to it failed, when it did.
    pub error: Option<String>,
}

/// The cards, strips and bars around the open session's conversation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extras {
    /// What runs beside the turn.
    pub activity: Vec<Activity>,
    /// The agent's plan.
    pub plan: Vec<PlanItem>,
    /// The permission the agent waits on.
    pub approval: Option<Approval>,
    /// The questions the agent waits on.
    pub inquiry: Option<InquiryForm>,
    /// The trust request the agent waits on.
    pub trust: Option<TrustAsk>,
    /// What the conversation has cost.
    pub spend: Option<SpendSummary>,
    /// The pull request of the branch, once the server said.
    pub pull_request: Option<PullRequestInfo>,
    /// Whether the open session is a chat that belongs to no project, and so
    /// can be continued in one.
    pub chat: bool,
    /// Text to put in the composer, such as a conversation command to finish.
    pub prefill: Option<String>,
    /// How many times text was put in the composer, so that the same text put there twice is put there again.
    pub prefills: u64,
    /// What the composer shows besides the text being written.
    pub composer: crate::composer::ComposerState,
    /// Why the session's account needs signing in again, while it does.
    pub signed_out: Option<String>,
    /// The page a tool server asks the reader to sign in on, until they do.
    pub tool_sign_in: Option<String>,
    /// The step the dev container the agent runs in is on, while it is built or started.
    pub container: Option<String>,
}
