//! The keymap page of the settings panel: the preset the keys start from, every action with its keys, and the reader's own keys recorded over the preset's.

use pm_editor::{Action, Preset, shown};
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, flex_col, flex_none, font_mono,
    gap, h, items_center, justify_between, justify_center, min_w_px, px, rounded, w_px,
};
use pm_ui::{Node, node};

use crate::parts::line;
use crate::state::WorkspaceState;

/// The first of the choices of preset; the rest follow it.
const PRESET_BASE: u64 = 20_001_900;

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

/// The preset the choice `id` picks.
fn preset_of(id: FocusId) -> Option<Preset> {
    let offset = id.0.checked_sub(PRESET_BASE)?;
    Preset::ALL.get(offset as usize).copied()
}

impl WorkspaceState {
    /// Carries out pressing the control `id` of the keymap page, and says whether it was one.
    pub(crate) fn keymap_pressed(&mut self, id: FocusId) -> bool {
        if let Some(preset) = preset_of(id) {
            self.editor.keymap = preset;
            self.recording = None;
            return true;
        }
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

/// The choices of preset, the one in use marked.
fn presets(picked: Preset) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            flex_none(),
            gap(0.5),
            px(0.5),
            h(8),
            rounded(Radius::Md),
            border(),
            border_color(Role::Border),
        ])
        .children(Preset::ALL.into_iter().enumerate().map(|(place, preset)| {
            let on = preset == picked;
            node()
                .styles([
                    flex(),
                    items_center(),
                    justify_center(),
                    h(6.5),
                    px(2.5),
                    rounded(Radius::Sm),
                    cursor_pointer(),
                ])
                .styles([if on {
                    bg(Role::SurfaceSelected)
                } else {
                    bg(Role::SurfaceHover).hover()
                }])
                .focusable(FocusId(PRESET_BASE + place as u64))
                .child(line(
                    preset.title(),
                    12,
                    1.4,
                    if on { Role::Text } else { Role::Muted },
                ))
        }))
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

/// The keymap page's body: the preset, then every action under its group with its keys.
pub(crate) fn rows(state: &WorkspaceState) -> Vec<Node> {
    let keymap = state.editor.keymap();
    let mut parts =
        vec![
        node()
            .styles([flex(), items_center(), justify_between(), gap(6), flex_none()])
            .child(
                node()
                    .styles([flex(), flex_col(), gap(0.5), min_w_px(0)])
                    .child(line("Base keymap", 13, 1.4, Role::Text))
                    .child(line(
                        "The keys start as another editor has them; your own keys go over them.",
                        12,
                        1.4,
                        Role::Muted,
                    )),
            )
            .child(presets(state.editor.keymap)),
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
    for action in ordered {
        if action.group() != group {
            flush(group, &mut rows, &mut parts);
            group = action.group();
        }
        let keys = keymap.keys_of(action);
        let own = state.editor.keys.contains_key(action.key());
        let clashes: Vec<&str> = keys
            .iter()
            .flat_map(|binding| keymap.clashes(action, binding))
            .map(Action::title)
            .collect();
        let recording = (state.recording == Some(action)).then_some(state.recorded.as_slice());
        let mut about = node()
            .styles([flex(), flex_col(), gap(0.5), min_w_px(0)])
            .child(line(action.title(), 13, 1.4, Role::Text));
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
    parts.push(node().styles([w_px(1.0), h(2), flex_none()]));
    parts
}
