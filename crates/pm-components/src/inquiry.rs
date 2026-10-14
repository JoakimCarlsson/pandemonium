//! The card that puts the agent's questions to the reader one at a time, each with the answers it offers and a box for an answer of the reader's own.

use pm_acp::Alternative;
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, flex_1, flex_col, flex_none,
    flex_wrap, gap, items_center, justify_center, justify_end, min_w_px, px, py, rounded,
    rounded_full, self_start, size, w_full,
};
use pm_ui::{Node, node};

use crate::Button;
use crate::button::{Size, Variant};
use crate::draft::{CLOSE, Draft, FOLD, NEXT, OTHER, PICK_BASE, SKIP};
use crate::input::{self, Input};
use crate::parts::line;

/// The small box holding the number of the key that picks a row.
fn key(number: usize) -> Node {
    node()
        .styles([
            flex(),
            flex_none(),
            items_center(),
            justify_center(),
            size(7),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Background),
        ])
        .child(line(number.to_string(), 13, 1.4, Role::Muted))
}

/// The row offering `alternative`, which the key `number` picks, drawn as taken when `picked`, answering to `id`.
fn row(alternative: &Alternative, number: usize, picked: bool, id: FocusId) -> Node {
    let mut words = node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0)])
        .child(line(&alternative.label, 15, 1.4, Role::Text));
    if !alternative.description.is_empty() {
        words = words.child(line(&alternative.description, 14, 1.4, Role::Muted));
    }
    node()
        .styles([
            flex(),
            items_center(),
            gap(3),
            px(4),
            py(3),
            rounded(Radius::Lg),
            border(),
            border_color(if picked { Role::Accent } else { Role::Surface }),
            bg(Role::SurfaceHover),
            cursor_pointer(),
            bg(Role::SurfaceActive).hover(),
        ])
        .child(words)
        .child(key(number))
        .focusable(id)
}

/// The row for an answer of the reader's own, with the box to type it in.
fn other(draft: &Draft, number: usize) -> Node {
    let field = &draft.others[draft.step];
    let typed = Input::new(field, OTHER)
        .variant(input::Variant::Ghost)
        .placeholder("Type your own answer here");
    node()
        .styles([
            flex(),
            flex_col(),
            gap(2.5),
            px(4),
            py(3),
            rounded(Radius::Lg),
            border(),
            border_color(if field.text().trim().is_empty() {
                Role::Surface
            } else {
                Role::Accent
            }),
            bg(Role::SurfaceHover),
        ])
        .child(
            node()
                .styles([flex(), items_center(), gap(3)])
                .child(
                    node()
                        .styles([flex_1()])
                        .child(line("Other", 15, 1.4, Role::Text)),
                )
                .child(key(number)),
        )
        .child(
            node()
                .styles([w_full(), rounded(Radius::Lg), bg(Role::Background)])
                .child(typed.build()),
        )
}

/// The card for `draft`: the question, the answers to pick from, and the buttons that skip it or move on, or only its title when folded.
pub fn inquiry(draft: &Draft) -> Node {
    let question = draft.question();
    let count = draft.inquiry.questions.len();
    let mut title = node().styles([flex(), items_center(), gap(3)]);
    if count > 1 {
        title = title.child(
            node()
                .styles([
                    flex(),
                    flex_none(),
                    items_center(),
                    px(3),
                    py(1),
                    rounded_full(),
                    bg(Role::SurfaceHover),
                ])
                .child(line(
                    format!("{}/{count}", draft.step + 1),
                    13,
                    1.4,
                    Role::Muted,
                )),
        );
    }
    let title = title
        .child(node().styles([flex_1(), min_w_px(0)]).child(line(
            &question.text,
            16,
            1.4,
            Role::Text,
        )))
        .child(
            Button::icon(
                if draft.folded {
                    IconName::ChevronRight
                } else {
                    IconName::ChevronDown
                },
                FOLD,
            )
            .build(),
        )
        .child(Button::icon(IconName::Close, CLOSE).build());
    let mut card = node()
        .styles([
            flex(),
            flex_col(),
            gap(3),
            px(4),
            py(3.5),
            rounded(Radius::Xl),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(title);
    if draft.folded {
        return card;
    }
    let picked = &draft.picks[draft.step];
    let rows = node()
        .styles([flex(), flex_col(), gap(2)])
        .children(
            question
                .alternatives
                .iter()
                .enumerate()
                .map(|(index, alternative)| {
                    row(
                        alternative,
                        index + 1,
                        picked.contains(&index),
                        FocusId(PICK_BASE + index as u64),
                    )
                }),
        )
        .child(other(draft, question.alternatives.len() + 1));
    let next = Button::new(if draft.last() { "Submit" } else { "Next" }, NEXT)
        .size(Size::Sm)
        .disabled(!draft.answered())
        .build();
    let buttons = node()
        .styles([flex(), flex_wrap(), items_center(), justify_end(), gap(2)])
        .child(
            Button::new("Skip", SKIP)
                .variant(Variant::Outline)
                .size(Size::Sm)
                .build(),
        )
        .child(next);
    card = card.child(rows).child(buttons);
    card
}

/// The card that records a form of questions the reader answered: each question with its answer, or `[No preference]` when it was skipped.
pub fn answered(answers: &[(String, Option<String>)]) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            gap(3),
            px(4),
            py(3.5),
            self_start(),
            rounded(Radius::Xl),
            border(),
            border_color(Role::Border),
            bg(Role::Background),
        ])
        .children(answers.iter().map(|(question, answer)| {
            node()
                .styles([flex(), flex_col(), gap(0.5)])
                .child(line(question, 15, 1.5, Role::Muted))
                .child(line(
                    answer.as_deref().unwrap_or("[No preference]"),
                    15,
                    1.5,
                    Role::Text,
                ))
        }))
}
