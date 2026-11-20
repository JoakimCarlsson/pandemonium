//! The keymap page of the settings panel: the preset the keys start from, every action with its keys, and the reader's own keys recorded over the preset's.

use crate::areas::SettingsEvent;
use crate::kit;
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use pm_editor::{Action, shown};
use rustle::Modifier;

use crate::theme::{Radius, Role};

use crate::input::Input;
use crate::parts::line;
use crate::settings_catalog::Setting;
use crate::settings_rows::{KEYMAP_CONTROLS, PER_ACTION};
use crate::settings_servers::boxed;
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

/// Every control of an action, in the order of their places among the action's controls.
const PARTS: [Part; 3] = [Part::Record, Part::Reset, Part::Clear];

/// The name of the control `part` of `action`, or nothing when the keymap page has no room for the action.
fn id_of(action: Action, part: Part) -> Option<Target> {
    let place = Action::ALL.iter().position(|held| *held == action)?;
    let offset = PARTS.iter().position(|held| *held == part)?;
    KEYMAP_CONTROLS.cell(place, offset, PER_ACTION)
}

/// The action and the control of it that `id` names.
fn control_of(id: Target) -> Option<(Action, Part)> {
    let (place, offset) = KEYMAP_CONTROLS.cell_of(id, PER_ACTION)?;
    Some((*Action::ALL.get(place)?, *PARTS.get(offset)?))
}

impl WorkspaceState {
    /// Carries out pressing the control `id` of the keymap page, and says whether it was one.
    pub(crate) fn keymap_pressed(&mut self, id: Target) -> bool {
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

/// Emits a small button of a row saying `words`, answering to `id`.
#[track_caller]
fn small(words: &str, id: Option<Target>) {
    let own = Modifier::new().height(theme::sp(6)).indicated(
        theme::InteractionTones::new()
            .hover(Role::SurfaceHover)
            .radius(Radius::Sm),
        theme::Focused::Rustle,
    );
    kit::row(
        kit::pressable(own, id).padding_xy(theme::sp(2), 0.0),
        0,
        || line(words, 11, 1.4, Role::Muted),
    );
}

/// Emits the keys of `action`, each as a key cap, or what recording asks for while it records, answering to `id`.
fn caps(keys: &[String], recording: Option<&[String]>, id: Option<Target>) {
    let edge = if recording.is_some() {
        Role::Accent
    } else {
        Role::Border
    };
    let own = Modifier::new()
        .height(theme::sp(7))
        .background_rounded(theme::color(Role::Background), Radius::Sm.px())
        .border(1.0, theme::color(edge), Radius::Sm.px())
        .padding(1.0);
    let code = kit::code(12, Role::Text);
    kit::row(
        kit::pressable(own, id).padding_xy(theme::sp(1.5), 0.0),
        1,
        move || {
            if let Some(so_far) = recording {
                let said = if so_far.is_empty() {
                    "Press keys…".to_owned()
                } else {
                    format!("{} …", shown(&so_far.join(" ")))
                };
                kit::text(Modifier::new(), said, code);
            } else if keys.is_empty() {
                line("Not bound", 12, 1.4, Role::Subtle);
            } else {
                for (place, binding) in keys.iter().enumerate() {
                    if place > 0 {
                        line("or", 11, 1.4, Role::Subtle);
                    }
                    kit::text(Modifier::new(), shown(binding), code);
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
    Input::search(
        &state.settings_page.keymap_find,
        Target::Settings(SettingsEvent::SettingsKeymapFind),
    )
    .placeholder("Search actions by name or keys")
    .build();
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
    let mut any = false;
    for group in groups {
        let mut members = Vec::new();
        for action in Action::ALL
            .into_iter()
            .filter(|action| action.group() == group)
        {
            let keys = keymap.keys_of(action);
            if !found(action, &keys, &needle) {
                continue;
            }
            let clashes: Vec<&'static str> = keys
                .iter()
                .flat_map(|binding| keymap.clashes(action, binding))
                .map(Action::title)
                .collect();
            members.push(Found {
                action,
                keys,
                own: state.editor.keys.contains_key(action.key()),
                clashes,
                recording: (state.recording == Some(action)).then_some(state.recorded.as_slice()),
            });
        }
        if !members.is_empty() {
            any = true;
            section(group, &members);
        }
    }
    if !any {
        line("No action matches.", 13, 1.5, Role::Muted);
    }
    rustle::spacer(Modifier::new().width(1.0).height(theme::sp(2)));
}

/// Emits the heading `group` over a bordered box holding the row of each of `members`, ruled between them.
#[track_caller]
fn section(group: &str, members: &[Found]) {
    kit::text(Modifier::new(), group, kit::strong(13, Role::Text));
    boxed(|| {
        for (place, entry) in members.iter().enumerate() {
            rustle::key(place, || action_row(entry));
        }
    });
}

/// Emits the row of `entry`: its title and clashes, then its keys and the controls that reset or clear them.
fn action_row(entry: &Found) {
    let action = entry.action;
    kit::row(
        Modifier::new().padding_xy(theme::sp(4), theme::sp(2)),
        6,
        || {
            kit::column(Modifier::new().weight(1.0), 0.5, || {
                line(action.title(), 13, 1.4, Role::Text);
                if !entry.clashes.is_empty() {
                    line(
                        format!("Also bound to {}", entry.clashes.join(", ")),
                        11,
                        1.4,
                        Role::Warning,
                    );
                }
            });
            kit::row(Modifier::new(), 1, || {
                caps(&entry.keys, entry.recording, id_of(action, Part::Record));
                if entry.own {
                    small("Reset", id_of(action, Part::Reset));
                }
                if !entry.keys.is_empty() {
                    small("Clear", id_of(action, Part::Clear));
                }
            });
        },
    );
}
