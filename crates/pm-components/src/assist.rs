//! What floats over the file shown while code is written in it: the list of completions at the caret and the card saying what is known about a place.

use crate::events::reduce;
use crate::theme;
use pm_protocol::CompletionKind;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::events::Interactive;
use crate::focus::FocusId;
use crate::icons::IconName;
use crate::marked::{CaretLook, Mark, Paint, marked_line};
use crate::popups::at_window;
use crate::theme::{Radius, Role, Tone, sp};

use crate::color_picker::{
    COLOR_BOX, COLOR_FORMAT_BASE, COLOR_FORMATS_SHOWN, COLOR_HEX, ColorPicking,
};
use crate::editor::COMPLETIONS_SHOWN;
use crate::input::Input;
use crate::line_edits::{GO_TO_LINE, GO_TO_LINE_BOX, GoingTo};
use crate::listing::{
    Items, LISTING_BODY, LISTING_FILTER, LISTING_ROW_BASE, LISTING_SHOWN, LISTING_WIDTH, Listing,
};
use crate::markdown::Markdown;
use crate::navigate::{RENAME_BOX, RENAME_SYMBOL, Renaming};
use crate::parts::{glyph, line};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::viewer::severity_role;
use pm_editor::{Completions, HoverCard};

/// The list of completions itself, which keeps a press on it from counting as outside.
pub const COMPLETION_BODY: FocusId = FocusId(7_200_000);

/// The card about a place, which keeps the pointer resting on it from counting as leaving the place.
pub const HOVER_CARD: FocusId = FocusId(7_199_999);

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

/// The shadow under what floats over the file: soft, to lift it off the code.
/// Each layer is `(x, y, blur, spread, alpha)`.
const SHADOW: [(f32, f32, f32, f32, f32); 1] = [(0.0, 8.0, 28.0, 0.0, 0.3)];

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

/// Emits a raised box at `at` in the window, the frame of what floats over the file, holding `content`.
///
/// `size` is what bounds the box (its width, its greatest height), `target` the name it answers
/// to, with which a press outside it asks the app to close what a press elsewhere closes,
/// `padding` the room inside its edge and `arrangement` how its column spreads what it holds.
#[track_caller]
fn floating(
    at: (f32, f32),
    size: Modifier,
    target: Option<FocusId>,
    padding: Modifier,
    arrangement: Arrangement,
    content: impl FnOnce(),
) {
    at_window(at.0, at.1, target.is_some(), move || {
        framed(size, target, padding, arrangement, content);
    });
}

/// Emits the raised column of what floats over the file, with `size`, answering to `target`,
/// `padding` inside its edge and spread by `arrangement`, holding `content`.
#[track_caller]
fn framed(
    size: Modifier,
    target: Option<FocusId>,
    padding: Modifier,
    arrangement: Arrangement,
    content: impl FnOnce(),
) {
    let own = size
        .draw_behind(theme::shadows(&SHADOW, Radius::Md))
        .background_rounded(theme::color(Role::Surface), Radius::Md.px())
        .border(1.0, theme::color(Role::Border), Radius::Md.px())
        .padding(1.0)
        .clip();
    let own = match target {
        Some(id) => own.control(id, reduce(id)),
        None => own,
    };
    column_arranged(own.chain(padding), arrangement, Alignment::Start, content);
}

/// Emits `content` in a box that takes the room left in its row and cuts it off at its edge.
#[track_caller]
fn clipped_box(content: impl FnOnce()) {
    column_arranged(
        Modifier::new().weight(1.0).clip(),
        Arrangement::Start,
        Alignment::Start,
        content,
    );
}

/// Emits the row of the list of completions at `place`, for `item`, filled while it is `picked`.
#[track_caller]
fn completion_row(place: usize, item: &pm_protocol::Completion, picked: bool) {
    let id = FocusId(COMPLETION_ROW_BASE + place as u64);
    let (letter, tone) = kind_mark(item.kind);
    let looks = theme::InteractionTones::new()
        .hover(Role::SurfaceHover)
        .radius(Radius::Sm);
    let looks = if picked {
        looks.idle(Role::SurfaceSelected)
    } else {
        looks
    };
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(6))
            .interaction_focused(crate::focus::is_held(id))
            .interaction_background((looks).colors())
            .cursor(CursorIcon::Pointer)
            .control(id, reduce(id))
            .padding_xy(theme::sp(1.5), 0.0),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new().size(theme::sp(4), theme::sp(4)),
                Arrangement::Center,
                Alignment::Center,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        letter,
                        theme::code_style(11, 1.2).color(theme::color(tone)),
                        rustle::TextOverflow::Ellipsis,
                    )
                },
            );
            clipped_box(|| {
                rustle::basic_text(
                    Modifier::new(),
                    item.label.clone(),
                    theme::code_style(12, 1.4).color(theme::color(Role::Text)),
                    rustle::TextOverflow::Ellipsis,
                )
            });
            if let Some(detail) = &item.detail {
                let short: String = detail.chars().take(48).collect();
                column_arranged(
                    Modifier::new().width_in(0.0, LIST_WIDTH * 0.45).clip(),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        rustle::basic_text(
                            Modifier::new(),
                            short,
                            theme::mono_style(11, 1.4)
                                .color(theme::color(Role::Subtle))
                                .max_lines(1),
                            rustle::TextOverflow::Ellipsis,
                        )
                    },
                );
            }
        },
    );
}

/// Emits the list of `completion`, the rows around the one picked, or nothing when it has none.
#[track_caller]
fn completion_list(completion: &Completions) {
    if completion.shown.is_empty() {
        return;
    }
    let first = completion
        .picked
        .saturating_sub(COMPLETIONS_SHOWN - 1)
        .min(completion.shown.len().saturating_sub(COMPLETIONS_SHOWN));
    floating(
        completion.at,
        Modifier::new().width(LIST_WIDTH),
        Some(COMPLETION_BODY),
        Modifier::new().padding(theme::sp(1)),
        Arrangement::Start,
        || {
            for (place, index) in completion
                .shown
                .iter()
                .enumerate()
                .skip(first)
                .take(COMPLETIONS_SHOWN)
            {
                if let Some(item) = completion.items.get(*index) {
                    completion_row(place, item, place == completion.picked);
                }
            }
        },
    );
}

/// How wide the documentation beside the list of completions is, in logical pixels.
const DOCS_WIDTH: f32 = 380.0;

/// How many lines of the documentation of a completion are read at most.
const DOCS_LINES: usize = 40;

/// Emits the documentation of the completion picked in `completion`, beside the list, when it is `shown` and the language server sent any: its detail as code, then what it says.
#[track_caller]
fn completion_docs(completion: &Completions, shown: bool, language: Option<&str>) {
    if !shown {
        return;
    }
    let Some(item) = completion
        .shown
        .get(completion.picked)
        .and_then(|index| completion.items.get(*index))
    else {
        return;
    };
    let Some(said) = item.documentation.as_deref() else {
        return;
    };
    let width = DOCS_WIDTH - 26.0;
    floating(
        (completion.at.0 + LIST_WIDTH + 4.0, completion.at.1),
        Modifier::new()
            .width(DOCS_WIDTH)
            .height_in(0.0, sp(list_height(completion).max(160.0) / 4.0)),
        None,
        Modifier::new()
            .padding_xy(theme::sp(3), 0.0)
            .padding_xy(0.0, theme::sp(2)),
        Arrangement::SpacedBy(sp(1.5)),
        || {
            if let Some(detail) = &item.detail {
                code_lines(detail, "", language, width);
                rule();
            }
            for piece in pieces(said, DOCS_LINES) {
                match piece {
                    Piece::Code(fence, body) => {
                        column_arranged(
                            Modifier::new().clip(),
                            Arrangement::Start,
                            Alignment::Start,
                            || {
                                code_lines(&body, &fence, language, width);
                            },
                        );
                    }
                    Piece::Prose(text) => Markdown::new(&text, width)
                        .size(12)
                        .tone(Role::Muted)
                        .build(),
                    Piece::Rule => rule(),
                }
            }
        },
    );
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

/// Emits the lines of `body`, code written in the language `language` or else `fallback`, each coloured by that language's grammar when it is loaded and wrapped at `width` logical pixels.
#[track_caller]
fn code_lines(body: &str, language: &str, fallback: Option<&str>, width: f32) {
    let body = body.replace('\t', "    ");
    let grammar = pm_treesitter::registry::loaded(language)
        .or_else(|| fallback.and_then(pm_treesitter::registry::loaded));
    let highlights = grammar
        .and_then(|grammar| pm_treesitter::Syntax::parse(grammar, &body))
        .map(|syntax| syntax.highlights(&body, 0..body.len()))
        .unwrap_or_default();
    let kind = theme::code_style(12, 1.5).color(theme::color(Role::Text));
    for piece in rustle::wrap_at_spaces(&body, &kind, width) {
        let range = piece.start..piece.end;
        marked_line(
            Modifier::new(),
            &body[range.clone()],
            kind,
            crate::viewer::highlight_marks(&highlights, range),
            CaretLook::default(),
        );
    }
}

/// Emits a hairline across the card.
#[track_caller]
fn rule() {
    rustle::horizontal_divider(
        Modifier::new(),
        rustle::DividerDefaults::THICKNESS,
        rustle::DividerDefaults::color(),
    );
}

/// Emits the card `card`, its text selected as `selection` holds and scrolled as `scrolled` says, of what the language server finds wrong at the place and what it says about it, its code coloured as the file's language `language` when no fence names another, with lines `row` logical pixels tall, in a window `height` logical pixels tall: below the place's line, or above it when there is more room there, and scrolling when it is taller than that room.
#[track_caller]
fn hover_card(
    row_height: f32,
    card: &HoverCard,
    selection: &rustle::SelectionState,
    scrolled: &rustle::ScrollState,
    language: Option<&str>,
    height: f32,
) {
    if card.text.is_none() && card.diagnostics.is_empty() {
        return;
    }
    let line_bottom = card.at.1 + row_height;
    let below = (height - line_bottom - CARD_GAP * 2.0).max(0.0);
    let above = (card.at.1 - CARD_GAP * 2.0).max(0.0);
    let downward = below >= CARD_HEIGHT.min(above) || below >= above;
    let room = sp(if downward { below } else { above }.min(CARD_HEIGHT) / 4.0);
    let body = move || {
        framed(
            Modifier::new().width(CARD_WIDTH),
            Some(HOVER_CARD),
            Modifier::new(),
            Arrangement::Start,
            || {
                rustle::selection_container_with(
                    Modifier::new()
                        .fill_max_width()
                        .height_in(0.0, room)
                        .vertical_scroll(scrolled)
                        .fill_max_width()
                        .padding_xy(theme::sp(3), 0.0)
                        .padding_xy(0.0, theme::sp(2.5)),
                    selection,
                    || {
                        column_arranged(
                            Modifier::new().fill_max_width(),
                            Arrangement::SpacedBy(sp(2)),
                            Alignment::Start,
                            || card_parts(card, language),
                        )
                    },
                );
            },
        );
    };
    if downward {
        at_window(card.at.0, line_bottom + CARD_GAP, false, body);
    } else {
        at_window(card.at.0, card.at.1 - CARD_GAP - room, false, || {
            column_arranged(
                Modifier::new().width(CARD_WIDTH).height(room),
                Arrangement::End,
                Alignment::Start,
                body,
            );
        });
    }
}

/// Emits what the card `card` says: what is wrong at the place, then what the language server says about it, its code coloured as `language` when no fence names another.
#[track_caller]
fn card_parts(card: &HoverCard, language: Option<&str>) {
    for diagnostic in &card.diagnostics {
        let source = diagnostic
            .source
            .as_deref()
            .map(|source| format!("{source}: "))
            .unwrap_or_default();
        let mut lines = diagnostic.message.lines();
        let first = lines.next().unwrap_or_default();
        column_arranged(
            Modifier::new(),
            Arrangement::Start,
            Alignment::Start,
            || {
                rustle::text_styled(
                    Modifier::new(),
                    format!("{source}{first}"),
                    theme::text_style(12, 1.5)
                        .color(theme::color(severity_role(diagnostic.severity)))
                        .max_lines(1),
                );
                for rest in lines.take(4) {
                    rustle::text_styled(
                        Modifier::new(),
                        rest,
                        theme::text_style(12, 1.5)
                            .color(theme::color(Role::Muted))
                            .max_lines(1),
                    );
                }
            },
        );
    }
    if let Some(said) = &card.text {
        if !card.diagnostics.is_empty() {
            rule();
        }
        let mut prose = 0;
        for piece in pieces(said, CARD_LINES) {
            match piece {
                Piece::Code(fence, body) => {
                    column_arranged(
                        Modifier::new().clip(),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            code_lines(&body, &fence, language, CARD_TEXT);
                        },
                    );
                }
                Piece::Prose(text) => {
                    prose += 1;
                    Markdown::new(&text, CARD_TEXT)
                        .size(12)
                        .tone(Role::Muted)
                        .links(HOVER_LINK_BASE + (prose - 1) * LINKS_PER_PROSE)
                        .build();
                }
                Piece::Rule => rule(),
            }
        }
    }
}

/// Emits what floats over the file shown, in a window `height` logical pixels tall: its list of completions and its card about a place, when they are open.
#[track_caller]
pub fn assist(state: &WorkspaceState, height: f32) {
    let Some(document) = state.shown_document() else {
        if let Some(listing) = state.listing_shown() {
            listing_or_quick(listing);
        }
        return;
    };
    if let (Some(card), Some(viewer)) = (&document.hover, state.shown_viewer()) {
        hover_card(
            state.editor.row_height(),
            card,
            &state.hover_selection,
            &viewer.hover_scroll,
            document.language.as_deref(),
            height,
        );
    }
    if let Some(completion) = document.completion.as_ref().filter(|held| !held.loading) {
        completion_list(completion);
        completion_docs(
            completion,
            state.editor.completion_documentation,
            document.language.as_deref(),
        );
    }
    if let Some(listing) = state.listing_shown() {
        listing_or_quick(listing);
    }
    if let Some(renaming) = &state.renaming {
        rename_box(renaming);
    }
    if let Some(picking) = &state.color_picking {
        color_box(picking);
    }
    if let Some(going) = &state.going_to {
        go_to_line_box(going, document.lines());
    }
    if let Some(signature) = document
        .signature
        .as_ref()
        .filter(|_| document.completion.is_none())
    {
        signature_card(signature, state.signature_at);
    }
}

/// How tall the card with the signature of the call the caret is in is, in logical pixels.
pub fn signature_height(signature: &pm_protocol::Signature) -> f32 {
    if signature.documentation.is_some() {
        52.0
    } else {
        30.0
    }
}

/// Emits the signature of the call the caret is in, the parameter the caret is at marked, with the first line of what its documentation says, at `at` in the window.
#[track_caller]
fn signature_card(signature: &pm_protocol::Signature, at: (f32, f32)) {
    let mut marks = Vec::new();
    if let Some((start, end)) = signature.active {
        marks.push(Mark::new(start..end, Role::Accent, Paint::Ink));
        marks.push(Mark::new(start..end, Role::Accent, Paint::Underline));
    }
    let mut words = signature.label.clone();
    if signature.others > 0 {
        words.push_str(&format!("   (+{} more)", signature.others));
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
    floating(
        at,
        Modifier::new().width_in(0.0, CARD_WIDTH + 120.0),
        None,
        Modifier::new()
            .padding_xy(theme::sp(2), 0.0)
            .padding_xy(0.0, theme::sp(1)),
        Arrangement::SpacedBy(sp(0.5)),
        || {
            column_arranged(
                Modifier::new().clip(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    marked_line(
                        Modifier::new(),
                        &words,
                        theme::code_style(12, 1.5).color(theme::color(Role::Text)),
                        marks,
                        CaretLook::default(),
                    );
                },
            );
            if let Some(said) = first_line {
                column_arranged(
                    Modifier::new().clip(),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        line(said, 11, 1.4, Role::Muted);
                    },
                );
            }
        },
    );
}

/// Emits the box the number of the line to go to is typed in, saying how many lines there are.
#[track_caller]
fn go_to_line_box(going: &GoingTo, lines: usize) {
    floating(
        going.at,
        Modifier::new().width(260.0),
        Some(GO_TO_LINE_BOX),
        Modifier::new().padding(theme::sp(1.5)),
        Arrangement::SpacedBy(sp(1)),
        || {
            Input::new(&going.field, GO_TO_LINE)
                .modifier(Modifier::new().fill_max_width())
                .size(Size::Sm)
                .placeholder("Line number")
                .build();
            line(
                format!("Go to a line from 1 to {lines}"),
                11,
                1.4,
                Role::Subtle,
            );
        },
    );
}

/// Emits a line of the list over the file, `content` in `pixels` and `tone`, cut short at the edge of the room it is given; in the font for code when `code`.
#[track_caller]
fn cell(content: impl Into<String>, pixels: u32, tone: Role, code: bool) {
    let kind = theme::text_style(pixels, 1.4).color(theme::color(tone));
    let kind = if code { theme::in_mono(kind) } else { kind };
    let content = content.into();
    clipped_box(move || {
        rustle::basic_text(
            Modifier::new(),
            content,
            kind,
            rustle::TextOverflow::Ellipsis,
        )
    });
}

/// Emits a line of the list over the file that keeps its own width, `content` in `pixels` and `tone`.
#[track_caller]
fn tag(content: impl Into<String>, pixels: u32, tone: Role) {
    let content = content.into();
    column_arranged(
        Modifier::new().width_in(0.0, LISTING_WIDTH * 0.5).clip(),
        Arrangement::Start,
        Alignment::Start,
        move || line(content, pixels, 1.4, tone),
    );
}

/// The file name and line of `location`, as a list names it.
fn place_name(location: &pm_protocol::Location) -> String {
    let name = location.path.rsplit('/').next().unwrap_or(&location.path);
    format!("{name}:{}", location.span.start.line + 1)
}

/// How big a file of `bytes` bytes is, in the largest unit that keeps the number above one.
fn size_label(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

/// Emits the cells of the `index`-th row of `items`.
#[track_caller]
pub(crate) fn listing_cells(items: &Items, index: usize) {
    match items {
        Items::Symbols { locations, .. } => {
            let Some(location) = locations.get(index) else {
                return;
            };
            cell(
                location.name.clone().unwrap_or_default(),
                12,
                Role::Text,
                true,
            );
            tag(place_name(location), 11, Role::Subtle);
        }
        Items::Files { paths, .. } => {
            let Some(path) = paths.get(index) else {
                return;
            };
            let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
            crate::file_icon::file_icon(path, 4.0);
            tag(name.to_owned(), 12, Role::Text);
            cell(folder.to_owned(), 11, Role::Subtle, false);
        }
        Items::Line { .. } => {}
        Items::Choices { choices, .. } => {
            let Some(choice) = choices.get(index) else {
                return;
            };
            cell(choice.label.clone(), 12, Role::Text, false);
            tag(choice.detail.clone(), 11, Role::Subtle);
        }
        Items::Sessions { sessions, .. } => {
            let Some((_, title, beside)) = sessions.get(index) else {
                return;
            };
            cell(title.clone(), 12, Role::Text, false);
            tag(beside.clone(), 11, Role::Subtle);
        }
        Items::Commands {
            commands,
            tasks,
            extras,
            ..
        } => match commands.get(index) {
            Some((action, keys)) => {
                cell(action.title(), 12, Role::Text, false);
                if let Some(keys) = keys.clone() {
                    tag(keys, 11, Role::Subtle);
                }
            }
            None => match tasks.get(index - commands.len()) {
                Some(task) => {
                    cell(crate::listing::task_title(task), 12, Role::Text, false);
                    tag(task.command.clone(), 11, Role::Subtle);
                }
                None => {
                    if let Some((_, command)) = extras.get(index - commands.len() - tasks.len()) {
                        cell(command.title.clone(), 12, Role::Text, false);
                        tag(format!("extension {}", command.extension), 11, Role::Subtle);
                    }
                }
            },
        },
        Items::Outline { items, .. } => {
            let Some(item) = items.get(index) else {
                return;
            };
            rustle::spacer(Modifier::new().width(item.depth.min(8) as f32 * 14.0));
            column_arranged(
                Modifier::new().width(52.0).clip(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    line(item.context.clone(), 11, 1.4, Role::SyntaxKeyword);
                },
            );
            cell(item.name.clone(), 12, Role::Text, true);
        }
        Items::Places { locations, .. } | Items::Search { locations, .. } => {
            let Some(location) = locations.get(index) else {
                return;
            };
            match &location.name {
                Some(name) => {
                    cell(name.clone(), 12, Role::Text, true);
                    tag(place_name(location), 11, Role::Subtle);
                }
                None => {
                    tag(place_name(location), 12, Role::Text);
                    cell(
                        location.line.as_deref().unwrap_or_default().trim(),
                        12,
                        Role::Muted,
                        true,
                    );
                }
            }
        }
        Items::Actions { offered, .. } => {
            let Some(action) = offered.get(index) else {
                return;
            };
            let tone = if action.disabled.is_some() {
                Role::Subtle
            } else {
                Role::Text
            };
            cell(action.title.clone(), 12, tone, false);
            if let Some(reason) = action.disabled.as_ref().or(action.kind.as_ref()) {
                tag(reason.clone(), 11, Role::Subtle);
            }
        }
        Items::History {
            commits, versions, ..
        } => match crate::listing::moments(commits, versions).get(index) {
            Some(crate::listing::Moment::Commit(place)) => {
                let Some(commit) = commits.get(*place) else {
                    return;
                };
                glyph(IconName::GitCommit, 3.5, Role::Muted);
                cell(commit.summary.clone(), 12, Role::Text, false);
                tag(
                    format!("{} · {} · {}", commit.id, commit.author, commit.when),
                    11,
                    Role::Subtle,
                );
            }
            Some(crate::listing::Moment::Saved(place)) => {
                let Some(version) = versions.get(*place) else {
                    return;
                };
                glyph(IconName::History, 3.5, Role::Muted);
                cell("Saved", 12, Role::Text, false);
                tag(
                    format!(
                        "{} · {}",
                        crate::age::label(version.id, crate::age::now()),
                        size_label(version.size)
                    ),
                    11,
                    Role::Subtle,
                );
            }
            None => {}
        },
    }
}

/// Emits the quick input when `listing` is one, and otherwise the list over the file.
#[track_caller]
fn listing_or_quick(listing: &Listing) {
    if listing.quick {
        crate::quick_input::quick_input(listing);
    } else {
        listing_panel(listing);
    }
}

/// Emits the list over the file: its title and how many rows it has, then the rows around the one picked, or what keeps it empty.
#[track_caller]
fn listing_panel(listing: &Listing) {
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
        Items::Files {
            filter,
            compare: Some(_),
            ..
        } => Some((filter, "Compare with a file of this project")),
        Items::Files { filter, .. } => Some((filter, "Open a file of this project")),
        Items::Commands { filter, .. } => Some((filter, "Run a command")),
        Items::Symbols { filter, .. } => Some((filter, "Go to a symbol of this project")),
        Items::Search { filter, .. } => Some((filter, "Search the files of this project")),
        _ => None,
    };
    let first = listing
        .picked
        .saturating_sub(LISTING_SHOWN - 1)
        .min(count.saturating_sub(LISTING_SHOWN));
    floating(
        listing.at,
        Modifier::new().width(LISTING_WIDTH),
        Some(LISTING_BODY),
        Modifier::new().padding(theme::sp(1)),
        Arrangement::Start,
        || {
            if let Some((filter, hint)) = filter {
                column_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .padding_xy(theme::sp(0.5), 0.0)
                        .padding_xy(0.0, theme::sp(0.5)),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        Input::new(filter, LISTING_FILTER)
                            .modifier(Modifier::new().fill_max_width())
                            .size(Size::Sm)
                            .placeholder(hint)
                            .build();
                    },
                );
            }
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(theme::sp(7))
                    .padding_xy(theme::sp(1.5), 0.0),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    cell(listing.items.title(), 12, Role::Text, false);
                    if count > 0 {
                        tag(count.to_string(), 11, Role::Subtle);
                    }
                },
            );
            if let Some(said) = status {
                column_arranged(
                    Modifier::new()
                        .padding_xy(theme::sp(1.5), 0.0)
                        .padding_xy(0.0, theme::sp(1)),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        line(said, 12, 1.4, Role::Muted);
                    },
                );
            }
            let end = count.min(first + LISTING_SHOWN);
            for (place, index) in shown.iter().copied().enumerate().take(end).skip(first) {
                let id = FocusId(LISTING_ROW_BASE + place as u64);
                let looks = theme::InteractionTones::new()
                    .hover(Role::SurfaceHover)
                    .radius(Radius::Sm);
                let looks = if place == listing.picked {
                    looks.idle(Role::SurfaceSelected)
                } else {
                    looks
                };
                row_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .height(theme::sp(6.5))
                        .interaction_focused(crate::focus::is_held(id))
                        .interaction_background((looks).colors())
                        .cursor(CursorIcon::Pointer)
                        .control(id, {
                            let operations = crate::events::operations();
                            move || operations.take_listing(Some(place))
                        })
                        .padding_xy(theme::sp(1.5), 0.0),
                    Arrangement::SpacedBy(sp(3)),
                    Alignment::Center,
                    || listing_cells(&listing.items, index),
                );
            }
        },
    );
}

/// Emits the box the new name of the thing at the caret is typed in, with why the last rename failed under it.
#[track_caller]
fn rename_box(renaming: &Renaming) {
    let hint = if renaming.busy {
        "Renaming…"
    } else {
        "Enter to rename, Escape to cancel"
    };
    floating(
        renaming.at,
        Modifier::new().width(320.0),
        Some(RENAME_BOX),
        Modifier::new().padding(theme::sp(1.5)),
        Arrangement::SpacedBy(sp(1)),
        || {
            Input::new(&renaming.field, RENAME_SYMBOL)
                .modifier(Modifier::new().fill_max_width())
                .size(Size::Sm)
                .invalid(renaming.problem.is_some())
                .build();
            line(
                renaming.problem.clone().unwrap_or_else(|| hint.to_owned()),
                11,
                1.4,
                if renaming.problem.is_some() {
                    Role::Danger
                } else {
                    Role::Subtle
                },
            );
        },
    );
}

/// Emits the box a colour of the file is changed in: the colour as a square beside its hex, and the ways the language server offers to write it, each a row to press.
#[track_caller]
fn color_box(picking: &ColorPicking) {
    let [red, green, blue, alpha] = picking.color;
    let status = if picking.loading {
        Some("Asking the language server…".to_owned())
    } else if let Some(problem) = &picking.problem {
        Some(problem.clone())
    } else if picking.formats.is_empty() {
        Some("The language server offers no way to write this colour".to_owned())
    } else {
        None
    };
    floating(
        picking.at,
        Modifier::new().width(260.0),
        Some(COLOR_BOX),
        Modifier::new().padding(theme::sp(1.5)),
        Arrangement::SpacedBy(sp(1)),
        || {
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    rustle::spacer(
                        Modifier::new()
                            .size(theme::sp(7), theme::sp(7))
                            .background_rounded(
                                theme::color(
                                    Tone::rgb(red, green, blue).alpha(f32::from(alpha) / 255.0),
                                ),
                                Radius::Sm.px(),
                            )
                            .border(1.0, theme::color(Role::Border), Radius::Sm.px())
                            .padding(1.0),
                    );
                    Input::new(&picking.hex, COLOR_HEX)
                        .size(Size::Sm)
                        .invalid(crate::color_picker::color_of(&picking.hex.text()).is_none())
                        .modifier(Modifier::new().weight(1.0))
                        .build();
                },
            );
            if let Some(said) = status {
                column_arranged(
                    Modifier::new()
                        .padding_xy(theme::sp(1.5), 0.0)
                        .padding_xy(0.0, theme::sp(1)),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        line(said, 11, 1.4, Role::Subtle);
                    },
                );
            }
            for (place, format) in picking.formats.iter().take(COLOR_FORMATS_SHOWN).enumerate() {
                let id = FocusId(COLOR_FORMAT_BASE + place as u64);
                row_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .height(theme::sp(6.5))
                        .interaction_focused(crate::focus::is_held(id))
                        .interaction_background(
                            (theme::InteractionTones::new()
                                .hover(Role::SurfaceHover)
                                .radius(Radius::Sm))
                            .colors(),
                        )
                        .cursor(CursorIcon::Pointer)
                        .control(id, reduce(id))
                        .padding_xy(theme::sp(1.5), 0.0),
                    Arrangement::Start,
                    Alignment::Center,
                    || {
                        rustle::basic_text(
                            Modifier::new(),
                            &format.label,
                            theme::mono_style(13, 1.4)
                                .color(theme::color(Role::Text))
                                .max_lines(1),
                            rustle::TextOverflow::Ellipsis,
                        )
                    },
                );
            }
            line(
                picking.formats.first().map_or_else(
                    || "Type a colour as hex".to_owned(),
                    |format| format!("Enter writes {}", format.label),
                ),
                11,
                1.4,
                Role::Subtle,
            );
        },
    );
}

/// How tall the list of completions of `completion` is, in logical pixels.
pub fn list_height(completion: &Completions) -> f32 {
    completion.shown.len().min(COMPLETIONS_SHOWN) as f32 * ROW_HEIGHT + 8.0
}
