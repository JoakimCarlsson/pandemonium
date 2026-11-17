//! The card that puts the agent's questions to the reader one at a time, each with the answers it offers and a box for an answer of the reader's own.

use pm_harness::Alternative;
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
use crate::draft::{CLOSE, Draft, FOLD, MOCKUP_OPEN_BASE, NEXT, OTHER, PICK_BASE, SKIP};
use crate::exhibit::{MOCKUP_OFFER_BASE, MOCKUP_VIEWPORT, Offer, Preview, wanted_document};
use crate::figures::Figures;
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

/// The narrowest a question card's answers may be, in logical pixels, for their tiles to sit two to a row; narrower, each tile takes a row.
const TWO_TILES: f32 = 480.0;

/// The gap between two tiles, in logical pixels: two steps of the spacing scale.
const TILE_GAP: f32 = 8.0;

/// What the padding and border of a tile take from its width, in logical pixels.
const TILE_INSET: f32 = 18.0;

/// How the label of an answer marks it as the one the agent recommends.
const RECOMMENDED: &str = "(recommended)";

/// Whether the answers of a question are compared as tiles: so when any of `alternatives` has a mockup.
pub(crate) fn compares(alternatives: &[Alternative]) -> bool {
    alternatives
        .iter()
        .any(|alternative| alternative.mockup.is_some())
}

/// How many tiles sit on a row when `count` answers share `width` logical pixels: two at the width of a conversation, one when the card is narrow or there is a single answer.
pub(crate) fn tiles_per_row(width: f32, count: usize) -> usize {
    if count < 2 || width < TWO_TILES { 1 } else { 2 }
}

/// How wide each tile is, in logical pixels, when `per_row` of them share `width` with a gap between.
pub(crate) fn tile_width(width: f32, per_row: usize) -> f32 {
    let per_row = per_row.max(1) as f32;
    ((width - TILE_GAP * (per_row - 1.0)) / per_row)
        .floor()
        .max(1.0)
}

/// The label of an answer without the mark that says the agent recommends it, and whether it had the mark.
pub(crate) fn recommended(label: &str) -> (&str, bool) {
    let trimmed = label.trim_end();
    let Some(at) = trimmed.len().checked_sub(RECOMMENDED.len()) else {
        return (label, false);
    };
    match trimmed.get(at..) {
        Some(mark) if mark.eq_ignore_ascii_case(RECOMMENDED) => (trimmed[..at].trim_end(), true),
        _ => (label, false),
    }
}

/// The least room the heading of a tile needs, in logical pixels, for its marks to be spelled out beside the key and the button that opens the mockup.
const MARKS_IN_WORDS: f32 = 240.0;

/// The least room the heading of a tile needs, in logical pixels, for its marks to show as icons alone; narrower, the heading has no marks and the tile's border alone says it is picked.
const MARKS_AS_ICONS: f32 = 140.0;

/// How the marks of a tile are drawn in the room its heading has.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Marks {
    /// Each mark as its icon and its words.
    Words,
    /// Each mark as its icon alone.
    Icons,
    /// No marks.
    Hidden,
}

/// How the marks of a tile are drawn when its heading has `room` logical pixels: spelled out when they fit, as icons when only those do, and not at all in a heading too narrow even for that, so that nothing is drawn outside the tile.
pub(crate) fn marks_in(room: f32) -> Marks {
    if room >= MARKS_IN_WORDS {
        Marks::Words
    } else if room >= MARKS_AS_ICONS {
        Marks::Icons
    } else {
        Marks::Hidden
    }
}

/// A small mark on a tile in `tone`: `icon`, then `words` when the heading has room to spell marks out as `form` says, and nothing at all when marks are hidden.
fn mark(icon: IconName, words: &str, tone: Role, form: Marks) -> Option<Node> {
    (form != Marks::Hidden).then(|| mark_node(icon, (form == Marks::Words).then_some(words), tone))
}

/// A small mark on a tile that shows `icon` in `tone`, followed by `words` when it has any.
fn mark_node(icon: IconName, words: Option<&str>, tone: Role) -> Node {
    node()
        .styles([
            flex(),
            flex_none(),
            items_center(),
            gap(1),
            px(1.5),
            rounded_full(),
            border(),
            border_color(tone),
        ])
        .child(crate::parts::glyph(icon, 3.0, tone))
        .children(words.map(|words| line(words, 11, 1.5, tone)))
}

/// The tile offering `alternative` among answers that are compared: the picture of its mockup when it has one, as `figures` holds it, over its label and description, `width` logical pixels wide. The key `number` picks it, as a press on it does; it is drawn as taken when `picked` and marked when the agent recommends it, and a tile with a mockup has a button that opens the mockup larger. The marks shrink to icons and then go, and the button goes with them, as the tile gets too narrow to hold them, and the tile cuts off whatever would still reach past its edge. A mockup whose picture waits for the embedded browser offers `offer` in its place, shrinking as the marks do.
fn tile(
    alternative: &Alternative,
    (figures, offer): (&Figures, &Offer),
    (index, picked): (usize, bool),
    width: f32,
    shaper: &mut Shaper,
) -> Node {
    let inner = width - TILE_INSET;
    let (label, suggested) = recommended(&alternative.label);
    let outdated = matches!(offer, Offer::Update(_));
    let enlarge = alternative.mockup.as_ref().map(|_| {
        Button::icon(IconName::Maximize, FocusId(MOCKUP_OPEN_BASE + index as u64))
            .disabled(outdated)
            .build()
            .tooltip(if outdated {
                "Update the browser to open mockups · it cannot open pages sealed from the network"
            } else {
                "Open larger in the browser pane · sealed from the network"
            })
    });
    let form = marks_in(inner);
    let suggestion = suggested
        .then(|| mark(IconName::Sparkles, "Recommended", Role::Accent, form))
        .flatten();
    let selection = picked
        .then(|| mark(IconName::Check, "Selected", Role::Accent, form))
        .flatten();
    let marks = node()
        .styles([
            flex(),
            items_center(),
            gap(1.5),
            w_full(),
            min_w_px(0),
            pm_style::overflow_hidden(),
        ])
        .child(key(index + 1))
        .children(suggestion.map(|mark| mark.tooltip("Recommended")))
        .children(selection.map(|mark| mark.tooltip("Selected")))
        .child(node().styles([flex_1(), min_w_px(0)]))
        .children(enlarge.filter(|_| form != Marks::Hidden));
    let picture = alternative.mockup.as_deref().map(|html| {
        let (key, _) = wanted_document(html, MOCKUP_VIEWPORT);
        Preview::new(figures.get(&key), MOCKUP_VIEWPORT, inner)
            .offer(offer, FocusId(MOCKUP_OFFER_BASE + index as u64))
            .build()
    });
    let mut words = node()
        .styles([flex(), flex_col(), min_w_px(0)])
        .child(wrapped(label, 14, Role::Text, inner, shaper));
    if !alternative.description.is_empty() {
        words = words.child(wrapped(
            &alternative.description,
            13,
            Role::Muted,
            inner,
            shaper,
        ));
    }
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            gap(2),
            px(2),
            py(2),
            pm_style::w_px(width),
            pm_style::overflow_hidden(),
            rounded(Radius::Lg),
            border(),
            border_color(if picked { Role::Accent } else { Role::Border }),
            bg(if picked {
                Role::SurfaceActive
            } else {
                Role::SurfaceHover
            }),
            cursor_pointer(),
            bg(Role::SurfaceActive).hover(),
        ])
        .child(marks)
        .children(picture)
        .child(words)
        .focusable(FocusId(PICK_BASE + index as u64))
}

/// The answers of a question side by side for comparing, as tiles that wrap onto rows within `width` logical pixels, those of `picked` drawn as taken, with the pictures and the offer of `pictures` as [`tile`] takes them.
fn tiles(
    alternatives: &[Alternative],
    pictures: (&Figures, &Offer),
    picked: &[usize],
    width: f32,
    shaper: &mut Shaper,
) -> Node {
    let each = tile_width(width, tiles_per_row(width, alternatives.len()));
    node()
        .styles([
            flex(),
            flex_wrap(),
            items_start(),
            gap(TILE_GAP / 4.0),
            w_full(),
        ])
        .children(
            alternatives
                .iter()
                .enumerate()
                .map(|(index, alternative)| {
                    tile(
                        alternative,
                        pictures,
                        (index, picked.contains(&index)),
                        each,
                        shaper,
                    )
                })
                .collect::<Vec<_>>(),
        )
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

/// The card for `draft`: the question, the answers to pick from, and the buttons that skip it or move on, or only its title when folded. When an answer of the question has a mockup, the answers are tiles side by side, each with the picture `figures` holds of its mockup, for the reader to compare, and a mockup whose picture waits for the embedded browser offers `offer` in its place.
pub fn inquiry(
    draft: &Draft,
    figures: &Figures,
    offer: &Offer,
    width: f32,
    shaper: &mut Shaper,
) -> Node {
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
    let rows = node().styles([flex(), flex_col(), gap(1)]);
    let rows = if compares(&question.alternatives) {
        rows.child(tiles(
            &question.alternatives,
            (figures, offer),
            picked,
            width - CARD_INSET,
            shaper,
        ))
    } else {
        rows.children(
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
    }
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

/// What the padding and border of the question card take from its width, in logical pixels.
const CARD_INSET: f32 = 26.0;

/// What the padding and border of the card that records answers take from its width, in logical pixels.
const ANSWERED_INSET: f32 = 34.0;

/// The card that records a form of questions the reader answered, at most `width` logical pixels wide: each question with its answer, or `[No preference]` when it was skipped, both wrapped to fit.
pub fn answered(answers: &[(String, Option<String>)], width: f32, shaper: &mut Shaper) -> Node {
    let room = width - ANSWERED_INSET;
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
                .child(wrapped(question, 15, Role::Muted, room, shaper))
                .child(wrapped(
                    answer.as_deref().unwrap_or("[No preference]"),
                    15,
                    Role::Text,
                    room,
                    shaper,
                ))
        }))
}

#[cfg(test)]
/// Tests for how the answers of a question are laid out for comparing.
mod tests {
    use super::*;

    /// An answer saying `label`, with a mockup when `pictured`.
    fn alternative(label: &str, pictured: bool) -> Alternative {
        Alternative {
            label: label.to_owned(),
            description: String::new(),
            mockup: pictured.then(|| "<p>mockup</p>".to_owned()),
        }
    }

    /// Answers are compared as tiles as soon as one of them has a mockup, and not otherwise.
    #[test]
    fn one_mockup_makes_the_answers_tiles() {
        assert!(!compares(&[
            alternative("a", false),
            alternative("b", false)
        ]));
        assert!(compares(&[alternative("a", false), alternative("b", true)]));
        assert!(!compares(&[]));
    }

    /// Tiles sit two to a row at the width of a conversation and one to a row when the card is narrow or holds one answer.
    #[test]
    fn tiles_wrap_two_to_a_row_and_one_when_narrow() {
        assert_eq!(tiles_per_row(686.0, 3), 2);
        assert_eq!(tiles_per_row(TWO_TILES, 2), 2);
        assert_eq!(tiles_per_row(TWO_TILES - 1.0, 3), 1);
        assert_eq!(tiles_per_row(686.0, 1), 1);
        assert_eq!(tile_width(686.0, 2), 339.0);
        assert_eq!(tile_width(400.0, 1), 400.0);
        assert!(tile_width(686.0, 2) * 2.0 + TILE_GAP <= 686.0);
    }

    /// The marks of a tile are spelled out only in a heading with room for them, shrink to icons in a narrower one and go in one too narrow for those, so they never reach outside the tile.
    #[test]
    fn marks_shrink_with_the_tile() {
        assert_eq!(marks_in(tile_width(686.0, 2) - TILE_INSET), Marks::Words);
        assert_eq!(marks_in(MARKS_IN_WORDS), Marks::Words);
        assert_eq!(marks_in(MARKS_IN_WORDS - 1.0), Marks::Icons);
        assert_eq!(marks_in(MARKS_AS_ICONS), Marks::Icons);
        assert_eq!(marks_in(MARKS_AS_ICONS - 1.0), Marks::Hidden);
        assert_eq!(marks_in(tile_width(100.0, 1) - TILE_INSET), Marks::Hidden);
    }

    /// The mark of a recommended answer is read off the end of its label, whatever its case, and taken out of what the tile says.
    #[test]
    fn the_recommended_answer_is_read_from_its_label() {
        assert_eq!(recommended("Sidebar (Recommended)"), ("Sidebar", true));
        assert_eq!(recommended("Sidebar (recommended) "), ("Sidebar", true));
        assert_eq!(recommended("Sidebar"), ("Sidebar", false));
        assert_eq!(
            recommended("(Recommended) first"),
            ("(Recommended) first", false)
        );
        assert_eq!(recommended("é"), ("é", false));
    }

    /// A card whose answers are tiles offers every answer to a press and to its key, and a button to open each mockup larger.
    #[test]
    fn every_tile_answers_to_its_pick_and_its_mockup_to_a_button() {
        let draft = Draft::new(pm_harness::Inquiry {
            ticket: 1,
            questions: vec![pm_harness::Question {
                header: "Layout".to_owned(),
                text: "Which layout?".to_owned(),
                multiple: false,
                alternatives: vec![
                    alternative("Sidebar (Recommended)", true),
                    alternative("Tabs", true),
                    alternative("Neither", false),
                ],
            }],
        });
        let mut shaper = Shaper::new();
        let card = inquiry(&draft, &Figures::new(), &Offer::Nothing, 700.0, &mut shaper);
        let output = pm_ui::draw(
            &card,
            700.0,
            2000.0,
            &pm_ui::Input::default(),
            &pm_theme::default_family().dark,
            &mut shaper,
            &mut pm_renderer::DrawList::new(),
        );
        let ids: Vec<u64> = output.regions.iter().map(|region| region.id.0).collect();
        for index in 0..3 {
            assert!(ids.contains(&(PICK_BASE + index)), "pick {index}");
        }
        assert!(ids.contains(&MOCKUP_OPEN_BASE) && ids.contains(&(MOCKUP_OPEN_BASE + 1)));
        assert!(!ids.contains(&(MOCKUP_OPEN_BASE + 2)));
    }
}
