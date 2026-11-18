//! The answers being given to a form of questions the agent asked, and what pressing its buttons does.

use pm_field::Field;
use pm_harness::{Inquiry, Question, Reply};

use crate::kit::FocusId;

use crate::state::{Outgoing, Page, WorkspaceState};

/// The first of the rows picking an answer; the rest follow it.
pub const PICK_BASE: u64 = 5200;

/// The field the reader types an answer of their own in.
pub const OTHER: FocusId = FocusId(5290);

/// Moves on to the next question, or hands the answers to the agent after the last.
pub const NEXT: FocusId = FocusId(5291);

/// Leaves the question unanswered and moves on.
pub const SKIP: FocusId = FocusId(5292);

/// Abandons the form.
pub const CLOSE: FocusId = FocusId(5293);

/// Folds the form down to its title, or opens it again.
pub const FOLD: FocusId = FocusId(5294);

/// Starts typing an answer of the reader's own.
pub const TYPE_OWN: FocusId = FocusId(5295);

/// The first of the buttons that open the mockup of an answer larger; the answer at `i` is `MOCKUP_OPEN_BASE + i`.
pub const MOCKUP_OPEN_BASE: u64 = 5240;

/// The most answers of a question that have a button to open their mockup larger, which is what fits between the buttons' first name and the next name taken.
const MOCKUPS: usize = 40;

/// A form of questions and how far the reader is in answering it.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// What the agent asked.
    pub inquiry: Inquiry,
    /// The question being answered, by position.
    pub step: usize,
    /// The alternatives picked for each question, by position.
    pub picks: Vec<Vec<usize>>,
    /// What was typed as an answer of the reader's own for each question.
    pub others: Vec<Field>,
    /// Whether the form is folded down to its title.
    pub folded: bool,
}

impl Draft {
    /// A form of `inquiry` with no question answered.
    pub fn new(inquiry: Inquiry) -> Self {
        let count = inquiry.questions.len();
        Self {
            inquiry,
            step: 0,
            picks: vec![Vec::new(); count],
            others: vec![Field::default(); count],
            folded: false,
        }
    }

    /// The question being answered.
    pub fn question(&self) -> &Question {
        &self.inquiry.questions[self.step]
    }

    /// Whether the question being answered is the last.
    pub fn last(&self) -> bool {
        self.step + 1 >= self.inquiry.questions.len()
    }

    /// Whether the question being answered has an answer.
    pub fn answered(&self) -> bool {
        !self.picks[self.step].is_empty() || !self.others[self.step].text().trim().is_empty()
    }

    /// Picks the alternative at `option` of the question being answered, or takes the pick back when it was picked.
    fn toggle(&mut self, option: usize) {
        let picks = &mut self.picks[self.step];
        if let Some(at) = picks.iter().position(|held| *held == option) {
            picks.remove(at);
        } else if self.inquiry.questions[self.step].multiple {
            picks.push(option);
        } else {
            *picks = vec![option];
        }
    }

    /// What was answered to each question.
    fn replies(&self) -> Vec<Reply> {
        self.inquiry
            .questions
            .iter()
            .enumerate()
            .map(|(index, question)| Reply {
                picks: self.picks[index]
                    .iter()
                    .filter_map(|pick| question.alternatives.get(*pick))
                    .map(|alternative| alternative.label.clone())
                    .collect(),
                other: self.others[index].text().to_owned(),
            })
            .collect()
    }
}

impl WorkspaceState {
    /// The form the open session's agent waits on, if it waits on one.
    pub fn draft(&self) -> Option<&Draft> {
        self.open().and_then(|session| session.inquiry.as_ref())
    }

    /// The form of the open session, to change.
    fn draft_mut(&mut self) -> Option<&mut Draft> {
        match self.page {
            Page::Session(open) => self.sessions.get_mut(open)?.inquiry.as_mut(),
            _ => None,
        }
    }

    /// The field the reader types their own answer in, while a form is open.
    pub fn other_field(&self) -> Option<&Field> {
        self.draft().map(|draft| &draft.others[draft.step])
    }

    /// What pressing the key for the number `digit` presses, while a form is open and not folded.
    pub fn inquiry_hotkey(&self, digit: usize) -> Option<FocusId> {
        let draft = self.draft().filter(|draft| !draft.folded)?;
        let count = draft.question().alternatives.len();
        match digit.checked_sub(1)? {
            index if index < count => Some(FocusId(PICK_BASE + index as u64)),
            index if index == count => Some(TYPE_OWN),
            _ => None,
        }
    }

    /// What pressing the button `id` does to the open form, when it is one of the form's.
    pub fn press_inquiry(&mut self, id: FocusId) -> bool {
        let FocusId(number) = id;
        let options = self
            .draft()
            .map_or(0, |draft| draft.question().alternatives.len());
        if let Some(index) = number
            .checked_sub(PICK_BASE)
            .map(|offset| offset as usize)
            .filter(|index| *index < options)
        {
            if let Some(draft) = self.draft_mut() {
                draft.toggle(index);
            }
            return true;
        }
        if let Some(index) = number
            .checked_sub(MOCKUP_OPEN_BASE)
            .map(|offset| offset as usize)
            .filter(|index| *index < options.min(MOCKUPS))
        {
            self.open_mockup(index);
            return true;
        }
        match id {
            NEXT => self.advance(true),
            SKIP => self.advance(false),
            CLOSE => self.abandon(),
            FOLD => {
                if let Some(draft) = self.draft_mut() {
                    draft.folded = !draft.folded;
                }
            }
            TYPE_OWN => {}
            _ => return false,
        }
        true
    }

    /// Opens the mockup of the answer at `option` of the question being answered, larger and live, in a tab of the session's browser that is sealed from the network; nothing when the answer has none.
    fn open_mockup(&mut self, option: usize) {
        let pictured = self.draft().and_then(|draft| {
            let alternative = draft.question().alternatives.get(option)?;
            Some((alternative.label.clone(), alternative.mockup.clone()?))
        });
        if let Some((label, html)) = pictured {
            self.open_sealed_document(crate::inquiry::recommended(&label).0, &html);
        }
    }

    /// Moves to the next question, keeping what was answered to this one when `keep`, and hands the answers over after the last.
    fn advance(&mut self, keep: bool) {
        let Some(draft) = self.draft_mut() else {
            return;
        };
        if keep && !draft.answered() {
            return;
        }
        if !keep {
            let step = draft.step;
            draft.picks[step].clear();
            draft.others[step] = Field::default();
        }
        if !draft.last() {
            draft.step += 1;
            return;
        }
        self.hand_over(true);
    }

    /// Abandons the form, which aborts what the agent was doing with it.
    fn abandon(&mut self) {
        self.hand_over(false);
    }

    /// Takes the form off the open session and queues its answers, or its abandoning when `answers` is false.
    fn hand_over(&mut self, answers: bool) {
        let Page::Session(open) = self.page else {
            return;
        };
        let Some(draft) = self
            .sessions
            .get_mut(open)
            .and_then(|session| session.inquiry.take())
        else {
            return;
        };
        self.outbox.push(Outgoing::Answer {
            session: open,
            ticket: draft.inquiry.ticket,
            replies: answers.then(|| draft.replies()),
        });
    }
}

#[cfg(test)]
/// Tests for answering a form whose answers have mockups.
mod tests {
    use pm_harness::{Alternative, Event};

    use super::*;
    use crate::state::Tab;

    /// The sample workspace with its first session open and waiting on one question with three answers, two of them with mockups, picked one at a time or several as `multiple` says.
    fn asked(multiple: bool) -> WorkspaceState {
        let alternative = |label: &str, mockup: Option<&str>| Alternative {
            label: label.to_owned(),
            description: String::new(),
            mockup: mockup.map(str::to_owned),
        };
        let mut state = WorkspaceState::sample();
        state.page = Page::Session(0);
        state.hear_in_place(
            0,
            Event::Inquire(Inquiry {
                ticket: 4,
                questions: vec![Question {
                    header: "Layout".to_owned(),
                    text: "Which layout?".to_owned(),
                    multiple,
                    alternatives: vec![
                        alternative("Sidebar", Some("<p>a</p>")),
                        alternative("Tabs", Some("<p>b</p>")),
                        alternative("Neither", None),
                    ],
                }],
            }),
        );
        state
    }

    /// Pressing a tile picks its answer as pressing a row does: one at a time for a single choice, a second press takes the pick back, and the key for a number presses the same tile.
    #[test]
    fn a_tile_picks_one_answer_of_a_single_choice() {
        let mut state = asked(false);
        assert!(state.press_inquiry(FocusId(PICK_BASE)));
        assert!(state.press_inquiry(FocusId(PICK_BASE + 1)));
        assert_eq!(state.draft().unwrap().picks[0], vec![1]);
        assert!(state.press_inquiry(FocusId(PICK_BASE + 1)));
        assert!(state.draft().unwrap().picks[0].is_empty());
        assert_eq!(state.inquiry_hotkey(2), Some(FocusId(PICK_BASE + 1)));
        assert_eq!(state.inquiry_hotkey(4), Some(TYPE_OWN));
    }

    /// A question that takes several answers keeps every tile pressed, and the answers go to the agent by their labels.
    #[test]
    fn tiles_of_a_multiple_choice_are_picked_together() {
        let mut state = asked(true);
        state.press_inquiry(FocusId(PICK_BASE));
        state.press_inquiry(FocusId(PICK_BASE + 2));
        assert_eq!(state.draft().unwrap().picks[0], vec![0, 2]);
        state.press_inquiry(NEXT);
        assert!(state.draft().is_none());
        assert!(state.outbox.iter().any(|asked| matches!(
            asked,
            Outgoing::Answer { ticket: 4, replies: Some(replies), .. }
                if replies[0].picks == ["Sidebar", "Neither"]
        )));
    }

    /// The button on a tile opens its mockup in a sealed tab of the browser without picking the answer, and an answer with no mockup has nothing to open.
    #[test]
    fn a_mockup_opens_larger_without_picking_its_answer() {
        let mut state = asked(false);
        assert!(state.press_inquiry(FocusId(MOCKUP_OPEN_BASE + 1)));
        assert!(state.draft().unwrap().picks[0].is_empty());
        let browser = state.browser.as_ref().unwrap();
        assert_eq!(&*browser.sealed.as_ref().unwrap().html, "<p>b</p>");
        assert_eq!(state.tab, Tab::Browser);
        let tabs = browser.tabs.len();
        assert!(state.press_inquiry(FocusId(MOCKUP_OPEN_BASE + 2)));
        assert_eq!(state.browser.as_ref().unwrap().tabs.len(), tabs);
        assert!(!state.press_inquiry(FocusId(MOCKUP_OPEN_BASE + 3)));
    }
}
