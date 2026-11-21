//! The quick input as it is drawn: a field over the command center it opens from, and under it the rows of what the field lists, the letters typed marked in their names and the keys of each command on caps.

use crate::areas::EditorEvent;
use crate::theme::Frame;
use crate::theme::Indicated;
use crate::theme::ShadowLayers;
use rustle::{Alignment, Arrangement, TextOverflow};

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::phosphor;
use rustle::{CursorIcon, Modifier};

use crate::assist::listing_cells;
use crate::events::Interactive;
use crate::input::Input;
use crate::listing::{
    Destination, Items, LISTING_WIDTH, Listing, Quick, line_named, matched_characters, task_title,
};
use crate::parts::{glyph, line};
use crate::size::Size;
use crate::theme::{Radius, Role};

/// How many rows the quick input shows at once.
pub const QUICK_SHOWN: usize = 16;

/// How tall a row is, in logical pixels.
const ROW: f32 = 22.0;

/// How wide the title of a session is when a preview of the message that matched follows it, in logical pixels.
const PREVIEWED_TITLE: f32 = 170.0;

/// How tall the field is, in logical pixels: as tall as the command center it lies over.
pub const FIELD_HEIGHT: f32 = 28.0;

/// The lambda that takes the row picked of the list over the file, as Enter in the field
/// narrowing it does: the command it holds when the list holds commands, and otherwise
/// the row as the workspace takes it.
pub(crate) fn take_picked() -> impl Fn() + 'static {
    let operations = crate::events::operations();
    move || operations.take_listing(None)
}

/// The shadow the quick input casts over the window, as `(x, y, blur, spread, alpha)`.
const SHADOW: [rustle::Shadow; 1] = [crate::theme::box_shadow(0.0, 10.0, 32.0, 0.0, 0.36)];

/// What a row of the quick input holds, emitted when the row is drawn.
type Cells<'a> = Box<dyn FnOnce() + 'a>;

/// Emits the quick input for `listing`: its field, then the rows around the one picked, or what keeps it empty.
#[track_caller]
pub fn quick_input(listing: &Listing) {
    let shown = listing.items.rows();
    let count = shown.len();
    let status = match (&listing.problem, listing.answered, count) {
        (Some(problem), _, _) => Some(problem.clone()),
        (None, false, _) => Some("Searching…".to_owned()),
        (None, true, 0) => Some(empty_words(&listing.items)),
        _ => None,
    };
    let list = rustle::remember_lazy_list_state();
    list.scroll().set_scrollbar(rustle::ScrollbarStyle {
        visibility: rustle::ScrollbarVisibility::Hidden,
        ..theme::scrollbar()
    });
    let picked = listing.picked;
    let shown_list = list.clone();
    rustle::launched_effect((picked, count), move |_| async move {
        crate::rows_in_view::keep_in_view(&shown_list, picked);
    });
    rustle::column_arranged(
        Modifier::new()
            .offset(listing.at.0, listing.at.1)
            .z_index(60)
            .width(LISTING_WIDTH)
            .shadow_layers(&SHADOW, Radius::Lg.px())
            .panel(Radius::Lg)
            .clip()
            .control(
                Target::Editor(EditorEvent::ListingBody),
                send(Event::Editor(EditorEvent::ListingBody)),
            ),
        Arrangement::Start,
        Alignment::Start,
        || {
            if let Some(filter) = listing.items.filter() {
                rustle::column_arranged(
                    Modifier::new().fill_max_width().padding_each(
                        theme::sp(1.5),
                        theme::sp(1.5),
                        theme::sp(1.5),
                        theme::sp(1),
                    ),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        crate::focus::request_focus_on(
                            Target::Editor(EditorEvent::ListingFilter),
                            filter,
                        );
                        Input::new(filter, Target::Editor(EditorEvent::ListingFilter))
                            .size(Size::Sm)
                            .placeholder(listing.items.placeholder())
                            .modifier(Modifier::new().fill_max_width().height(FIELD_HEIGHT))
                            .on_submit(crate::quick_input::take_picked())
                            .hurries(Target::Editor(EditorEvent::ListingPrompt))
                            .build();
                    },
                );
            }
            rustle::column_arranged(
                Modifier::new().fill_max_width().padding_each(
                    theme::sp(1.5),
                    0.0,
                    theme::sp(1.5),
                    theme::sp(1.75),
                ),
                Arrangement::Start,
                Alignment::Start,
                || {
                    if let Some(said) = status {
                        rustle::row_arranged(
                            Modifier::new()
                                .fill_max_width()
                                .height(ROW)
                                .padding_xy(theme::sp(1.5), 0.0),
                            Arrangement::Start,
                            Alignment::Center,
                            || line(said, 13, 1.4, Role::Muted),
                        );
                    }
                    rustle::lazy_list_with(
                        Modifier::new()
                            .fill_max_width()
                            .height_in(0.0, QUICK_SHOWN as f32 * ROW),
                        &list,
                        rustle::LazyContent::new(),
                        count,
                        |place| place,
                        |place| quick_row(listing, place, shown[place], place == picked),
                    );
                },
            );
        },
    );
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

/// Emits `content` in a line of `pixels` tall type in `tone`, in a box `modifier` shapes that cuts it short at its edge.
#[track_caller]
fn cut(modifier: Modifier, content: impl Into<String>, pixels: u32, tone: Role) {
    rustle::row_arranged(
        modifier.clip(),
        Arrangement::Start,
        Alignment::Start,
        || line(content, pixels, 1.4, tone),
    );
}

/// Emits the row at `place` among those shown, of the item at `index`, filled under the pointer and when `picked`.
fn quick_row(listing: &Listing, place: usize, index: usize, picked: bool) {
    let query = &listing.items.query();
    let (cells, group): (Cells<'_>, Option<(&str, bool)>) = match &listing.items {
        Items::Commands {
            commands,
            tasks,
            extras,
            recent,
            ..
        } => {
            let shown = listing.items.rows();
            let kind = |place: usize| {
                shown.get(place).map(|held| {
                    u8::from(*held >= commands.len())
                        + u8::from(*held >= commands.len() + tasks.len())
                })
            };
            let starts = place == 0 || kind(place - 1) != kind(place);
            let group = match kind(place) {
                Some(1) if starts => Some(("tasks", place > 0)),
                Some(2) if starts => Some(("extensions", place > 0)),
                Some(0) if *recent > 0 && place == 0 => Some(("recently used", false)),
                Some(0) if *recent > 0 && place == *recent => Some(("other commands", true)),
                _ => None,
            };
            let cells: Cells<'_> = Box::new(move || {
                let task = index.checked_sub(commands.len());
                if let Some((action, keys)) = commands.get(index) {
                    marked(Modifier::new().weight(1.0), &action.palette_title(), query);
                    group_name(group);
                    if let Some(keys) = keys.as_deref() {
                        keycaps(keys);
                    }
                } else if let Some(task) = task.and_then(|task| tasks.get(task)) {
                    marked(Modifier::new().weight(1.0), &task_title(task), query);
                    group_name(group);
                    cut(Modifier::new(), task.command.clone(), 12, Role::Subtle);
                } else if let Some((_, command)) = extras.get(index - commands.len() - tasks.len())
                {
                    marked(Modifier::new().weight(1.0), &command.title, query);
                    group_name(group);
                    line(
                        format!("extension {}", command.extension),
                        11,
                        1.4,
                        Role::Subtle,
                    );
                }
            });
            (cells, group)
        }
        Items::Files { paths, recent, .. } => {
            let group = match (*recent, place) {
                (0, _) => None,
                (_, 0) => Some(("recently opened", false)),
                (recent, place) if place == recent => Some(("files", true)),
                _ => None,
            };
            let cells: Cells<'_> = Box::new(move || {
                if let Some(path) = paths.get(index) {
                    let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
                    crate::file_icon::file_icon(path, 4.0);
                    marked(Modifier::new().clip(), name, query);
                    cut(Modifier::new().weight(1.0), folder, 12, Role::Subtle);
                    group_name(group);
                }
            });
            (cells, group)
        }
        Items::Sessions {
            sessions, found, ..
        } => {
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
            let cells: Cells<'_> = Box::new(move || {
                let Some((_, title, beside)) = sessions.get(index) else {
                    return;
                };
                let preview = found
                    .iter()
                    .find(|(place, _)| *place == index)
                    .map(|(_, preview)| preview);
                match preview {
                    Some(preview) => {
                        marked(Modifier::new().width(PREVIEWED_TITLE).clip(), title, query);
                        cut(
                            Modifier::new().weight(1.0),
                            preview.clone(),
                            12,
                            Role::Muted,
                        );
                    }
                    None => marked(Modifier::new().weight(1.0), title, query),
                }
                group_name(group);
                let beside = match preview {
                    Some(_) => beside.rsplit(" · ").next().unwrap_or_default(),
                    None => beside.as_str(),
                };
                if !beside.is_empty() {
                    line(beside.to_owned(), 12, 1.4, Role::Subtle);
                }
            });
            (cells, group)
        }
        Items::Line { lines, .. } => {
            let cells: Cells<'_> = Box::new(move || {
                if let Some(number) = line_named(query, *lines) {
                    marked(
                        Modifier::new().weight(1.0),
                        &format!("Go to line {number}"),
                        "",
                    );
                    line(format!("of {lines}"), 12, 1.4, Role::Subtle);
                }
            });
            (cells, None)
        }
        Items::Choices { choices, .. } => {
            let cells: Cells<'_> = Box::new(move || {
                if let Some(choice) = choices.get(index) {
                    marked(
                        Modifier::new().weight(1.0),
                        &choice.label,
                        query.trim_start_matches('?'),
                    );
                    if !choice.detail.is_empty() {
                        cut(Modifier::new(), choice.detail.clone(), 12, Role::Subtle);
                    }
                    if choice.current {
                        glyph(phosphor::CHECK, 3.5, Role::Muted);
                    }
                }
            });
            (cells, None)
        }
        items => {
            let cells: Cells<'_> = Box::new(move || listing_cells(items, index));
            (cells, None)
        }
    };
    let id = Target::Editor(EditorEvent::ListingRow(place));
    let looks = if picked {
        theme::InteractionTones::new()
            .idle(Role::SurfaceSelected)
            .hover(Role::SurfaceSelected)
    } else {
        theme::InteractionTones::new().hover(Role::SurfaceHover)
    };
    let own = Modifier::new().fill_max_width().height(ROW).indicated(
        looks.radius(Radius::Sm),
        theme::Focused::Control(crate::focus::is_focused(id)),
    );
    let ruled = group.is_some_and(|(_, ruled)| ruled);
    rustle::column_arranged(own, Arrangement::Start, Alignment::Start, || {
        if ruled {
            rustle::horizontal_divider(Modifier::new(), 1.0, theme::color(Role::Border));
        }
        rustle::row_arranged(
            Modifier::new()
                .fill_max_width()
                .weight(1.0)
                .cursor(CursorIcon::Pointer)
                .control(id, {
                    let operations = crate::events::operations();
                    move || operations.take_listing(Some(place))
                })
                .padding_xy(theme::sp(1.5), 0.0),
            Arrangement::SpacedBy(theme::sp(1.5)),
            Alignment::Center,
            cells,
        );
    });
}

/// Emits `name` in a line of its own width, in a box `modifier` shapes that cuts it short at its edge, the characters `query` matches set heavier and in the accent.
#[track_caller]
fn marked(modifier: Modifier, name: &str, query: &str) {
    let places = matched_characters(name, query);
    let mut runs: Vec<(String, bool)> = Vec::new();
    for (place, character) in name.chars().enumerate() {
        let lit = places.contains(&place);
        match runs.last_mut() {
            Some((run, held)) if *held == lit => run.push(character),
            _ => runs.push((character.to_string(), lit)),
        }
    }
    rustle::row_arranged(
        modifier.clip(),
        Arrangement::Start,
        Alignment::Center,
        move || {
            let kind = theme::text_style(13, 1.4);
            for (run, lit) in runs {
                if lit {
                    rustle::basic_text(
                        Modifier::new(),
                        run,
                        kind.font_weight(rustle::FontWeight::SEMI_BOLD)
                            .color(theme::color(Role::Accent)),
                        TextOverflow::Ellipsis,
                    );
                } else {
                    rustle::basic_text(
                        Modifier::new(),
                        run,
                        kind.color(theme::color(Role::Text)),
                        TextOverflow::Ellipsis,
                    );
                }
            }
        },
    );
}

/// Emits the name of the group `group` begins, at the end of its first row, when the row begins one.
#[track_caller]
fn group_name(group: Option<(&str, bool)>) {
    if let Some((said, _)) = group {
        line(said, 11, 1.4, Role::Subtle);
    }
}

/// Emits the keys `binding` is pressed with, such as `Ctrl+K Ctrl+I`, each on a cap of its own, joined by `+` within a chord and set apart between chords.
fn keycaps(binding: &str) {
    rustle::row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(theme::sp(1.5)),
        Alignment::Center,
        || {
            for chord in binding.split(' ') {
                rustle::row_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(theme::sp(0.5)),
                    Alignment::Center,
                    || {
                        for (place, key) in chord.split('+').enumerate() {
                            if place > 0 {
                                line("+", 11, 1.0, Role::Subtle);
                            }
                            keycap(key);
                        }
                    },
                );
            }
        },
    );
}

/// Emits one key, on a cap.
fn keycap(key: &str) {
    rustle::row_arranged(
        Modifier::new()
            .height(17.0)
            .background_rounded(theme::color(Role::Background), Radius::Sm.px())
            .outlined(Radius::Sm)
            .padding_xy(theme::sp(1.25), 0.0),
        Arrangement::Start,
        Alignment::Center,
        || line(key, 11, 1.0, Role::Muted),
    );
}
