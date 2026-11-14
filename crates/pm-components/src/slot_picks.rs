//! The model slots offered in a conversation's model picker: each slot but `main` as a choice named `@slot`, labelled with the model it resolves to, after the models the agent lists.

use pm_harness::{Pick, Setting, SlotInfo};

/// The category of the settings that name a model.
const MODEL: &str = "model";

/// The slot that is the conversation's own model, which the picker does not offer.
const MAIN: &str = "main";

/// What a pick of a slot says the slot `slot` resolves to: its model, or the conversation's when it inherits.
pub fn resolves_to(slot: &SlotInfo) -> String {
    let resolved = slot.resolved.as_deref().or(slot.model.as_deref());
    match (slot.inherited, resolved) {
        (true, Some(model)) => format!("Same as main · {model}"),
        (true, None) => "Same as main".to_owned(),
        (false, Some(model)) => format!("Uses {model}"),
        (false, None) => "No model".to_owned(),
    }
}

/// `settings` with the picks of every setting naming a model ending with the slots `slots` offer as `@slot`, in place of those it ended with before; settings of other kinds are left as they are.
pub fn with_slot_picks(mut settings: Vec<Setting>, slots: &[SlotInfo]) -> Vec<Setting> {
    for setting in settings
        .iter_mut()
        .filter(|setting| setting.category == MODEL)
    {
        setting.picks.retain(|pick| !pick.value.starts_with('@'));
        setting.picks.extend(
            slots
                .iter()
                .filter(|slot| slot.name != MAIN)
                .map(|slot| Pick {
                    value: format!("@{}", slot.name),
                    name: format!("@{}", slot.name),
                    description: resolves_to(slot),
                }),
        );
    }
    settings
}

#[cfg(test)]
/// Tests for the slots offered in the model picker.
mod tests {
    use super::*;

    /// A slot as the agent tells it, set to `model` or inheriting when there is none.
    fn slot(name: &str, model: Option<&str>) -> SlotInfo {
        SlotInfo {
            name: name.to_owned(),
            model: model.map(str::to_owned),
            backups: Vec::new(),
            resolved: Some(model.unwrap_or("claude-opus-4-5").to_owned()),
            source: if model.is_some() {
                pm_harness::SlotSource::User
            } else {
                pm_harness::SlotSource::Main
            },
            inherited: model.is_none(),
            suggested: None,
            providers: Vec::new(),
        }
    }

    /// A model setting listing one model.
    fn models() -> Setting {
        Setting {
            id: "model".to_owned(),
            category: MODEL.to_owned(),
            name: "Model".to_owned(),
            current: "claude-opus-4-5".to_owned(),
            picks: vec![Pick {
                value: "claude-opus-4-5".to_owned(),
                name: "Opus".to_owned(),
                description: String::new(),
            }],
        }
    }

    /// The slots but `main` follow the models, each labelled with what it resolves to, and telling them again replaces them rather than adding them twice.
    #[test]
    fn slots_follow_the_models_once() {
        let slots = [
            slot("main", None),
            slot("fast", Some("claude/haiku:low")),
            slot("plan", None),
        ];
        let settings = with_slot_picks(vec![models()], &slots);
        let settings = with_slot_picks(settings, &slots);
        let picks: Vec<(&str, &str)> = settings[0]
            .picks
            .iter()
            .map(|pick| (pick.value.as_str(), pick.description.as_str()))
            .collect();
        assert_eq!(
            picks,
            [
                ("claude-opus-4-5", ""),
                ("@fast", "Uses claude/haiku:low"),
                ("@plan", "Same as main · claude-opus-4-5"),
            ]
        );
    }

    /// Settings that do not name a model are given no slots.
    #[test]
    fn other_settings_are_left_alone() {
        let mut mode = models();
        mode.category = "mode".to_owned();
        let settings = with_slot_picks(vec![mode.clone()], &[slot("fast", Some("x"))]);
        assert_eq!(settings, [mode]);
    }
}
