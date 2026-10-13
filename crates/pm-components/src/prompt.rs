//! The lines of text of the prompt box: the prompt wrapped to the room it has, with the caret and selection placed in them.

use pm_style::{
    Role, cursor_text, flex, flex_col, flex_none, leading_ratio, min_h, min_w_px, pb, pt, px,
    text_color, text_px, w_full,
};
use pm_text::Shaper;
use pm_ui::{Node, node, text, wrap};

use crate::state::PROMPT;

/// How tall a line of the prompt is as a share of its type size.
const LINE_RATIO: f32 = 1.57;

/// How tall a line of the prompt is, in logical pixels.
const LINE: f32 = 22.0;

/// How tall the box is above and below its lines together, in logical pixels.
const PADDING: f32 = 18.0;

/// How many lines the box shows at most; a longer prompt scrolls to keep the caret in view.
const MAX_LINES: usize = 8;

/// The size and leading every line of the prompt is set in.
fn typography() -> [pm_style::Utility; 2] {
    [text_px(14), leading_ratio(LINE_RATIO)]
}

/// The prompt in `field`, wrapped to `width` logical pixels, as a box at least `rows` lines tall.
pub fn prompt(
    field: &pm_text::Field,
    placeholder: &str,
    rows: u32,
    width: f32,
    shaper: &mut Shaper,
) -> Node {
    let content = field.text();
    let lines = wrap(shaper, content, typography(), width);
    let caret = field.caret();
    let caret_line = lines
        .iter()
        .position(|line| caret < line.next)
        .unwrap_or(lines.len() - 1);
    let first = (caret_line + 1).saturating_sub(MAX_LINES);
    let selection = field.selection();
    let mut column = node()
        .styles([
            flex(),
            flex_col(),
            w_full(),
            min_w_px(0),
            px(4),
            pt(3.5),
            pb(1),
            min_h((rows as f32 * LINE + PADDING) / 4.0),
            cursor_text(),
        ])
        .focusable(PROMPT);
    if content.is_empty() {
        return column.child(
            text(placeholder)
                .styles(typography())
                .styles([text_color(Role::Subtle), flex_none()])
                .caret(0, PROMPT)
                .field_line(PROMPT, 0),
        );
    }
    for (index, line) in lines.iter().enumerate().skip(first).take(MAX_LINES) {
        let mut row = text(&content[line.start..line.end])
            .styles(typography())
            .styles([text_color(Role::Text), flex_none()])
            .field_line(PROMPT, line.start);
        if index == caret_line {
            row = row.caret(caret - line.start, PROMPT);
        }
        if let Some(range) = &selection {
            let (low, high) = (range.start.max(line.start), range.end.min(line.end));
            if low < high {
                row = row.selection(low - line.start..high - line.start, PROMPT);
            }
        }
        column = column.child(row);
    }
    column
}

/// How wide the lines of a prompt box `outer` logical pixels wide are, inside its border and padding.
pub fn inner_width(outer: f32) -> f32 {
    (outer - 2.0 - 32.0).max(40.0)
}
