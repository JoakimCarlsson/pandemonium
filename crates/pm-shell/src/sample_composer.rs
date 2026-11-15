//! Sample states of the composer for snapshots: the agent's settings with
//! the model slots, the slash commands, the usage rings, queued prompts,
//! attachments and the open menus.

use crate::composer::{
    Capture, CaptureKind, ComposerMenu, ComposerState, ConversationSetting, Gauge, PickedElement,
    PromptFile, QueuedPrompt, SettingPick, SlashCommand,
};
use crate::conversation::Extras;

/// A state of the composer a snapshot can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleComposer {
    /// The settings, commands and rings, nothing open.
    Plain,
    /// The model picker open, its `@slot` picks last.
    ModelMenu,
    /// The slash command menu narrowed by a slash.
    Slash,
    /// Prompts queued behind a running turn.
    Queued,
    /// Files, a pasted picture, a browser element and captures attached.
    Attached,
    /// The text field's menu open.
    Field,
    /// Three lines written, the last mentioning a file after an `@`.
    Mention,
}

/// The files the sample composer can mention.
pub fn sample_mentions() -> Vec<String> {
    [
        "src/main.rs",
        "src/maintenance.rs",
        "crates/void-core/src/main.rs",
        "README.md",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// Puts the sample composer state `sample` into `extras`.
pub fn sample_composer(extras: &mut Extras, sample: SampleComposer) {
    let mut composer = ComposerState {
        settings: sample_settings(),
        commands: sample_commands(),
        context: Some(Gauge {
            fraction: 0.42,
            detail: "Context 84k of 200k tokens".to_owned(),
        }),
        allowance: Some(Gauge {
            fraction: 0.86,
            detail: "5 hour allowance 86% used".to_owned(),
        }),
        ..ComposerState::default()
    };
    match sample {
        SampleComposer::Plain => {}
        SampleComposer::ModelMenu => {
            composer.menu = Some(ComposerMenu::Setting("model".to_owned()))
        }
        SampleComposer::Slash => extras.prefill = Some("/".to_owned()),
        SampleComposer::Queued => {
            composer.working = true;
            composer.queued = vec![
                QueuedPrompt {
                    id: 3,
                    text: "Then run the snapshot tests and fix what breaks".to_owned(),
                    files: Vec::new(),
                },
                QueuedPrompt {
                    id: 4,
                    text: "Match the spacing of this mock-up".to_owned(),
                    files: vec!["mock-up.png".to_owned()],
                },
            ];
        }
        SampleComposer::Attached => {
            composer.files = vec![
                PromptFile {
                    name: "trace.log".to_owned(),
                    path: Some("/home/reader/trace.log".to_owned()),
                    data: Vec::new(),
                    picture: false,
                },
                PromptFile {
                    name: "pasted-1794691070887.png".to_owned(),
                    path: None,
                    data: Vec::new(),
                    picture: true,
                },
            ];
            composer.elements = vec![PickedElement {
                label: "button.primary".to_owned(),
                address: "http://localhost:3000/".to_owned(),
                markup: "<button class=\"primary\">Save</button>".to_owned(),
            }];
            composer.captures = vec![
                Capture {
                    kind: CaptureKind::Screenshot,
                    label: "Screenshot of localhost:3000".to_owned(),
                    data: Vec::new(),
                },
                Capture {
                    kind: CaptureKind::Console,
                    label: "Console · 3 errors".to_owned(),
                    data: Vec::new(),
                },
            ];
        }
        SampleComposer::Mention => {
            extras.prefill =
                Some("Compare the two entry points\nline by line, then\nlook at @mai".to_owned());
        }
        SampleComposer::Field => {
            extras.prefill = Some("Explain why the build fails".to_owned());
            composer.menu = Some(ComposerMenu::Field(120.0, 12.0));
        }
    }
    extras.composer = composer;
}

/// The settings the sample agent lists: its model with two slots, its mode and its effort.
fn sample_settings() -> Vec<ConversationSetting> {
    let pick = |value: &str, name: &str, description: &str| SettingPick {
        value: value.to_owned(),
        name: name.to_owned(),
        description: description.to_owned(),
    };
    vec![
        ConversationSetting {
            id: "model".to_owned(),
            category: "model".to_owned(),
            name: "Model".to_owned(),
            current: "opus".to_owned(),
            picks: vec![
                pick("opus", "Opus 4.5", "Most capable"),
                pick("sonnet", "Sonnet 4.5", "Fast and capable"),
                pick("haiku", "Haiku 4.5", "Fastest"),
                pick("@fast", "@fast", "Uses claude/haiku"),
                pick("@plan", "@plan", "Same as main · claude-opus-4-5"),
            ],
        },
        ConversationSetting {
            id: "mode".to_owned(),
            category: "mode".to_owned(),
            name: "Permissions".to_owned(),
            current: "ask".to_owned(),
            picks: vec![
                pick("ask", "Ask first", ""),
                pick("edits", "Accept edits", ""),
                pick("plan", "Plan only", ""),
            ],
        },
        ConversationSetting {
            id: "effort".to_owned(),
            category: "thought_level".to_owned(),
            name: "Effort".to_owned(),
            current: "high".to_owned(),
            picks: vec![
                pick("low", "Low", ""),
                pick("medium", "Medium", ""),
                pick("high", "High", ""),
            ],
        },
    ]
}

/// The commands the sample agent offers behind a slash.
fn sample_commands() -> Vec<SlashCommand> {
    [
        ("compact", "Summarise the conversation to free context"),
        ("clear", "Start the conversation afresh"),
        ("review", "Review the changes on this branch"),
        ("init", "Write an AGENTS.md for the project"),
    ]
    .into_iter()
    .map(|(name, description)| SlashCommand {
        name: name.to_owned(),
        description: description.to_owned(),
    })
    .collect()
}
