//! The prompts above the prompt box that wait for the agent's turn to end, each of which can be read in full, edited in place, sent now or removed.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_1, flex_col, flex_none, gap, items_center,
    items_start, mb, min_w_px, pl, pr, pt, py, rounded, w_full,
};
use pm_text::{Field, Shaper};
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::parts::{glyph, line, paragraph};
use crate::sample::{Queued, Session};
use crate::size::Size;
use crate::state::{HURRY_BASE, REMOVE_QUEUED_BASE, WorkspaceState, prompt_field};
use crate::{Button, Input, input};

/// The field the text of a waiting prompt is edited in.
pub const QUEUED_FIELD: FocusId = FocusId(7_402_300);

/// Keeps the edit of a waiting prompt.
pub const QUEUED_SAVE: FocusId = FocusId(7_402_301);

/// Throws away the edit of a waiting prompt.
pub const QUEUED_CANCEL: FocusId = FocusId(7_402_302);

/// The first of the buttons opening a waiting prompt for editing; the rest follow it.
pub const EDIT_QUEUED_BASE: u64 = 4_901_000_000_000;

/// How wide the three buttons beside the text of a waiting prompt are together, in logical pixels, which the text leaves free.
const BESIDE_TEXT: f32 = 240.0;

/// How wide the count of files beside the text of a waiting prompt is, in logical pixels.
const FILE_COUNT: f32 = 60.0;

/// How wide the buttons beside the field a waiting prompt is edited in are together, in logical pixels.
const BESIDE_FIELD: f32 = 170.0;

/// How much of a row's width its border, padding, glyph and gaps take, in logical pixels.
const CHROME: f32 = 50.0;

/// A waiting prompt whose text is being edited in place.
#[derive(Clone, Debug, PartialEq)]
pub struct EditingQueued {
    /// The session it waits in, by the scope that names it however the sessions are renumbered.
    pub scope: u64,
    /// The prompt, by the id the server gave it.
    pub id: u64,
    /// The text being edited.
    pub field: Field,
}

impl WorkspaceState {
    /// Opens the waiting prompt at `index` of the open session for editing, with its text in a field of its own.
    pub(crate) fn open_queued_edit(&mut self, index: usize) {
        let Some(session) = self.open() else {
            return;
        };
        let Some(queued) = session.queued.get(index) else {
            return;
        };
        self.editing_queued = Some(EditingQueued {
            scope: session.scope,
            id: queued.id,
            field: prompt_field(&queued.text),
        });
    }

    /// Hands the server the edited text of the waiting prompt and closes the edit, or leaves it open when the text is empty and no file goes with the prompt. An edit that changed nothing is closed without asking anything.
    pub(crate) fn save_queued_edit(&mut self) {
        let Some(editing) = self.editing_queued.as_ref() else {
            return;
        };
        let Some(session) = self
            .sessions
            .iter()
            .position(|held| held.scope == editing.scope)
        else {
            self.editing_queued = None;
            return;
        };
        let Some(queued) = self.sessions[session]
            .queued
            .iter()
            .find(|queued| queued.id == editing.id)
        else {
            self.editing_queued = None;
            return;
        };
        let text = editing.field.text().trim().to_owned();
        if text.is_empty() && queued.files.is_empty() {
            return;
        }
        let (id, changed) = (queued.id, text != queued.text);
        self.editing_queued = None;
        if changed {
            self.outbox.push(crate::state::Outgoing::EditQueued {
                session,
                queued: id,
                text,
            });
        }
    }
}

/// A button showing only `icon`, saying what it does with `tooltip`, answering to `id`.
fn icon_button(icon: IconName, tooltip: &str, id: FocusId) -> Node {
    Button::icon(icon, id)
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .build()
        .tooltip(tooltip)
}

/// The box a row of a waiting prompt is drawn in, its contents aligned to the top.
fn frame() -> Node {
    node().styles([
        flex(),
        items_start(),
        gap(2),
        w_full(),
        pl(3),
        pr(1),
        py(1),
        rounded(Radius::Lg),
        border(),
        border_color(Role::Border),
        bg(Role::Surface),
    ])
}

/// One waiting prompt, at `index`, in a row `width` logical pixels wide: all its text wrapped, how many files go with it and buttons to edit it, send it now or remove it.
fn waiting(index: usize, queued: &Queued, width: f32, shaper: &mut Shaper) -> Node {
    let counted = if queued.files.is_empty() {
        0.0
    } else {
        FILE_COUNT
    };
    let room = (width - CHROME - BESIDE_TEXT - counted).max(120.0);
    let mut buttons = node()
        .styles([flex(), items_center(), gap(1), flex_none()])
        .child(icon_button(
            IconName::Pencil,
            "Edit",
            FocusId(EDIT_QUEUED_BASE + index as u64),
        ));
    buttons = buttons
        .child(
            Button::new("Send now", FocusId(HURRY_BASE + index as u64))
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .with_icon(IconName::Zap)
                .build(),
        )
        .child(
            Button::new("Remove", FocusId(REMOVE_QUEUED_BASE + index as u64))
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .with_icon(IconName::Trash)
                .build(),
        );
    let mut row = frame()
        .child(
            node()
                .styles([pt(1)])
                .child(glyph(IconName::Progress, 3.5, Role::Muted)),
        )
        .child(
            node()
                .styles([flex_1(), min_w_px(0), pt(1)])
                .child(paragraph(&queued.text, 13, 1.5, Role::Text, room, shaper)),
        );
    if !queued.files.is_empty() {
        let count = queued.files.len();
        let files = if count == 1 { "file" } else { "files" };
        row = row.child(node().styles([flex_none(), pt(1.5)]).child(line(
            format!("{count} {files}"),
            12,
            1.4,
            Role::Muted,
        )));
    }
    row.child(buttons)
}

/// The waiting prompt being edited, in a row `width` logical pixels wide: its text in a field that wraps, and buttons to keep the edit or throw it away.
fn editing(editing: &EditingQueued, width: f32, shaper: &mut Shaper) -> Node {
    let room = (width - CHROME - BESIDE_FIELD).max(120.0);
    frame()
        .child(
            node()
                .styles([pt(1)])
                .child(glyph(IconName::Pencil, 3.5, Role::Muted)),
        )
        .child(
            node().styles([flex_1(), min_w_px(0)]).child(
                Input::new(&editing.field, QUEUED_FIELD)
                    .variant(input::Variant::Multiline)
                    .placeholder("Prompt")
                    .wrap(input::inner_width(room), shaper)
                    .build(),
            ),
        )
        .child(
            node()
                .styles([flex(), items_center(), gap(1), flex_none()])
                .child(
                    Button::new("Save", QUEUED_SAVE)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .with_icon(IconName::Check)
                        .build(),
                )
                .child(
                    Button::new("Cancel", QUEUED_CANCEL)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .with_icon(IconName::Close)
                        .build(),
                ),
        )
}

/// The prompts waiting for `session`'s agent, oldest first, in a column `width` logical pixels wide, or nothing when none waits.
pub fn queued(
    state: &WorkspaceState,
    session: &Session,
    width: f32,
    shaper: &mut Shaper,
) -> Option<Node> {
    if session.queued.is_empty() {
        return None;
    }
    let editing_id = state
        .editing_queued
        .as_ref()
        .filter(|open| open.scope == session.scope)
        .map(|open| open.id);
    let mut rows = node().styles([flex(), flex_col(), gap(1), mb(2)]);
    for (index, waiting_prompt) in session.queued.iter().enumerate() {
        let open = state
            .editing_queued
            .as_ref()
            .filter(|_| editing_id == Some(waiting_prompt.id));
        rows = rows.child(match open {
            Some(open) => editing(open, width, shaper),
            None => waiting(index, waiting_prompt, width, shaper),
        });
    }
    Some(rows)
}
