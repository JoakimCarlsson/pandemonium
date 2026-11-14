//! The model slots the harness keeps: the named jobs a conversation asks a model for, such as `fast` for summaries and `plan` for plan mode, each with backups, and the presets that set them all at once.
//!
//! The harness reads and writes the files that hold them. This module asks it
//! to, either through a conversation of its own that opens nothing, which
//! reaches the user's file alone, or through a running chat, whose trusted
//! project also has a file of its own.

use std::time::{Duration, Instant};

use omc_wire::{Command, Event};
pub use omc_wire::{PresetInfo, PresetSlot, SlotInfo, SlotProvider, SlotScope, SlotSource};

use crate::inprocess::{Harness, Heard};

/// How long the harness is given to answer a question about the slots.
const PATIENCE: Duration = Duration::from_secs(10);

/// How long one look at the harness waits for an event.
const GLANCE: Duration = Duration::from_millis(20);

/// The slots built into the harness, in the order it lists them, with what asks each one's model.
pub const BUILT_IN_SLOTS: [(&str, &str); 6] = [
    ("main", "The conversation's own turns."),
    (
        "fast",
        "Summaries when the conversation is compacted, questions on the side and handoff prompts.",
    ),
    ("plan", "Turns in plan mode."),
    (
        "subagent",
        "Subagents started without a model of their own.",
    ),
    ("review", "Reviews, such as judging a goal; not asked yet."),
    (
        "vision",
        "Describing images to a model that cannot read them; not asked yet.",
    ),
];

/// What a slot is for, in a line: the description of a built-in slot, or of one the reader added.
pub fn slot_purpose(name: &str) -> &'static str {
    BUILT_IN_SLOTS
        .iter()
        .find(|(held, _)| *held == name)
        .map_or(
            "A slot of your own, named as @name wherever a model is named.",
            |(_, purpose)| purpose,
        )
}

/// A change to the slots or the presets, as the reader asks for it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotChange {
    /// Sets the assignment of `provider`, or the any-provider one when it is nothing, of the slot `name` in the file of `scope` to `model` and `backups`; a name that is not built in makes a slot of the reader's own.
    Set {
        /// The slot.
        name: String,
        /// The file written.
        scope: SlotScope,
        /// The model selector, as `provider/model` with `:effort` when one is given; nothing keeps only the backups.
        model: Option<String>,
        /// The selectors tried in order when the model fails.
        backups: Vec<String>,
        /// The provider id whose assignment is set, such as `codex`; nothing sets the any-provider one.
        #[serde(default)]
        provider: Option<String>,
    },
    /// Takes the assignment of `provider`, or the any-provider one when it is nothing, of the slot `name` out of the file of `scope`, so it inherits again.
    Clear {
        /// The slot.
        name: String,
        /// The file changed.
        scope: SlotScope,
        /// The provider id whose assignment is taken out; nothing takes out the any-provider one.
        #[serde(default)]
        provider: Option<String>,
    },
    /// Saves every slot as it stands as the preset `name`, with `effort` or the conversation's.
    SavePreset {
        /// The preset.
        name: String,
        /// The effort kept with it, such as `high`.
        effort: Option<String>,
    },
    /// Replaces every slot of the file of `scope` with those of the preset `name`.
    ApplyPreset {
        /// The preset.
        name: String,
        /// The file whose slots are replaced.
        scope: SlotScope,
    },
    /// Forgets the preset `name`.
    DeletePreset {
        /// The preset.
        name: String,
    },
}

impl SlotChange {
    /// The command that asks the harness for the change.
    fn command(self) -> Command {
        match self {
            Self::Set {
                name,
                scope,
                model,
                backups,
                provider,
            } => Command::SetSlot {
                name,
                scope,
                model,
                backups,
                provider,
            },
            Self::Clear {
                name,
                scope,
                provider,
            } => Command::ClearSlot {
                name,
                scope,
                provider,
            },
            Self::SavePreset { name, effort } => Command::SavePreset { name, effort },
            Self::ApplyPreset { name, scope } => Command::ApplyPreset { name, scope },
            Self::DeletePreset { name } => Command::DeletePreset { name },
        }
    }
}

/// The slots and the presets as the harness holds them.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SlotBook {
    /// Every slot, the built-in ones first, then the reader's by name.
    pub slots: Vec<SlotInfo>,
    /// Every preset, by name.
    pub presets: Vec<PresetInfo>,
}

/// Makes `change`, when there is one, then reads the slots and the presets, through a conversation of its own that opens nothing, so only the user's file and the environment are seen; a change to the project's file fails, saying so.
///
/// # Errors
///
/// Fails with the harness's reason when the change is refused, or when it cannot be started or does not answer.
pub fn model_slots(change: Option<SlotChange>) -> Result<SlotBook, String> {
    let mut harness = Harness::start()?;
    exchange(&mut harness, change).0
}

/// Makes `change` through `harness`, when there is one, then reads the slots and the presets; also gives back everything else the harness said meanwhile, the slots it told included, for the conversation to take in.
pub(crate) fn exchange(
    harness: &mut Harness,
    change: Option<SlotChange>,
) -> (Result<SlotBook, String>, Vec<Event>) {
    let mut heard = Vec::new();
    let answer = ask(harness, change, &mut heard);
    (answer, heard)
}

/// What [`exchange`] answers, with what else the harness said put in `heard`.
fn ask(
    harness: &mut Harness,
    change: Option<SlotChange>,
    heard: &mut Vec<Event>,
) -> Result<SlotBook, String> {
    if let Some(change) = change {
        harness.send(&change.command())?;
        wait_for_change(harness, heard)?;
    }
    harness.send(&Command::Slots)?;
    harness.send(&Command::Presets)?;
    let mut slots = None;
    let mut presets = None;
    let deadline = Instant::now() + PATIENCE;
    while slots.is_none() || presets.is_none() {
        match next(harness, deadline)? {
            Event::Slots(told) => {
                slots = Some(told.clone());
                heard.push(Event::Slots(told));
            }
            Event::Presets(told) => presets = Some(told),
            other => heard.push(other),
        }
    }
    Ok(SlotBook {
        slots: slots.unwrap_or_default(),
        presets: presets.unwrap_or_default(),
    })
}

/// Waits for the harness to answer a change: with the slots or the presets as they now stand, or with why it refused.
///
/// # Errors
///
/// Fails with the harness's reason when it refused, or when it does not answer in time.
fn wait_for_change(harness: &Harness, heard: &mut Vec<Event>) -> Result<(), String> {
    let deadline = Instant::now() + PATIENCE;
    loop {
        match next(harness, deadline)? {
            Event::Failed(reason) => return Err(reason),
            Event::Slots(told) => {
                heard.push(Event::Slots(told));
                return Ok(());
            }
            Event::Presets(_) => return Ok(()),
            other => heard.push(other),
        }
    }
}

/// The next event of `harness`, waiting until `deadline` at most.
///
/// # Errors
///
/// Fails when the deadline passes or the conversation ends.
fn next(harness: &Harness, deadline: Instant) -> Result<Event, String> {
    loop {
        if Instant::now() >= deadline {
            return Err("the agent did not answer about its model slots".to_owned());
        }
        match harness.listen(GLANCE) {
            Heard::Event(event) => return Ok(event),
            Heard::Silence => {}
            Heard::Closed(reason) => return Err(reason),
        }
    }
}

#[cfg(test)]
/// Tests for the changes to the slots and what they are sent as.
mod tests {
    use super::*;

    /// Each change becomes the command the harness reads.
    #[test]
    fn changes_become_commands() {
        let set = SlotChange::Set {
            name: "fast".to_owned(),
            scope: SlotScope::Project,
            model: Some("claude/haiku:low".to_owned()),
            backups: vec!["openai/gpt-5-mini".to_owned()],
            provider: Some("claude".to_owned()),
        };
        assert_eq!(
            set.command(),
            Command::SetSlot {
                name: "fast".to_owned(),
                scope: SlotScope::Project,
                model: Some("claude/haiku:low".to_owned()),
                backups: vec!["openai/gpt-5-mini".to_owned()],
                provider: Some("claude".to_owned()),
            }
        );
        assert_eq!(
            SlotChange::DeletePreset {
                name: "cheap".to_owned()
            }
            .command(),
            Command::DeletePreset {
                name: "cheap".to_owned()
            }
        );
    }

    /// The built-in slots say what they are for, and any other slot says it is the reader's own.
    #[test]
    fn slots_say_what_they_are_for() {
        assert!(slot_purpose("fast").contains("Summaries"));
        assert!(slot_purpose("docs").contains("of your own"));
    }
}
