//! Draws every facility of the kit into PNG files under `build/kit/`, with
//! the exemplar components ported onto it, for comparing with the snapshots
//! of the old interface.
//!
//! Run it with `CARGO_TARGET_DIR=build cargo run -p pm-components --example kit_gallery`.

use std::sync::Arc;

use pm_components::kit::snapshot::{Shot, lay_out, write_png};
use pm_components::kit::{
    Appearance, CaretLook, CaretShape, Flex, FocusId, IconName, Mark, Paint, Picture, Placement,
    Radius, Role, Scroll, Side, Styled, TextPoint, TextSelection, Tone, Tooltip, Type, anchored,
    block, column, gap_box, icon, label, label_with, layer, marked_field_line, marked_line,
    paragraph, picture, rings, row, scroll_area, selectable, spinner, wrapping,
};
use pm_field::{Edit, Field, Motion};
use rustle::{BoxAlignment, CursorIcon, Modifier};

mod ported;

use ported::{Button, Checkbox, Input, InputVariant, Menu, Popover, Size, Surface, Tile};

/// The lines of the selectable text in the misc shot.
const SELECTABLE: [&str; 2] = [
    "The agent restored the session from its checkpoint.",
    "Its worktree and conversation are as they were.",
];

/// The selection of the misc shot: from the seventh byte of the first line to the eleventh of the second.
const SELECTED: TextSelection = TextSelection {
    id: FocusId(700),
    anchor: TextPoint { piece: 0, byte: 10 },
    head: TextPoint { piece: 1, byte: 12 },
};

/// Shots whose pointer rests on a control found by laying the shot out first, by the control's name.
const HOVERED: [(&str, FocusId); 3] = [
    ("floating", FocusId(210)),
    ("buttons", FocusId(30)),
    ("controls", FocusId(111)),
];

/// Where the pictures go: `KIT_GALLERY_OUT` when set, else `build/kit` of the repository.
fn out() -> std::path::PathBuf {
    std::env::var_os("KIT_GALLERY_OUT").map_or_else(
        || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/kit"),
        std::path::PathBuf::from,
    )
}

/// Draws each shot of the gallery.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = out();
    let shots: Vec<(&str, Shot, fn())> = vec![
        ("layout", Shot::new(720.0, 580.0), layout_shot),
        ("text", Shot::new(720.0, 420.0), text_shot),
        ("icons", Shot::new(720.0, 220.0), icons_shot),
        (
            "buttons",
            Shot {
                pointer: Some((60.0, 132.0)),
                focused: Some(FocusId(13)),
                ..Shot::new(720.0, 420.0)
            },
            buttons_shot,
        ),
        (
            "controls",
            Shot {
                pointer: Some((120.0, 154.0)),
                ..Shot::new(720.0, 300.0)
            },
            controls_shot,
        ),
        (
            "floating",
            Shot {
                pointer: Some((96.0, 360.0)),
                ..Shot::new(720.0, 420.0)
            },
            floating_shot,
        ),
        (
            "inputs",
            Shot {
                focused: Some(FocusId(300)),
                clock_ms: 100.0,
                ..Shot::new(720.0, 420.0)
            },
            inputs_shot,
        ),
        (
            "inputs-multiline",
            Shot {
                focused: Some(FocusId(301)),
                clock_ms: 100.0,
                ..Shot::new(720.0, 300.0)
            },
            multiline_shot,
        ),
        (
            "scroll",
            Shot {
                pointer: Some((352.0, 120.0)),
                ..Shot::new(720.0, 320.0)
            },
            scroll_shot,
        ),
        (
            "code",
            Shot {
                focused: Some(FocusId(500)),
                clock_ms: 100.0,
                ..Shot::new(720.0, 420.0)
            },
            code_shot,
        ),
        (
            "misc",
            Shot {
                selection: Some(SELECTED),
                ..Shot::new(720.0, 300.0)
            },
            misc_shot,
        ),
        (
            "settings",
            Shot {
                pointer: Some((120.0, 196.0)),
                focused: None,
                ..Shot::new(960.0, 720.0)
            },
            settings_shot,
        ),
        (
            "settings-light",
            Shot {
                appearance: Appearance::Light,
                ..Shot::new(960.0, 720.0)
            },
            settings_shot,
        ),
    ];
    for (name, mut shot, content) in shots {
        let path = out.join(format!("{name}.png"));
        let mut report = write_png(&path, &shot, content)?;
        if let Some(rect) = HOVERED
            .iter()
            .find(|(shot, _)| *shot == name)
            .and_then(|(_, id)| report.region(*id))
        {
            shot.pointer = Some((
                rect.origin.x + rect.size.width / 2.0,
                rect.origin.y + rect.size.height / 2.0,
            ));
            report = write_png(&path, &shot, content)?;
        }
        println!(
            "{name}: {} regions, {} scrollers, {} fields, {} field lines, tab order {:?}",
            report.regions.len(),
            report.scrollers.len(),
            report.fields.len(),
            report.field_lines.len(),
            report.focus_order().iter().take(6).collect::<Vec<_>>()
        );
    }
    let misc = lay_out(720.0, 300.0, None, misc_shot);
    println!(
        "misc: copying the selection gives {:?}",
        misc.selected_text(&SELECTED)
    );
    let code = lay_out(720.0, 420.0, Some(FocusId(500)), code_shot);
    let below = code.vertical(FocusId(500), 12, 1);
    let edge = code.line_edge(FocusId(500), 12, true);
    println!("code field: byte 12 one line down is {below:?}, its line ends at {edge:?}");
    Ok(())
}

/// A page on the background with a caption, holding `content` in a column.
fn page(content: impl FnOnce()) {
    column(
        Modifier::new()
            .fill_max_size()
            .fill(Role::Background)
            .pad(4),
        Flex::new().gap(3),
        content,
    );
}

/// A small caption naming what follows.
fn caption(words: &str) {
    label(words, Type::new(11).ratio(1.4).tone(Role::Subtle).medium());
}

/// Rows and columns as the old flex boxes laid them out.
fn layout_shot() {
    page(|| {
        caption("row: items_center, gap(2), a flex_1 label cut short, flex_none badge");
        row(
            Modifier::new()
                .fill_max_width()
                .h(9)
                .fill_rounded(Role::Surface, Radius::Md)
                .pad_x(2),
            Flex::new().center().gap(2),
            || {
                icon(IconName::Folder, 4.0, Role::Muted);
                label_with(
                    Modifier::new().grow(),
                    "A long name that does not fit in the room the row leaves it, so it ends in an ellipsis at its edge, however wide the window happens to be",
                    Type::new(13).ratio(1.4).truncate(),
                );
                label("12m ago", Type::new(12).ratio(1.4).tone(Role::Subtle));
            },
        );
        caption(
            "row: justify_between with gap; default stretch makes the boxes as tall as the tallest",
        );
        row(
            Modifier::new()
                .fill_max_width()
                .stroke(Role::Border, Radius::Md)
                .pad(2),
            Flex::new().between().gap(2),
            || {
                for (index, height) in [6.0, 10.0, 8.0].into_iter().enumerate() {
                    column(
                        Modifier::new()
                            .w(16)
                            .fill_rounded(Role::SurfaceHover, Radius::Sm)
                            .pad(1),
                        Flex::new(),
                        || {
                            label(format!("box {index}"), Type::new(12).ratio(1.4));
                            rustle::spacer(Modifier::new().h(height));
                        },
                    );
                }
            },
        );
        caption("column: a block stretches children across; items_start, items_center, items_end");
        row(Modifier::new().fill_max_width(), Flex::new().gap(3), || {
            for (name, flex) in [
                ("stretch", Flex::new().gap(1)),
                ("start", Flex::new().gap(1).start()),
                ("center", Flex::new().gap(1).center()),
                ("end", Flex::new().gap(1).end()),
            ] {
                column(
                    Modifier::new()
                        .grow()
                        .stroke(Role::Border, Radius::Md)
                        .pad(2),
                    flex,
                    || {
                        label(name, Type::new(12).ratio(1.4).tone(Role::Muted));
                        block(
                            Modifier::new()
                                .fill_rounded(Role::SurfaceSelected, Radius::Sm)
                                .pad_x(2),
                            || label("child", Type::new(12).ratio(1.4)),
                        );
                    },
                );
            }
        });
        caption("flex_wrap: chips wrap onto further lines");
        wrapping(Modifier::new().fill_max_width(), 1.5, || {
            for word in [
                "rustle",
                "compose",
                "flex",
                "gap",
                "stretch",
                "truncate",
                "scroll",
                "popover",
                "marks",
                "caret",
                "selection",
                "ime",
                "tooltip",
                "spinner",
            ] {
                block(
                    Modifier::new()
                        .fill_rounded(Role::SurfaceHover, Radius::Full)
                        .pad_x(2.5)
                        .pad_y(0.5),
                    || label(word, Type::new(12).ratio(1.4).tone(Role::Muted)),
                );
            }
        });
        caption("layer: absolute children placed at the corners of a relative box");
        layer(
            Modifier::new()
                .fill_max_width()
                .height(70.0)
                .stroke(Role::Border, Radius::Lg),
            || {
                for (alignment, words) in [
                    (BoxAlignment::TopStart, "top left"),
                    (BoxAlignment::TopEnd, "top right"),
                    (BoxAlignment::Center, "centre"),
                    (BoxAlignment::BottomEnd, "bottom right"),
                ] {
                    block(Modifier::new().placed(alignment).pad(1.5), || {
                        label(words, Type::new(12).ratio(1.4).tone(Role::Muted))
                    });
                }
            },
        );
        caption("cover: an absolute inset-0 tint sized by its sibling, not by the page");
        layer(Modifier::new(), || {
            block(Modifier::new().pad_x(3).pad_y(1.5), || {
                label(
                    "content that sizes the layer",
                    Type::new(12).ratio(1.4).tone(Role::Text),
                )
            });
            gap_box(
                Modifier::new()
                    .cover()
                    .fill_rounded(Tone::from(Role::Accent).alpha(0.25), Radius::Md),
            );
        });
    });
}

/// Lines, weights, families and a wrapped paragraph.
fn text_shot() {
    page(|| {
        for (size, ratio) in [
            (11, 1.6),
            (12, 1.4),
            (13, 1.4),
            (14, 1.57),
            (16, 1.5),
            (22, 1.3),
        ] {
            row(Modifier::new(), Flex::new().gap(4).center(), || {
                label(format!("{size}px"), Type::code(12).tone(Role::Subtle));
                label("The quick brown fox jumps", Type::new(size).ratio(ratio));
                label(
                    "medium",
                    Type::new(size).ratio(ratio).medium().tone(Role::Muted),
                );
                label(
                    "let mono = 1;",
                    Type::code(size).ratio(ratio).tone(Role::SyntaxKeyword),
                );
            });
        }
        row(Modifier::new(), Flex::new().gap(4).center(), || {
            label("underline", Type::new(13).underline());
            label("struck", Type::new(13).strike().tone(Role::Muted));
            label("italic", Type::new(13).italic());
            label("semibold", Type::new(13).semibold());
            label("danger", Type::new(13).tone(Role::Danger));
            label("success", Type::new(13).tone(Role::Success));
            label("warning", Type::new(13).tone(Role::Warning));
        });
        paragraph(
            Modifier::new().width(420.0),
            "A paragraph wraps at the width its box gives it, between words where it can. This replaces the shaper wrapping each line and one line node per wrapped line, which is what the old paragraph helper built.",
            Type::new(13).ratio(1.6).tone(Role::Muted),
        );
    });
}

/// Every named icon, and the same icons in the tones views use.
fn icons_shot() {
    page(|| {
        wrapping(Modifier::new().fill_max_width(), 3, || {
            for name in ALL_ICONS {
                icon(name, 4.0, Role::Text);
            }
        });
        row(Modifier::new(), Flex::new().gap(3).center(), || {
            for tone in [
                Role::Muted,
                Role::Subtle,
                Role::Accent,
                Role::Danger,
                Role::Success,
            ] {
                icon(IconName::GitFork, 5.0, tone);
                icon(IconName::Bug, 5.0, tone);
            }
        });
    });
}

/// The ported button in every variant and size, hovered and focused.
fn buttons_shot() {
    page(|| {
        caption("variants; the second button is under the pointer, the outline one holds focus");
        row(Modifier::new(), Flex::new().gap(2).center(), || {
            let variants = [
                ("Default", ported::Variant::Default),
                ("Secondary", ported::Variant::Secondary),
                ("Destructive", ported::Variant::Destructive),
                ("Outline", ported::Variant::Outline),
                ("Ghost", ported::Variant::Ghost),
                ("Link", ported::Variant::Link),
                ("Field", ported::Variant::Field),
            ];
            for (index, (name, variant)) in variants.into_iter().enumerate() {
                Button::new(name, FocusId(10 + index as u64))
                    .variant(variant)
                    .build();
            }
        });
        caption("hovered (pointer on the left button)");
        row(Modifier::new(), Flex::new().gap(2).center(), || {
            Button::new("Hover me", FocusId(30))
                .variant(ported::Variant::Secondary)
                .build();
            Button::new("Ghost", FocusId(31))
                .variant(ported::Variant::Ghost)
                .build();
        });
        caption("sizes Xs, Sm, Md, Lg");
        row(Modifier::new(), Flex::new().gap(2).center(), || {
            for (index, size) in [Size::Xs, Size::Sm, Size::Md, Size::Lg]
                .into_iter()
                .enumerate()
            {
                Button::new("Open Release Page", FocusId(40 + index as u64))
                    .variant(ported::Variant::Outline)
                    .size(size)
                    .build();
            }
        });
        caption("icons, trailing icon, round, selected, disabled, spinning, monospace");
        row(Modifier::new(), Flex::new().gap(2).center(), || {
            Button::icon(IconName::Plus, FocusId(50)).build();
            Button::icon(IconName::Settings, FocusId(51))
                .tone(Role::Text)
                .build();
            Button::new("New session", FocusId(52))
                .with_icon(IconName::Plus)
                .variant(ported::Variant::Ghost)
                .build();
            Button::new("Auto", FocusId(53))
                .trailing_icon(IconName::ChevronDown)
                .variant(ported::Variant::Ghost)
                .size(Size::Sm)
                .build();
            Button::icon(IconName::ArrowUp, FocusId(54))
                .variant(ported::Variant::Default)
                .round()
                .build();
            Button::new("Picked", FocusId(55))
                .variant(ported::Variant::Ghost)
                .selected(true)
                .build();
            Button::new("Disabled", FocusId(56)).disabled(true).build();
            Button::new("Working", FocusId(57))
                .with_icon(IconName::Loading)
                .spinning(true)
                .variant(ported::Variant::Secondary)
                .build();
            Button::new("task/227", FocusId(58))
                .monospace()
                .variant(ported::Variant::Ghost)
                .size(Size::Sm)
                .build();
        });
        caption("fixed width, and fill: the label is cut short at the edge");
        row(
            Modifier::new().width(360.0),
            Flex::new().gap(2).center(),
            || {
                Button::new("Fixed", FocusId(61))
                    .variant(ported::Variant::Outline)
                    .width(72.0)
                    .build();
                Button::new(
                    "A label much too long for the room it is given",
                    FocusId(60),
                )
                .variant(ported::Variant::Field)
                .trailing_icon(IconName::ChevronDown)
                .fill()
                .build();
            },
        );
    });
}

/// Check boxes, switches and tiles; the pointer is over the second tile.
fn controls_shot() {
    page(|| {
        caption("check boxes: ticked, mixed, clear; switches on and off");
        row(Modifier::new(), Flex::new().gap(4).center(), || {
            Checkbox::new(true, FocusId(100)).build();
            Checkbox::new(false, FocusId(101)).mixed(true).build();
            Checkbox::new(false, FocusId(102)).build();
            Checkbox::new(true, FocusId(103))
                .variant(ported::CheckVariant::Switch)
                .build();
            Checkbox::new(false, FocusId(104))
                .variant(ported::CheckVariant::Switch)
                .build();
            ported::switch(true, Some(FocusId(105)));
            ported::switch(false, Some(FocusId(106)));
        });
        caption("tiles: selected, hovered, plain");
        column(Modifier::new().width(240.0), Flex::new().gap(1), || {
            for (index, (name, glyph)) in [
                ("General", IconName::Settings),
                ("Appearance", IconName::Pencil),
                ("Editor", IconName::FileCode),
            ]
            .into_iter()
            .enumerate()
            {
                Tile::new(FocusId(110 + index as u64))
                    .selected(index == 0)
                    .gap(3.0)
                    .inner(Modifier::new().pad_x(2))
                    .modifier(Modifier::new().fill_max_width())
                    .build_with(|| {
                        let tone = if index == 0 { Role::Text } else { Role::Muted };
                        icon(glyph, 4.0, tone);
                        label_with(
                            Modifier::new().grow(),
                            name,
                            Type::new(13).ratio(1.4).tone(tone).truncate(),
                        );
                    });
            }
        });
        caption("choices: a picked one in a bordered strip");
        ported::choices(&["Last workspace", "Fresh start"], 0, |option| {
            Some(FocusId(120 + option as u64))
        });
    });
}

/// A popover over a trigger near the right edge, kept in the window, a menu at a point, and a tooltip.
fn floating_shot() {
    page(|| {
        caption(
            "a popover above a trigger at the right edge is moved left to stay inside the window",
        );
        row(
            Modifier::new().fill_max_width().height(180.0),
            Flex::new().trailing().end(),
            || {
                anchored(
                    Modifier::new(),
                    Placement::Above,
                    2.0,
                    || {
                        Button::new("Model", FocusId(200))
                            .variant(ported::Variant::Outline)
                            .trailing_icon(IconName::ChevronDown)
                            .build();
                    },
                    Some(|| {
                        Popover::new().width(280.0).build_with(FocusId(201), || {
                            ported::popover_heading("Model", Some(("Tab", "next")));
                            ported::popover_row(
                                "Fast",
                                "Quick answers for small tasks",
                                false,
                                FocusId(202),
                            );
                            ported::popover_row(
                                "Deep",
                                "Thinks longer before it answers",
                                true,
                                FocusId(203),
                            );
                        });
                    }),
                );
            },
        );
        caption("a tooltip shows over the button under the pointer");
        row(Modifier::new(), Flex::new().gap(2).center(), || {
            Button::icon(IconName::Refresh, FocusId(210))
                .modifier(Modifier::new().tooltip("Refresh the list"))
                .build();
            Button::icon(IconName::Trash, FocusId(211)).build();
        });
        Menu::new(420.0, 250.0)
            .item("Cut", Some("Ctrl+X"), FocusId(220), false)
            .item("Copy", Some("Ctrl+C"), FocusId(221), false)
            .item("Paste", Some("Ctrl+V"), FocusId(222), true)
            .separator()
            .item("Select All", Some("Ctrl+A"), FocusId(223), false)
            .build();
        let _surface = Surface::new();
    });
}

/// A field with a caret, one with a selection and one composing, from real `Field` models.
fn inputs_shot() {
    let typed = {
        let mut field = Field::new("pandemonium ");
        field.apply(Edit::Move(Motion::End, false));
        field.apply(Edit::Compose("にほん".to_owned()));
        field
    };
    let selected = {
        let mut field = Field::new("Restore sessions from checkpoints");
        field.apply(Edit::Select(8, 16));
        field
    };
    let composing = {
        let mut field = Field::new("日本");
        field.apply(Edit::Move(Motion::End, false));
        field.apply(Edit::Compose("語を".to_owned()));
        field
    };
    let empty = Field::new("");
    page(move || {
        caption(
            "default (focused: composing with an input method, caret at the end); a disabled ghost input beside",
        );
        row(
            Modifier::new().fill_max_width(),
            Flex::new().gap(3).center(),
            || {
                Input::new(&typed, FocusId(300))
                    .modifier(Modifier::new().grow())
                    .build();
                Input::new(&empty, FocusId(307))
                    .variant(InputVariant::Ghost)
                    .placeholder("Disabled")
                    .disabled(true)
                    .modifier(Modifier::new().width(160.0))
                    .build();
            },
        );
        caption("bordered with a search icon and placeholder");
        Input::new(&empty, FocusId(302))
            .variant(InputVariant::Bordered)
            .leading_icon(IconName::Search)
            .placeholder("Search settings")
            .size(Size::Sm)
            .build();
        caption("a selection (shown while focused: this one is not, so it shows none)");
        Input::new(&selected, FocusId(303))
            .variant(InputVariant::Bordered)
            .build();
        caption("invalid");
        Input::new(&selected, FocusId(304))
            .variant(InputVariant::Bordered)
            .invalid(true)
            .build();
        caption("composing with an input method (not focused)");
        Input::new(&composing, FocusId(305))
            .variant(InputVariant::Bordered)
            .build();
    });
}

/// A multi-line wrapped field holding focus, with a selection over two lines and a composition.
fn multiline_shot() {
    let field = {
        let mut field = Field::new("Reply to the agent with a long message that wraps over several lines of the box, so the caret, the selection and the composition can be seen on wrapped lines. ").multiline(true);
        field.apply(Edit::Select(20, 80));
        field
    };
    let composing = {
        let mut field = Field::new("Composing: ").multiline(true);
        field.apply(Edit::Move(Motion::End, false));
        field.apply(Edit::Compose("にほんご".to_owned()));
        field
    };
    page(move || {
        caption("multi-line, wrapped at 420 pixels, focused with a selection");
        Input::new(&field, FocusId(301))
            .variant(InputVariant::Multiline)
            .wrap(480.0)
            .rows(3)
            .build();
        caption("multi-line composing (not focused)");
        Input::new(&composing, FocusId(306))
            .variant(InputVariant::Multiline)
            .wrap(480.0)
            .build();
    });
}

/// A list scrolled by an offset kept in the model, with its bar under the pointer.
fn scroll_shot() {
    page(|| {
        caption("scroll_area at offset 140 of 40 rows; the pointer is on the bar, so it is wide");
        row(Modifier::new(), Flex::new().gap(4), || {
            scroll_area(
                Modifier::new()
                    .width(340.0)
                    .height(220.0)
                    .stroke(Role::Border, Radius::Md),
                Scroll::from_start(FocusId(400), 140.0),
                || {
                    column(Modifier::new().fill_max_width().pad(1), Flex::new(), || {
                        for index in 0..40 {
                            Tile::new(FocusId(410 + index))
                                .height(8.0)
                                .inner(Modifier::new().pad_x(2))
                                .build_with(|| {
                                    icon(IconName::File, 4.0, Role::Muted);
                                    label_with(
                                        Modifier::new().grow(),
                                        format!("file_{index}.rs"),
                                        Type::new(13).ratio(1.4).truncate(),
                                    );
                                    label("M", Type::new(12).tone(Role::Warning).medium());
                                });
                        }
                    });
                },
            );
            scroll_area(
                Modifier::new()
                    .width(300.0)
                    .height(220.0)
                    .stroke(Role::Border, Radius::Md),
                Scroll::from_end(FocusId(401), 0.0),
                || {
                    column(
                        Modifier::new().fill_max_width().pad(3),
                        Flex::new().gap(2),
                        || {
                            for index in 0..12 {
                                paragraph(
                                    Modifier::new(),
                                    format!(
                                        "Message {index}: anchored to the end, as the conversation scrolls from its bottom."
                                    ),
                                    Type::new(13).ratio(1.6).tone(Role::Muted),
                                );
                            }
                        },
                    );
                },
            );
        });
    });
}

/// Code lines with marks of every kind, and a grid of cells like the terminal.
fn code_shot() {
    let lines: [(&str, Vec<Mark>); 6] = [
        (
            "fn restore(session: &Session) -> Result<(), Error> {",
            vec![
                Mark::new(0..2, Role::SyntaxKeyword, Paint::Ink),
                Mark::new(3..10, Role::SyntaxFunction, Paint::Ink),
                Mark::new(21..28, Role::SyntaxType, Paint::Ink),
                Mark::new(33..39, Role::SyntaxType, Paint::Ink),
                Mark::new(44..49, Role::SyntaxType, Paint::Ink),
            ],
        ),
        (
            "    let before = session.checkpoint().id;",
            vec![
                Mark::new(4..7, Role::SyntaxKeyword, Paint::Ink),
                Mark::new(0..0, Role::Border, Paint::Guide),
                Mark::new(8..14, Tone::from(Role::Accent).alpha(0.25), Paint::Fill),
                Mark::new(25..35, Role::SyntaxFunction, Paint::Ink),
            ],
        ),
        (
            "    session.restore(target)?;",
            vec![
                Mark::new(0..0, Role::Border, Paint::Guide),
                Mark::new(12..19, Role::Danger, Paint::Wave),
                Mark::new(20..26, Role::Accent, Paint::Underline),
            ],
        ),
        (
            "    // keep the checkpoint taken on the way",
            vec![
                Mark::new(0..0, Role::Border, Paint::Guide),
                Mark::new(4..42, Role::SyntaxComment, Paint::Ink),
                Mark::new(12..22, Role::Text, Paint::Selected),
            ],
        ),
        (
            "    Ok(())",
            vec![
                Mark::new(0..0, Role::Border, Paint::Guide),
                Mark::new(4..6, Role::SyntaxType, Paint::Ink),
                Mark::new(6..6, Role::Text, Paint::Caret),
                Mark::new(9..9, Role::Accent, Paint::Drop),
            ],
        ),
        (
            "}",
            vec![Mark::new(0..1, Role::SyntaxPunctuation, Paint::Ink)],
        ),
    ];
    page(move || {
        caption("marked lines: ink, fill, wave, underline, selected, caret, guide, drop");
        column(
            Modifier::new()
                .fill_max_width()
                .fill_rounded(Role::Surface, Radius::Md)
                .pad(2),
            Flex::new(),
            || {
                let mut start = 0;
                for (number, (content, marks)) in lines.iter().enumerate() {
                    let begins = start;
                    start += content.len() + 1;
                    row(Modifier::new(), Flex::new().center().gap(4), || {
                        label_with(
                            Modifier::new().width(28.0),
                            format!("{:>3}", number + 1),
                            Type::code(13).ratio(1.5).tone(Role::Subtle),
                        );
                        marked_field_line(
                            Modifier::new(),
                            FocusId(500),
                            begins,
                            content,
                            Type::code(13).ratio(1.5),
                            marks.clone(),
                            CaretLook::default(),
                        );
                    });
                }
            },
        );
        caption(
            "terminal grid: one marked line per row, cells coloured with ink and fill, a block caret",
        );
        column(
            Modifier::new()
                .fill_max_width()
                .fill_rounded(Role::Background, Radius::Md)
                .stroke(Role::Border, Radius::Md)
                .pad(2),
            Flex::new(),
            || {
                let rows = [
                    (
                        "$ cargo build -p pandemonium",
                        vec![Mark::new(0..1, Role::Success, Paint::Ink)],
                    ),
                    (
                        "   Compiling pm-components v0.15.0",
                        vec![Mark::new(3..12, Role::Success, Paint::Ink)],
                    ),
                    (
                        "warning: unused variable `x`",
                        vec![Mark::new(0..7, Role::Warning, Paint::Ink)],
                    ),
                    (
                        "error[E0063]: missing field",
                        vec![
                            Mark::new(0..12, Role::OnDanger, Paint::Ink),
                            Mark::new(0..12, Role::Danger, Paint::Fill),
                        ],
                    ),
                    (
                        "$ ",
                        vec![
                            Mark::new(0..1, Role::Success, Paint::Ink),
                            Mark::new(2..2, Role::Text, Paint::Caret),
                        ],
                    ),
                ];
                for (content, marks) in rows {
                    marked_line(
                        Modifier::new(),
                        content,
                        Type::code(13).ratio(1.4),
                        marks,
                        CaretLook {
                            shape: CaretShape::Block,
                            steady: true,
                        },
                    );
                }
            },
        );
    });
}

/// A picture, a spinner and progress rings.
fn misc_shot() {
    let pixels: Vec<u8> = (0..96u32 * 64)
        .flat_map(|index| {
            let (x, y) = (index % 96, index / 96);
            [(x * 255 / 95) as u8, (y * 255 / 63) as u8, 160, 255]
        })
        .collect();
    let still = Picture {
        key: 7,
        width: 96,
        height: 64,
        pixels: Arc::new(pixels),
    };
    page(move || {
        caption("picture (cropped into a rounded box), spinner, rings, a ring with two fills");
        row(Modifier::new(), Flex::new().gap(6).center(), || {
            block(
                Modifier::new().size(160.0, 90.0).clip_to(Radius::Lg),
                || picture(Modifier::new().fill_max_size(), &still),
            );
            spinner(6.0, Role::Muted);
            spinner(4.0, Role::Accent);
            pm_components::kit::arc(
                Modifier::new().size(40.0, 40.0),
                0.65,
                4.0,
                Role::Accent,
                Role::Border,
            );
            rings(
                Modifier::new().size(56.0, 56.0),
                5.0,
                2.0,
                Role::Border,
                &[
                    (0.8, Tone::from(Role::Success)),
                    (0.4, Tone::from(Role::Warning)),
                ],
            );
        });
        caption("selectable text: one selection running from the first box into the second");
        column(Modifier::new(), Flex::new().gap(1), || {
            for line in SELECTABLE {
                selectable(
                    Modifier::new(),
                    FocusId(700),
                    line,
                    Type::new(13).ratio(1.4),
                );
            }
        });
    });
}

/// The settings window's general page, as `settings.png` of the old snapshots shows it.
fn settings_shot() {
    let find = Field::new("");
    column(
        Modifier::new().fill_max_size().fill(Role::Surface),
        Flex::new(),
        move || {
            row(
                Modifier::new().fill_max_width().height(40.0).stroke_side(
                    Side::Bottom,
                    1.0,
                    Role::Border,
                ),
                Flex::new().center().trailing(),
                || {
                    block(Modifier::new().pad_x(4).cursor(CursorIcon::Pointer), || {
                        icon(IconName::Close, 4.0, Role::Muted)
                    });
                },
            );
            row(
                Modifier::new().fill_max_width().weight(1.0),
                Flex::new(),
                || {
                    column(
                        Modifier::new()
                            .width(220.0)
                            .fill_max_height()
                            .stroke_side(Side::Right, 1.0, Role::Border)
                            .pad(3),
                        Flex::new().gap(1),
                        || {
                            row(Modifier::new().h(9).pad_x(2), Flex::new().center(), || {
                                label("Settings", Type::new(13).ratio(1.4).tone(Role::Subtle))
                            });
                            block(Modifier::new().pad_side(Side::Bottom, 2), || {
                                Input::new(&find, FocusId(600))
                                    .variant(InputVariant::Bordered)
                                    .leading_icon(IconName::Search)
                                    .size(Size::Sm)
                                    .placeholder("Search settings")
                                    .build();
                            });
                            for (index, (name, glyph)) in [
                                ("General", IconName::Settings),
                                ("Appearance", IconName::Pencil),
                                ("Editor", IconName::FileCode),
                                ("Languages", IconName::File),
                                ("Keymap", IconName::SquareSlash),
                                ("Terminal", IconName::Terminal),
                                ("Browser", IconName::Globe),
                                ("Git", IconName::Git),
                                ("Agents", IconName::ChevronRight),
                                ("Servers", IconName::Plug),
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                let tone = if index == 0 { Role::Text } else { Role::Muted };
                                Tile::new(FocusId(610 + index as u64))
                                    .selected(index == 0)
                                    .inner(Modifier::new().pad_x(2))
                                    .build_with(|| {
                                        icon(glyph, 4.0, tone);
                                        label_with(
                                            Modifier::new().grow(),
                                            name,
                                            Type::new(13).ratio(1.4).tone(tone).truncate(),
                                        );
                                    });
                            }
                        },
                    );
                    column(
                        Modifier::new()
                            .grow()
                            .fill_max_height()
                            .fill(Role::Background),
                        Flex::new(),
                        || {
                            block(
                                Modifier::new()
                                    .pad_side(Side::Left, 8)
                                    .pad_side(Side::Right, 8)
                                    .pad_side(Side::Top, 6)
                                    .pad_side(Side::Bottom, 4),
                                || label("General", Type::new(22).ratio(1.3).medium()),
                            );
                            column(
                                Modifier::new().fill_max_width().pad_x(8),
                                Flex::new(),
                                || {
                                    paragraph(
                                        Modifier::new().pad_side(Side::Bottom, 4),
                                        "How the window opens, closes and keeps itself up to date.",
                                        Type::new(13).ratio(1.4).tone(Role::Muted),
                                    );
                                    ported::setting_row(
                                        "Open on launch",
                                        "The last workspace with its open files and layout, or a fresh start.",
                                        || {
                                            ported::choices(
                                                &["Last workspace", "Fresh start"],
                                                0,
                                                |option| Some(FocusId(630 + option as u64)),
                                            );
                                        },
                                    );
                                    ported::setting_row(
                                        "Confirm before closing",
                                        "Ask before closing the window while agents are working.",
                                        || {
                                            ported::switch(true, Some(FocusId(640)));
                                        },
                                    );
                                    ported::setting_row(
                                        "Check for updates",
                                        "Look for a newer release while the window is open.",
                                        || {
                                            ported::switch(true, Some(FocusId(641)));
                                        },
                                    );
                                    ported::setting_row(
                                        "Update channel",
                                        "Follow the stable releases, or the pre-releases.",
                                        || {
                                            ported::choices(&["Stable", "Preview"], 0, |option| {
                                                Some(FocusId(650 + option as u64))
                                            });
                                        },
                                    );
                                    row(
                                        Modifier::new().fill_max_width().pad_y(4),
                                        Flex::new().center().between().gap(6),
                                        || {
                                            label(
                                                "Pandemonium 0.14.0 is available; this is 0.15.0-rc.4.",
                                                Type::new(13).ratio(1.4).tone(Role::Muted),
                                            );
                                            Button::new("Open Release Page", FocusId(660))
                                                .variant(ported::Variant::Outline)
                                                .size(Size::Sm)
                                                .build();
                                        },
                                    );
                                },
                            );
                        },
                    );
                },
            );
        },
    );
}

/// Every named icon, in the order the enum lists them.
const ALL_ICONS: [IconName; 74] = [
    IconName::Git,
    IconName::GitFilled,
    IconName::ArrowLeft,
    IconName::ArrowRight,
    IconName::ArrowUp,
    IconName::ArrowDown,
    IconName::ChevronDown,
    IconName::ChevronRight,
    IconName::Search,
    IconName::SearchFilled,
    IconName::Settings,
    IconName::Split,
    IconName::Close,
    IconName::Pin,
    IconName::Plus,
    IconName::Minus,
    IconName::Check,
    IconName::CheckCircle,
    IconName::Progress,
    IconName::Sparkles,
    IconName::Copy,
    IconName::PanelLeft,
    IconName::PanelLeftFilled,
    IconName::PanelRight,
    IconName::PanelRightFilled,
    IconName::PanelBottom,
    IconName::PanelBottomFilled,
    IconName::SquareSplit,
    IconName::Play,
    IconName::More,
    IconName::At,
    IconName::File,
    IconName::FileCode,
    IconName::Folder,
    IconName::FolderFilled,
    IconName::FolderOpen,
    IconName::GitCompare,
    IconName::GitFork,
    IconName::History,
    IconName::Refresh,
    IconName::Pick,
    IconName::Globe,
    IconName::Undo,
    IconName::Message,
    IconName::CircleUser,
    IconName::Plug,
    IconName::Zap,
    IconName::SquareSlash,
    IconName::Terminal,
    IconName::FilePlus,
    IconName::FolderPlus,
    IconName::Trash,
    IconName::Pencil,
    IconName::Collapse,
    IconName::Maximize,
    IconName::Minimize,
    IconName::Loading,
    IconName::Stop,
    IconName::GitCommit,
    IconName::GitFetch,
    IconName::GitPull,
    IconName::GitPush,
    IconName::Target,
    IconName::Bug,
    IconName::Continue,
    IconName::Pause,
    IconName::StepInto,
    IconName::StepOut,
    IconName::StepOver,
    IconName::Lightbulb,
    IconName::Camera,
    IconName::Scan,
    IconName::Lock,
    IconName::LockOpen,
];
