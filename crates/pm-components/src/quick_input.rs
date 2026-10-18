//! The quick input as it is drawn: a field over the command center it opens from, and under it the rows of what the field lists, the letters typed marked in their names and the keys of each command on caps.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, ShadowLayer, absolute, bg, border, border_color, border_t, cursor_pointer, flex,
    flex_1, flex_col, flex_none, font_semibold, gap, h_px, items_center, leading_ratio, left,
    min_w_px, overflow_hidden, pb, pt, px, rounded, shadow_layers, text_color, text_px, top, w_px,
    z,
};
use pm_ui::{Node, node, text};

use crate::assist::listing_cells;
use crate::input::{Input, Size as InputSize};
use crate::listing::{
    Destination, Items, LISTING_BODY, LISTING_FILTER, LISTING_ROW_BASE, LISTING_WIDTH, Listing,
    Quick, line_named, matched_characters,
};
use crate::parts::{glyph, line};
use pm_icon::IconName;

/// How many rows the quick input shows at once.
pub const QUICK_SHOWN: usize = 16;

/// How tall a row is, in logical pixels.
const ROW: f32 = 22.0;

/// How tall the field is, in logical pixels: as tall as the command center it lies over.
pub const FIELD_HEIGHT: f32 = 28.0;

/// The shadow the quick input casts over the window.
const SHADOW: [ShadowLayer; 1] = [ShadowLayer::new(0.0, 10.0, 32.0, 0.0, 0.36, false)];

/// The quick input for `listing`: its field, then the rows around the one picked, or what keeps it empty.
pub fn quick_input(listing: &Listing) -> Node {
    let field = listing.items.filter().map(|filter| {
        node().styles([px(1.5), pt(1.5), pb(1)]).child(
            Input::new(filter, LISTING_FILTER)
                .size(InputSize::Sm)
                .placeholder(listing.items.placeholder())
                .build()
                .styles([h_px(FIELD_HEIGHT)]),
        )
    });
    let shown = listing.items.rows();
    let count = shown.len();
    let status = match (&listing.problem, listing.answered, count) {
        (Some(problem), _, _) => Some(problem.clone()),
        (None, false, _) => Some("Searching…".to_owned()),
        (None, true, 0) => Some(empty_words(&listing.items)),
        _ => None,
    };
    let first = listing
        .picked
        .saturating_sub(QUICK_SHOWN - 1)
        .min(count.saturating_sub(QUICK_SHOWN));
    let rows = (first..count.min(first + QUICK_SHOWN))
        .map(|place| row(listing, place, shown[place], place == listing.picked));
    node()
        .styles([
            absolute(),
            left(listing.at.0 / 4.0),
            top(listing.at.1 / 4.0),
            z(60),
            flex(),
            flex_col(),
            w_px(LISTING_WIDTH),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
            shadow_layers(&SHADOW),
            overflow_hidden(),
        ])
        .focusable(LISTING_BODY)
        .children(field)
        .child(
            node()
                .styles([flex(), flex_col(), px(1.5), pb(1.75)])
                .children(status.map(|said| {
                    node()
                        .styles([flex(), items_center(), h_px(ROW), px(1.5)])
                        .child(line(said, 13, 1.4, Role::Muted))
                }))
                .children(rows),
        )
}

/// What the quick input says when nothing of `items` is left to pick.
fn empty_words(items: &Items) -> String {
    match items {
        Items::Line { lines, current, .. } => {
            format!("Type a line number between 1 and {lines} (now on line {current})")
        }
        Items::Choices { choosing, .. } => choosing.empty().to_owned(),
        items => match items.quick().unwrap_or(Quick::Files) {
            Quick::Commands => "No matching commands",
            Quick::Files => "No matching files",
            Quick::Symbols | Quick::Outline => "No matching symbols",
            Quick::Search => "Type to search the text of the project",
            Quick::Sessions => "No matching sessions",
            Quick::Line | Quick::Help | Quick::Choices => "Nothing matches",
        }
        .to_owned(),
    }
}

/// The row at `place` among those shown, of the item at `index`, filled under the pointer and when `picked`.
fn row(listing: &Listing, place: usize, index: usize, picked: bool) -> Node {
    let query = listing.items.query();
    let (cells, group) = match &listing.items {
        Items::Commands {
            commands,
            extras,
            recent,
            ..
        } => {
            let group = match (*recent, place) {
                (0, _) => None,
                (_, 0) => Some(("recently used", false)),
                (recent, place) if place == recent => Some(("other commands", true)),
                _ => None,
            };
            let cells = match commands.get(index) {
                Some((action, keys)) => {
                    let mut cells = vec![marked(&action.palette_title(), query)];
                    cells.extend(group.map(|(said, _)| separator(said)));
                    cells.extend(keys.as_deref().map(keycaps));
                    cells
                }
                None => extras
                    .get(index - commands.len())
                    .map_or_else(Vec::new, |(_, command)| {
                        vec![
                            marked(&command.title, query),
                            separator(&format!("extension {}", command.extension)),
                        ]
                    }),
            };
            (cells, group)
        }
        Items::Files { paths, recent, .. } => {
            let group = match (*recent, place) {
                (0, _) => None,
                (_, 0) => Some(("recently opened", false)),
                (recent, place) if place == recent => Some(("files", true)),
                _ => None,
            };
            let cells = paths.get(index).map_or_else(Vec::new, |path| {
                let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
                let mut cells = vec![
                    crate::file_icon::file_icon(path, 4.0),
                    node()
                        .styles([flex_none(), overflow_hidden()])
                        .child(marked(name, query)),
                    node()
                        .styles([flex_1(), min_w_px(0), overflow_hidden()])
                        .child(line(folder, 12, 1.4, Role::Subtle)),
                ];
                cells.extend(group.map(|(said, _)| separator(said)));
                cells
            });
            (cells, group)
        }
        Items::Sessions { sessions, .. } => {
            let project = |index: usize| {
                sessions
                    .get(index)
                    .is_some_and(|(destination, ..)| matches!(destination, Destination::Project(_)))
            };
            let before = place
                .checked_sub(1)
                .and_then(|earlier| listing.items.rows().get(earlier).copied());
            let group = (project(index) && !before.is_some_and(project))
                .then_some(("new session in", place > 0));
            let cells = sessions
                .get(index)
                .map_or_else(Vec::new, |(_, title, beside)| {
                    let mut cells = vec![marked(title, query)];
                    cells.extend(group.map(|(said, _)| separator(said)));
                    if !beside.is_empty() {
                        cells.push(node().styles([flex_none()]).child(line(
                            beside.clone(),
                            12,
                            1.4,
                            Role::Subtle,
                        )));
                    }
                    cells
                });
            (cells, group)
        }
        Items::Line { lines, .. } => {
            let cells = line_named(query, *lines).map_or_else(Vec::new, |number| {
                vec![
                    marked(&format!("Go to line {number}"), ""),
                    node().styles([flex_none()]).child(line(
                        format!("of {lines}"),
                        12,
                        1.4,
                        Role::Subtle,
                    )),
                ]
            });
            (cells, None)
        }
        Items::Choices { choices, .. } => {
            let cells = choices.get(index).map_or_else(Vec::new, |choice| {
                let mut cells = vec![marked(&choice.label, query.trim_start_matches('?'))];
                if !choice.detail.is_empty() {
                    cells.push(node().styles([flex_none(), overflow_hidden()]).child(line(
                        choice.detail.clone(),
                        12,
                        1.4,
                        Role::Subtle,
                    )));
                }
                if choice.current {
                    cells.push(glyph(IconName::Check, 3.5, Role::Muted));
                }
                cells
            });
            (cells, None)
        }
        items => (listing_cells(items, index), None),
    };
    let mut built = node()
        .styles([
            flex(),
            items_center(),
            gap(1.5),
            h_px(ROW),
            px(1.5),
            rounded(Radius::Sm),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .focusable(FocusId(LISTING_ROW_BASE + place as u64))
        .children(cells);
    if group.is_some_and(|(_, ruled)| ruled) {
        built = built.styles([border_t(), border_color(Role::Border)]);
    }
    if picked {
        built = built.styles([bg(Role::SurfaceSelected)]);
    }
    built
}

/// `name` in a line of its own width, the characters `query` matches set heavier and in the accent.
fn marked(name: &str, query: &str) -> Node {
    let places = matched_characters(name, query);
    let mut runs: Vec<(String, bool)> = Vec::new();
    for (place, character) in name.chars().enumerate() {
        let lit = places.contains(&place);
        match runs.last_mut() {
            Some((run, held)) if *held == lit => run.push(character),
            _ => runs.push((character.to_string(), lit)),
        }
    }
    node()
        .styles([
            flex(),
            items_center(),
            flex_1(),
            min_w_px(0),
            overflow_hidden(),
        ])
        .children(runs.into_iter().map(|(run, lit)| {
            let piece = text(run).styles([text_px(13), leading_ratio(1.4), flex_none()]);
            if lit {
                piece.styles([font_semibold(), text_color(Role::Accent)])
            } else {
                piece.styles([text_color(Role::Text)])
            }
        }))
}

/// The name of a group of rows, at the end of its first row.
fn separator(said: &str) -> Node {
    node()
        .styles([flex_none()])
        .child(line(said, 11, 1.4, Role::Subtle))
}

/// The keys `binding` is pressed with, such as `Ctrl+K Ctrl+I`, each on a cap of its own, joined by `+` within a chord and set apart between chords.
fn keycaps(binding: &str) -> Node {
    node()
        .styles([flex(), items_center(), gap(1.5), flex_none()])
        .children(binding.split(' ').map(|chord| {
            let mut keys = node().styles([flex(), items_center(), gap(0.5)]);
            for (place, key) in chord.split('+').enumerate() {
                if place > 0 {
                    keys = keys.child(line("+", 11, 1.0, Role::Subtle));
                }
                keys = keys.child(keycap(key));
            }
            keys
        }))
}

/// One key, on a cap.
fn keycap(key: &str) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            h_px(17.0),
            px(1.25),
            rounded(Radius::Sm),
            border(),
            border_color(Role::Border),
            bg(Role::Background),
        ])
        .child(line(key, 11, 1.0, Role::Muted))
}
