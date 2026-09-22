//! The small panel that says what the editor knows about one place.
//!
//! A hover, a signature and the text of a diagnostic are three things a
//! server says about where the cursor is and one way of showing them: a
//! panel beside the place, holding a few lines of plain text. It is not a
//! document view — a hover that runs to forty lines is a hover the reader
//! reads the top of and then goes to the definition.
//!
//! [`Shown`] is what the panel is drawn from. A fault the editor already
//! knows about goes up the moment the pointer stops on it, and what the
//! server says about the same place joins it when the answer arrives, so
//! pointing at a squiggle does not hide the type it is on.

use std::ops::Range;

use pm_gfx::Point;
use pm_text::Position;
use pm_ui::{Div, Styled, Theme, text, v_flex};

use crate::editor::FileId;
use crate::message::Message;

/// Widest the panel is drawn.
const WIDTH: f32 = 520.0;

/// Most lines of it shown, however much the server said.
const LINES: usize = 16;

/// What is being said about one place, and where it is being said.
#[derive(Clone, Debug, Default)]
pub struct Shown {
    /// Where the panel is drawn.
    pub at: Point,
    /// The fault under the place, which the editor knew without asking.
    pub fault: Option<String>,
    /// What a server said about it, once it has said anything.
    pub said: Option<String>,
    /// The word it is about, while it is the pointer that is asking.
    ///
    /// A panel about a word stays up for as long as the pointer is still on
    /// that word: reading a hover means crossing the name it is about, and
    /// one that went away at the first pixel of that would never be read.
    pub about: Option<(FileId, Range<Position>)>,
}

impl Shown {
    /// A panel at `at`, with nothing in it yet.
    pub fn at(at: Point) -> Self {
        Self {
            at,
            ..Self::default()
        }
    }

    /// Everything the panel has to say, the fault first.
    pub fn text(&self) -> String {
        [self.fault.as_deref(), self.said.as_deref()]
            .into_iter()
            .flatten()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Whether there is nothing to show yet.
    pub fn is_empty(&self) -> bool {
        self.text().trim().is_empty()
    }
}

/// Builds the panel saying `content`, beside the place it is about.
pub fn hint(theme: &Theme, content: &str) -> Div<Message> {
    let lines = readable(content)
        .take(LINES)
        .map(|line| {
            text(line)
                .text_sm()
                .font_mono()
                .color(theme.colors.text_muted)
        })
        .collect::<Vec<_>>();

    v_flex()
        .max_w_px(WIDTH)
        .px(1.5)
        .py(1)
        .gap(0.25)
        .items_stretch()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.md)
        .on_click(Message::DismissPopup)
        .children(lines)
}

/// What a server said, with the markdown it is written in taken off.
///
/// The panel draws plain lines, so the marks that would only be read as
/// themselves go: the fences around a signature, the rules between one
/// section and the next, and the blank lines those leave behind.
fn readable(content: &str) -> impl Iterator<Item = String> + '_ {
    let mut blank = true;
    content
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.starts_with("```"))
        .filter(|line| !is_rule(line))
        .filter(move |line| {
            let empty = line.is_empty();
            let keep = !(empty && blank);
            blank = empty;
            keep
        })
        .map(str::to_owned)
}

/// Whether `line` is a rule between sections rather than something to read.
fn is_rule(line: &str) -> bool {
    let line = line.trim();
    line.len() >= 3 && line.chars().all(|ch| matches!(ch, '-' | '_' | '*'))
}
