//! The orchestration page of the settings window: the switch that starts new conversations in orchestrator mode, and the work categories the agent then hands work to, each a row like a slot of the models page with its model, effort, backups, description and prompt addition.

use crate::areas::SettingsEvent;
use crate::target::Target;
use crate::theme;
use rustle::{Alignment, Arrangement, Modifier};

use crate::settings::Section;
use crate::settings_servers::{boxed, heading_only, note, paragraph};
use crate::settings_slots::{
    SlotRow, entry, error_note, provider_choice, scope_choice, slot_row, split_effort,
};
use crate::state::WorkspaceState;

/// What the page says about itself, for the search and the overview.
pub(crate) const SUMMARY: &str = "Orchestrator mode makes the agent a lead that hands each piece of work to a subagent by category.";

/// What the page says the categories are, above their rows.
const CATEGORIES_NOTE: &str = "The kinds of work the agent picks from in orchestrator mode. A subagent started in a category asks that category's model, with its prompt addition; a category without a model asks the lead's.";

/// One work category as the page lists it for the provider viewed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CategoryCard {
    /// The category's name.
    pub name: String,
    /// Its place among the slots, which names its controls.
    pub row: usize,
    /// The model its subagents ask, without the effort; nothing when it sets none and they ask the lead's.
    pub model: Option<String>,
    /// The effort its model is asked with, when the category sets one.
    pub effort: Option<String>,
    /// The name of the provider whose own assignment the model is, when it is not the one for every provider.
    pub provider: Option<String>,
    /// What kind of work it is for, when it says.
    pub description: Option<String>,
}

impl WorkspaceState {
    /// The work categories the orchestration page shows, in order, each as it stands for the provider viewed: that provider's own model when it has one, else the one for every provider, else the one the harness resolved from elsewhere.
    pub fn category_cards(&self) -> Vec<CategoryCard> {
        let page = &self.settings_page.slots;
        let viewed = self.slot_provider();
        let provider_name = viewed.as_ref().map(|viewed| {
            self.slot_providers()
                .into_iter()
                .find(|(id, _)| id == viewed)
                .map_or_else(|| viewed.clone(), |(_, name)| name)
        });
        self.slot_rows()
            .into_iter()
            .enumerate()
            .filter(|(_, slot)| slot.category)
            .map(|(row, slot)| {
                let told = page.slots.iter().find(|held| held.name == slot.name);
                let own = slot.model.clone().filter(|_| viewed.is_some());
                let selector = own
                    .clone()
                    .or_else(|| told.and_then(|told| told.model.clone()))
                    .or_else(|| {
                        told.filter(|told| !told.inherited)
                            .and_then(|told| told.resolved.clone())
                    });
                let (model, effort) = match selector.as_deref().map(split_effort) {
                    Some((model, effort)) => (Some(model.to_owned()), effort.map(str::to_owned)),
                    None => (None, None),
                };
                let written = page
                    .notes_of(&slot.name)
                    .map(|notes| notes.description.text().trim().to_owned())
                    .filter(|written| !written.is_empty());
                CategoryCard {
                    name: slot.name.clone(),
                    row,
                    model,
                    effort,
                    provider: own.and(provider_name.clone()),
                    description: written.or(slot.description),
                }
            })
            .collect()
    }
}

/// Emits the body of the orchestration page, `room` logical pixels across, after what `page` emits, into the box the caller holds it in: the switch for new conversations, the provider whose assignments are shown, why the last change failed, the work categories as rows like the slots of the models page, and a box to add a category.
#[track_caller]
pub(crate) fn body(state: &WorkspaceState, page: impl FnOnce(), room: f32) {
    let slots = &state.settings_page.slots;
    let rows: Vec<(usize, SlotRow)> = state
        .slot_rows()
        .into_iter()
        .enumerate()
        .filter(|(_, slot)| slot.category)
        .collect();
    page();
    crate::settings_pages::page_rows(state, Section::Orchestration, room);
    provider_choice(state);
    error_note(state, room);
    rustle::flow_row_with(
        Modifier::new().fill_max_width(),
        rustle::FlowOptions::new(theme::sp(2), theme::sp(2))
            .item_alignment(Alignment::Center)
            .arrangement(Arrangement::SpaceBetween),
        || {
            heading_only("Categories", rows.len());
            scope_choice(state);
        },
    );
    paragraph(CATEGORIES_NOTE, room);
    if !rows.is_empty() {
        boxed(|| {
            for (place, slot) in &rows {
                rustle::key(*place, || slot_row(state, *place, slot, room));
            }
        });
    } else if slots.slots.is_empty() {
        note("The agent has not told its slots yet.");
    } else {
        note("No categories yet. Add one below.");
    }
    entry(
        &slots.new_category,
        Target::Settings(SettingsEvent::CategoryNewName),
        "Add a category of your own, e.g. docs",
        ("Add Category", Target::Settings(SettingsEvent::CategoryAdd)),
    );
}

/// How many work categories there are, in words, for the page's card on the Agents overview.
pub(crate) fn count(state: &WorkspaceState) -> String {
    match state.category_cards().len() {
        1 => "1 category".to_owned(),
        count => format!("{count} categories"),
    }
}

#[cfg(test)]
/// Tests for the orchestration page: asking for the slots, the cards of the categories, opening one and describing it, and the switch for new conversations.
mod tests {
    use pm_harness::{SlotBook, SlotChange, SlotInfo, SlotProvider, SlotScope, SlotSource};

    use super::*;
    use crate::settings_catalog::Setting;
    use crate::settings_rows::SLOT_CONTROLS;
    use crate::settings_slots::{DESCRIPTION_COLUMN, SLOT_COLUMNS, card_id};
    use crate::state::Outgoing;

    /// A slot as the harness tells it: a work category when `category`, set to `model` from the user's file or inheriting, and saying `description`.
    fn slot(
        name: &str,
        category: bool,
        model: Option<&str>,
        description: Option<&str>,
    ) -> SlotInfo {
        SlotInfo {
            name: name.to_owned(),
            model: model.map(str::to_owned),
            backups: Vec::new(),
            resolved: Some(model.unwrap_or("claude-opus-4-5").to_owned()),
            source: if model.is_some() {
                SlotSource::User
            } else {
                SlotSource::Main
            },
            inherited: model.is_none(),
            suggested: None,
            providers: Vec::new(),
            description: description.map(str::to_owned),
            prompt: None,
            category,
        }
    }

    /// The state with the settings window open on the orchestration page, all providers viewed, holding the job slots `main` and `fast` and the categories `quick`, described only, `deep`, with a model and an effort, and `docs`, with neither, and nothing waiting to be sent.
    fn open() -> WorkspaceState {
        let mut state = WorkspaceState::new(&[])
            .with_settings()
            .settings_pressed(Section::Orchestration.id())
            .with_model_slots(SlotBook {
                slots: vec![
                    slot("main", false, None, None),
                    slot("fast", false, Some("claude/haiku:low"), None),
                    slot("quick", true, None, Some("Small changes.")),
                    slot(
                        "deep",
                        true,
                        Some("claude/opus:high"),
                        Some("Hard problems."),
                    ),
                    slot("docs", true, None, None),
                ],
                presets: Vec::new(),
            });
        state.settings_page.slots.view = crate::settings_slots::SlotView::Any;
        state.take_outbox();
        state
    }

    /// Opening the page asks for the slots, as the models page does.
    #[test]
    fn opening_the_page_asks_for_the_slots() {
        let mut state = WorkspaceState::new(&[]).with_settings();
        state.take_outbox();
        let mut state = state.settings_pressed(Section::Orchestration.id());
        assert_eq!(state.settings, Some(Section::Orchestration));
        assert!(
            state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::ModelSlots { change: None, .. }))
        );
    }

    /// The cards are the categories only, each with its model, effort and description; a category without a model says nothing of one, so its card reads as the lead's.
    #[test]
    fn cards_list_only_the_categories() {
        let state = open();
        let cards = state.category_cards();
        let names: Vec<&str> = cards.iter().map(|card| card.name.as_str()).collect();
        assert_eq!(names, ["quick", "deep", "docs"]);
        assert_eq!(cards[0].model, None);
        assert_eq!(cards[0].description.as_deref(), Some("Small changes."));
        assert_eq!(cards[1].model.as_deref(), Some("claude/opus"));
        assert_eq!(cards[1].effort.as_deref(), Some("high"));
        assert_eq!(cards[1].provider, None);
        assert_eq!(cards[1].row, 3);
        assert_eq!(cards[2].description, None);
        assert_eq!(count(&state), "3 categories");
        let models: Vec<String> = state
            .model_rows()
            .into_iter()
            .map(|(_, row)| row.name)
            .collect();
        assert_eq!(models, ["main", "fast"]);
    }

    /// A card names the provider only when the model is that provider's own assignment.
    #[test]
    fn a_provider_specific_model_is_badged() {
        let mut state = open();
        state.settings_page.slots.slots[3].providers = vec![SlotProvider {
            provider: "codex".to_owned(),
            model: Some("codex/gpt-5:low".to_owned()),
            backups: Vec::new(),
            suggested: None,
        }];
        state.settings_page.slots.view =
            crate::settings_slots::SlotView::Provider("codex".to_owned());
        let cards = state.category_cards();
        assert_eq!(cards[1].model.as_deref(), Some("codex/gpt-5"));
        assert_eq!(cards[1].effort.as_deref(), Some("low"));
        assert_eq!(cards[1].provider.as_deref(), Some("codex"));
        assert_eq!(cards[0].provider, None);
    }

    /// Pressing a card opens its editor, pressing another moves it there, and pressing the open one closes it: one is open at a time.
    #[test]
    fn one_card_is_open_at_a_time() {
        let open_of = |state: &WorkspaceState| state.settings_page.slots.expanded.clone();
        let state = open().settings_pressed(card_id(3).unwrap());
        assert_eq!(open_of(&state).as_deref(), Some("deep"));
        let state = state.settings_pressed(card_id(2).unwrap());
        assert_eq!(open_of(&state).as_deref(), Some("quick"));
        let state = state.settings_pressed(card_id(2).unwrap());
        assert_eq!(open_of(&state), None);
        let state = state.settings_pressed(card_id(1).unwrap());
        assert_eq!(open_of(&state), None);
    }

    /// Opening a card and editing its description sends the description when the box is left, and the card shows what was written meanwhile.
    #[test]
    fn editing_an_open_card_describes_the_category() {
        let description = SLOT_CONTROLS
            .cell(2, DESCRIPTION_COLUMN, SLOT_COLUMNS)
            .unwrap();
        assert!(WorkspaceState::saves_on_leaving(description));
        let state = open()
            .settings_pressed(card_id(2).unwrap())
            .typed(description, " Fast.");
        assert_eq!(
            state.category_cards()[0].description.as_deref(),
            Some("Small changes. Fast.")
        );
        let mut state = state.field_left(description);
        let changes: Vec<SlotChange> = state
            .take_outbox()
            .into_iter()
            .filter_map(|outgoing| match outgoing {
                Outgoing::ModelSlots { change, .. } => change,
                _ => None,
            })
            .collect();
        assert_eq!(
            changes,
            [SlotChange::Describe {
                name: "quick".to_owned(),
                scope: SlotScope::User,
                description: Some("Small changes. Fast.".to_owned()),
                prompt: None,
            }]
        );
    }

    /// The switch on the page flips the preference new conversations start with, and it is the only place the setting is shown.
    #[test]
    fn the_switch_flips_the_preference() {
        assert_eq!(Setting::Orchestrator.page(), Section::Orchestration);
        let state = open();
        assert!(!state.machine.agents.orchestrator);
        let state = state.settings_pressed(Setting::Orchestrator.id(0).unwrap());
        assert!(state.machine.agents.orchestrator);
        let state = state.settings_pressed(Setting::Orchestrator.id(0).unwrap());
        assert!(!state.machine.agents.orchestrator);
    }

    /// Searching the settings for any of the words the page goes by finds its switch.
    #[test]
    fn the_page_is_found_by_its_words() {
        for word in [
            "orchestration",
            "orchestrator",
            "category",
            "categories",
            "delegate",
        ] {
            assert_eq!(
                crate::settings_catalog::found(word),
                [Setting::Orchestrator],
                "{word}"
            );
        }
    }
}
