//! The card that puts the agent's questions to the reader one at a time, each with the answers it offers and a box for an answer of the reader's own.

use pm_acp::Alternative;
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, flex_1, flex_col, flex_none,
    flex_wrap, gap, items_center, items_start, justify_center, justify_end, leading_ratio,
    min_w_px, px, py, rounded, rounded_full, self_start, size, text_color, text_px, w_full,
};
use pm_text::Shaper;
use pm_ui::{Node, node, text};

use crate::Button;
use crate::button::Variant;
use crate::draft::{CLOSE, Draft, FOLD, NEXT, OTHER, PICK_BASE, SKIP};
use crate::input::{self, Input};
use crate::parts::line;
use crate::size::Size;

/// Draws every measured line of agent text within the available width.
fn wrapped(content: &str, pixels: u32, tone: Role, width: f32, shaper: &mut Shaper) -> Node {
    let styles = [pm_style::text_px(pixels), pm_style::leading_ratio(1.4)];
    node().styles([flex(), flex_col(), min_w_px(0)]).children(
        pm_ui::wrap(shaper, content, styles, width.max(32.0))
            .into_iter()
            .map(|part| {
                text(&content[part.start..part.end]).styles([
                    text_px(pixels),
                    leading_ratio(1.4),
                    text_color(tone),
                    min_w_px(0),
                ])
            }),
    )
}

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
fn row(
    alternative: &Alternative,
    number: usize,
    picked: bool,
    id: FocusId,
    width: f32,
    shaper: &mut Shaper,
) -> Node {
    let mut words = node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0)])
        .child(wrapped(&alternative.label, 14, Role::Text, width, shaper));
    if !alternative.description.is_empty() {
        words = words.child(wrapped(
            &alternative.description,
            13,
            Role::Muted,
            width,
            shaper,
        ));
    }
    node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            px(3),
            py(2),
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
            gap(2),
            px(3),
            py(2),
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
pub fn inquiry(draft: &Draft, width: f32, shaper: &mut Shaper) -> Node {
    let question = draft.question();
    let count = draft.inquiry.questions.len();
    let mut title = node().styles([flex(), items_start(), gap(2)]);
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
        .child(node().styles([flex_1(), min_w_px(0)]).child(wrapped(
            &question.text,
            14,
            Role::Text,
            width - if count > 1 { 145.0 } else { 105.0 },
            shaper,
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
            gap(2),
            px(3),
            py(2.5),
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
        .styles([flex(), flex_col(), gap(1)])
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
                        width - 92.0,
                        shaper,
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
