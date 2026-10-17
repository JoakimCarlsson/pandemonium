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

use crate::editor::COMPLETIONS_SHOWN;
use crate::input::{Input, Size as InputSize};
use crate::line_edits::{GO_TO_LINE, GO_TO_LINE_BOX, GoingTo};
use crate::listing::{
    Items, LISTING_BODY, LISTING_FILTER, LISTING_ROW_BASE, LISTING_SHOWN, LISTING_WIDTH, Listing,
};
use crate::markdown::Markdown;
use crate::navigate::{RENAME_BOX, RENAME_SYMBOL, Renaming};
use crate::parts::line;
use crate::state::WorkspaceState;
use crate::viewer::severity_role;
use pm_editor::{Completions, HoverCard};

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

/// How wide the documentation beside the list of completions is, in logical pixels.
const DOCS_WIDTH: f32 = 380.0;

/// How many lines of the documentation of a completion are read at most.
const DOCS_LINES: usize = 40;

/// The documentation of the completion picked in `completion`, beside the list, when the language server sent any: its detail as code, then what it says.
fn completion_docs(
    completion: &Completions,
    shown: bool,
    language: Option<&str>,
    shaper: &mut Shaper,
) -> Option<Node> {
    if !shown {
        return None;
    }
    let item = completion
        .shown
        .get(completion.picked)
        .and_then(|index| completion.items.get(*index))?;
    let said = item.documentation.as_deref()?;
    let width = DOCS_WIDTH - 26.0;
    let mut parts: Vec<Node> = Vec::new();
    if let Some(detail) = &item.detail {
        parts.extend(code_lines(detail, "", language, width, shaper));
        parts.push(rule());
    }
    for piece in pieces(said, DOCS_LINES) {
        parts.push(match piece {
            Piece::Code(fence, body) => node()
                .styles([flex(), flex_col(), min_w_px(0), overflow_hidden()])
                .children(code_lines(&body, &fence, language, width, shaper)),
            Piece::Prose(text) => Markdown::new(&text, width)
                .size(12)
                .tone(Role::Muted)
                .build(shaper),
            Piece::Rule => rule(),
        });
    }
    Some(
        floating((completion.at.0 + LIST_WIDTH + 4.0, completion.at.1))
            .styles([
                w_px(DOCS_WIDTH),
                max_h(list_height(completion).max(160.0) / 4.0),
                gap(1.5),
                px(3),
                py(2),
            ])
            .children(parts),
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
            text(&body[range.clone()])
                .styles(setting)
                .styles([text_color(Role::Text)])
                .marks(crate::viewer::highlight_marks(&highlights, range))
        })
        .collect()
}

/// A hairline across the card.
fn rule() -> Node {
    node().styles([h_px(1), w_full(), flex_none(), bg(Role::Border)])
}

/// The card `card`, of what the language server finds wrong at the place and what it says about it, its code coloured as the file's language `language` when no fence names another, in a window `height` logical pixels tall, laid out with `shaper`: below the place's line, or above it when there is more room there, and scrolling when it is taller than that room.
fn hover_card(
    row: f32,
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
    let line_bottom = card.at.1 + row;
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
        return state
            .listing_shown()
            .map(listing_or_quick)
            .into_iter()
            .collect();
    };
    let mut floating = Vec::new();
    if let Some(card) = &document.hover {
        floating.extend(hover_card(
            state.editor.row_height(),
            card,
            document.language.as_deref(),
            height,
            shaper,
        ));
    }
    if let Some(completion) = document.completion.as_ref().filter(|held| !held.loading) {
        floating.extend(completion_list(completion));
        floating.extend(completion_docs(
            completion,
            state.editor.completion_documentation,
            document.language.as_deref(),
            shaper,
        ));
    }
    if let Some(listing) = state.listing_shown() {
        floating.push(listing_or_quick(listing));
    }
    if let Some(renaming) = &state.renaming {
        floating.push(rename_box(renaming));
    }
    if let Some(going) = &state.going_to {
        floating.push(go_to_line_box(going, document.lines()));
    }
    if let Some(signature) = document
        .signature
        .as_ref()
        .filter(|_| document.completion.is_none())
    {
        floating.push(signature_card(signature, state.signature_at));
    }
    floating
}

/// How tall the card with the signature of the call the caret is in is, in logical pixels.
pub fn signature_height(signature: &pm_protocol::Signature) -> f32 {
    if signature.documentation.is_some() {
        52.0
    } else {
        30.0
    }
}

/// The signature of the call the caret is in, the parameter the caret is at marked, with the first line of what its documentation says.
fn signature_card(signature: &pm_protocol::Signature, at: (f32, f32)) -> Node {
    let mut marks = Vec::new();
    if let Some((start, end)) = signature.active {
        marks.push(Mark {
            range: start..end,
            color: Role::Accent.into(),
            paint: Paint::Ink,
        });
        marks.push(Mark {
            range: start..end,
            color: Role::Accent.into(),
            paint: Paint::Underline,
        });
    }
    let mut label = signature.label.clone();
    if signature.others > 0 {
        label.push_str(&format!("   (+{} more)", signature.others));
    }
    let first_line = signature
        .documentation
        .as_deref()
        .and_then(|said| {
            said.lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with("```"))
        })
        .map(|line| line.replace('`', ""));
    floating(at)
        .styles([max_w_px(CARD_WIDTH + 120.0), gap(0.5), px(2), py(1)])
        .child(
            node().styles([overflow_hidden()]).child(
                text(label)
                    .styles([
                        text_px(12),
                        leading_ratio(1.5),
                        font_mono(),
                        text_color(Role::Text),
                    ])
                    .marks(marks),
            ),
        )
        .children(first_line.map(|said| {
            node()
                .styles([overflow_hidden()])
                .child(line(said, 11, 1.4, Role::Muted))
        }))
}

/// The box the number of the line to go to is typed in, saying how many lines there are.
fn go_to_line_box(going: &GoingTo, lines: usize) -> Node {
    floating(going.at)
        .styles([w_px(260.0), gap(1), p(1.5)])
        .focusable(GO_TO_LINE_BOX)
        .child(
            Input::new(&going.field, GO_TO_LINE)
                .size(InputSize::Sm)
                .placeholder("Line number")
                .build(),
        )
        .child(line(
            format!("Go to a line from 1 to {lines}"),
            11,
            1.4,
            Role::Subtle,
        ))
}

/// A line of the list over the file, `content` in `pixels` and `tone`, cut short at the edge of the room it is given; in the font for code when `code`.
fn cell(content: impl Into<String>, pixels: u32, tone: Role, code: bool) -> Node {
    let mut line =
        text(content.into()).styles([text_px(pixels), leading_ratio(1.4), text_color(tone)]);
    if code {
        line = line.styles([font_mono()]);
    }
    node()
        .styles([flex_1(), min_w_px(0), overflow_hidden()])
        .child(line)
}

/// A line of the list over the file that keeps its own width, `content` in `pixels` and `tone`.
fn tag(content: impl Into<String>, pixels: u32, tone: Role) -> Node {
    node()
        .styles([
            flex_none(),
            max_w_px(LISTING_WIDTH * 0.5),
            overflow_hidden(),
        ])
        .child(line(content.into(), pixels, 1.4, tone))
}

/// The file name and line of `location`, as a list names it.
fn place_name(location: &pm_protocol::Location) -> String {
    let name = location.path.rsplit('/').next().unwrap_or(&location.path);
    format!("{name}:{}", location.span.start.line + 1)
}

/// The cells of the `index`-th row of `items`.
pub(crate) fn listing_cells(items: &Items, index: usize) -> Vec<Node> {
    match items {
        Items::Symbols { locations, .. } => {
            let Some(location) = locations.get(index) else {
                return Vec::new();
            };
            vec![
                cell(
                    location.name.clone().unwrap_or_default(),
                    12,
                    Role::Text,
                    true,
                ),
                tag(place_name(location), 11, Role::Subtle),
            ]
        }
        Items::Problems { problems } => {
            let Some(problem) = problems.get(index) else {
                return Vec::new();
            };
            let diagnostic = &problem.diagnostic;
            let first = diagnostic.message.lines().next().unwrap_or_default();
            let name = problem.path.rsplit('/').next().unwrap_or(&problem.path);
            vec![
                node().styles([
                    size(2),
                    rounded(4.0),
                    flex_none(),
                    bg(severity_role(diagnostic.severity)),
                ]),
                cell(first.to_owned(), 12, Role::Text, false),
                tag(
                    format!("{name}:{}", diagnostic.span.start.line + 1),
                    11,
                    Role::Subtle,
                ),
            ]
        }
        Items::Files { paths, .. } => {
            let Some(path) = paths.get(index) else {
                return Vec::new();
            };
            let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
            vec![
                crate::file_icon::file_icon(path, 4.0),
                tag(name.to_owned(), 12, Role::Text),
                cell(folder.to_owned(), 11, Role::Subtle, false),
            ]
        }
        Items::Line { .. } => Vec::new(),
        Items::Choices { choices, .. } => {
            let Some(choice) = choices.get(index) else {
                return Vec::new();
            };
            vec![
                cell(choice.label.clone(), 12, Role::Text, false),
                tag(choice.detail.clone(), 11, Role::Subtle),
            ]
        }
        Items::Sessions { sessions, .. } => {
            let Some((_, title, beside)) = sessions.get(index) else {
                return Vec::new();
            };
            vec![
                cell(title.clone(), 12, Role::Text, false),
                tag(beside.clone(), 11, Role::Subtle),
            ]
        }
        Items::Commands { commands, .. } => {
            let Some((action, keys)) = commands.get(index) else {
                return Vec::new();
            };
            let mut cells = vec![cell(action.title(), 12, Role::Text, false)];
            cells.extend(keys.clone().map(|keys| tag(keys, 11, Role::Subtle)));
            cells
        }
        Items::Outline { items, .. } => {
            let Some(item) = items.get(index) else {
                return Vec::new();
            };
            vec![
                node().styles([w_px(item.depth.min(8) as f32 * 14.0), flex_none()]),
                node()
                    .styles([w_px(52.0), flex_none(), overflow_hidden()])
                    .child(line(item.context.clone(), 11, 1.4, Role::SyntaxKeyword)),
                cell(item.name.clone(), 12, Role::Text, true),
            ]
        }
        Items::Places { locations, .. } | Items::Search { locations, .. } => {
            let Some(location) = locations.get(index) else {
                return Vec::new();
            };
            match &location.name {
                Some(name) => vec![
                    cell(name.clone(), 12, Role::Text, true),
                    tag(place_name(location), 11, Role::Subtle),
                ],
                None => vec![
                    tag(place_name(location), 12, Role::Text),
                    cell(
                        location.line.as_deref().unwrap_or_default().trim(),
                        12,
                        Role::Muted,
                        true,
                    ),
                ],
            }
        }
        Items::Actions { offered, .. } => {
            let Some(action) = offered.get(index) else {
                return Vec::new();
            };
            let tone = if action.disabled.is_some() {
                Role::Subtle
            } else {
                Role::Text
            };
            let mut cells = vec![cell(action.title.clone(), 12, tone, false)];
            if let Some(reason) = action.disabled.as_ref().or(action.kind.as_ref()) {
                cells.push(tag(reason.clone(), 11, Role::Subtle));
            }
            cells
        }
        Items::History { commits, .. } => {
            let Some(commit) = commits.get(index) else {
                return Vec::new();
            };
            vec![
                cell(commit.summary.clone(), 12, Role::Text, false),
                tag(
                    format!("{} · {} · {}", commit.id, commit.author, commit.when),
                    11,
                    Role::Subtle,
                ),
            ]
        }
    }
}

/// The quick input when `listing` is one, and otherwise the list over the file.
fn listing_or_quick(listing: &Listing) -> Node {
    if listing.quick {
        crate::quick_input::quick_input(listing)
    } else {
        listing_panel(listing)
    }
}

/// The list over the file: its title and how many rows it has, then the rows around the one picked, or what keeps it empty.
fn listing_panel(listing: &Listing) -> Node {
    let shown = listing.items.rows();
    let count = shown.len();
    let status = match (&listing.problem, listing.answered, count) {
        (Some(problem), _, _) => Some(problem.clone()),
        (None, false, _) => Some("Asking…".to_owned()),
        (None, true, 0) => Some("Nothing found".to_owned()),
        _ => None,
    };
    let filter = match &listing.items {
        Items::Outline { filter, .. } => Some((filter, "Go to an item of this file")),
        Items::Files { filter, .. } => Some((filter, "Open a file of this project")),
        Items::Commands { filter, .. } => Some((filter, "Run a command")),
        Items::Symbols { filter, .. } => Some((filter, "Go to a symbol of this project")),
        Items::Search { filter, .. } => Some((filter, "Search the files of this project")),
        _ => None,
    }
    .map(|(filter, hint)| {
        node().styles([px(0.5), py(0.5)]).child(
            Input::new(filter, LISTING_FILTER)
                .size(InputSize::Sm)
                .placeholder(hint)
                .build(),
        )
    });
    let header = node()
        .styles([flex(), items_center(), gap(2), h(7), px(1.5)])
        .child(cell(listing.items.title(), 12, Role::Text, false))
        .children((count > 0).then(|| tag(count.to_string(), 11, Role::Subtle)));
    let first = listing
        .picked
        .saturating_sub(LISTING_SHOWN - 1)
        .min(count.saturating_sub(LISTING_SHOWN));
    let rows = (first..count.min(first + LISTING_SHOWN)).map(|place| {
        let index = shown[place];
        let mut row = node()
            .styles([
                flex(),
                items_center(),
                gap(3),
                h(6.5),
                px(1.5),
                rounded(Radius::Sm),
                cursor_pointer(),
                bg(Role::SurfaceHover).hover(),
            ])
            .focusable(FocusId(LISTING_ROW_BASE + place as u64))
            .children(listing_cells(&listing.items, index));
        if place == listing.picked {
            row = row.styles([bg(Role::SurfaceSelected)]);
        }
        row
    });
    floating(listing.at)
        .styles([w_px(LISTING_WIDTH)])
        .focusable(LISTING_BODY)
        .children(filter)
        .child(header)
        .children(status.map(|said| {
            node()
                .styles([px(1.5), py(1)])
                .child(line(said, 12, 1.4, Role::Muted))
        }))
        .children(rows)
}

/// The box the new name of the thing at the caret is typed in, with why the last rename failed under it.
fn rename_box(renaming: &Renaming) -> Node {
    let hint = if renaming.busy {
        "Renaming…"
    } else {
        "Enter to rename, Escape to cancel"
    };
    floating(renaming.at)
        .styles([w_px(320.0), gap(1), p(1.5)])
        .focusable(RENAME_BOX)
        .child(
            Input::new(&renaming.field, RENAME_SYMBOL)
                .size(InputSize::Sm)
                .invalid(renaming.problem.is_some())
                .build(),
        )
        .child(line(
            renaming.problem.clone().unwrap_or_else(|| hint.to_owned()),
            11,
            1.4,
            if renaming.problem.is_some() {
                Role::Danger
            } else {
                Role::Subtle
            },
        ))
}

/// How tall the list of completions of `completion` is, in logical pixels.
pub fn list_height(completion: &Completions) -> f32 {
    completion.shown.len().min(COMPLETIONS_SHOWN) as f32 * ROW_HEIGHT + 8.0
}
