//! The keymap page of the settings panel: the preset the keys start from, every action with its keys, and the reader's own keys recorded over the preset's.

use crate::kit::sp;
use pm_editor::{Action, shown};
use rustle::{CursorIcon, Modifier};
use rustle::{Flex, flex_column, flex_row};

use crate::kit::{FocusId, Hovered, Interactive, Looks, Radius, Role, Side, Styled, Type, label};

use crate::input::Input;
use crate::parts::{clipped, line};
use crate::settings::SETTINGS_KEYMAP_FIND;
use crate::settings_catalog::Setting;
use crate::settings_ids::{KEYMAP_CONTROLS, PER_ACTION};
use crate::state::WorkspaceState;

/// What a control of an action does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Part {
    /// Records new keys for it.
    Record,
    /// Puts back the preset's keys.
    Reset,
    /// Takes its keys away.
    Clear,
}

/// The name of the control `part` of `action`, or nothing when the keymap page has no room for the action.
fn id_of(action: Action, part: Part) -> Option<FocusId> {
    let place = Action::ALL.iter().position(|held| *held == action)?;
    let offset = match part {
        Part::Record => 0,
        Part::Reset => 1,
        Part::Clear => 2,
    };
    KEYMAP_CONTROLS.cell(place, offset, PER_ACTION)
}

/// The action and the control of it that `id` names.
fn control_of(id: FocusId) -> Option<(Action, Part)> {
    let (place, offset) = KEYMAP_CONTROLS.cell_of(id, PER_ACTION)?;
    let action = *Action::ALL.get(place)?;
    let part = match offset {
        0 => Part::Record,
        1 => Part::Reset,
        2 => Part::Clear,
        _ => return None,
    };
    Some((action, part))
}

impl WorkspaceState {
    /// Carries out pressing the control `id` of the keymap page, and says whether it was one.
    pub(crate) fn keymap_pressed(&mut self, id: FocusId) -> bool {
        let Some((action, part)) = control_of(id) else {
            return false;
        };
        match part {
            Part::Record => {
                self.recording = (self.recording != Some(action)).then_some(action);
            }
            Part::Reset => {
                self.editor.keys.remove(action.key());
                self.recording = None;
            }
            Part::Clear => {
                self.editor.keys.insert(action.key().to_owned(), Vec::new());
                self.recording = None;
            }
        }
        true
    }

    /// The state with `chords` pressed so far while keys are recorded, as they are written.
    pub fn with_recorded(mut self, chords: Vec<String>) -> Self {
        self.recorded = chords;
        self
    }

    /// Whether keys are being recorded for an action.
    pub fn recording_keys(&self) -> bool {
        self.recording.is_some()
    }

    /// The state after keys were recorded for the action being recorded: `binding` as it is written, or nothing when the recording was given up.
    pub fn keys_recorded(mut self, binding: Option<String>) -> Self {
        self.recorded.clear();
        if let (Some(action), Some(binding)) = (self.recording.take(), binding) {
            self.editor
                .keys
                .insert(action.key().to_owned(), vec![binding]);
        }
        self
    }
}

/// `own`, the modifier of a control, answering to `id` and showing it can be pressed, or as it is when it has no name.
fn pressable(own: Modifier, id: Option<FocusId>) -> Modifier {
    match id {
        Some(id) => own.cursor(CursorIcon::Pointer).target(id),
        None => own,
    }
}

/// Emits a small button of a row saying `words`, answering to `id`.
#[track_caller]
fn small(words: &str, id: Option<FocusId>) {
    let own = Modifier::new()
        .h(6)
        .looks(Looks::new().hover(Role::SurfaceHover).radius(Radius::Sm));
    flex_row(
        pressable(own, id).pad_x(2),
        Flex::new().items_center(),
        || line(words, 11, 1.4, Role::Muted),
    );
}

/// Emits the keys of `action`, each as a key cap, or what recording asks for while it records, answering to `id`.
#[track_caller]
fn caps(keys: &[String], recording: Option<&[String]>, id: Option<FocusId>) {
    let edge = if recording.is_some() {
        Role::Accent
    } else {
        Role::Border
    };
    let own = Modifier::new()
        .h(7)
        .fill_rounded(Role::Background, Radius::Sm)
        .stroke(edge, Radius::Sm);
    let code = Type::new(12).ratio(1.4).tone(Role::Text).truncate().mono();
    flex_row(
        pressable(own, id).pad_x(1.5),
        Flex::new().items_center().gap(sp(1)),
        move || {
            if let Some(so_far) = recording {
                let said = if so_far.is_empty() {
                    "Press keys…".to_owned()
                } else {
                    format!("{} …", shown(&so_far.join(" ")))
                };
                label(said, code);
            } else if keys.is_empty() {
                line("Not bound", 12, 1.4, Role::Subtle);
            } else {
                for (place, binding) in keys.iter().enumerate() {
                    if place > 0 {
                        line("or", 11, 1.4, Role::Subtle);
                    }
                    label(shown(binding), code);
                }
            }
        },
    );
}

/// Whether `action`, bound to `keys`, is one `needle`, lowercased, finds: by its name, its group, or its keys as they are written or shown.
fn found(action: Action, keys: &[String], needle: &str) -> bool {
    needle.is_empty()
        || action.title().to_lowercase().contains(needle)
        || action.group().to_lowercase().contains(needle)
        || keys.iter().any(|binding| {
            binding.to_lowercase().contains(needle)
                || shown(binding).to_lowercase().contains(needle)
        })
}

/// Emits the box the actions are searched with.
#[track_caller]
fn search_box(state: &WorkspaceState) {
    Input::search(&state.settings_page.keymap_find, SETTINGS_KEYMAP_FIND)
        .placeholder("Search actions by name or keys")
        .build();
}

/// One action the search found, with what its row shows.
struct Found<'a> {
    /// The action.
    action: Action,
    /// The keys it is bound to.
    keys: Vec<String>,
    /// Whether the reader recorded its keys over the preset's.
    own: bool,
    /// The titles of the other actions bound to one of its keys.
    clashes: Vec<&'static str>,
    /// The keys recorded so far while it records, or nothing.
    recording: Option<&'a [String]>,
}

/// Emits the keymap page's body, `room` logical pixels across: the preset, the search box, then every action it finds under its group with its keys.
#[track_caller]
pub(crate) fn rows(state: &WorkspaceState, room: f32) {
    let keymap = state.editor.keymap();
    let needle = state.settings_page.keymap_find.text().trim().to_lowercase();
    crate::settings_pages::setting_row(state, Setting::BaseKeymap, room);
    search_box(state);
    let groups = [
        "Window",
        "Session",
        "Source Control",
        "Tabs",
        "Navigation",
        "Code Intelligence",
        "Editing",
        "Selection",
        "Search",
        "File",
        "Terminal",
    ];
    let ordered = groups.iter().flat_map(|wanted| {
        Action::ALL
            .into_iter()
            .filter(move |action| action.group() == *wanted)
    });
    let mut sections: Vec<(&str, Vec<Found>)> = Vec::new();
    for action in ordered {
        let keys = keymap.keys_of(action);
        if !found(action, &keys, &needle) {
            continue;
        }
        let own = state.editor.keys.contains_key(action.key());
        let clashes: Vec<&'static str> = keys
            .iter()
            .flat_map(|binding| keymap.clashes(action, binding))
            .map(Action::title)
            .collect();
        let recording = (state.recording == Some(action)).then_some(state.recorded.as_slice());
        let entry = Found {
            action,
            keys,
            own,
            clashes,
            recording,
        };
        match sections.last_mut() {
            Some((group, members)) if *group == action.group() => members.push(entry),
            _ => sections.push((action.group(), vec![entry])),
        }
    }
    let any = !sections.is_empty();
    for (group, members) in &sections {
        section(group, members);
    }
    if !any {
        line("No action matches.", 13, 1.5, Role::Muted);
    }
    rustle::spacer(Modifier::new().width(1.0).h(2));
}

/// Emits the heading `group` over a bordered box holding the row of each of `members`, ruled between them.
#[track_caller]
fn section(group: &str, members: &[Found]) {
    label(
        group,
        Type::new(13)
            .ratio(1.4)
            .tone(Role::Text)
            .truncate()
            .medium(),
    );
    let last = members.len().saturating_sub(1);
    flex_column(
        Modifier::new()
            .fill_rounded(Role::Surface, Radius::Lg)
            .stroke(Role::Border, Radius::Lg),
        Flex::new(),
        || {
            for (place, entry) in members.iter().enumerate() {
                rustle::key(place, || action_row(entry, place < last));
            }
        },
    );
}

/// Emits the row of `entry`: its title and clashes, then its keys and the controls that reset or clear them; ruled below when `ruled`.
#[track_caller]
fn action_row(entry: &Found, ruled: bool) {
    let action = entry.action;
    let own = if ruled {
        Modifier::new().stroke_side(Side::Bottom, 1.0, Role::Border)
    } else {
        Modifier::new()
    };
    flex_row(
        own.pad_x(4).pad_y(2),
        Flex::new().items_center().justify_between().gap(sp(6)),
        || {
            flex_column(Modifier::new().grow(), Flex::new().gap(sp(0.5)), || {
                clipped(action.title(), 13, Role::Text);
                if !entry.clashes.is_empty() {
                    line(
                        format!("Also bound to {}", entry.clashes.join(", ")),
                        11,
                        1.4,
                        Role::Warning,
                    );
                }
            });
            flex_row(
                Modifier::new(),
                Flex::new().items_center().gap(sp(1)),
                || {
                    caps(&entry.keys, entry.recording, id_of(action, Part::Record));
                    if entry.own {
                        small("Reset", id_of(action, Part::Reset));
                    }
                    if !entry.keys.is_empty() {
                        small("Clear", id_of(action, Part::Clear));
                    }
                },
            );
        },
    );
}
