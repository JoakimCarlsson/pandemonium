//! What floats over the file shown while code is written in it: the list of completions at the caret and the card saying what is known about a place.

use pm_input::FocusId;
use pm_protocol::CompletionKind;
use pm_style::{
    Radius, Role, ShadowLayer, absolute, bg, border, border_color, bottom, cursor_pointer, flex,
    flex_1, flex_col, flex_none, font_mono, gap, h, h_px, items_center, justify_center,
    leading_ratio, left, max_h, max_w_px, min_h, min_w_px, overflow_hidden, p, px, py, rounded,
    shadow_layers, size, text_color, text_px, top, w_full, w_px, z,
};
use pm_text::Shaper;
use pm_ui::{Mark, Node, Paint, node, text};

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

/// The text of the card about a place, which the pointer selects as one.
pub const HOVER_TEXT: FocusId = FocusId(7_199_998);

/// The first of the links of the card about a place; each stretch of prose has a hundred after its own base, the stretches following one another.
pub const HOVER_LINK_BASE: u64 = 7_190_000;

/// One past the last of the links of the card about a place.
pub const HOVER_LINK_END: u64 = 7_199_000;

/// How many links one stretch of prose of the card has room for.
const LINKS_PER_PROSE: u64 = 100;

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

/// How many lines of what the language server says the card reads at most.
const CARD_LINES: usize = 2000;

/// How tall the card about a place is at most, in logical pixels, when the window has room.
const CARD_HEIGHT: f32 = 420.0;

/// How far the card keeps from the line it is about and from the window's edges, in logical pixels.
const CARD_GAP: f32 = 4.0;

/// How wide the card's text is: the card less its padding, in logical pixels.
const CARD_TEXT: f32 = CARD_WIDTH - 26.0;

/// The scrolling body of the card about a place.
pub const HOVER_SCROLL: FocusId = FocusId(7_199_997);

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

/// One stretch of what a language server says about a place.
enum Piece {
    /// A block of code, in the language a fence names or that of the file.
    Code(String, String),
    /// Prose, in markdown.
    Prose(String),
    /// A rule between two parts.
    Rule,
}

/// What a language server said, `said`, cut into its blocks of code, its prose and the rules between them, at most `most` lines of it.
fn pieces(said: &str, most: usize) -> Vec<Piece> {
    let mut found = Vec::new();
    let mut prose = String::new();
    let mut code: Option<(String, String)> = None;
    let flush = |prose: &mut String, found: &mut Vec<Piece>| {
        if !prose.trim().is_empty() {
            found.push(Piece::Prose(prose.trim().to_owned()));
        }
        prose.clear();
    };
    for text in said.lines().take(most) {
        let fence = text.trim_start().strip_prefix("```");
        match (&mut code, fence) {
            (Some(_), Some(_)) => {
                if let Some((language, body)) = code.take()
                    && !body.trim().is_empty()
                {
                    found.push(Piece::Code(language, body.trim_end().to_owned()));
                }
            }
            (Some((_, body)), None) => {
                body.push_str(text);
                body.push('\n');
            }
            (None, Some(language)) => {
                flush(&mut prose, &mut found);
                code = Some((language.trim().to_owned(), String::new()));
            }
            (None, None) if matches!(text.trim(), "---" | "___" | "***") => {
                flush(&mut prose, &mut found);
                found.push(Piece::Rule);
            }
            (None, None) => {
                prose.push_str(text);
                prose.push('\n');
            }
        }
    }
    if let Some((language, body)) = code
        && !body.trim().is_empty()
    {
        found.push(Piece::Code(language, body.trim_end().to_owned()));
    }
    flush(&mut prose, &mut found);
    while matches!(found.last(), Some(Piece::Rule)) {
        found.pop();
    }
    while matches!(found.first(), Some(Piece::Rule)) {
        found.remove(0);
    }
    found
}

/// The address of the link of the card that says `said` that `id` names, when it names one.
pub fn hover_link(said: &str, id: FocusId) -> Option<String> {
    let offset =
        id.0.checked_sub(HOVER_LINK_BASE)
            .filter(|_| id.0 < HOVER_LINK_END)?;
    let (stretch, link) = (offset / LINKS_PER_PROSE, offset % LINKS_PER_PROSE);
    let prose = pieces(said, CARD_LINES)
        .into_iter()
        .filter_map(|piece| match piece {
            Piece::Prose(text) => Some(text),
            _ => None,
        })
        .nth(stretch as usize)?;
    pm_markdown::links(&prose).into_iter().nth(link as usize)
}

/// The lines of `body`, code written in the language `language` or else `fallback`, each coloured by that language's grammar when it is loaded and wrapped at `width` logical pixels as `shaper` measures it.
fn code_lines(
    body: &str,
    language: &str,
    fallback: Option<&str>,
    width: f32,
    shaper: &mut Shaper,
) -> Vec<Node> {
    let body = body.replace('\t', "    ");
    let grammar = pm_treesitter::registry::loaded(language)
        .or_else(|| fallback.and_then(pm_treesitter::registry::loaded));
    let highlights = grammar
        .and_then(|grammar| pm_treesitter::Syntax::parse(grammar, &body))
        .map(|syntax| syntax.highlights(&body, 0..body.len()))
        .unwrap_or_default();
    let setting = [text_px(12), leading_ratio(1.5), font_mono()];
    pm_ui::wrap(shaper, &body, setting, width)
        .into_iter()
        .map(|piece| {
            let range = piece.start..piece.end;
            let marks = highlights.iter().filter_map(|highlight| {
                let low = highlight.range.start.max(range.start);
                let high = highlight.range.end.min(range.end);
                let role = crate::viewer::kind_role(highlight.kind)?;
                (low < high).then(|| Mark {
                    range: low - range.start..high - range.start,
                    color: role.into(),
                    paint: Paint::Ink,
                })
            });
            text(&body[range.clone()])
                .styles(setting)
                .styles([text_color(Role::Text)])
                .marks(marks.collect::<Vec<_>>())
        })
        .collect()
}

/// A hairline across the card.
fn rule() -> Node {
    node().styles([h_px(1), w_full(), flex_none(), bg(Role::Border)])
}

/// The card `card`, of what the language server finds wrong at the place and what it says about it, its code coloured as the file's language `language` when no fence names another, in a window `height` logical pixels tall, laid out with `shaper`: below the place's line, or above it when there is more room there, and scrolling when it is taller than that room.
fn hover_card(
    card: &HoverCard,
    language: Option<&str>,
    height: f32,
    shaper: &mut Shaper,
) -> Option<Node> {
    if card.text.is_none() && card.diagnostics.is_empty() {
        return None;
    }
    let mut parts: Vec<Node> = Vec::new();
    for diagnostic in &card.diagnostics {
        let source = diagnostic
            .source
            .as_deref()
            .map(|source| format!("{source}: "))
            .unwrap_or_default();
        let mut lines = diagnostic.message.lines();
        let first = lines.next().unwrap_or_default();
        parts.push(
            node()
                .styles([flex(), flex_col()])
                .child(line(
                    format!("{source}{first}"),
                    12,
                    1.5,
                    severity_role(diagnostic.severity),
                ))
                .children(
                    lines
                        .take(4)
                        .map(|rest| line(rest.to_owned(), 12, 1.5, Role::Muted)),
                ),
        );
    }
    if let Some(said) = &card.text {
        if !parts.is_empty() {
            parts.push(rule());
        }
        let mut prose = 0;
        for piece in pieces(said, CARD_LINES) {
            parts.push(match piece {
                Piece::Code(fence, body) => node()
                    .styles([flex(), flex_col(), min_w_px(0), overflow_hidden()])
                    .children(code_lines(&body, &fence, language, CARD_TEXT, shaper)),
                Piece::Prose(text) => {
                    prose += 1;
                    Markdown::new(&text, CARD_TEXT)
                        .size(12)
                        .tone(Role::Muted)
                        .links(HOVER_LINK_BASE + (prose - 1) * LINKS_PER_PROSE)
                        .build(shaper)
                }
                Piece::Rule => rule(),
            });
        }
    }
    let line_bottom = card.at.1 + crate::open_files::ROW;
    let below = (height - line_bottom - CARD_GAP * 2.0).max(0.0);
    let above = (card.at.1 - CARD_GAP * 2.0).max(0.0);
    let downward = below >= CARD_HEIGHT.min(above) || below >= above;
    let room = if downward { below } else { above }.min(CARD_HEIGHT);
    let placed = if downward {
        top((line_bottom + CARD_GAP) / 4.0)
    } else {
        bottom((height - card.at.1 + CARD_GAP) / 4.0)
    };
    Some(
        node()
            .styles([
                absolute(),
                left(card.at.0 / 4.0),
                placed,
                z(60),
                flex(),
                flex_col(),
                w_px(CARD_WIDTH),
                max_h(room / 4.0),
                rounded(Radius::Md),
                border(),
                border_color(Role::Border),
                bg(Role::Surface),
                shadow_layers(&SHADOW),
                overflow_hidden(),
            ])
            .focusable(HOVER_CARD)
            .child(
                node()
                    .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
                    .scroll_from_start(HOVER_SCROLL, card.scroll)
                    .child(
                        node()
                            .styles([flex(), flex_col(), flex_none(), px(3), py(2.5), gap(2)])
                            .selectable(HOVER_TEXT)
                            .children(parts),
                    ),
            ),
    )
}

/// What floats over the file shown, in a window `height` logical pixels tall: its list of completions and its card about a place, when they are open.
pub fn assist(state: &WorkspaceState, height: f32, shaper: &mut Shaper) -> Vec<Node> {
    let Some(document) = state.shown_document() else {
        return Vec::new();
    };
    let mut floating = Vec::new();
    if let Some(card) = &document.hover {
        floating.extend(hover_card(
            card,
            document.language.as_deref(),
            height,
            shaper,
        ));
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
