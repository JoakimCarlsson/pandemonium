//! The text of an editable field drawn from its [`Field`] model: one line
//! scrolled sideways, or wrapped lines scrolled down, with the selection, the
//! carets and an input method's composition.
//!
//! The [`Field`] stays the source of truth: the host edits it with `Edit`
//! values on keys and on the [`Action::Field`](crate::kit::Action::Field)
//! actions this sends for presses and drags (the shown byte under the
//! pointer and the click count, for word and line reach). The field reports
//! its shaped text to the frame's [`Report`](crate::kit::Report), which
//! answers the questions the host asked the old frame output: the byte under
//! a point, the byte a line up or down, the ends of a line as drawn, and the
//! caret rectangle the input method is told about. The text shaped is the
//! whole shown text, so moving through lines that are not on screen works.
//! Wrapped text that is taller than the lines it shows is also a scrolling
//! box named by the field's id: it reports its room and turns the wheel into
//! [`Action::Scroll`](crate::kit::Action::Scroll), as the old field did.

use std::rc::Rc;

use pm_field::Field;
use rustle::{Modifier, Offset, PointerButton, PointerInputPhase, PointerPhase, TextLayout};

use crate::kit::bus::{self, Action, Phase};
use crate::kit::focus::FocusId;
use crate::kit::frame;
use crate::kit::keys::Modifiers;
use crate::kit::marks::{self, CaretLook};
use crate::kit::report::{self, FieldBox, Region, Scroller};
use crate::kit::text::Type;
use crate::kit::theme::{self, Role, Tone};

/// The text of a field being described: the model, what names it, how its
/// text is set, what it shows while empty, and whether it wraps.
#[derive(Clone, Debug)]
pub struct FieldText<'a> {
    /// The model.
    field: &'a Field,
    /// What names it.
    id: FocusId,
    /// How its text is set.
    kind: Type,
    /// What it shows while empty.
    placeholder: &'a str,
    /// The tone of the placeholder.
    placeholder_tone: Tone,
    /// The width its lines wrap at, when it wraps.
    wrap: Option<f32>,
    /// How many lines it shows at most before it scrolls, when it wraps.
    visible: Option<usize>,
    /// How its caret is drawn.
    look: CaretLook,
}

impl<'a> FieldText<'a> {
    /// The text of `field`, named `id`, set in `kind`, on one line.
    pub fn new(field: &'a Field, id: FocusId, kind: Type) -> Self {
        Self {
            field,
            id,
            kind,
            placeholder: "",
            placeholder_tone: Tone::from(Role::Subtle),
            wrap: None,
            visible: None,
            look: CaretLook::default(),
        }
    }

    /// Shows `placeholder` in `tone` while the field is empty, cut short at the
    /// edge of the field.
    pub fn placeholder(mut self, placeholder: &'a str, tone: impl Into<Tone>) -> Self {
        self.placeholder = placeholder;
        self.placeholder_tone = tone.into();
        self
    }

    /// Wraps the text at `width` logical pixels, its placeholder kept to one
    /// line cut short at that width, showing at most `visible`
    /// lines before it scrolls, scrolled by the field's own scroll and kept on
    /// the caret's line while the field follows its caret.
    pub fn wrap(mut self, width: f32, visible: Option<usize>) -> Self {
        self.wrap = Some(width.max(1.0));
        self.visible = visible;
        self
    }

    /// Draws the caret as `look` says.
    pub fn caret_look(mut self, look: CaretLook) -> Self {
        self.look = look;
        self
    }

    /// The shaped shown text, wrapped when the field wraps.
    pub fn layout(&self) -> TextLayout {
        let shown = self.field.shown();
        let style = match self.wrap {
            Some(_) => self.kind.wrap().style(),
            None => self.kind.style(),
        };
        rustle::layout_text(&shown, &style, self.wrap)
    }

    /// Emits the field's text with `modifier` around it.
    ///
    /// One line is as tall as a line and as wide as its box allows, its text
    /// scrolled sideways by the field's scroll; wrapped text is as tall as its
    /// lines, or as `visible` lines when there are more, scrolled down by the
    /// field's scroll. The caret, selection and composition show while the
    /// host's focus is on the field.
    #[track_caller]
    pub fn build(self, modifier: Modifier) {
        let id = self.id;
        let field = self.field;
        let shown = field.shown().into_owned();
        let empty = shown.is_empty();
        let line = self.kind.line_height();
        let kind = self.kind;
        let ink = if empty {
            kind.tone(self.placeholder_tone)
        } else {
            kind
        };
        let theme = theme::theme();
        let style = match self.wrap {
            Some(_) => ink.wrap().style_in(&theme),
            None => ink.style_in(&theme),
        };
        let drawn = match (empty, self.wrap) {
            (true, Some(width)) => {
                crate::kit::text::ellipsized(self.placeholder, &ink.style_in(&theme), width)
            }
            (true, None) => self.placeholder.to_owned(),
            (false, _) => shown.clone(),
        };
        let layout = Rc::new(rustle::layout_text(&drawn, &style, self.wrap));
        let reported = if empty {
            Rc::new(rustle::layout_text("", &style, self.wrap))
        } else {
            layout.clone()
        };
        let lines = layout.line_count().max(1);
        let height = match (self.wrap, self.visible) {
            (Some(_), Some(most)) => line * lines.min(most) as f32,
            (Some(_), None) => line * lines as f32,
            (None, _) => line,
        };
        let room = match self.wrap {
            Some(_) => (line * lines as f32 - height).max(0.0),
            None => 0.0,
        };
        let wraps = self.wrap.is_some();
        let caret = field.to_shown(field.caret());
        let caret_line = if empty {
            0
        } else {
            layout.line_of_offset(caret, rustle::Affinity::Downstream)
        };
        let (dx, dy) = match self.wrap {
            Some(_) => (
                0.0,
                field.scroll_for_lines(caret_line, lines, self.visible.unwrap_or(lines), line),
            ),
            None => (field.scroll(), 0.0),
        };
        let selection = field
            .selection()
            .map(|range| field.to_shown(range.start)..field.to_shown(range.end));
        let marked = field.marked();
        let look = self.look;
        let text_color = theme::color_in(&theme, kind.tone_of());
        let highlight = marks::selection_color(&theme);
        let size = style.font_size;
        let hit = reported.clone();
        if wraps {
            crate::kit::scroll::remember(id, room, dy, height);
        }
        let modifier = modifier
            .height(height)
            .clip()
            .on_placed_visible(move |rect| {
                report::record(|report| {
                    report.regions.push(Region {
                        id,
                        rect,
                        focusable: true,
                    })
                })
            })
            .on_placed(move |rect, _| {
                report::record(|report| {
                    report.fields.push(FieldBox {
                        id,
                        layout: reported.clone(),
                        origin: Offset::new(rect.origin.x - dx, rect.origin.y - dy),
                        view: rect,
                    });
                    if wraps {
                        report.scrollers.push(Scroller {
                            id,
                            rect,
                            room,
                            offset: dy,
                        });
                    }
                });
            })
            .pointer_input(move |event| {
                if !wraps || room <= 0.0 || event.phase != PointerInputPhase::Scroll {
                    return false;
                }
                bus::send(Action::Scroll {
                    id,
                    pixels: -event.scroll_delta.y,
                    room,
                });
                true
            })
            .on_pointer_button(PointerButton::Primary, move |event| {
                let phase = match event.phase {
                    PointerPhase::Press => Phase::Start,
                    PointerPhase::Drag => Phase::Move,
                    PointerPhase::Release => Phase::End,
                    PointerPhase::Click => return bus::send(Action::Press(id)),
                };
                let offset =
                    hit.hit_test(Offset::new(event.position.x + dx, event.position.y + dy));
                bus::send(Action::Field {
                    id,
                    phase,
                    offset,
                    clicks: event.click_count,
                    at: (event.window_position.x, event.window_position.y),
                    modifiers: Modifiers::from_window(event.modifiers),
                });
            })
            .on_secondary_click(move |event| {
                bus::send(Action::Menu {
                    id,
                    at: (event.window_position.x, event.window_position.y),
                })
            })
            .cursor(rustle::CursorIcon::Text);
        rustle::canvas(modifier, move |canvas| {
            let (x, y) = (-dx, -dy);
            let focused = frame::is_focused(id);
            if focused
                && !empty
                && let Some(range) = selection.clone()
            {
                marks::draw_fill(canvas, &layout, (x, y), range, highlight);
            }
            let first = layout.line_at_y(dy);
            let last = layout.line_at_y(dy + canvas.height());
            for index in first..=last.min(layout.line_count().saturating_sub(1)) {
                let line = &layout.lines()[index];
                let piece = &drawn[line.start..line.end.min(drawn.len())];
                let single = rustle::TextStyle {
                    wrap: rustle::TextWrap::None,
                    max_lines: None,
                    ..style
                };
                let piece = if empty && !wraps {
                    std::borrow::Cow::Owned(crate::kit::text::ellipsized(
                        piece,
                        &single,
                        canvas.width(),
                    ))
                } else {
                    std::borrow::Cow::Borrowed(piece)
                };
                canvas.draw_text(x + line_left(line), y + line.top, &piece, &single, None);
            }
            if !focused {
                return;
            }
            if let Some(range) = marked.clone() {
                marks::draw_underline(canvas, &layout, (x, y), range, size, false, text_color);
            }
            if marks::caret_lit(look) {
                let at = if empty { 0 } else { caret };
                marks::draw_caret(canvas, &layout, (x, y), at, look.shape, text_color);
            }
        });
    }
}

/// Where a shaped line starts across its box: the left of its leftmost cluster, or nought when it is empty.
fn line_left(line: &rustle::LineLayout) -> f32 {
    line.clusters
        .iter()
        .map(|cluster| cluster.x)
        .reduce(f32::min)
        .unwrap_or(0.0)
}
