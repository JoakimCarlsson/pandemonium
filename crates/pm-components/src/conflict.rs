//! The resolver of a file that a merge, a rebase or a cherry-pick stopped on: each place where the two sides disagree with both sides shown and a choice to take ours, theirs or both, and under them the result, which is the file itself, edited by hand, until the file is marked resolved and staged.

use std::ops::Range;

use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, bg, border, border_b, border_color, code_px, flex, flex_1, flex_col,
    flex_none, font_medium, font_mono, gap, h_px, items_center, leading_ratio, min_h, min_w_px,
    overflow_hidden, p, px, rounded, text_color, truncate,
};
use pm_text::Edit;
use pm_ui::{Node, node, text};

use crate::Button;
use crate::button::{Size as ButtonSize, Variant as ButtonVariant};
use crate::excerpts::{Block, EXCERPTS_SCROLL, ExcerptFile, Source, plain_block};
use crate::open_files::Contents;
use crate::parts::{line, spacer};
use crate::state::{Outgoing, WorkspaceState};

/// Takes everything of the file from our side.
pub const CONFLICT_OURS_ALL: FocusId = FocusId(7_414_001);

/// Takes everything of the file from their side.
pub const CONFLICT_THEIRS_ALL: FocusId = FocusId(7_414_002);

/// Writes the result and stages the file.
pub const CONFLICT_CONCLUDE: FocusId = FocusId(7_414_003);

/// The first of the buttons that settle one conflict, three for each in the order of the file: ours, theirs and both.
pub const CONFLICT_CHOICE_BASE: u64 = 7_414_100;

/// One past the last of the buttons that settle one conflict.
const CONFLICT_CHOICE_END: u64 = 7_419_000;

/// How tall the strip above the conflicts is, in logical pixels.
const STRIP: f32 = 44.0;

/// How tall the row naming a side or the result is, in logical pixels.
const LABEL: f32 = 24.0;

/// How tall the row of buttons under a conflict is, in logical pixels.
const BUTTONS: f32 = 40.0;

/// The space around a conflict, in logical pixels.
const SPACE: f32 = 10.0;

/// How many lines of a side are shown before it is cut short.
const MOST_LINES: usize = 12;

/// How wide the room must be for the two sides of a conflict to stand beside each other, in logical pixels.
const BESIDE: f32 = 760.0;

/// How far beyond the view, above and below, rows are still built, in logical pixels.
const OVERSCAN: f32 = 200.0;

/// The marker that starts the lines of our side.
const OURS_MARK: &str = "<<<<<<<";

/// The marker that starts the lines of the common ancestor, which a merge written in the style that shows it has.
const BASE_MARK: &str = "|||||||";

/// The marker that parts our side from theirs.
const PART_MARK: &str = "=======";

/// The marker that ends the lines of their side.
const THEIRS_MARK: &str = ">>>>>>>";

/// Which of the sides a conflict is settled with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Choice {
    /// Our side only.
    Ours,
    /// Their side only.
    Theirs,
    /// Our side, then theirs.
    Both,
}

impl Choice {
    /// Every choice, in the order of their buttons.
    const ALL: [Self; 3] = [Self::Ours, Self::Theirs, Self::Both];

    /// What the button for the choice says.
    const fn label(self) -> &'static str {
        match self {
            Self::Ours => "Take ours",
            Self::Theirs => "Take theirs",
            Self::Both => "Take both",
        }
    }
}

/// A place in a text where the two sides disagree, as the markers git wrote mark it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Region {
    /// Where its first marker starts and where its last marker's line ends, as byte offsets.
    pub span: Range<usize>,
    /// The line its first marker is on, counted from 0.
    pub line: usize,
    /// What our side holds, line breaks included.
    pub ours: String,
    /// What their side holds, line breaks included.
    pub theirs: String,
    /// What the common ancestor held, when the markers say.
    pub base: Option<String>,
    /// What our marker names, such as `HEAD`.
    pub ours_name: String,
    /// What their marker names, such as the branch merged in.
    pub theirs_name: String,
}

impl Region {
    /// What stands in the place of the region once it is settled with `choice`.
    fn taken(&self, choice: Choice) -> String {
        match choice {
            Choice::Ours => self.ours.clone(),
            Choice::Theirs => self.theirs.clone(),
            Choice::Both => format!("{}{}", self.ours, self.theirs),
        }
    }
}

/// Which part of a conflict the line being read is in.
#[derive(Clone, Copy, PartialEq)]
enum Part {
    /// Not inside one.
    Outside,
    /// Between the first marker and the one that parts the sides or starts the ancestor.
    Ours,
    /// Between the ancestor's marker and the one that parts the sides.
    Base,
    /// Between the one that parts the sides and the last.
    Theirs,
}

/// What follows `mark` on `line`, when `line` starts with it as a marker does: alone or followed by a space.
fn marker<'a>(line: &'a str, mark: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(mark)?;
    (rest.is_empty() || rest.starts_with(' ')).then(|| rest.trim())
}

/// The places in `text` where the two sides disagree, in the order of the text.
pub fn regions_in(text: &str) -> Vec<Region> {
    let mut found = Vec::new();
    let mut part = Part::Outside;
    let mut started = (0, 0);
    let mut open = Region {
        span: 0..0,
        line: 0,
        ours: String::new(),
        theirs: String::new(),
        base: None,
        ours_name: String::new(),
        theirs_name: String::new(),
    };
    let mut offset = 0;
    for (number, whole) in text.split_inclusive('\n').enumerate() {
        let here = offset;
        offset += whole.len();
        let line = whole.trim_end_matches(['\n', '\r']);
        if let Some(name) = marker(line, OURS_MARK) {
            part = Part::Ours;
            started = (here, number);
            open = Region {
                span: here..here,
                line: number,
                ours: String::new(),
                theirs: String::new(),
                base: None,
                ours_name: name.to_owned(),
                theirs_name: String::new(),
            };
            continue;
        }
        match (part, line) {
            (Part::Outside, _) => {}
            (Part::Ours, _) if marker(line, BASE_MARK).is_some() => {
                part = Part::Base;
                open.base = Some(String::new());
            }
            (Part::Ours | Part::Base, PART_MARK) => part = Part::Theirs,
            (Part::Theirs, _) if marker(line, THEIRS_MARK).is_some() => {
                open.theirs_name = marker(line, THEIRS_MARK).unwrap_or_default().to_owned();
                open.span = started.0..offset;
                open.line = started.1;
                found.push(open.clone());
                part = Part::Outside;
            }
            (Part::Ours, _) => open.ours.push_str(whole),
            (Part::Base, _) => {
                if let Some(base) = open.base.as_mut() {
                    base.push_str(whole);
                }
            }
            (Part::Theirs, _) => open.theirs.push_str(whole),
        }
    }
    found
}

/// What the resolver of one file holds besides the file itself, which is edited as an open file.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolver {
    /// The file, from the root of the project.
    pub path: String,
    /// The file as our side has it, nothing when it has none.
    pub ours: Option<String>,
    /// The file as their side has it, nothing when it has none.
    pub theirs: Option<String>,
    /// The file as the two sides last had it in common, nothing when they had none.
    pub base: Option<String>,
}

/// Whether `id` is something of the resolver the state handles when it is pressed.
pub fn is_conflict_part(id: FocusId) -> bool {
    (CONFLICT_OURS_ALL.0..CONFLICT_CHOICE_END).contains(&id.0)
}

/// How the two sides of a conflict lie.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Variant {
    /// Ours and theirs side by side, for a pane with room.
    SideBySide,
    /// Ours above theirs, for a narrow pane.
    Stacked,
}

/// How big the resolver's strip and conflicts are set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Size {
    /// Buttons 32 logical pixels tall, in snug cards.
    Sm,
    /// Buttons 36 logical pixels tall.
    #[default]
    Md,
}

impl Size {
    /// The size of the buttons.
    const fn button(self) -> ButtonSize {
        match self {
            Self::Sm => ButtonSize::Sm,
            Self::Md => ButtonSize::Default,
        }
    }
}

/// The resolver being described: the room it has, how the sides lie and how big it is.
///
/// Build it with [`Conflict::new`], change it with the methods that follow,
/// and turn it into a node with [`Conflict::build`].
#[derive(Clone, Copy)]
pub struct Conflict<'a> {
    /// The state the file and the sides are drawn from.
    state: &'a WorkspaceState,
    /// How tall the room for the resolver is, in logical pixels.
    height: f32,
    /// How the two sides of a conflict lie.
    variant: Variant,
    /// How big it is.
    size: Size,
}

/// The conflict under `region`'s place among those of the text, as its card is drawn.
struct Card<'a> {
    /// The conflict.
    region: &'a Region,
    /// Its place among the conflicts, counted from 0.
    place: usize,
    /// How many conflicts there are.
    count: usize,
}

impl<'a> Conflict<'a> {
    /// The resolver of the file `state` holds the excerpts of, in a room `width` wide and `height` tall, its two sides beside each other when the room allows.
    pub fn new(state: &'a WorkspaceState, width: f32, height: f32) -> Self {
        Self {
            state,
            height,
            variant: if width >= BESIDE {
                Variant::SideBySide
            } else {
                Variant::Stacked
            },
            size: Size::default(),
        }
    }

    /// The resolver with the sides lying as `variant` says.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The resolver `size` big.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// How tall the lines of a side are shown, in lines, cut short at the most.
    fn side_lines(text: &str) -> usize {
        let count = text.lines().count();
        count.min(MOST_LINES) + usize::from(count > MOST_LINES)
    }

    /// How tall the card of `region` is, in logical pixels.
    fn card_height(&self, region: &Region, row: f32) -> f32 {
        let (ours, theirs) = (
            Self::side_lines(&region.ours),
            Self::side_lines(&region.theirs),
        );
        let sides = match self.variant {
            Variant::SideBySide => LABEL + ours.max(theirs).max(1) as f32 * row,
            Variant::Stacked => 2.0 * LABEL + (ours.max(1) + theirs.max(1)) as f32 * row,
        };
        LABEL + sides + BUTTONS + SPACE
    }

    /// One side of a conflict: what it is called, and its lines in the code font on a tint, the first few of them.
    fn side(&self, name: &str, text_of: &str, tint: Role, row: f32) -> Node {
        let font = self.state.editor.font_size;
        let shown = text_of.lines().take(MOST_LINES).map(|content| {
            node()
                .styles([flex(), items_center(), h_px(row), flex_none(), px(2)])
                .child(text(crate::diff_syntax::shown(content)).styles([
                    code_px(font),
                    leading_ratio(1.4),
                    font_mono(),
                    text_color(Role::Text),
                    truncate(),
                ]))
        });
        let more = text_of.lines().count().saturating_sub(MOST_LINES);
        let cut = (more > 0).then(|| {
            node()
                .styles([flex(), items_center(), h_px(row), flex_none(), px(2)])
                .child(line(format!("… {more} more lines"), 11, 1.4, Role::Subtle))
        });
        node()
            .styles([flex(), flex_col(), flex_1(), min_w_px(0), overflow_hidden()])
            .child(
                node()
                    .styles([flex(), items_center(), h_px(LABEL), flex_none(), px(2)])
                    .child(line(name, 11, 1.4, Role::Muted)),
            )
            .child(
                node()
                    .styles([
                        flex(),
                        flex_col(),
                        flex_1(),
                        min_h(0),
                        bg(Color::from(tint).alpha(0.12)),
                    ])
                    .children(shown)
                    .children(cut),
            )
    }

    /// The card of one conflict: where it is, both sides, and the buttons that settle it.
    fn card(&self, card: &Card, row: f32) -> Node {
        let region = card.region;
        let ours = format!("Ours · {}", name_or(&region.ours_name, "HEAD"));
        let theirs = format!("Theirs · {}", name_or(&region.theirs_name, "incoming"));
        let sides = match self.variant {
            Variant::SideBySide => node()
                .styles([flex(), gap(2), px(3)])
                .child(self.side(&ours, &region.ours, Role::Success, row))
                .child(self.side(&theirs, &region.theirs, Role::Warning, row)),
            Variant::Stacked => node()
                .styles([flex(), flex_col(), px(3)])
                .child(self.side(&ours, &region.ours, Role::Success, row))
                .child(self.side(&theirs, &region.theirs, Role::Warning, row)),
        };
        let buttons = Choice::ALL.into_iter().enumerate().map(|(step, choice)| {
            Button::new(
                choice.label(),
                FocusId(CONFLICT_CHOICE_BASE + card.place as u64 * 3 + step as u64),
            )
            .variant(if choice == Choice::Both {
                ButtonVariant::Ghost
            } else {
                ButtonVariant::Outline
            })
            .size(self.size.button())
            .build()
        });
        node()
            .styles([
                flex(),
                flex_col(),
                flex_none(),
                h_px(self.card_height(region, row) - SPACE),
                rounded(Radius::Lg),
                border(),
                border_color(Role::Border),
                overflow_hidden(),
            ])
            .child(
                node()
                    .styles([
                        flex(),
                        items_center(),
                        h_px(LABEL),
                        flex_none(),
                        px(3),
                        bg(Role::Surface),
                        border_b(),
                        border_color(Role::Border),
                    ])
                    .child(
                        line(
                            format!(
                                "Conflict {} of {} · line {}",
                                card.place + 1,
                                card.count,
                                region.line + 1
                            ),
                            12,
                            1.4,
                            Role::Text,
                        )
                        .styles([font_medium()]),
                    ),
            )
            .child(sides)
            .child(
                node()
                    .styles([
                        flex(),
                        items_center(),
                        gap(2),
                        h_px(BUTTONS),
                        flex_none(),
                        px(3),
                    ])
                    .children(buttons),
            )
    }

    /// The strip above the conflicts: the file, how many conflicts are left, and what can be done with all of them at once.
    fn strip(&self, resolver: &Resolver, left: usize, loaded: bool) -> Node {
        let name = resolver.path.rsplit('/').next().unwrap_or(&resolver.path);
        let said = match (loaded, left) {
            (false, _) => "Reading the file".to_owned(),
            (true, 0) => "No conflicts left".to_owned(),
            (true, 1) => "1 conflict left".to_owned(),
            (true, left) => format!("{left} conflicts left"),
        };
        node()
            .styles([
                flex(),
                items_center(),
                gap(3),
                h_px(STRIP),
                px(4),
                flex_none(),
                border_b(),
                border_color(Role::Border),
            ])
            .child(line(name, 13, 1.4, Role::Text).styles([font_medium(), flex_none()]))
            .child(line(
                said,
                12,
                1.4,
                if left == 0 {
                    Role::Success
                } else {
                    Role::Warning
                },
            ))
            .child(spacer())
            .child(
                Button::new("Take all ours", CONFLICT_OURS_ALL)
                    .variant(ButtonVariant::Ghost)
                    .size(self.size.button())
                    .disabled(resolver.ours.is_none())
                    .build(),
            )
            .child(
                Button::new("Take all theirs", CONFLICT_THEIRS_ALL)
                    .variant(ButtonVariant::Ghost)
                    .size(self.size.button())
                    .disabled(resolver.theirs.is_none())
                    .build(),
            )
            .child(
                Button::new("Mark as resolved", CONFLICT_CONCLUDE)
                    .size(self.size.button())
                    .disabled(!loaded || left > 0)
                    .build(),
            )
    }

    /// The resolver: its strip, then the conflicts one under another, then the result, which is the file edited in place.
    pub fn build(self) -> Node {
        let state = self.state;
        let Some(excerpts) = state.excerpts.as_ref() else {
            return node();
        };
        let Source::Conflict(resolver) = &excerpts.source else {
            return node();
        };
        let index = state.viewer_at(&resolver.path);
        let text_now = index
            .and_then(|index| state.git.viewers.get(index))
            .and_then(|viewer| match &viewer.contents {
                Contents::Text(document) => Some(document.field.text()),
                _ => None,
            });
        let regions = text_now.map(regions_in).unwrap_or_default();
        let row = state.editor.row_height();
        let strip = self.strip(resolver, regions.len(), text_now.is_some());
        let (low, high) = (
            excerpts.scroll - OVERSCAN,
            excerpts.scroll + (self.height - STRIP).max(0.0) + OVERSCAN,
        );
        let (mut before, mut after, mut top) = (0.0_f32, 0.0_f32, 0.0_f32);
        let mut built = Vec::new();
        for (place, region) in regions.iter().enumerate() {
            let size = self.card_height(region, row);
            let (start, end) = (top, top + size);
            top = end;
            if end < low {
                before += size;
            } else if start > high {
                after += size;
            } else {
                built.push(
                    node()
                        .styles([flex(), flex_col(), flex_none(), p(3)])
                        .child(self.card(
                            &Card {
                                region,
                                place,
                                count: regions.len(),
                            },
                            row,
                        )),
                );
            }
        }
        let result = node()
            .styles([
                flex(),
                items_center(),
                h_px(LABEL),
                flex_none(),
                px(3),
                bg(Role::Surface),
                border_b(),
                border_color(Role::Border),
            ])
            .child(line("Result", 12, 1.4, Role::Text).styles([font_medium()]));
        let doc_top = top + LABEL;
        let whole = Block::whole();
        let rows = match index {
            Some(index) => plain_block(state, index, &whole, doc_top, (low, high)),
            None => node(),
        };
        node()
            .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
            .child(strip)
            .child(
                node()
                    .styles([
                        flex(),
                        flex_col(),
                        flex_1(),
                        min_w_px(0),
                        min_h(0),
                        overflow_hidden(),
                        bg(Role::Background),
                    ])
                    .child(
                        node()
                            .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
                            .scroll_from_start(EXCERPTS_SCROLL, excerpts.scroll)
                            .child(node().styles([h_px(before), flex_none()]))
                            .children(built)
                            .child(node().styles([h_px(after), flex_none()]))
                            .child(result)
                            .child(rows),
                    ),
            )
    }
}

/// `name`, or `fallback` when it is empty.
fn name_or<'a>(name: &'a str, fallback: &'a str) -> &'a str {
    if name.is_empty() { fallback } else { name }
}

impl WorkspaceState {
    /// Asks the server for the sides of the conflicted file at `index` among the open session's changes, to open the resolver on it, and says whether the file is one in conflict.
    pub(crate) fn request_resolver(&mut self, index: usize) -> bool {
        let Some(session) = self.review_index() else {
            return false;
        };
        let Some(path) = self
            .review_session()
            .and_then(|held| held.changes.get(index))
            .filter(|change| change.conflicted)
            .map(|change| change.path.clone())
        else {
            return false;
        };
        self.outbox.push(Outgoing::ReadConflict { session, path });
        true
    }

    /// The state after the server gave the sides of the conflicted file at `path` of the session at `session`: the resolver opens on it, the file itself being the result.
    pub fn with_conflict(
        mut self,
        session: usize,
        path: &str,
        ours: Option<String>,
        theirs: Option<String>,
        base: Option<String>,
    ) -> Self {
        if self.review_index() != Some(session) {
            return self;
        }
        let resolver = Resolver {
            path: path.to_owned(),
            ours,
            theirs,
            base,
        };
        let file = ExcerptFile::new(path.to_owned(), vec![Block::whole()]);
        self.excerpts_shown(session, Source::Conflict(Box::new(resolver)), vec![file]);
        self
    }

    /// The position among the open files of the file of the resolver shown, when one is.
    fn resolver_file(&self) -> Option<usize> {
        let excerpts = self.excerpts.as_ref()?;
        let Source::Conflict(resolver) = &excerpts.source else {
            return None;
        };
        self.viewer_at(&resolver.path)
    }

    /// Replaces the stretch `span` of the text of the resolver's file with `replacement`, as an edit that can be taken back.
    fn resolver_replaced(&mut self, span: Range<usize>, replacement: String) {
        let Some(index) = self.resolver_file() else {
            return;
        };
        self.git.shown = Some(index);
        let end = span.start + replacement.len();
        self.file_edited(Edit::Replace(span.clone(), replacement, (span.start, end)));
    }

    /// Settles the `place`-th conflict of the resolver's file with `choice`.
    fn conflict_settled(&mut self, place: usize, choice: Choice) {
        let Some(index) = self.resolver_file() else {
            return;
        };
        let region = match &self.git.viewers[index].contents {
            Contents::Text(document) => regions_in(document.field.text()).into_iter().nth(place),
            _ => None,
        };
        if let Some(region) = region {
            let replacement = region.taken(choice);
            self.resolver_replaced(region.span, replacement);
        }
    }

    /// Makes the whole of the resolver's file what one side has it as.
    fn conflict_side_taken(&mut self, ours: bool) {
        let Some(index) = self.resolver_file() else {
            return;
        };
        let Some(Source::Conflict(resolver)) = self.excerpts.as_ref().map(|e| &e.source) else {
            return;
        };
        let Some(side) = (if ours {
            &resolver.ours
        } else {
            &resolver.theirs
        })
        .clone() else {
            return;
        };
        let length = match &self.git.viewers[index].contents {
            Contents::Text(document) => document.field.text().len(),
            _ => return,
        };
        self.resolver_replaced(0..length, side);
    }

    /// Writes the result as the file, stages it, and closes the resolver, once no conflict is left in it.
    fn conflict_concluded(mut self) -> Self {
        let (Some(session), Some(index)) = (self.review_index(), self.resolver_file()) else {
            return self;
        };
        let path = self.git.viewers[index].path.clone();
        let Contents::Text(document) = &mut self.git.viewers[index].contents else {
            return self;
        };
        if !regions_in(document.field.text()).is_empty() {
            return self;
        }
        document.mark_saved();
        let text = document.to_save();
        self.outbox.push(Outgoing::ResolveConflict {
            session,
            path,
            text,
        });
        self.excerpts_closed()
    }

    /// Carries out what pressing `id` of the resolver does.
    pub(crate) fn conflict_pressed(mut self, id: FocusId) -> Self {
        match id {
            CONFLICT_OURS_ALL => self.conflict_side_taken(true),
            CONFLICT_THEIRS_ALL => self.conflict_side_taken(false),
            CONFLICT_CONCLUDE => return self.conflict_concluded(),
            id => {
                if let Some(offset) = id.0.checked_sub(CONFLICT_CHOICE_BASE) {
                    let choice = Choice::ALL[(offset % 3) as usize];
                    self.conflict_settled((offset / 3) as usize, choice);
                }
            }
        }
        self
    }
}
