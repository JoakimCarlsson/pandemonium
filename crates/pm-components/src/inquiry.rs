//! The card that puts the agent's questions to the reader one at a time, each with the answers it offers and a box for an answer of the reader's own.

use crate::kit::sp;
use pm_harness::Alternative;
use rustle::{Alignment, Arrangement, FlowOptions, column_arranged, flow_row_with, row_arranged};
use rustle::{BoxAlignment, CursorIcon, Modifier};

use crate::Button;
use crate::button::Variant;
use crate::draft::{CLOSE, Draft, FOLD, MOCKUP_OPEN_BASE, NEXT, OTHER, PICK_BASE, SKIP};
use crate::exhibit::{MOCKUP_OFFER_BASE, MOCKUP_VIEWPORT, Offer, Preview, wanted_document};
use crate::figures::Figures;
use crate::input::{self, Input};
use crate::kit::{
    FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Styled, Tooltip, Type, label,
    wrap_lines,
};
use crate::parts::line;
use crate::size::Size;

/// Emits every measured line of agent text within `width`, set in `pixels` tall type in `tone`.
#[track_caller]
fn wrapped(content: &str, pixels: u32, tone: Role, width: f32) {
    let kind = Type::new(pixels).ratio(1.4);
    let lines = wrap_lines(content, kind, width.max(32.0));
    column_arranged(
        Modifier::new(),
        Arrangement::Start,
        Alignment::Start,
        || {
            for part in lines {
                label(&content[part.start..part.end], kind.tone(tone));
            }
        },
    );
}

/// Emits the small box holding the number of the key that picks a row.
#[track_caller]
fn key(number: usize) {
    row_arranged(
        Modifier::new()
            .square(7)
            .fill_rounded(Role::Background, Radius::Lg)
            .stroke(Role::Border, Radius::Lg),
        Arrangement::Center,
        Alignment::Center,
        || line(number.to_string(), 13, 1.4, Role::Muted),
    );
}

/// Emits the row offering `alternative`, which the key `number` picks, drawn as taken when `picked`, answering to `id`, its words wrapped to `width`, with `modifier` outermost on the row.
#[track_caller]
fn choice(
    alternative: &Alternative,
    number: usize,
    picked: bool,
    id: FocusId,
    width: f32,
    modifier: Modifier,
) {
    row_arranged(
        modifier
            .looks_of(
                Looks::new()
                    .idle(Role::SurfaceHover)
                    .hover(Role::SurfaceActive)
                    .radius(Radius::Lg),
                id,
            )
            .stroke(
                if picked { Role::Accent } else { Role::Surface },
                Radius::Lg,
            )
            .cursor(CursorIcon::Pointer)
            .target(id)
            .pad_x(3)
            .pad_y(2),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            column_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    wrapped(&alternative.label, 14, Role::Text, width);
                    if !alternative.description.is_empty() {
                        wrapped(&alternative.description, 13, Role::Muted, width);
                    }
                },
            );
            key(number);
        },
    );
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

/// Emits a small mark on a tile in `tone`, saying `tip` when hovered: `icon`, then `words` when the heading has room to spell marks out as `form` says, and nothing at all when marks are hidden.
#[track_caller]
fn mark(icon: IconName, words: &str, tone: Role, form: Marks, tip: &str) {
    if form != Marks::Hidden {
        mark_node(icon, (form == Marks::Words).then_some(words), tone, tip);
    }
}

/// Emits a small mark on a tile that shows `icon` in `tone`, followed by `words` when it has any, saying `tip` when hovered.
#[track_caller]
fn mark_node(icon: IconName, words: Option<&str>, tone: Role, tip: &str) {
    row_arranged(
        Modifier::new()
            .tooltip(tip)
            .stroke(tone, Radius::Full)
            .pad_x(1.5),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Center,
        || {
            crate::parts::glyph(icon, 3.0, tone);
            if let Some(words) = words {
                line(words, 11, 1.5, tone);
            }
        },
    );
}

/// Emits the tile offering `alternative` among answers that are compared: the picture of its mockup when it has one, as `figures` holds it, over its label and description, `width` logical pixels wide. The key `number` picks it, as a press on it does; it is drawn as taken when `picked` and marked when the agent recommends it, and a tile with a mockup has a button that opens the mockup larger. The marks shrink to icons and then go, and the button goes with them, as the tile gets too narrow to hold them, and the tile cuts off whatever would still reach past its edge. A mockup whose picture waits for the embedded browser offers `offer` in its place, shrinking as the marks do.
#[track_caller]
fn tile(
    alternative: &Alternative,
    (figures, offer): (&Figures, &Offer),
    (index, picked): (usize, bool),
    width: f32,
) {
    let inner = width - TILE_INSET;
    let (label, suggested) = recommended(&alternative.label);
    let outdated = matches!(offer, Offer::Update(_));
    let form = marks_in(inner);
    let id = FocusId(PICK_BASE + index as u64);
    column_arranged(
        Modifier::new()
            .width(width)
            .looks_of(
                Looks::new()
                    .idle(if picked {
                        Role::SurfaceActive
                    } else {
                        Role::SurfaceHover
                    })
                    .hover(Role::SurfaceActive)
                    .radius(Radius::Lg),
                id,
            )
            .stroke(if picked { Role::Accent } else { Role::Border }, Radius::Lg)
            .clip_to(Radius::None)
            .cursor(CursorIcon::Pointer)
            .target(id)
            .pad_x(2)
            .pad_y(2),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new().fill_max_width().clip_to(Radius::None),
                Arrangement::SpacedBy(sp(1.5)),
                Alignment::Center,
                || {
                    key(index + 1);
                    if suggested {
                        mark(
                            IconName::Sparkles,
                            "Recommended",
                            Role::Accent,
                            form,
                            "Recommended",
                        );
                    }
                    if picked {
                        mark(IconName::Check, "Selected", Role::Accent, form, "Selected");
                    }
                    rustle::spacer(rustle::Modifier::new().weight(1.0));
                    if alternative.mockup.is_some() && form != Marks::Hidden {
                        Button::icon(IconName::Maximize, FocusId(MOCKUP_OPEN_BASE + index as u64))
                            .disabled(outdated)
                            .modifier(Modifier::new().tooltip(if outdated {
                                "Update the browser to open mockups · it cannot open pages sealed from the network"
                            } else {
                                "Open larger in the browser pane · sealed from the network"
                            }))
                            .build();
                    }
                },
            );
            if let Some(html) = alternative.mockup.as_deref() {
                let (key, _) = wanted_document(html, MOCKUP_VIEWPORT);
                Preview::new(figures.get(&key), MOCKUP_VIEWPORT, inner)
                    .offer(offer, FocusId(MOCKUP_OFFER_BASE + index as u64))
                    .build();
            }
            column_arranged(
                Modifier::new(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    wrapped(label, 14, Role::Text, inner);
                    if !alternative.description.is_empty() {
                        wrapped(&alternative.description, 13, Role::Muted, inner);
                    }
                },
            );
        },
    );
}

/// Emits the answers of a question side by side for comparing, as tiles that wrap onto rows within `width` logical pixels, those of `picked` drawn as taken, with the pictures and the offer of `pictures` as [`tile`] takes them.
#[track_caller]
fn tiles(alternatives: &[Alternative], pictures: (&Figures, &Offer), picked: &[usize], width: f32) {
    let each = tile_width(width, tiles_per_row(width, alternatives.len()));
    flow_row_with(
        Modifier::new().fill_max_width(),
        FlowOptions::new(sp(TILE_GAP / 4.0), sp(TILE_GAP / 4.0)),
        || {
            for (index, alternative) in alternatives.iter().enumerate() {
                rustle::key(index, || {
                    tile(
                        alternative,
                        pictures,
                        (index, picked.contains(&index)),
                        each,
                    );
                });
            }
        },
    );
}

/// Emits the row for an answer of the reader's own, with the box to type it in, with `modifier` outermost on the row.
#[track_caller]
fn other(draft: &Draft, number: usize, modifier: Modifier) {
    let field = &draft.others[draft.step];
    column_arranged(
        modifier
            .fill_rounded(Role::SurfaceHover, Radius::Lg)
            .stroke(
                if field.text().trim().is_empty() {
                    Role::Surface
                } else {
                    Role::Accent
                },
                Radius::Lg,
            )
            .pad_x(3)
            .pad_y(2),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(sp(3)),
                Alignment::Center,
                || {
                    row_arranged(
                        Modifier::new().weight(1.0),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            line("Other", 15, 1.4, Role::Text);
                        },
                    );
                    key(number);
                },
            );
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .fill_rounded(Role::Background, Radius::Lg),
                Arrangement::Start,
                Alignment::Start,
                || {
                    Input::new(field, OTHER)
                        .variant(input::Variant::Ghost)
                        .placeholder("Type your own answer here")
                        .build();
                },
            );
        },
    );
}

/// Emits the card for `draft`: the question, the answers to pick from, and the buttons that skip it or move on, or only its title when folded. When an answer of the question has a mockup, the answers are tiles side by side, each with the picture `figures` holds of its mockup, for the reader to compare, and a mockup whose picture waits for the embedded browser offers `offer` in its place.
#[track_caller]
pub fn inquiry(draft: &Draft, figures: &Figures, offer: &Offer, width: f32) {
    let question = draft.question();
    let count = draft.inquiry.questions.len();
    column_arranged(
        Modifier::new()
            .fill_rounded(Role::Surface, Radius::Xl)
            .stroke(Role::Border, Radius::Xl)
            .pad_x(3)
            .pad_y(2.5),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Start,
                || {
                    if count > 1 {
                        row_arranged(
                            Modifier::new()
                                .fill_rounded(Role::SurfaceHover, Radius::Full)
                                .pad_x(3)
                                .pad_y(1),
                            Arrangement::Start,
                            Alignment::Center,
                            || {
                                line(format!("{}/{count}", draft.step + 1), 13, 1.4, Role::Muted);
                            },
                        );
                    }
                    row_arranged(
                        Modifier::new().weight(1.0),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            wrapped(
                                &question.text,
                                14,
                                Role::Text,
                                width - if count > 1 { 145.0 } else { 105.0 },
                            );
                        },
                    );
                    Button::icon(
                        if draft.folded {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        },
                        FOLD,
                    )
                    .build();
                    Button::icon(IconName::Close, CLOSE).build();
                },
            );
            if draft.folded {
                return;
            }
            let picked = &draft.picks[draft.step];
            column_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(sp(1)),
                Alignment::Start,
                || {
                    if compares(&question.alternatives) {
                        tiles(
                            &question.alternatives,
                            (figures, offer),
                            picked,
                            width - CARD_INSET,
                        );
                    } else {
                        for (index, alternative) in question.alternatives.iter().enumerate() {
                            choice(
                                alternative,
                                index + 1,
                                picked.contains(&index),
                                FocusId(PICK_BASE + index as u64),
                                width - 92.0,
                                Modifier::new().fill_max_width(),
                            );
                        }
                    }
                    other(
                        draft,
                        question.alternatives.len() + 1,
                        Modifier::new().fill_max_width(),
                    );
                },
            );
            flow_row_with(
                Modifier::new().fill_max_width(),
                FlowOptions::new(sp(2), sp(2))
                    .arrangement(Arrangement::End)
                    .item_alignment(Alignment::Center),
                || {
                    Button::new("Skip", SKIP)
                        .variant(Variant::Outline)
                        .size(Size::Sm)
                        .build();
                    Button::new(if draft.last() { "Submit" } else { "Next" }, NEXT)
                        .size(Size::Sm)
                        .disabled(!draft.answered())
                        .build();
                },
            );
        },
    );
}

/// What the padding and border of the question card take from its width, in logical pixels.
const CARD_INSET: f32 = 26.0;

/// What the padding and border of the card that records answers take from its width, in logical pixels.
const ANSWERED_INSET: f32 = 34.0;

/// Emits the card that records a form of questions the reader answered, at most `width` logical pixels wide: each question with its answer, or `[No preference]` when it was skipped, both wrapped to fit.
#[track_caller]
pub fn answered(answers: &[(String, Option<String>)], width: f32) {
    let room = width - ANSWERED_INSET;
    column_arranged(
        Modifier::new()
            .align(BoxAlignment::TopStart)
            .fill_rounded(Role::Background, Radius::Xl)
            .stroke(Role::Border, Radius::Xl)
            .pad_x(4)
            .pad_y(3.5),
        Arrangement::SpacedBy(sp(3)),
        Alignment::Start,
        || {
            for (question, answer) in answers {
                column_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(sp(0.5)),
                    Alignment::Start,
                    || {
                        wrapped(question, 15, Role::Muted, room);
                        wrapped(
                            answer.as_deref().unwrap_or("[No preference]"),
                            15,
                            Role::Text,
                            room,
                        );
                    },
                );
            }
        },
    );
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
        let figures = Figures::new();
        let report = crate::kit::snapshot::lay_out(700.0, 2000.0, None, move || {
            inquiry(&draft, &figures, &Offer::Nothing, 700.0)
        });
        let ids: Vec<u64> = report.regions.iter().map(|region| region.id.0).collect();
        for index in 0..3 {
            assert!(ids.contains(&(PICK_BASE + index)), "pick {index}");
        }
        assert!(ids.contains(&MOCKUP_OPEN_BASE) && ids.contains(&(MOCKUP_OPEN_BASE + 1)));
        assert!(!ids.contains(&(MOCKUP_OPEN_BASE + 2)));
    }
}
