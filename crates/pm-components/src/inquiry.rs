//! The card asking the reader the agent's form of questions: each question
//! with the answers offered and room for their own words, then a button to
//! send the answers and one to skip the form.

use std::rc::Rc;

use rustle::prelude::*;

use crate::button::{Button, ButtonVariant};
use crate::card::{Card, CardVariant};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::text_input::TextInput;
use rustle::color_scheme;

/// One question of the form.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InquiryPrompt {
    /// A short tag naming it.
    pub header: String,
    /// The question.
    pub text: String,
    /// Whether more than one answer can be picked.
    pub multiple: bool,
    /// The answers offered: each a label and what it means.
    pub options: Vec<(String, String)>,
}

/// What was answered to one question: the labels picked and the words written.
pub type InquiryAnswer = (Vec<String>, String);

/// The visual treatment of the inquiry card.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InquiryVariant {
    /// On an outlined card, focused.
    #[default]
    Card,
    /// Flat, without a card.
    Flat,
}

/// The card asking a form of questions.
#[derive(Clone)]
pub struct InquiryCard {
    /// The questions, in order.
    questions: Vec<InquiryPrompt>,
    /// What sending does with the answers, one per question.
    on_submit: Option<Rc<dyn Fn(Vec<InquiryAnswer>)>>,
    /// What skipping the form does.
    on_skip: Option<Rc<dyn Fn()>>,
    /// Its visual treatment.
    variant: InquiryVariant,
    /// Its text size.
    size: Size,
}

impl InquiryCard {
    /// A card asking `questions`.
    pub fn new(questions: Vec<InquiryPrompt>) -> Self {
        Self {
            questions,
            on_submit: None,
            on_skip: None,
            variant: InquiryVariant::Card,
            size: Size::Sm,
        }
    }

    /// The card running `on_submit` with the answers when sent.
    pub fn on_submit(mut self, on_submit: impl Fn(Vec<InquiryAnswer>) + 'static) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }

    /// The card running `on_skip` when the form is skipped.
    pub fn on_skip(mut self, on_skip: impl Fn() + 'static) -> Self {
        self.on_skip = Some(Rc::new(on_skip));
        self
    }

    /// The card with `variant`.
    pub fn variant(mut self, variant: InquiryVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The card at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the card.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let count = self.questions.len();
        let answers = remember(move || vec![(Vec::<String>::new(), String::new()); count]);
        let body = move || {
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(8.0),
                Alignment::Center,
                || {
                    icon(phosphor::CHAT_CIRCLE_DOTS, 15.0, scheme.primary);
                    Label::new("The agent asks")
                        .variant(LabelVariant::Heading)
                        .size(self.size)
                        .show();
                },
            );
            for (index, question) in self.questions.iter().enumerate() {
                let picked = answers.get().get(index).cloned().unwrap_or_default();
                column_arranged(
                    Modifier::new().fill_max_width(),
                    Arrangement::SpacedBy(6.0),
                    Alignment::Start,
                    || {
                        if !question.header.is_empty() {
                            Label::new(question.header.to_uppercase())
                                .variant(LabelVariant::Subtle)
                                .size(Size::Xs)
                                .show();
                        }
                        Label::new(question.text.clone())
                            .size(self.size)
                            .wrap()
                            .show();
                        for (label, meaning) in &question.options {
                            let chosen = picked.0.contains(label);
                            let answers = answers.clone();
                            let label_owned = label.clone();
                            let multiple = question.multiple;
                            row_arranged(
                                Modifier::new()
                                    .fill_max_width()
                                    .background_rounded(
                                        if chosen {
                                            scheme.secondary_container
                                        } else {
                                            scheme.surface_container
                                        },
                                        6.0,
                                    )
                                    .border(
                                        1.0,
                                        if chosen {
                                            scheme.primary
                                        } else {
                                            scheme.outline_variant
                                        },
                                        6.0,
                                    )
                                    .padding_xy(10.0, 6.0)
                                    .cursor(CursorIcon::Pointer)
                                    .clickable(move || {
                                        let mut all = answers.get();
                                        if let Some((picks, _)) = all.get_mut(index) {
                                            pick(picks, &label_owned, multiple);
                                        }
                                        answers.set(all);
                                    }),
                                Arrangement::SpacedBy(8.0),
                                Alignment::Center,
                                || {
                                    let mark = match (question.multiple, chosen) {
                                        (true, true) => phosphor::CHECK_SQUARE,
                                        (true, false) => phosphor::SQUARE,
                                        (false, true) => phosphor::RADIO_BUTTON,
                                        (false, false) => phosphor::CIRCLE,
                                    };
                                    icon(
                                        mark,
                                        14.0,
                                        if chosen {
                                            scheme.primary
                                        } else {
                                            scheme.on_surface_variant
                                        },
                                    );
                                    Label::new(label.clone()).size(self.size).show();
                                    if !meaning.is_empty() {
                                        Label::new(meaning.clone())
                                            .variant(LabelVariant::Subtle)
                                            .size(Size::Xs)
                                            .modifier(Modifier::new().weight(1.0))
                                            .show();
                                    }
                                },
                            );
                        }
                        let answers = answers.clone();
                        TextInput::new(picked.1.clone())
                            .placeholder("Or answer in your own words")
                            .size(Size::Sm)
                            .modifier(Modifier::new().fill_max_width())
                            .on_change(move |text| {
                                let mut all = answers.get();
                                if let Some((_, other)) = all.get_mut(index) {
                                    *other = text;
                                }
                                answers.set(all);
                            })
                            .show();
                    },
                );
            }
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(8.0),
                Alignment::Center,
                || {
                    spacer(Modifier::new().weight(1.0));
                    if let Some(on_skip) = self.on_skip.clone() {
                        Button::new("Skip")
                            .variant(ButtonVariant::Ghost)
                            .size(Size::Sm)
                            .on_click(move || on_skip())
                            .show();
                    }
                    let on_submit = self.on_submit.clone();
                    let answers = answers.clone();
                    Button::new("Send answers")
                        .variant(ButtonVariant::Primary)
                        .size(Size::Sm)
                        .on_click(move || {
                            if let Some(on_submit) = &on_submit {
                                on_submit(answers.get());
                            }
                        })
                        .show();
                },
            );
        };
        let column = move || {
            column_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(14.0),
                Alignment::Start,
                body,
            )
        };
        match self.variant {
            InquiryVariant::Card => Card::new()
                .variant(CardVariant::Outlined)
                .size(Size::Md)
                .focused(true)
                .modifier(Modifier::new().fill_max_width())
                .show(column),
            InquiryVariant::Flat => column(),
        }
    }
}

/// Picks or unpicks `label` among `picks`; a question with one answer keeps only the last.
fn pick(picks: &mut Vec<String>, label: &str, multiple: bool) {
    if let Some(at) = picks.iter().position(|picked| picked == label) {
        picks.remove(at);
    } else {
        if !multiple {
            picks.clear();
        }
        picks.push(label.to_owned());
    }
}
