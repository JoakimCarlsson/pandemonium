//! The pointer on code text drawn line by line: the press, drag and release of the
//! primary button that place the caret and select, the selected text carried elsewhere by
//! rustle's drag and drop, and the pointer resting over a character, each told to the app
//! as a typed operation with the byte of the text under the pointer, found in the
//! [`TextLines`] the text was drawn into.

use crate::events::{self, OnPointer};
use crate::pointing::{CodePointer, Stroke};
use crate::target::Target;
use crate::text_lines::TextLines;
use std::cell::Cell;
use std::rc::Rc;

use rustle::{
    DragAndDropEvent, DragAndDropTarget, DragAndDropTransferData, Modifier, PointerButton,
    PointerEvent, PointerInputPhase, PointerPhase, Rect,
};

/// What a control drawing the code text of `owner` into `lines` does with its own presses,
/// for its `on_pointer`: a press of the primary button tells the app the byte under the
/// pointer, how many presses in a row it makes and the modifier keys held.
pub fn on_press(owner: Target, lines: &TextLines) -> OnPointer {
    let operations = events::operations();
    let lines = lines.clone();
    Rc::new(move |event: &PointerEvent| {
        if event.button != PointerButton::Primary || event.phase != PointerPhase::Press {
            return;
        }
        let at = events::window_at(event);
        operations.code_pointer(CodePointer {
            owner,
            offset: lines.offset_at_window(owner, at),
            at,
            stroke: Stroke::Down {
                clicks: event.click_count,
                modifiers: event.modifiers,
            },
        });
    })
}

/// The modifier that follows the primary button held after a press on the code text of
/// `owner` drawn into `lines`: each move, wherever the pointer is, and the release, told to
/// the app with the byte of the text nearest the pointer.
pub fn held(owner: Target, lines: &TextLines) -> Modifier {
    let operations = events::operations();
    let lines = lines.clone();
    Modifier::new().pointer_input(move |event| {
        let stroke = match event.phase {
            PointerInputPhase::Move => Stroke::Move,
            PointerInputPhase::Up | PointerInputPhase::Cancel => Stroke::Up,
            PointerInputPhase::Down | PointerInputPhase::Scroll => return false,
        };
        let at = (event.window_position.x, event.window_position.y);
        operations.code_pointer(CodePointer {
            owner,
            offset: lines.offset_at_window(owner, at),
            at,
            stroke,
        });
        false
    })
}

/// The modifier that tells the app which drawn character of the code text of `owner`, drawn
/// into `lines`, the pointer is over while it is over the area, and that it left.
pub fn hover(owner: Target, lines: &TextLines) -> Modifier {
    let operations = events::operations();
    let lines = lines.clone();
    let area: Rc<Cell<Rect>> = Rc::default();
    let placed = Rc::clone(&area);
    Modifier::new()
        .on_globally_positioned(move |rect| placed.set(rect))
        .on_pointer_move(move |local| {
            let origin = area.get().origin;
            let byte = local.and_then(|local| {
                lines.character_at_window(owner, (origin.x + local.x, origin.y + local.y))
            });
            operations.code_hovered(byte);
        })
}

/// What a drag of selected code text carries: the selection the press picked up, which the
/// app holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CarriedText;

/// The code text of `owner`, drawn into `lines`, as the place carried text is let go over:
/// a drag and drop target telling the app which byte the text is over and that the drag
/// ended, which puts the text down where it was last over.
struct TextDrop {
    /// The app's state holder.
    operations: Rc<dyn crate::events::Operations>,
    /// The field whose text it is.
    owner: Target,
    /// Where the text was drawn.
    lines: TextLines,
}

impl DragAndDropTarget for TextDrop {
    /// Takes the drop; the drag's end puts the text down.
    fn on_drop(&self, _: &DragAndDropEvent) -> bool {
        true
    }

    /// Tells the app the byte of the text nearest the pointer.
    fn on_moved(&self, event: &DragAndDropEvent) {
        let at = (event.position.x, event.position.y);
        if let Some(offset) = self.lines.offset_at_window(self.owner, at) {
            self.operations.text_dragged_over(offset);
        }
    }

    /// Tells the app the drag ended, with the keys held as it was let go.
    fn on_ended(&self, event: &DragAndDropEvent) {
        if event.payload::<CarriedText>().is_some() {
            self.operations.text_drag_ended(event.modifiers);
        }
    }
}

/// The modifier that carries the selected code text of `owner`, drawn into `lines`, to
/// another place with rustle's drag and drop: a drag from a press the app took as picking
/// up the selection starts once past the touch slop, the text follows the pointer over the
/// code, and letting go puts it down there.
pub fn text_drag(owner: Target, lines: &TextLines) -> Modifier {
    let operations = events::operations();
    let picking = Rc::clone(&operations);
    let lines = lines.clone();
    Modifier::new()
        .drag_and_drop_source(move |_| {
            picking
                .text_drag_started()
                .then(|| DragAndDropTransferData::new(CarriedText))
        })
        .drag_and_drop_target(
            |event| event.payload::<CarriedText>().is_some(),
            TextDrop {
                operations,
                owner,
                lines,
            },
        )
}
