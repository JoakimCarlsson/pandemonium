//! What the language server puts over a code editor's text: the list of
//! completions at the caret, the card saying what a place is, the signature
//! of the call the caret is in, the changes the lightbulb offers, the field
//! a name is renamed in and the picker a colour is changed with.

use std::ops::Range;
use std::rc::Rc;

use pm_editor::{Document, ServerAsk};
use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::code_editor::Handle;
use crate::code_text::column_of;
use crate::label::{Label, LabelVariant};
use crate::markdown::Markdown;
use crate::menu::Menu;
use crate::popover::Popover;
use crate::size::Size;
use crate::text_input::TextInput;
use rustle::{ColorScheme, color_scheme};

/// The most completions the list shows at once.
const COMPLETIONS_SHOWN: usize = 10;

/// How wide the list of completions is.
const COMPLETION_WIDTH: f32 = 340.0;

/// How wide the hover card and the signature card are.
const CARD_WIDTH: f32 = 420.0;

/// The colours the picker offers besides the one typed.
const PRESETS: [[u8; 4]; 12] = [
    [0xef, 0x44, 0x44, 0xff],
    [0xf9, 0x73, 0x16, 0xff],
    [0xea, 0xb3, 0x08, 0xff],
    [0x22, 0xc5, 0x5e, 0xff],
    [0x14, 0xb8, 0xa6, 0xff],
    [0x0e, 0xa5, 0xe9, 0xff],
    [0x3b, 0x82, 0xf6, 0xff],
    [0x8b, 0x5c, 0xf6, 0xff],
    [0xec, 0x48, 0x99, 0xff],
    [0xff, 0xff, 0xff, 0xff],
    [0x71, 0x71, 0x7a, 0xff],
    [0x00, 0x00, 0x00, 0xff],
];

/// A popup a code editor can open with over its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeEditorPopup {
    /// The changes the lightbulb offers at the caret.
    Actions,
    /// The colour picker over the colour at this place of the file's list of colours.
    Colour(usize),
}

/// What a code editor needs to put the language server's popups over its text.
#[derive(Clone, Default)]
pub(crate) struct Assist {
    /// What asks the language server, when the file is read by one.
    pub(crate) on_ask: Option<Rc<dyn Fn(ServerAsk)>>,
    /// The titles of the changes offered at the caret, in the server's order.
    pub(crate) actions: Rc<Vec<String>>,
    /// What renaming starts from while its field is open: the stretch holding the name and the name.
    pub(crate) rename: Option<(Range<usize>, String)>,
    /// What closes the popups the window opened: the rename field and the one asked to open with.
    pub(crate) on_dismiss: Option<Rc<dyn Fn()>>,
}

/// Where things are drawn in the text area: the width and height of a
/// character, the gutter's width and how far the text is scrolled down.
#[derive(Clone, Copy)]
pub(crate) struct Grid {
    /// The width and height of a character.
    pub(crate) cell: (f32, f32),
    /// How wide the gutter of numbers is.
    pub(crate) gutter: f32,
    /// How far the text is scrolled down.
    pub(crate) scrolled: f32,
}

impl Grid {
    /// The point of the text area the byte `offset` of `document` is drawn
    /// at: its left edge and the top of its line.
    pub(crate) fn point_of(&self, document: &Document, offset: usize) -> (f32, f32) {
        let offset = offset.min(document.field.text().len());
        let line = document.line_of(offset);
        let range = document.line(line);
        let text = &document.field.text()[range.clone()];
        let column = column_of(text, offset - range.start);
        (
            self.gutter + column as f32 * self.cell.0,
            document.row_of_line(line) as f32 * self.cell.1 - self.scrolled,
        )
    }
}

/// Emits the popups the language server and the window put over the text
/// of `handle`'s document, at the places of the text they are about.
pub(crate) fn assist_layer(handle: &Handle, grid: Grid, popup: Option<CodeEditorPopup>) {
    let document = handle.document.borrow();
    let completion = document.completion.clone().map(|completion| {
        let at = grid.point_of(&document, completion.start);
        (completion, at)
    });
    let hover = document.hover.clone().map(|hover| {
        let at = grid.point_of(&document, hover.offset);
        (hover, at)
    });
    let caret = grid.point_of(&document, document.field.caret());
    let signature = document.signature.clone();
    let rename = handle.assist.rename.clone().map(|(range, name)| {
        let at = grid.point_of(&document, range.start);
        (name, at)
    });
    let colour = match popup {
        Some(CodeEditorPopup::Colour(index)) => document.color_at(index).map(|(range, colour)| {
            let at = grid.point_of(&document, range.start);
            let span = pm_editor::Span::new(
                document.position_of(range.start),
                document.position_of(range.end),
            );
            (span, colour, at)
        }),
        _ => None,
    };
    drop(document);
    let below = grid.cell.1;
    if let Some((completion, at)) = completion {
        completion_list(handle, &completion, (at.0, at.1 + below));
    } else if let Some(signature) = signature {
        signature_card(handle, &signature, (caret.0, caret.1 - below * 3.0));
    }
    if let Some((hover, at)) = hover
        && (hover.text.is_some() || !hover.diagnostics.is_empty())
    {
        hover_card(handle, &hover, (at.0, at.1 + below));
    }
    if popup == Some(CodeEditorPopup::Actions) {
        actions_menu(handle, (caret.0, caret.1 + below));
    }
    if let Some((name, at)) = rename {
        rename_field(handle, name, (at.0, at.1 + below));
    }
    if let Some((span, colour, at)) = colour {
        colour_picker(handle, span, colour, (at.0, at.1 + below));
    }
}

/// Emits the list of completions `completion` hanging at `at`, the one
/// picked marked; pressing one writes it.
fn completion_list(handle: &Handle, completion: &pm_editor::Completions, at: (f32, f32)) {
    let scheme = color_scheme();
    let extended = extended_colors();
    let close = handle.clone();
    let rows: Vec<(usize, String, String, String)> = completion
        .shown
        .iter()
        .enumerate()
        .take(COMPLETIONS_SHOWN.max(completion.picked + 1))
        .skip((completion.picked + 1).saturating_sub(COMPLETIONS_SHOWN))
        .filter_map(|(row, place)| {
            let item = completion.items.get(*place)?;
            let kind = format!("{:?}", item.kind);
            Some((
                row,
                kind.chars().next().unwrap_or(' ').to_string(),
                item.label.clone(),
                item.detail.clone().unwrap_or_default(),
            ))
        })
        .collect();
    let loading = completion.loading;
    let picked = completion.picked;
    let documentation = completion
        .shown
        .get(picked)
        .and_then(|place| completion.items.get(*place))
        .and_then(|item| item.documentation.clone());
    Popover::new()
        .width(COMPLETION_WIDTH)
        .offset(at.0, at.1)
        .size(Size::Xs)
        .on_dismiss(move || {
            close.document.borrow_mut().completion = None;
            close.redraw();
        })
        .show(move || {
            if loading && rows.is_empty() {
                Label::new("Loading…")
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .modifier(Modifier::new().padding_xy(8.0, 4.0))
                    .show();
            }
            for (row, kind, label, detail) in rows {
                let pick = handle.clone();
                let selected = row == picked;
                key(row, move || {
                    row_arranged(
                        Modifier::new()
                            .fill_max_width()
                            .height(22.0)
                            .background_rounded(
                                if selected {
                                    scheme.secondary_container
                                } else {
                                    scheme.surface_container
                                },
                                4.0,
                            )
                            .clickable_plain(move || pick.accept_completion(Some(row)))
                            .padding_xy(6.0, 0.0),
                        Arrangement::SpacedBy(crate::spacing(6.0)),
                        Alignment::Center,
                        move || {
                            Label::new(kind)
                                .variant(LabelVariant::Code)
                                .size(Size::Xs)
                                .colour(extended.syntax_function)
                                .show();
                            Label::new(label)
                                .variant(LabelVariant::Code)
                                .size(Size::Xs)
                                .modifier(Modifier::new().weight(1.0))
                                .show();
                            Label::new(detail.chars().take(32).collect::<String>())
                                .variant(LabelVariant::Subtle)
                                .size(Size::Xs)
                                .show();
                        },
                    );
                });
            }
            if let Some(documentation) = documentation {
                column_with(
                    Modifier::new()
                        .fill_max_width()
                        .border_bottom_hairline(scheme.outline_variant)
                        .padding_xy(6.0, 4.0),
                    move || {
                        Label::new(documentation.lines().take(4).collect::<Vec<_>>().join("\n"))
                            .variant(LabelVariant::Muted)
                            .size(Size::Xs)
                            .wrap()
                            .show();
                    },
                );
            }
        });
}

/// Emits the card saying what the place of `hover` is, hanging at `at`.
fn hover_card(handle: &Handle, hover: &pm_editor::HoverCard, at: (f32, f32)) {
    let scheme = color_scheme();
    let close = handle.clone();
    let text = hover.text.clone();
    let problems: Vec<String> = hover
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    Popover::new()
        .width(CARD_WIDTH)
        .offset(at.0, at.1)
        .size(Size::Md)
        .on_dismiss(move || {
            close.document.borrow_mut().hover = None;
            close.redraw();
        })
        .show(move || {
            for problem in problems {
                Label::new(problem)
                    .size(Size::Xs)
                    .colour(scheme.error)
                    .wrap()
                    .modifier(Modifier::new().padding_xy(4.0, 2.0))
                    .show();
            }
            if let Some(text) = text {
                Markdown::new(pm_markdown::parse(&text))
                    .size(Size::Xs)
                    .show();
            }
        });
}

/// Emits the signature of the call the caret is in, `signature`, at `at`,
/// the parameter the caret is at marked.
fn signature_card(handle: &Handle, signature: &pm_editor::Signature, at: (f32, f32)) {
    let scheme = color_scheme();
    let close = handle.clone();
    let (before, active, after) = match signature.active {
        Some((start, end)) if end <= signature.label.len() && start <= end => (
            signature.label[..start].to_owned(),
            signature.label[start..end].to_owned(),
            signature.label[end..].to_owned(),
        ),
        _ => (signature.label.clone(), String::new(), String::new()),
    };
    let others = signature.others;
    let documentation = signature.documentation.clone();
    Popover::new()
        .width(CARD_WIDTH)
        .offset(at.0, at.1.max(0.0))
        .size(Size::Sm)
        .on_dismiss(move || {
            close.document.borrow_mut().signature = None;
            close.redraw();
        })
        .show(move || {
            row_arranged(
                Modifier::new().fill_max_width().padding_xy(4.0, 2.0),
                Arrangement::Start,
                Alignment::Center,
                move || {
                    Label::new(before)
                        .variant(LabelVariant::Code)
                        .size(Size::Xs)
                        .show();
                    Label::new(active)
                        .variant(LabelVariant::Code)
                        .size(Size::Xs)
                        .colour(scheme.primary)
                        .modifier(Modifier::new().border_bottom_hairline(scheme.primary))
                        .show();
                    Label::new(after)
                        .variant(LabelVariant::Code)
                        .size(Size::Xs)
                        .show();
                    if others > 0 {
                        Label::new(format!("  +{others}"))
                            .variant(LabelVariant::Subtle)
                            .size(Size::Xs)
                            .show();
                    }
                },
            );
            if let Some(documentation) = documentation {
                Label::new(documentation.lines().take(3).collect::<Vec<_>>().join("\n"))
                    .variant(LabelVariant::Muted)
                    .size(Size::Xs)
                    .wrap()
                    .modifier(Modifier::new().padding_xy(4.0, 2.0))
                    .show();
            }
        });
}

/// Emits the menu of the changes offered at the caret at `at`; choosing
/// one takes it.
fn actions_menu(handle: &Handle, at: (f32, f32)) {
    let close = handle.clone();
    let mut menu = Menu::new()
        .width(320.0)
        .offset(at.0, at.1)
        .on_dismiss(move || close.dismiss());
    if handle.assist.actions.is_empty() {
        menu = menu.heading("No changes offered here");
    }
    for (place, title) in handle.assist.actions.iter().enumerate() {
        let take = handle.clone();
        menu = menu.item(title.clone(), move || {
            take.ask(ServerAsk::TakeAction(place));
            take.dismiss();
        });
    }
    menu.show();
}

/// Emits the field the name at the caret is renamed in, holding `name`, at
/// `at`; Enter renames it everywhere.
fn rename_field(handle: &Handle, name: String, at: (f32, f32)) {
    let scheme = color_scheme();
    let close = handle.clone();
    let submit = handle.clone();
    Popover::new()
        .width(260.0)
        .offset(at.0, at.1)
        .size(Size::Sm)
        .on_dismiss(move || close.dismiss())
        .show(move || {
            Label::new("Rename to")
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .modifier(Modifier::new().padding_xy(4.0, 2.0))
                .show();
            TextInput::new(name)
                .code()
                .size(Size::Xs)
                .autofocus()
                .modifier(Modifier::new().fill_max_width())
                .on_submit(move |name| {
                    let caret = {
                        let document = submit.document.borrow();
                        document.position_of(document.field.caret())
                    };
                    if !name.trim().is_empty() {
                        submit.ask(ServerAsk::Rename(caret, name.trim().to_owned()));
                    }
                    submit.dismiss();
                })
                .show();
            Label::new("Enter to rename, Escape to cancel")
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .colour(scheme.on_surface_variant)
                .modifier(Modifier::new().padding_xy(4.0, 2.0))
                .show();
        });
}

/// Emits the picker changing the colour `colour` written over `span`, at
/// `at`: the colour typed in hex and the colours on offer; choosing one has
/// the language server write it in place of the stretch.
fn colour_picker(handle: &Handle, span: pm_editor::Span, colour: [u8; 4], at: (f32, f32)) {
    let scheme = color_scheme();
    let close = handle.clone();
    let typed = handle.clone();
    let chosen = handle.clone();
    Popover::new()
        .width(232.0)
        .offset(at.0, at.1)
        .size(Size::Md)
        .on_dismiss(move || close.dismiss())
        .show(move || {
            row_arranged(
                Modifier::new().fill_max_width().padding(4.0),
                Arrangement::SpacedBy(crate::spacing(8.0)),
                Alignment::Center,
                move || {
                    spacer(
                        Modifier::new()
                            .size(28.0, 28.0)
                            .background_rounded(colour_of(colour), 6.0)
                            .border(1.0, scheme.outline_variant, 6.0),
                    );
                    TextInput::new(hex_of(colour))
                        .code()
                        .size(Size::Xs)
                        .modifier(Modifier::new().weight(1.0))
                        .on_submit(move |text| {
                            if let Some(colour) = parse_hex(&text) {
                                typed.ask(ServerAsk::Colour(span, colour));
                                typed.dismiss();
                            }
                        })
                        .show();
                },
            );
            grid_of_presets(&chosen, span, &scheme);
        });
}

/// Emits the colours on offer in rows of six; pressing one writes it over `span`.
fn grid_of_presets(handle: &Handle, span: pm_editor::Span, scheme: &ColorScheme) {
    let border = scheme.outline_variant;
    for (row, colours) in PRESETS.chunks(6).enumerate() {
        let handle = handle.clone();
        let colours = colours.to_vec();
        key(row, move || {
            row_arranged(
                Modifier::new().fill_max_width().padding_xy(4.0, 2.0),
                Arrangement::SpacedBy(crate::spacing(6.0)),
                Alignment::Center,
                move || {
                    for colour in colours {
                        let pick = handle.clone();
                        spacer(
                            Modifier::new()
                                .size(28.0, 22.0)
                                .background_rounded(colour_of(colour), 4.0)
                                .border(1.0, border, 4.0)
                                .clickable_plain(move || {
                                    pick.ask(ServerAsk::Colour(span, colour));
                                    pick.dismiss();
                                }),
                        );
                    }
                },
            );
        });
    }
}

/// The colour of `rgba`, red, green, blue and opacity from 0 to 255.
pub(crate) fn colour_of(rgba: [u8; 4]) -> Color {
    Color {
        r: rgba[0],
        g: rgba[1],
        b: rgba[2],
        a: rgba[3],
    }
}

/// `rgba` written in hex, with its opacity only when it is not whole.
fn hex_of(rgba: [u8; 4]) -> String {
    let [red, green, blue, alpha] = rgba;
    if alpha == 0xff {
        format!("#{red:02x}{green:02x}{blue:02x}")
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
    }
}

/// The colour `text` spells in hex, with or without its `#`, in three, six or eight digits.
fn parse_hex(text: &str) -> Option<[u8; 4]> {
    let digits = text.trim().trim_start_matches('#');
    let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
    match digits.len() {
        3 => {
            let mut colour = [0xff; 4];
            for (place, digit) in digits.chars().enumerate() {
                let value = digit.to_digit(16)? as u8;
                colour[place] = value * 17;
            }
            Some(colour)
        }
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 0xff]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hex is read in its three lengths and written back.
    #[test]
    fn hex_round_trips() {
        assert_eq!(parse_hex("#fff"), Some([0xff; 4]));
        assert_eq!(parse_hex("102030"), Some([0x10, 0x20, 0x30, 0xff]));
        assert_eq!(parse_hex("#10203040"), Some([0x10, 0x20, 0x30, 0x40]));
        assert_eq!(parse_hex("#12"), None);
        assert_eq!(hex_of([0x10, 0x20, 0x30, 0xff]), "#102030");
    }
}
