//! Choosing another colour for a stretch of the file that spells one: the colour typed in hex, the ways the language server offers to write it, and the one picked written in place of the stretch.

use pm_input::FocusId;
use pm_protocol::{ColorPresentation, Span};
use pm_text::{Edit, Field};

use crate::state::{Outgoing, WorkspaceState};

/// The field the colour is typed in, as hex.
pub const COLOR_HEX: FocusId = FocusId(7_420_000);

/// Hands in the colour typed: the first way to write it is taken.
pub const COLOR_HEX_SUBMIT: FocusId = FocusId(7_420_001);

/// The box the colour is chosen in, which keeps a press on it from counting as outside.
pub const COLOR_BOX: FocusId = FocusId(7_420_002);

/// The first of the rows that each write the colour one way.
pub const COLOR_FORMAT_BASE: u64 = 7_420_100;

/// The end of the rows that each write the colour one way.
const COLOR_FORMAT_END: u64 = 7_420_200;

/// The first of the squares of colour in the file, one for each colour the language server said.
pub const COLOR_SWATCH_BASE: u64 = 7_421_000;

/// The end of the squares of colour in the file.
const COLOR_SWATCH_END: u64 = 7_423_000;

/// How many ways of writing the colour are listed.
pub const COLOR_FORMATS_SHOWN: usize = 8;

/// The way of writing the colour the row `id` stands for, counted from the first.
pub fn color_format_of(id: FocusId) -> Option<usize> {
    (COLOR_FORMAT_BASE..COLOR_FORMAT_END)
        .contains(&id.0)
        .then(|| (id.0 - COLOR_FORMAT_BASE) as usize)
}

/// The colour of the file the square `id` stands for, counted from the first.
pub fn color_swatch_of(id: FocusId) -> Option<usize> {
    (COLOR_SWATCH_BASE..COLOR_SWATCH_END)
        .contains(&id.0)
        .then(|| (id.0 - COLOR_SWATCH_BASE) as usize)
}

/// A colour of the file being changed.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorPicking {
    /// The session of the file, by position.
    pub session: usize,
    /// The file.
    pub path: String,
    /// The version of the file the stretch is in.
    pub version: i32,
    /// The stretch that spells the colour.
    pub span: Span,
    /// The colour as it is now, as red, green, blue and opacity from 0 to 255.
    pub color: [u8; 4],
    /// The colour as typed, in hex.
    pub hex: Field,
    /// The ways the language server offers to write the colour, as it last said.
    pub formats: Vec<ColorPresentation>,
    /// Where the box hangs, in logical pixels of the window.
    pub at: (f32, f32),
    /// Whether the language server has yet to say how the colour can be written.
    pub loading: bool,
    /// Why the language server could not say, when it could not.
    pub problem: Option<String>,
}

/// The colour as hex: `#rrggbb`, with two more digits for its opacity when it is not fully opaque.
pub fn hex_of(color: [u8; 4]) -> String {
    let [red, green, blue, alpha] = color;
    if alpha == 255 {
        format!("#{red:02x}{green:02x}{blue:02x}")
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
    }
}

/// The colour hex digits spell, with or without the `#`: three, four, six or eight of them.
pub fn color_of(text: &str) -> Option<[u8; 4]> {
    let digits = text.trim().trim_start_matches('#');
    if !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    let pair = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
    let single = |at: usize| {
        u8::from_str_radix(digits.get(at..at + 1)?, 16)
            .ok()
            .map(|n| n * 17)
    };
    match digits.len() {
        3 => Some([single(0)?, single(1)?, single(2)?, 255]),
        4 => Some([single(0)?, single(1)?, single(2)?, single(3)?]),
        6 => Some([pair(0)?, pair(2)?, pair(4)?, 255]),
        8 => Some([pair(0)?, pair(2)?, pair(4)?, pair(6)?]),
        _ => None,
    }
}

impl WorkspaceState {
    /// The byte of the file shown that the colour at `index` of its list starts at.
    pub fn color_swatch_offset(&self, index: usize) -> Option<usize> {
        self.shown_document()?
            .color_at(index)
            .map(|(range, _)| range.start)
    }

    /// Opens a box at `at` to change the colour at `index` of the list of colours of the file shown, and asks the language server how it can be written.
    pub fn color_picker_requested(mut self, index: usize, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self.shown_viewer().map(|viewer| viewer.path.clone()) else {
            return self;
        };
        let Some(document) = self.shown_document() else {
            return self;
        };
        let Some((range, color)) = document.color_at(index) else {
            return self;
        };
        let span = Span::new(
            document.position_of(range.start),
            document.position_of(range.end),
        );
        let version = document.version;
        let mut hex = Field::new(hex_of(color));
        hex.apply(Edit::SelectAll);
        self.color_picking = Some(ColorPicking {
            session,
            path: path.clone(),
            version,
            span,
            color,
            hex,
            formats: Vec::new(),
            at,
            loading: true,
            problem: None,
        });
        self.outbox.push(Outgoing::ColorPresentations {
            session,
            path,
            version,
            span,
            color,
        });
        self
    }

    /// Whether the box for changing a colour is open.
    pub fn color_picker_open(&self) -> bool {
        self.color_picking.is_some()
    }

    /// Closes the box for changing a colour.
    pub fn color_picker_closed(mut self) -> Self {
        self.color_picking = None;
        self
    }

    /// Takes `edit` to the hex typed in the box; when it spells a colour other than the one held, the language server is asked how that can be written.
    pub(crate) fn color_hex_edited(&mut self, edit: Edit) {
        let Some(picking) = self.color_picking.as_mut() else {
            return;
        };
        picking.hex.apply(edit);
        let Some(color) = color_of(picking.hex.text()).filter(|color| *color != picking.color)
        else {
            return;
        };
        picking.color = color;
        picking.loading = true;
        picking.problem = None;
        let request = Outgoing::ColorPresentations {
            session: picking.session,
            path: picking.path.clone(),
            version: picking.version,
            span: picking.span,
            color,
        };
        self.outbox.push(request);
    }

    /// The state after the language server said how `color` can be written in the file at `path` of the session at `session`; what it says of a colour that is no longer the one in the box is let go.
    pub fn with_color_presentations(
        mut self,
        session: usize,
        path: &str,
        color: [u8; 4],
        presentations: Vec<ColorPresentation>,
    ) -> Self {
        if let Some(picking) = self.color_picking.as_mut().filter(|held| {
            held.session == session && held.path == path && held.color == color && held.loading
        }) {
            picking.formats = presentations;
            picking.loading = false;
        }
        self
    }

    /// The state after asking the language server how a colour can be written failed for `problem`.
    pub fn color_picker_failed(mut self, problem: String) -> Self {
        if let Some(picking) = self.color_picking.as_mut() {
            picking.loading = false;
            picking.problem = Some(problem);
        }
        self
    }

    /// Writes the colour in the file shown in the way at `row` of the list, or the first when none is given, in place of the stretch that spells it, and closes the box; a file that changed since is left as it is.
    pub fn color_format_taken(mut self, row: Option<usize>) -> Self {
        let Some(picking) = self.color_picking.take() else {
            return self;
        };
        let Some(format) = picking.formats.get(row.unwrap_or(0)) else {
            self.color_picking = Some(picking);
            return self;
        };
        let current = self
            .shown_viewer()
            .is_some_and(|viewer| viewer.path == picking.path);
        let edits = self
            .shown_document()
            .filter(|document| current && document.version == picking.version)
            .map(|document| {
                let range = document.range_of(format.span);
                document.replacement_edits(
                    range.start,
                    range.end,
                    &format.text,
                    &format.additional_edits,
                )
            });
        self.file_edits(edits.unwrap_or_default());
        self
    }
}
