//! The pointer on code text drawn line by line: the press, drag and release of the
//! primary button that place the caret and select, and the pointer resting over a
//! character, each told to the app as a typed operation with the byte of the text under
//! the pointer, found in the [`TextLines`] the text was drawn into.

use crate::events::{self, OnPointer};
use crate::pointing::{CodePointer, Stroke};
use crate::target::Target;
use crate::text_lines::TextLines;
use std::cell::Cell;
use std::rc::Rc;

use rustle::{
    LOCAL_VIEW_CONFIGURATION, Modifier, PointerButton, PointerEvent, PointerInputPhase,
    PointerPhase, Rect,
};

/// What a control drawing the code text of `owner` into `lines` does with its own presses,
/// for its `on_pointer`: a press of the primary button tells the app the byte under the
/// pointer, how many presses in a row it makes and the modifier keys held.
pub fn on_press(owner: Target, lines: &TextLines) -> OnPointer {
    let operations = events::operations();
    let lines = lines.clone();
    let touch_slop = LOCAL_VIEW_CONFIGURATION.current().touch_slop;
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
            touch_slop,
        });
    })
}

/// The modifier that follows the primary button held after a press on the code text of
/// `owner` drawn into `lines`: each move, wherever the pointer is, and the release, told to
/// the app with the byte of the text nearest the pointer.
pub fn held(owner: Target, lines: &TextLines) -> Modifier {
    let operations = events::operations();
    let lines = lines.clone();
    let touch_slop = LOCAL_VIEW_CONFIGURATION.current().touch_slop;
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
            touch_slop,
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
