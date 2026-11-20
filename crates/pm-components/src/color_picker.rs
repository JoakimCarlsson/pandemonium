//! Choosing another colour for a stretch of the file that spells one: the colour typed in hex, the ways the language server offers to write it, and the one picked written in place of the stretch.

use pm_protocol::{ColorPresentation, Span};
use rustle::TextFieldState;

use crate::state::{Outgoing, WorkspaceState};

/// How many ways of writing the colour are listed.
pub const COLOR_FORMATS_SHOWN: usize = 8;

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
    pub hex: TextFieldState,
    /// The ways the language server offers to write the colour, as it last said.
    pub formats: Vec<ColorPresentation>,
    /// Where the box hangs, in logical pixels of the window.
    pub at: (f32, f32),
    /// Whether the language server has yet to say how the colour can be written.
    pub loading: bool,
    /// Why the language server could not say, when it could not.
    pub problem: Option<String>,
}

impl ColorPicking {
    /// The request asking the language server how the colour held can be written in the file.
    fn presentations(&self) -> Outgoing {
        Outgoing::ColorPresentations {
            session: self.session,
            path: self.path.clone(),
            version: self.version,
            span: self.span,
            color: self.color,
        }
    }
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
        let hex = TextFieldState::new(hex_of(color));
        hex.select_all();
        self.color_picking = Some(ColorPicking {
            session,
            path,
            version,
            span,
            color,
            hex,
            formats: Vec::new(),
            at,
            loading: true,
            problem: None,
        });
        self.outbox
            .extend(self.color_picking.as_ref().map(ColorPicking::presentations));
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

    /// Takes in the hex the reader typed in the box; when it spells a colour other than the one held, the language server is asked how that can be written.
    pub(crate) fn color_hex_typed(&mut self) {
        let Some(picking) = self.color_picking.as_mut() else {
            return;
        };
        let Some(color) = color_of(&picking.hex.text()).filter(|color| *color != picking.color)
        else {
            return;
        };
        picking.color = color;
        picking.loading = true;
        picking.problem = None;
        let request = picking.presentations();
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
