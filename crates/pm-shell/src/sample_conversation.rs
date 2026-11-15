//! The conversation of the sample session and the cards around it, so every
//! kind of conversation item renders in a snapshot without a server.

use crate::conversation::{
    Activity, ActivityKind, Approval, ApprovalChoice, Block, Extras, InquiryForm, InquiryQuestion,
    Message, Notice, NoticeKind, PlanItem, PlanProgress, PullRequestInfo, Speaker, SpendSummary,
    StepState, ToolKind, ToolStep, TrustAsk,
};

/// The question a snapshot shows the agent waiting on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleAsk {
    /// Approving the plan.
    Plan,
    /// A form of questions.
    Inquiry,
    /// Trusting the project.
    Trust,
    /// Continuing the session, a chat, in a project.
    Continue,
    /// Signing in again, to the account and to a tool server, with a goal set.
    SignIn,
}

/// The markdown of the first agent reply: fold-outs, a table, a diagram, a
/// formula and an image.
const RICH_REPLY: &str = r#"Here is how the request flows through **VOID** today, with the parts that need *work* struck: ~~caching~~.

| Stage | Crate | State |
| --- | --- | --- |
| Fetch | `void-fetch` | done |
| Extract | `void-extract` | in progress |
| Rank | `void-rank` | not started |

```mermaid
graph LR
    Query --> Fetch --> Extract --> Rank --> Evidence
```

Ranking scores each passage by

$$
s(p) = \sum_{t \in q} \mathrm{idf}(t) \cdot \frac{f(t,p)\,(k+1)}{f(t,p) + k}
$$

<details>
<summary>Why not a full search engine?</summary>

A crawler and an inverted index are months of work. Live fetching answers the first queries today, and the index slots in behind the same contract later.

</details>

![The evidence card](data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNDAiIGhlaWdodD0iOTAiPjxyZWN0IHdpZHRoPSIyNDAiIGhlaWdodD0iOTAiIHJ4PSIxMCIgZmlsbD0iIzNiODJmNiIvPjxjaXJjbGUgY3g9IjQ1IiBjeT0iNDUiIHI9IjI0IiBmaWxsPSIjZmJiZjI0Ii8+PHJlY3QgeD0iODQiIHk9IjMwIiB3aWR0aD0iMTMwIiBoZWlnaHQ9IjEwIiByeD0iNSIgZmlsbD0iI2ZmZmZmZiIvPjxyZWN0IHg9Ijg0IiB5PSI1MCIgd2lkdGg9IjkwIiBoZWlnaHQ9IjEwIiByeD0iNSIgZmlsbD0iI2JmZGJmZSIvPjwvc3ZnPg==)
"#;

/// The markdown of the last agent reply.
const LAST_REPLY: &str = r#"I think that's the right call. Focus on VOID.

## Why

- **VOID's hard problems are elsewhere.** Crawling, extraction, indexing, ranking and picking evidence are where the work and the value are. The HTTP API is a handful of endpoints (`/v1/search`, maybe `/v1/extract`, plus streaming).
- **Porting minmux is a whole project by itself.** Router, derive macros, an OpenAPI 3.2 model, clientgen, auth and caching would mean months before VOID answers a single query.

> Keep all HTTP code in `void-server`; the other crates never import axum.

## A suggested start

1. `void-core`: the evidence contract (`Query`, `Document`, `Passage`, `Evidence`)
2. `void-extract`: URL to clean text, testable on its own
3. `void-server`: `POST /v1/search` on axum

```rust
pub struct Evidence {
    pub passage: Passage,
    pub score: f32,
}
```

---

Want me to write a VOID brief, or pin down the `/v1/search` contract first?"#;

/// A finished step of `kind` titled `title`.
fn step(kind: ToolKind, title: &str, target: Option<&str>) -> Block {
    Block::Tool(ToolStep {
        id: title.to_owned(),
        title: title.to_owned(),
        kind,
        state: StepState::Done,
        target: target.map(str::to_owned),
        output: None,
        lines: None,
        ..ToolStep::default()
    })
}

/// The conversation of the sample session.
pub fn conversation() -> Vec<Message> {
    vec![
        Message {
            speaker: Speaker::User,
            blocks: vec![Block::markdown("How does a query flow through VOID right now?")],
            footer: None,
            turn: Some(0),
            checkpoint: true,
            files: vec!["architecture.png".to_owned()],
        },
        Message {
            speaker: Speaker::Agent,
            blocks: vec![
                Block::Thought(
                    "The reader wants the pipeline. I should read the workspace manifest and the server's router before answering.".to_owned(),
                ),
                step(ToolKind::Read, "Read Cargo.toml", Some("Cargo.toml")),
                step(ToolKind::Search, "Search for `fn search`", None),
                Block::Tool(ToolStep {
                    id: "task".to_owned(),
                    title: "Survey ranking crates".to_owned(),
                    state: StepState::Running,
                    agent: Some("scout".to_owned()),
                    subagent: Some("scout-1".to_owned()),
                    worktree: Some("In its own worktree".to_owned()),
                    children: vec![
                        ToolStep {
                            id: "scout-read".to_owned(),
                            title: "Read void-rank/Cargo.toml".to_owned(),
                            kind: ToolKind::Read,
                            state: StepState::Done,
                            ..ToolStep::default()
                        },
                        ToolStep {
                            id: "scout-fetch".to_owned(),
                            title: "Fetch crates.io/search?q=bm25".to_owned(),
                            kind: ToolKind::Fetch,
                            state: StepState::Running,
                            ..ToolStep::default()
                        },
                    ],
                    ..ToolStep::default()
                }),
                Block::Tool(ToolStep {
                    id: "edit".to_owned(),
                    title: "Edit void-server/src/router.rs".to_owned(),
                    kind: ToolKind::Edit,
                    state: StepState::Done,
                    target: Some("void-server/src/router.rs".to_owned()),
                    output: None,
                    lines: Some((12, 3)),
                    ..ToolStep::default()
                }),
                Block::Tool(ToolStep {
                    id: "test".to_owned(),
                    title: "cargo test -p void-extract".to_owned(),
                    kind: ToolKind::Execute,
                    state: StepState::Failed,
                    target: None,
                    output: Some(
                        "test extract::strips_scripts ... FAILED\nassertion failed: text.contains(\"body\")".to_owned(),
                    ),
                    lines: None,
                    ..ToolStep::default()
                }),
                Block::Notice(Notice {
                    kind: NoticeKind::Fallback,
                    title: "Switched main from Opus 5.5 to its backup Sonnet 5".to_owned(),
                    detail: "overloaded_error: the model is under heavy load".to_owned(),
                }),
                Block::markdown(RICH_REPLY),
            ],
            footer: Some("Worked for 42s".to_owned()),
            turn: None,
            ..Message::default()
        },
        Message {
            speaker: Speaker::User,
            blocks: vec![Block::markdown(
                "or maybe porting over minmux is taking it too far, maybe we should focus on void as is",
            )],
            footer: None,
            turn: Some(1),
            ..Message::default()
        },
        Message {
            speaker: Speaker::Agent,
            blocks: vec![
                Block::Notice(Notice {
                    kind: NoticeKind::Aside,
                    title: "Is axum still maintained?".to_owned(),
                    detail: "Yes, by the tokio team; 0.8 shipped this year.".to_owned(),
                }),
                Block::markdown(LAST_REPLY),
            ],
            footer: Some("Worked for 6s".to_owned()),
            turn: None,
            ..Message::default()
        },
    ]
}

/// The cards, strips and bars around the sample conversation.
pub fn extras() -> Extras {
    Extras {
        activity: vec![
            Activity {
                id: "watch".to_owned(),
                kind: ActivityKind::Background,
                label: "cargo watch -x test".to_owned(),
            },
            Activity {
                id: "scout".to_owned(),
                kind: ActivityKind::Subagent,
                label: "Survey ranking crates".to_owned(),
            },
        ],
        plan: vec![
            PlanItem {
                text: "Define the evidence contract in void-core".to_owned(),
                progress: PlanProgress::Done,
            },
            PlanItem {
                text: "Extract clean text from fetched pages".to_owned(),
                progress: PlanProgress::Doing,
            },
            PlanItem {
                text: "Serve POST /v1/search on axum".to_owned(),
                progress: PlanProgress::Pending,
            },
        ],
        spend: Some(SpendSummary {
            cost: Some(1.84),
            partial: false,
            tokens: 412_000,
            requests: 23,
            models: vec![
                ("Opus 5.5".to_owned(), Some(1.62), 301_000),
                ("Sonnet 5".to_owned(), Some(0.22), 111_000),
            ],
        }),
        pull_request: Some(PullRequestInfo {
            number: 42,
            title: "Extract clean text from fetched pages".to_owned(),
            state: "open".to_owned(),
            draft: false,
            review: "Review required".to_owned(),
            checks: (4, 1, 1),
            url: "https://github.com/void/void/pull/42".to_owned(),
        }),
        ..Extras::default()
    }
}

/// Sets the agent of `extras` waiting on `ask`.
pub fn sample_ask(extras: &mut Extras, ask: SampleAsk) {
    match ask {
        SampleAsk::SignIn => {
            extras.signed_out = Some("Claude: the token expired".to_owned());
            extras.tool_sign_in = Some("https://mcp.example.com/login".to_owned());
            extras.composer.goal = Some(crate::composer::GoalChip {
                objective: "Ship /v1/search".to_owned(),
                state: "Active".to_owned(),
                detail: "41k of 200k tokens".to_owned(),
                active: true,
            });
        }
        SampleAsk::Plan => {
            extras.approval = Some(Approval {
                ticket: 1,
                title: "Ready to code?".to_owned(),
                command: None,
                plan: Some(pm_markdown::parse(
                    "1. Add `Evidence` to `void-core`\n2. Strip scripts in `void-extract`\n3. Route `POST /v1/search`",
                )),
                choices: vec![
                    ApprovalChoice {
                        id: "auto".to_owned(),
                        name: "Yes, and auto-accept edits".to_owned(),
                        refuses: false,
                    },
                    ApprovalChoice {
                        id: "ask".to_owned(),
                        name: "Yes, ask before edits".to_owned(),
                        refuses: false,
                    },
                    ApprovalChoice {
                        id: "no".to_owned(),
                        name: "Keep planning".to_owned(),
                        refuses: true,
                    },
                ],
            });
        }
        SampleAsk::Inquiry => {
            extras.inquiry = Some(InquiryForm {
                ticket: 2,
                questions: vec![
                    InquiryQuestion {
                        header: "Framework".to_owned(),
                        text: "Which HTTP framework should void-server use?".to_owned(),
                        multiple: false,
                        options: vec![
                            (
                                "axum".to_owned(),
                                "Tower based, the common choice".to_owned(),
                            ),
                            ("hyper".to_owned(), "Write the router by hand".to_owned()),
                        ],
                    },
                    InquiryQuestion {
                        header: "Endpoints".to_owned(),
                        text: "Which endpoints come first?".to_owned(),
                        multiple: true,
                        options: vec![
                            ("/v1/search".to_owned(), String::new()),
                            ("/v1/extract".to_owned(), String::new()),
                            ("streaming".to_owned(), "Server-sent events".to_owned()),
                        ],
                    },
                ],
            });
        }
        SampleAsk::Trust => {
            extras.trust = Some(TrustAsk {
                ticket: 3,
                root: "~/repos/void".to_owned(),
                reason: "This project has not been trusted yet.".to_owned(),
                skipped: vec![
                    ("Hook".to_owned(), "pre-commit fmt".to_owned()),
                    ("Instruction".to_owned(), "AGENTS.md".to_owned()),
                    ("Skill".to_owned(), "release".to_owned()),
                ],
            });
        }
        SampleAsk::Continue => extras.chat = true,
    }
}
