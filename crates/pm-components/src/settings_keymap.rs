//! The keymap page of the settings panel: the preset the keys start from, every action with its keys, and the reader's own keys recorded over the preset's.

use pm_editor::{Action, shown};
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, flex_1, flex_col, flex_none,
    font_mono, gap, h, items_center, justify_between, min_w_px, px, rounded, w_px,
};
use pm_ui::{Node, node};

use crate::parts::{clipped, line};
use crate::settings::SETTINGS_KEYMAP_FIND;
use crate::settings_catalog::Setting;
use crate::state::WorkspaceState;

/// The first of the controls of the actions; each action has four after its own base, its place among them times four.
const ACTION_BASE: u64 = 20_002_000;

/// How many controls an action has room for.
const PER_ACTION: u64 = 4;

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

/// The name of the control `part` of `action`.
fn id_of(action: Action, part: Part) -> FocusId {
    let place = Action::ALL
        .iter()
        .position(|held| *held == action)
        .unwrap_or_default() as u64;
    let offset = match part {
        Part::Record => 0,
        Part::Reset => 1,
        Part::Clear => 2,
    };
    FocusId(ACTION_BASE + place * PER_ACTION + offset)
}

/// The action and the control of it that `id` names.
fn control_of(id: FocusId) -> Option<(Action, Part)> {
    let offset = id.0.checked_sub(ACTION_BASE)?;
    let action = *Action::ALL.get((offset / PER_ACTION) as usize)?;
    let part = match offset % PER_ACTION {
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

/// A small button of a row saying `label`.
fn small(label: &str, id: FocusId) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            h(6),
            px(2),
            rounded(Radius::Sm),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .focusable(id)
        .child(line(label, 11, 1.4, Role::Muted))
}

/// The keys of `action`, each as a key cap, or what recording asks for while it records.
fn caps(keys: &[String], recording: Option<&[String]>, id: FocusId) -> Node {
    let mut held = node()
        .styles([
            flex(),
            items_center(),
            gap(1),
            flex_none(),
            h(7),
            px(1.5),
            rounded(Radius::Sm),
            cursor_pointer(),
            border(),
            border_color(if recording.is_some() {
                Role::Accent
            } else {
                Role::Border
            }),
            bg(Role::Background),
        ])
        .focusable(id);
    if let Some(so_far) = recording {
        let said = if so_far.is_empty() {
            "Press keys…".to_owned()
        } else {
            format!("{} …", shown(&so_far.join(" ")))
        };
        held = held.child(line(said, 12, 1.4, Role::Text).styles([font_mono()]));
    } else if keys.is_empty() {
        held = held.child(line("Not bound", 12, 1.4, Role::Subtle));
    } else {
        for (place, binding) in keys.iter().enumerate() {
            if place > 0 {
                held = held.child(line("or", 11, 1.4, Role::Subtle));
            }
            held = held.child(line(shown(binding), 12, 1.4, Role::Text).styles([font_mono()]));
        }
    }
    held
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

/// The box the actions are searched with.
fn search_box(state: &WorkspaceState) -> Node {
    crate::search_box::SearchBox::new(&state.settings_page.keymap_find, SETTINGS_KEYMAP_FIND)
        .placeholder("Search actions by name or keys")
        .build()
}

/// The keymap page's body, `room` logical pixels across: the preset, the search box, then every action it finds under its group with its keys.
pub(crate) fn rows(state: &WorkspaceState, room: f32) -> Vec<Node> {
    let keymap = state.editor.keymap();
    let needle = state.settings_page.keymap_find.text().trim().to_lowercase();
    let mut parts = vec![
        crate::settings_pages::setting_row(state, Setting::BaseKeymap, room),
        search_box(state),
    ];
    let mut group = "";
    let mut rows: Vec<Node> = Vec::new();
    let groups = [
        "Navigation",
        "Code Intelligence",
        "Editing",
        "Selection",
        "Search",
        "File",
    ];
    let ordered = groups.iter().flat_map(|wanted| {
        Action::ALL
            .into_iter()
            .filter(move |action| action.group() == *wanted)
    });
    let flush = |group: &str, rows: &mut Vec<Node>, parts: &mut Vec<Node>| {
        if rows.is_empty() {
            return;
        }
        parts.push(line(group, 13, 1.4, Role::Text).styles([pm_style::font_medium()]));
        let last = rows.len() - 1;
        parts.push(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    flex_none(),
                    rounded(Radius::Lg),
                    border(),
                    border_color(Role::Border),
                    bg(Role::Surface),
                ])
                .children(
                    std::mem::take(rows)
                        .into_iter()
                        .enumerate()
                        .map(|(place, row)| {
                            if place < last {
                                row.styles([pm_style::border_b(), border_color(Role::Border)])
                            } else {
                                row
                            }
                        }),
                ),
        );
    };
    let mut any = false;
    for action in ordered {
        let keys = keymap.keys_of(action);
        if !found(action, &keys, &needle) {
            continue;
        }
        any = true;
        if action.group() != group {
            flush(group, &mut rows, &mut parts);
            group = action.group();
        }
        let own = state.editor.keys.contains_key(action.key());
        let clashes: Vec<&str> = keys
            .iter()
            .flat_map(|binding| keymap.clashes(action, binding))
            .map(Action::title)
            .collect();
        let recording = (state.recording == Some(action)).then_some(state.recorded.as_slice());
        let mut about = node()
            .styles([flex(), flex_col(), flex_1(), gap(0.5), min_w_px(0)])
            .child(clipped(action.title(), 13, Role::Text));
        if !clashes.is_empty() {
            about = about.child(line(
                format!("Also bound to {}", clashes.join(", ")),
                11,
                1.4,
                Role::Warning,
            ));
        }
        let mut controls = node()
            .styles([flex(), items_center(), gap(1), flex_none()])
            .child(caps(&keys, recording, id_of(action, Part::Record)));
        if own {
            controls = controls.child(small("Reset", id_of(action, Part::Reset)));
        }
        if !keys.is_empty() {
            controls = controls.child(small("Clear", id_of(action, Part::Clear)));
        }
        rows.push(
            node()
                .styles([
                    flex(),
                    items_center(),
                    justify_between(),
                    gap(6),
                    px(4),
                    pm_style::py(2),
                    flex_none(),
                ])
                .child(about)
                .child(controls),
        );
    }
    flush(group, &mut rows, &mut parts);
    if !any {
        parts.push(line("No action matches.", 13, 1.5, Role::Muted));
    }
    parts.push(node().styles([w_px(1.0), h(2), flex_none()]));
    parts
}
