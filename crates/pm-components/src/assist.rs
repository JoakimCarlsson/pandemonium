//! What floats over the file shown while code is written in it: the list of completions at the caret and the card saying what is known about a place.

use crate::areas::EditorEvent;
use crate::events::{Event, send};
use crate::kit::{self, Frame};
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use crate::theme::ShadowLayers;
use pm_protocol::CompletionKind;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, Color, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::events::Interactive;
use crate::marked::{CaretLook, Mark, Paint, marked_line};
use crate::popups::at_window;
use crate::theme::{Radius, Role, sp};

use crate::color_picker::{COLOR_FORMATS_SHOWN, ColorPicking};
use crate::editor::COMPLETIONS_SHOWN;
use crate::input::Input;
use crate::line_edits::GoingTo;
use crate::listing::{Items, LISTING_SHOWN, LISTING_WIDTH, Listing};
use crate::markdown::Markdown;
use crate::navigate::Renaming;
use crate::parts::{glyph, line};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::viewer::severity_role;
use pm_editor::{Completions, HoverCard};

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
const SHADOW: [rustle::Shadow; 1] = [crate::theme::box_shadow(0.0, 8.0, 28.0, 0.0, 0.3)];

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
    target: Option<Target>,
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
    target: Option<Target>,
    padding: Modifier,
    arrangement: Arrangement,
    content: impl FnOnce(),
) {
    let own = size
        .shadow_layers(&SHADOW, Radius::Md.px())
        .panel(Radius::Md)
        .clip();
    let own = match target {
        Some(id) => own.control(id, send(Event::pressing(id))),
        None => own,
    };
    column_arranged(own.chain(padding), arrangement, Alignment::Start, content);
}

/// Emits `content` in a box that takes the room left in its row and cuts it off at its edge.
#[track_caller]
fn clipped_box(content: impl FnOnce()) {
    kit::column(Modifier::new().weight(1.0).clip(), 0, content);
}

/// The row of a list over the file `height` theme steps tall that answers to `id`, filled
/// while it is `picked`, lit under the pointer and ringed while held: chain its press and
/// padding onto it.
fn pickable(id: Target, picked: bool, height: f64) -> Modifier {
    let looks = theme::InteractionTones::new()
        .hover(Role::SurfaceHover)
        .radius(Radius::Sm);
    let looks = if picked {
        looks.idle(Role::SurfaceSelected)
    } else {
        looks
    };
    Modifier::new()
        .fill_max_width()
        .height(theme::sp(height))
        .indicated(looks, theme::Focused::Held(crate::focus::is_held(id)))
        .cursor(CursorIcon::Pointer)
}

/// Emits the row of the list of completions at `place`, for `item`, filled while it is `picked`.
fn completion_row(place: usize, item: &pm_protocol::Completion, picked: bool) {
    let id = Target::Editor(EditorEvent::CompletionRow(place));
    let (letter, tone) = kind_mark(item.kind);
    kit::row(
        pickable(id, picked, 6.0)
            .control(id, send(Event::pressing(id)))
            .padding_xy(theme::sp(1.5), 0.0),
        2,
        || {
            row_arranged(
                Modifier::new().size(theme::sp(4), theme::sp(4)),
                Arrangement::Center,
                Alignment::Center,
                || {
                    kit::text(
                        Modifier::new(),
                        letter,
                        theme::code_style(11, 1.2).color(theme::color(tone)),
                    )
                },
            );
            clipped_box(|| {
                kit::text(
                    Modifier::new(),
                    item.label.clone(),
                    theme::code_style(12, 1.4).color(theme::color(Role::Text)),
                )
            });
            if let Some(detail) = &item.detail {
                let short: String = detail.chars().take(48).collect();
                kit::column(
                    Modifier::new().width_in(0.0, LIST_WIDTH * 0.45).clip(),
                    0,
                    || kit::text(Modifier::new(), short, kit::code(11, Role::Subtle)),
                );
            }
        },
    );
}

/// Emits the list of `completion`, the rows around the one picked, or nothing when it has none.
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
        Some(Target::Editor(EditorEvent::CompletionBody)),
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
        Modifier::new().padding_xy(theme::sp(3), theme::sp(2)),
        Arrangement::SpacedBy(sp(1.5)),
        || {
            if let Some(detail) = &item.detail {
                code_lines(detail, "", language, width);
                rule();
            }
            said_parts(said, DOCS_LINES, language, width, false);
        },
    );
}

/// Emits what a language server said, `said`, at most `most` lines of it and `width` logical
/// pixels wide: its blocks of code coloured as their fence or else `language` names, its prose
/// in markdown whose links answer as the card's links do when `linked`, and its rules.
#[track_caller]
fn said_parts(said: &str, most: usize, language: Option<&str>, width: f32, linked: bool) {
    let mut prose = 0;
    for piece in pieces(said, most) {
        match piece {
            Piece::Code(fence, body) => {
                kit::column(Modifier::new().clip(), 0, || {
                    code_lines(&body, &fence, language, width);
                });
            }
            Piece::Prose(text) => {
                let markdown = Markdown::new(&text, width).size(12).tone(Role::Muted);
                let markdown = if linked {
                    markdown.links(Target::Editor(EditorEvent::HoverLink { prose, link: 0 }))
                } else {
                    markdown
                };
                prose += 1;
                markdown.build();
            }
            Piece::Rule => rule(),
        }
    }
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
pub fn hover_link(said: &str, id: Target) -> Option<String> {
    let Target::Editor(EditorEvent::HoverLink {
        prose: stretch,
        link,
    }) = id
    else {
        return None;
    };
    let prose = pieces(said, CARD_LINES)
        .into_iter()
        .filter_map(|piece| match piece {
            Piece::Prose(text) => Some(text),
            _ => None,
        })
        .nth(stretch)?;
    pm_markdown::links(&prose).into_iter().nth(link)
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
            Some(Target::Editor(EditorEvent::HoverCard)),
            Modifier::new(),
            Arrangement::Start,
            || {
                rustle::selection_container_with(
                    Modifier::new()
                        .fill_max_width()
                        .height_in(0.0, room)
                        .vertical_scroll(scrolled)
                        .fill_max_width()
                        .padding_xy(theme::sp(3), theme::sp(2.5)),
                    selection,
                    || {
                        kit::column(Modifier::new().fill_max_width(), 2, || {
                            card_parts(card, language)
                        })
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
fn card_parts(card: &HoverCard, language: Option<&str>) {
    for diagnostic in &card.diagnostics {
        let source = diagnostic
            .source
            .as_deref()
            .map(|source| format!("{source}: "))
            .unwrap_or_default();
        let mut lines = diagnostic.message.lines();
        let first = lines.next().unwrap_or_default();
        kit::column(Modifier::new(), 0, || {
            let tones = std::iter::once(severity_role(diagnostic.severity))
                .chain(std::iter::repeat(Role::Muted));
            for (words, tone) in std::iter::once(format!("{source}{first}"))
                .chain(lines.take(4).map(str::to_owned))
                .zip(tones)
            {
                rustle::text_styled(
                    Modifier::new(),
                    words,
                    theme::text_style(12, 1.5)
                        .color(theme::color(tone))
                        .max_lines(1),
                );
            }
        });
    }
    if let Some(said) = &card.text {
        if !card.diagnostics.is_empty() {
            rule();
        }
        said_parts(said, CARD_LINES, language, CARD_TEXT, true);
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
        Modifier::new().padding_xy(theme::sp(2), theme::sp(1)),
        Arrangement::SpacedBy(sp(0.5)),
        || {
            kit::column(Modifier::new().clip(), 0, || {
                marked_line(
                    Modifier::new(),
                    &words,
                    theme::code_style(12, 1.5).color(theme::color(Role::Text)),
                    marks,
                    CaretLook::default(),
                );
            });
            if let Some(said) = first_line {
                kit::column(Modifier::new().clip(), 0, || {
                    line(said, 11, 1.4, Role::Muted);
                });
            }
        },
    );
}

/// Emits the box the number of the line to go to is typed in, saying how many lines there are.
fn go_to_line_box(going: &GoingTo, lines: usize) {
    small_box(going.at, 260.0, EditorEvent::GoToLineBox, || {
        Input::new(&going.field, Target::Editor(EditorEvent::GoToLine))
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
    });
}

/// Emits a line of the list over the file, `content` in `pixels` and `tone`, cut short at the edge of the room it is given; in the font for code when `code`.
#[track_caller]
fn cell(content: impl Into<String>, pixels: u32, tone: Role, code: bool) {
    let kind = theme::text_style(pixels, 1.4).color(theme::color(tone));
    let kind = if code { theme::in_mono(kind) } else { kind };
    let content = content.into();
    clipped_box(move || kit::text(Modifier::new(), content, kind));
}

/// Emits a line of the list over the file that keeps its own width, `content` in `pixels` and `tone`.
#[track_caller]
fn tag(content: impl Into<String>, pixels: u32, tone: Role) {
    let content = content.into();
    kit::column(
        Modifier::new().width_in(0.0, LISTING_WIDTH * 0.5).clip(),
        0,
        move || line(content, pixels, 1.4, tone),
    );
}

/// Emits the two lines of most rows of the list over the file: `title` in the text tone, in
/// the font for code when `code`, then `detail` beside it, smaller and subtler.
#[track_caller]
fn pair(title: impl Into<String>, code: bool, detail: impl Into<String>) {
    cell(title, 12, Role::Text, code);
    tag(detail, 11, Role::Subtle);
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
            pair(
                location.name.clone().unwrap_or_default(),
                true,
                place_name(location),
            );
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
            pair(choice.label.clone(), false, choice.detail.clone());
        }
        Items::Sessions { sessions, .. } => {
            let Some((_, title, beside)) = sessions.get(index) else {
                return;
            };
            pair(title.clone(), false, beside.clone());
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
                Some(task) => pair(
                    crate::listing::task_title(task),
                    false,
                    task.command.clone(),
                ),
                None => {
                    if let Some((_, command)) = extras.get(index - commands.len() - tasks.len()) {
                        pair(
                            command.title.clone(),
                            false,
                            format!("extension {}", command.extension),
                        );
                    }
                }
            },
        },
        Items::Outline { items, .. } => {
            let Some(item) = items.get(index) else {
                return;
            };
            rustle::spacer(Modifier::new().width(item.depth.min(8) as f32 * 14.0));
            kit::column(Modifier::new().width(52.0).clip(), 0, || {
                line(item.context.clone(), 11, 1.4, Role::SyntaxKeyword);
            });
            cell(item.name.clone(), 12, Role::Text, true);
        }
        Items::Places { locations, .. } | Items::Search { locations, .. } => {
            let Some(location) = locations.get(index) else {
                return;
            };
            match &location.name {
                Some(name) => pair(name.clone(), true, place_name(location)),
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
                glyph(phosphor::GIT_COMMIT, 3.5, Role::Muted);
                pair(
                    commit.summary.clone(),
                    false,
                    format!("{} · {} · {}", commit.id, commit.author, commit.when),
                );
            }
            Some(crate::listing::Moment::Saved(place)) => {
                let Some(version) = versions.get(*place) else {
                    return;
                };
                glyph(phosphor::CLOCK_COUNTER_CLOCKWISE, 3.5, Role::Muted);
                pair(
                    "Saved",
                    false,
                    format!(
                        "{} · {}",
                        crate::age::label(version.id, crate::age::now()),
                        size_label(version.size)
                    ),
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
        Some(Target::Editor(EditorEvent::ListingBody)),
        Modifier::new().padding(theme::sp(1)),
        Arrangement::Start,
        || {
            if let Some((filter, hint)) = filter {
                kit::column(
                    Modifier::new().fill_max_width().padding(theme::sp(0.5)),
                    0,
                    || {
                        Input::new(filter, Target::Editor(EditorEvent::ListingFilter))
                            .modifier(Modifier::new().fill_max_width())
                            .size(Size::Sm)
                            .placeholder(hint)
                            .build();
                    },
                );
            }
            kit::row(
                Modifier::new()
                    .fill_max_width()
                    .height(theme::sp(7))
                    .padding_xy(theme::sp(1.5), 0.0),
                2,
                || {
                    cell(listing.items.title(), 12, Role::Text, false);
                    if count > 0 {
                        tag(count.to_string(), 11, Role::Subtle);
                    }
                },
            );
            if let Some(said) = status {
                kit::note(1.5, 1, said, 12, Role::Muted);
            }
            let end = count.min(first + LISTING_SHOWN);
            for (place, index) in shown.iter().copied().enumerate().take(end).skip(first) {
                let id = Target::Editor(EditorEvent::ListingRow(place));
                kit::row(
                    pickable(id, place == listing.picked, 6.5)
                        .control(id, {
                            let operations = crate::events::operations();
                            move || operations.take_listing(Some(place))
                        })
                        .padding_xy(theme::sp(1.5), 0.0),
                    3,
                    || listing_cells(&listing.items, index),
                );
            }
        },
    );
}

/// Emits the box the new name of the thing at the caret is typed in, with why the last rename failed under it.
fn rename_box(renaming: &Renaming) {
    let hint = if renaming.busy {
        "Renaming…"
    } else {
        "Enter to rename, Escape to cancel"
    };
    small_box(renaming.at, 320.0, EditorEvent::RenameBox, || {
        Input::new(&renaming.field, Target::Editor(EditorEvent::RenameSymbol))
            .modifier(Modifier::new().fill_max_width())
            .size(Size::Sm)
            .invalid(renaming.problem.is_some())
            .build();
        let tone = if renaming.problem.is_some() {
            Role::Danger
        } else {
            Role::Subtle
        };
        line(
            renaming.problem.clone().unwrap_or_else(|| hint.to_owned()),
            11,
            1.4,
            tone,
        );
    });
}

/// Emits a small raised box at `at`, `width` logical pixels wide, answering to `target` and
/// holding `content` one theme step apart: the frame of the boxes typed in over the file.
#[track_caller]
fn small_box(at: (f32, f32), width: f32, target: EditorEvent, content: impl FnOnce()) {
    floating(
        at,
        Modifier::new().width(width),
        Some(Target::Editor(target)),
        Modifier::new().padding(theme::sp(1.5)),
        Arrangement::SpacedBy(sp(1)),
        content,
    );
}

/// Emits the box a colour of the file is changed in: the colour as a square beside its hex, and the ways the language server offers to write it, each a row to press.
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
    small_box(picking.at, 260.0, EditorEvent::ColorBox, || {
        kit::row(Modifier::new().fill_max_width(), 2, || {
            rustle::spacer(
                Modifier::new()
                    .size(theme::sp(7), theme::sp(7))
                    .background_rounded(
                        theme::color(Color::rgba(red, green, blue, alpha)),
                        Radius::Sm.px(),
                    )
                    .outlined(Radius::Sm),
            );
            Input::new(&picking.hex, Target::Editor(EditorEvent::ColorHex))
                .size(Size::Sm)
                .invalid(crate::color_picker::color_of(&picking.hex.text()).is_none())
                .modifier(Modifier::new().weight(1.0))
                .build();
        });
        if let Some(said) = status {
            kit::note(1.5, 1, said, 11, Role::Subtle);
        }
        for (place, format) in picking.formats.iter().take(COLOR_FORMATS_SHOWN).enumerate() {
            let id = Target::Editor(EditorEvent::ColorFormat(place));
            kit::row(
                pickable(id, false, 6.5)
                    .control(id, send(Event::pressing(id)))
                    .padding_xy(theme::sp(1.5), 0.0),
                0,
                || kit::text(Modifier::new(), &format.label, kit::code(13, Role::Text)),
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
    });
}

/// How tall the list of completions of `completion` is, in logical pixels.
pub fn list_height(completion: &Completions) -> f32 {
    completion.shown.len().min(COMPLETIONS_SHOWN) as f32 * ROW_HEIGHT + 8.0
}
