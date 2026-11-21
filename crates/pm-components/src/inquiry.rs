//! The card that puts the agent's questions to the reader one at a time, each with the answers it offers and a box for an answer of the reader's own.

use crate::areas::ChatEvent;
use crate::button;
use crate::events;
use crate::icons;
use crate::target::Target;
use crate::theme;
use crate::theme::Frame;
use crate::theme::Indicated;
use crate::theme::sp;
use pm_harness::Alternative;
use rustle::TextOverflow;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{BoxAlignment, Modifier};
use rustle::{Icon, phosphor};

use crate::Button;
use crate::button::Variant;
use crate::draft::Draft;
use crate::exhibit::{MOCKUP_VIEWPORT, Offer, Preview, wanted_document};
use crate::figures::{Drawing, Figures};
use crate::input::{self, Input};
use crate::parts::line;
use crate::theme::{Radius, Role};

/// Emits every measured line of agent text within `width`, set in `pixels` tall type in `tone`.
#[track_caller]
fn wrapped(content: &str, pixels: u32, tone: Role, width: f32) {
    let kind = theme::text_style(pixels, 1.4);
    let lines = rustle::wrap_at_spaces(content, &kind, width.max(32.0));
    rustle::column_arranged(
        Modifier::new(),
        Arrangement::Start,
        Alignment::Start,
        || {
            for part in lines {
                rustle::basic_text(
                    Modifier::new(),
                    &content[part.start..part.end],
                    kind.color(theme::color(tone)),
                    TextOverflow::Ellipsis,
                );
            }
        },
    );
}

/// Emits the small box holding the number of the key that picks a row.
#[track_caller]
fn key(number: usize) {
    row_arranged(
        Modifier::new()
            .size(theme::sp(7), theme::sp(7))
            .background_rounded(theme::color(Role::Background), Radius::Lg.px())
            .outlined(Radius::Lg),
        Arrangement::Center,
        Alignment::Center,
        || line(number.to_string(), 13, 1.4, Role::Muted),
    );
}

/// `modifier` as an offer that picks `id`: filled `idle` and the active fill
/// under the pointer, outlined in `edge` with large corners and pressable.
#[track_caller]
fn offered(modifier: Modifier, id: Target, idle: Role, edge: Role) -> Modifier {
    events::pressed(
        modifier
            .indicated(
                theme::InteractionTones::new()
                    .idle(idle)
                    .hover(Role::SurfaceActive)
                    .radius(Radius::Lg),
                theme::Focused::Control(crate::focus::is_focused(id)),
            )
            .border(1.0, theme::color(edge), Radius::Lg.px())
            .padding(1.0),
        id,
    )
}

/// Emits the row offering `alternative`, the one at `index`, which the key after it picks, drawn as taken when `picked`, its words wrapped to `width`.
#[track_caller]
fn choice(alternative: &Alternative, index: usize, picked: bool, width: f32) {
    let id = Target::Chat(ChatEvent::InquiryPick(index));
    rustle::row_arranged(
        offered(
            Modifier::new().fill_max_width(),
            id,
            Role::SurfaceHover,
            if picked { Role::Accent } else { Role::Surface },
        )
        .padding_xy(theme::sp(3), theme::sp(2)),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            said(
                Modifier::new().weight(1.0),
                &alternative.label,
                &alternative.description,
                width,
            );
            key(index + 1);
        },
    );
}

/// Emits the words of an answer wrapped to `width` in a column with `modifier`: its `label`, over its `description`, when it has one.
#[track_caller]
fn said(modifier: Modifier, label: &str, description: &str, width: f32) {
    rustle::column_arranged(modifier, Arrangement::Start, Alignment::Start, || {
        wrapped(label, 14, Role::Text, width);
        if !description.is_empty() {
            wrapped(description, 13, Role::Muted, width);
        }
    });
}

/// The narrowest a question card's answers may be, in logical pixels, for their tiles to sit two to a row; narrower, each tile takes a row.
const TWO_TILES: f32 = 480.0;

/// The gap between two tiles, in logical pixels: two steps of the spacing scale.
const TILE_GAP: f32 = 8.0;

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
fn mark(icon: Icon, words: &str, tone: Role, form: Marks, tip: &str) {
    if form == Marks::Hidden {
        return;
    }
    crate::tooltip::with_tooltip(tip, || {
        rustle::row_arranged(
            Modifier::new()
                .border(1.0, theme::color(tone), Radius::Full.px())
                .padding(1.0)
                .padding_xy(theme::sp(1.5), 0.0),
            Arrangement::SpacedBy(theme::sp(1)),
            Alignment::Center,
            || {
                crate::parts::glyph(icon, 3.0, tone);
                if form == Marks::Words {
                    line(words, 11, 1.5, tone);
                }
            },
        );
    });
}

/// Emits the tile offering `alternative` among answers that are compared, sharing its line of tiles with the others by weight: the picture `figure` of its mockup when it has one, over its label and description. The key `number` picks it, as a press on it does; it is drawn as taken when `picked` and marked when the agent recommends it, and a tile with a mockup has a button that opens the mockup larger. What is inside the tile is chosen from the width it is given, as Compose's `BoxWithConstraints` chooses: the marks shrink to icons and then go, and the button goes with them, as the tile gets too narrow to hold them, and the tile cuts off whatever would still reach past its edge. A mockup whose picture waits for the embedded browser offers `offer` in its place, shrinking as the marks do.
#[track_caller]
fn tile(
    alternative: &Alternative,
    (figure, offer): (Option<Result<Drawing, String>>, &Offer),
    (index, picked): (usize, bool),
) {
    let id = Target::Chat(ChatEvent::InquiryPick(index));
    let alternative = alternative.clone();
    let offer = offer.clone();
    rustle::with_constraints(
        offered(
            Modifier::new().weight(1.0),
            id,
            if picked {
                Role::SurfaceActive
            } else {
                Role::SurfaceHover
            },
            if picked { Role::Accent } else { Role::Border },
        )
        .clip()
        .padding_xy(theme::sp(2), theme::sp(2)),
        move |constraints| {
            tile_content(
                &alternative,
                (figure.as_ref(), &offer),
                (index, picked),
                constraints.max_width,
            );
        },
    );
}

/// Emits what a tile offering `alternative` holds in `inner` logical pixels across: its heading with the key, the marks and the button that opens the mockup, the mockup's picture `figure` or the offer `offer` in its place, and its label and description; as [`tile`] describes.
#[track_caller]
fn tile_content(
    alternative: &Alternative,
    (figure, offer): (Option<&Result<Drawing, String>>, &Offer),
    (index, picked): (usize, bool),
    inner: f32,
) {
    let (label, suggested) = recommended(&alternative.label);
    let outdated = matches!(offer, Offer::Update(_));
    let form = marks_in(inner);
    rustle::column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width().clip(),
                Arrangement::SpacedBy(theme::sp(1.5)),
                Alignment::Center,
                || {
                    key(index + 1);
                    if suggested {
                        mark(
                            phosphor::SPARKLE,
                            "Recommended",
                            Role::Accent,
                            form,
                            "Recommended",
                        );
                    }
                    if picked {
                        mark(phosphor::CHECK, "Selected", Role::Accent, form, "Selected");
                    }
                    rustle::spacer(rustle::Modifier::new().weight(1.0));
                    if alternative.mockup.is_some() && form != Marks::Hidden {
                        Button::icon(phosphor::ARROWS_OUT_SIMPLE, Target::Chat(ChatEvent::MockupOpen(index)))
                            .disabled(outdated)
                            .tooltip(if outdated {
                                "Update the browser to open mockups · it cannot open pages sealed from the network"
                            } else {
                                "Open larger in the browser pane · sealed from the network"
                            })
                            .build();
                    }
                },
            );
            if alternative.mockup.is_some() {
                Preview::new(figure, MOCKUP_VIEWPORT, inner)
                    .offer(offer, Target::Chat(ChatEvent::MockupOffer(index)))
                    .build();
            }
            said(Modifier::new(), label, &alternative.description, inner);
        },
    );
}

/// Emits the answers of a question side by side for comparing, as tiles in lines across the card, as many to a line as [`tiles_per_row`] says for the width the card gives them, as Compose's `BoxWithConstraints` chooses, each line shared by weight and a short last line kept to the same share with weighted spacers; those of `picked` are drawn as taken, with the pictures `figures` holds of their mockups and the offer `offer` as [`tile`] takes them.
fn tiles(alternatives: &[Alternative], (figures, offer): (&Figures, &Offer), picked: &[usize]) {
    let alternatives = alternatives.to_vec();
    let pictures: Vec<Option<Result<Drawing, String>>> = alternatives
        .iter()
        .map(|alternative| {
            let html = alternative.mockup.as_deref()?;
            let (key, _) = wanted_document(html, MOCKUP_VIEWPORT);
            figures.get(&key).cloned()
        })
        .collect();
    let offer = offer.clone();
    let picked = picked.to_vec();
    rustle::with_constraints(Modifier::new().fill_max_width(), move |constraints| {
        let per_row = tiles_per_row(constraints.max_width, alternatives.len());
        rustle::column_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::SpacedBy(sp(TILE_GAP / 4.0)),
            Alignment::Start,
            || {
                for (line, start) in (0..alternatives.len()).step_by(per_row).enumerate() {
                    let end = (start + per_row).min(alternatives.len());
                    rustle::key(line, || {
                        row_arranged(
                            Modifier::new().fill_max_width(),
                            Arrangement::SpacedBy(sp(TILE_GAP / 4.0)),
                            Alignment::Start,
                            || {
                                for index in start..end {
                                    tile(
                                        &alternatives[index],
                                        (pictures[index].clone(), &offer),
                                        (index, picked.contains(&index)),
                                    );
                                }
                                for _ in end..start + per_row {
                                    rustle::spacer(Modifier::new().weight(1.0));
                                }
                            },
                        );
                    });
                }
            },
        );
    });
}

/// Emits the row for an answer of the reader's own, with the box to type it in, which the key `number` picks.
#[track_caller]
fn other(draft: &Draft, number: usize) {
    let field = &draft.others[draft.step];
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .background_rounded(theme::color(Role::SurfaceHover), Radius::Lg.px())
            .border(
                1.0,
                theme::color(if field.text().trim().is_empty() {
                    Role::Surface
                } else {
                    Role::Accent
                }),
                Radius::Lg.px(),
            )
            .padding(1.0)
            .padding_xy(theme::sp(3), theme::sp(2)),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(3)),
                Alignment::Center,
                || {
                    rustle::row_arranged(
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
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .background_rounded(theme::color(Role::Background), Radius::Lg.px()),
                Arrangement::Start,
                Alignment::Start,
                || {
                    Input::new(field, Target::Chat(ChatEvent::Other))
                        .variant(input::Variant::Ghost)
                        .placeholder("Type your own answer here")
                        .submits(Target::Chat(ChatEvent::Next))
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
    rustle::column_arranged(
        Modifier::new()
            .panel(Radius::Xl)
            .padding_xy(theme::sp(3), theme::sp(2.5)),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(2)),
                Alignment::Start,
                || {
                    if count > 1 {
                        rustle::row_arranged(
                            Modifier::new()
                                .background_rounded(
                                    theme::color(Role::SurfaceHover),
                                    Radius::Full.px(),
                                )
                                .padding_xy(theme::sp(3), theme::sp(1)),
                            Arrangement::Start,
                            Alignment::Center,
                            || {
                                line(format!("{}/{count}", draft.step + 1), 13, 1.4, Role::Muted);
                            },
                        );
                    }
                    rustle::row_arranged(
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
                    Button::icon(icons::chevron(!draft.folded), Target::Chat(ChatEvent::Fold))
                        .build();
                    Button::icon(phosphor::X, Target::Chat(ChatEvent::Close)).build();
                },
            );
            if draft.folded {
                return;
            }
            let picked = &draft.picks[draft.step];
            rustle::column_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(1)),
                Alignment::Start,
                || {
                    if compares(&question.alternatives) {
                        tiles(&question.alternatives, (figures, offer), picked);
                    } else {
                        for (index, alternative) in question.alternatives.iter().enumerate() {
                            choice(alternative, index, picked.contains(&index), width - 92.0);
                        }
                    }
                    other(draft, question.alternatives.len() + 1);
                },
            );
            rustle::flow_row_with(
                Modifier::new().fill_max_width(),
                rustle::FlowOptions::new(theme::sp(2), theme::sp(2))
                    .arrangement(Arrangement::End)
                    .item_alignment(Alignment::Center),
                || {
                    button::action("Skip", Target::Chat(ChatEvent::Skip), Variant::Outline);
                    button::small_button(
                        if draft.last() { "Submit" } else { "Next" },
                        Target::Chat(ChatEvent::Next),
                        Variant::Default,
                    )
                    .disabled(!draft.answered())
                    .build();
                },
            );
        },
    );
}

/// What the padding and border of the card that records answers take from its width, in logical pixels.
const ANSWERED_INSET: f32 = 34.0;

/// Emits the card that records a form of questions the reader answered, at most `width` logical pixels wide: each question with its answer, or `[No preference]` when it was skipped, both wrapped to fit.
#[track_caller]
pub fn answered(answers: &[(String, Option<String>)], width: f32) {
    let room = width - ANSWERED_INSET;
    rustle::column_arranged(
        Modifier::new()
            .align(BoxAlignment::TopStart)
            .background_rounded(theme::color(Role::Background), Radius::Xl.px())
            .outlined(Radius::Xl)
            .padding_xy(theme::sp(4), theme::sp(3.5)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Start,
        || {
            for (question, answer) in answers {
                rustle::column_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(theme::sp(0.5)),
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

    /// What the padding and border of a tile take from its width, in logical pixels.
    const TILE_INSET: f32 = 18.0;

    /// How thick a tile's border is, in logical pixels: the press area a tile is tagged with lies inside it.
    const TILE_BORDER: f32 = 1.0;

    /// The question with three answers that each have a mockup.
    fn three_mockups() -> Draft {
        Draft::new(pm_harness::Inquiry {
            ticket: 1,
            questions: vec![pm_harness::Question {
                header: "Layout".to_owned(),
                text: "Which layout?".to_owned(),
                multiple: false,
                alternatives: vec![
                    alternative("Sidebar (Recommended)", true),
                    alternative("Tabs", true),
                    alternative("Neither", true),
                ],
            }],
        })
    }

    /// Where the three tiles of [`three_mockups`] are laid out when the card is `width` logical pixels wide, in the order of their answers.
    fn tiles_laid(width: f32) -> Vec<rustle::Rect> {
        let draft = three_mockups();
        let figures = Figures::new();
        let laid = crate::tagged::lay_out(width, 2000.0, None, move || {
            inquiry(&draft, &figures, &Offer::Nothing, width)
        });
        (0..3)
            .map(|index| {
                laid.node(Target::Chat(ChatEvent::InquiryPick(index)))
                    .expect("every tile is laid out")
                    .bounds
            })
            .collect()
    }

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

    /// Tiles sit two to a row, sharing it evenly with a gap between, at the width of a conversation, the third under the first at the same width, and one to a row when the card is narrow or holds one answer.
    #[test]
    fn tiles_wrap_two_to_a_row_and_one_when_narrow() {
        assert_eq!(tiles_per_row(686.0, 3), 2);
        assert_eq!(tiles_per_row(TWO_TILES, 2), 2);
        assert_eq!(tiles_per_row(TWO_TILES - 1.0, 3), 1);
        assert_eq!(tiles_per_row(686.0, 1), 1);
        let wide = tiles_laid(712.0);
        let line = wide[1].origin.x + wide[1].size.width - wide[0].origin.x;
        assert!(line > TWO_TILES);
        assert_eq!(wide[0].size.width, wide[1].size.width);
        assert_eq!(wide[0].origin.y, wide[1].origin.y);
        assert_eq!(
            wide[1].origin.x - wide[0].origin.x - wide[0].size.width,
            TILE_GAP + 2.0 * TILE_BORDER
        );
        assert!(wide[2].origin.y > wide[0].origin.y);
        assert_eq!(wide[2].origin.x, wide[0].origin.x);
        assert_eq!(wide[2].size.width, wide[0].size.width);
        let narrow = tiles_laid(426.0);
        assert!(narrow[0].size.width < TWO_TILES);
        assert!(
            narrow
                .iter()
                .all(|tile| tile.size.width == narrow[0].size.width)
        );
        assert!(
            narrow
                .iter()
                .all(|tile| tile.origin.x == narrow[0].origin.x)
        );
        assert!(narrow[1].origin.y > narrow[0].origin.y);
        assert!(narrow[2].origin.y > narrow[1].origin.y);
    }

    /// The marks of a tile are spelled out only in a heading with room for them, shrink to icons in a narrower one and go in one too narrow for those, so they never reach outside the tile.
    #[test]
    fn marks_shrink_with_the_tile() {
        assert_eq!(marks_in(339.0 - TILE_INSET), Marks::Words);
        assert_eq!(marks_in(MARKS_IN_WORDS), Marks::Words);
        assert_eq!(marks_in(MARKS_IN_WORDS - 1.0), Marks::Icons);
        assert_eq!(marks_in(MARKS_AS_ICONS), Marks::Icons);
        assert_eq!(marks_in(MARKS_AS_ICONS - 1.0), Marks::Hidden);
        assert_eq!(marks_in(100.0 - TILE_INSET), Marks::Hidden);
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
        let laid = crate::tagged::lay_out(700.0, 2000.0, None, move || {
            inquiry(&draft, &figures, &Offer::Nothing, 700.0)
        });
        let ids = laid.ids();
        for index in 0..3 {
            assert!(
                ids.contains(&Target::Chat(ChatEvent::InquiryPick(index))),
                "pick {index}"
            );
        }
        assert!(
            ids.contains(&Target::Chat(ChatEvent::MockupOpen(0)))
                && ids.contains(&Target::Chat(ChatEvent::MockupOpen(1)))
        );
        assert!(!ids.contains(&Target::Chat(ChatEvent::MockupOpen(2))));
    }
}
