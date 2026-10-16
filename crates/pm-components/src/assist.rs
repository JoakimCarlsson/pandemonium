//! What floats over the file shown while code is written in it: the list of completions at the caret and the card saying what is known about a place.

use pm_input::FocusId;
use pm_protocol::CompletionKind;
use pm_style::{
    Radius, Role, ShadowLayer, absolute, bg, border, border_color, cursor_pointer, flex, flex_1,
    flex_col, flex_none, font_mono, gap, h, items_center, justify_center, leading_ratio, left,
    max_w_px, min_w_px, overflow_hidden, p, px, rounded, shadow_layers, size, text_color, text_px,
    top, w_px, z,
};
use pm_text::Shaper;
use pm_ui::{Node, node, text};

use crate::document::{Completions, HoverCard};
use crate::editor::COMPLETIONS_SHOWN;
use crate::markdown::Markdown;
use crate::parts::line;
use crate::state::WorkspaceState;
use crate::viewer::severity_role;

/// The list of completions itself, which keeps a press on it from counting as outside.
pub const COMPLETION_BODY: FocusId = FocusId(7_200_000);

/// The card about a place, which keeps the pointer resting on it from counting as leaving the place.
pub const HOVER_CARD: FocusId = FocusId(7_199_999);

/// The first of the rows of the list of completions; the rest follow it.
pub const COMPLETION_ROW_BASE: u64 = 7_200_001;

/// One past the last row of the list of completions.
pub const COMPLETION_ROW_END: u64 = 7_300_000;

/// How wide the list of completions is, in logical pixels.
const LIST_WIDTH: f32 = 420.0;

/// How tall a row of the list of completions is, in logical pixels.
const ROW_HEIGHT: f32 = 24.0;

/// How wide the card about a place is at most, in logical pixels.
const CARD_WIDTH: f32 = 520.0;

/// How many lines of what the language server says the card shows at most.
const CARD_LINES: usize = 24;

/// The shadow under what floats over the file: soft, to lift it off the code.
const SHADOW: [ShadowLayer; 1] = [ShadowLayer::new(0.0, 8.0, 28.0, 0.0, 0.3, false)];

/// The letter and colour that mark a completion of `kind`.
fn kind_mark(kind: CompletionKind) -> (&'static str, Role) {
    match kind {
        CompletionKind::Function => ("f", Role::SyntaxFunction),
        CompletionKind::Variable => ("v", Role::SyntaxProperty),
        CompletionKind::Type => ("T", Role::SyntaxType),
        CompletionKind::Module => ("m", Role::SyntaxType),
        CompletionKind::Keyword => ("k", Role::SyntaxKeyword),
        CompletionKind::Constant => ("c", Role::SyntaxConstant),
        CompletionKind::Snippet => ("s", Role::SyntaxString),
        CompletionKind::Other => ("·", Role::Muted),
    }
}

/// A raised box at `at` in the window, the frame of what floats over the file.
fn floating(at: (f32, f32)) -> Node {
    node().styles([
        absolute(),
        left(at.0 / 4.0),
        top(at.1 / 4.0),
        z(60),
        flex(),
        flex_col(),
        p(1),
        rounded(Radius::Md),
        border(),
        border_color(Role::Border),
        bg(Role::Surface),
        shadow_layers(&SHADOW),
        overflow_hidden(),
    ])
}

/// The list of `completion`, the rows around the one picked.
fn completion_list(completion: &Completions) -> Option<Node> {
    if completion.shown.is_empty() {
        return None;
    }
    let first = completion
        .picked
        .saturating_sub(COMPLETIONS_SHOWN - 1)
        .min(completion.shown.len().saturating_sub(COMPLETIONS_SHOWN));
    let rows = completion
        .shown
        .iter()
        .enumerate()
        .skip(first)
        .take(COMPLETIONS_SHOWN)
        .filter_map(|(place, index)| {
            let item = completion.items.get(*index)?;
            let picked = place == completion.picked;
            let (letter, tone) = kind_mark(item.kind);
            let mut row = node()
                .styles([
                    flex(),
                    items_center(),
                    gap(2),
                    h(6),
                    px(1.5),
                    rounded(Radius::Sm),
                    cursor_pointer(),
                    bg(Role::SurfaceHover).hover(),
                ])
                .focusable(FocusId(COMPLETION_ROW_BASE + place as u64))
                .child(
                    node()
                        .styles([
                            flex(),
                            items_center(),
                            justify_center(),
                            size(4),
                            flex_none(),
                        ])
                        .child(text(letter).styles([
                            text_px(11),
                            leading_ratio(1.2),
                            font_mono(),
                            text_color(tone),
                        ])),
                )
                .child(
                    node()
                        .styles([flex_1(), min_w_px(0), overflow_hidden()])
                        .child(text(item.label.clone()).styles([
                            text_px(12),
                            leading_ratio(1.4),
                            font_mono(),
                            text_color(Role::Text),
                        ])),
                );
            if let Some(detail) = &item.detail {
                let short: String = detail.chars().take(48).collect();
                row = row.child(
                    node()
                        .styles([flex_none(), max_w_px(LIST_WIDTH * 0.45), overflow_hidden()])
                        .child(line(short, 11, 1.4, Role::Subtle).styles([font_mono()])),
                );
            }
            if picked {
                row = row.styles([bg(Role::SurfaceSelected)]);
            }
            Some(row)
        });
    Some(
        floating(completion.at)
            .styles([w_px(LIST_WIDTH)])
            .focusable(COMPLETION_BODY)
            .children(rows),
    )
}

/// The card `card`, of what the language server finds wrong at the place and what it says about it, laid out with `shaper`.
fn hover_card(card: &HoverCard, shaper: &mut Shaper) -> Option<Node> {
    if card.text.is_none() && card.diagnostics.is_empty() {
        return None;
    }
    let mut body = floating(card.at)
        .styles([max_w_px(CARD_WIDTH), p(3), gap(2)])
        .focusable(HOVER_CARD);
    for diagnostic in &card.diagnostics {
        let source = diagnostic
            .source
            .as_deref()
            .map(|source| format!("{source}: "))
            .unwrap_or_default();
        let mut lines = diagnostic.message.lines();
        let first = lines.next().unwrap_or_default();
        body = body.child(
            node()
                .styles([flex(), flex_col()])
                .child(line(
                    format!("{source}{first}"),
                    12,
                    1.4,
                    severity_role(diagnostic.severity),
                ))
                .children(
                    lines
                        .take(4)
                        .map(|rest| line(rest.to_owned(), 12, 1.4, Role::Muted)),
                ),
        );
    }
    if let Some(said) = &card.text {
        let clipped: String = said
            .trim()
            .lines()
            .take(CARD_LINES)
            .collect::<Vec<_>>()
            .join("\n");
        body = body.child(
            Markdown::new(&clipped, CARD_WIDTH - 24.0)
                .size(12)
                .build(shaper),
        );
    }
    Some(body)
}

/// What floats over the file shown: its list of completions and its card about a place, when they are open.
pub fn assist(state: &WorkspaceState, shaper: &mut Shaper) -> Vec<Node> {
    let Some(document) = state.shown_document() else {
        return Vec::new();
    };
    let mut floating = Vec::new();
    if let Some(card) = &document.hover {
        floating.extend(hover_card(card, shaper));
    }
    if let Some(completion) = document.completion.as_ref().filter(|held| !held.loading) {
        floating.extend(completion_list(completion));
    }
    floating
}

/// How tall the list of completions of `completion` is, in logical pixels.
pub fn list_height(completion: &Completions) -> f32 {
    completion.shown.len().min(COMPLETIONS_SHOWN) as f32 * ROW_HEIGHT + 8.0
}
