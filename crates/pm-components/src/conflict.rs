//! The resolver of a file that a merge, a rebase or a cherry-pick stopped on: each place where the two sides disagree with both sides shown and a choice to take ours, theirs or both, and under them the result, which is the file itself, edited by hand, until the file is marked resolved and staged.

use crate::areas::SearchEvent;
use crate::target::Target;
use crate::theme;
use crate::theme::Frame;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, TextOverflow};
use std::ops::Range;

use pm_editor::Edit;

use crate::Button;
use crate::button::Variant;
use crate::excerpts::{Block, Drawing, ExcerptFile, Source, block_gap};
use crate::open_files::Contents;
use crate::parts::{line, spacer};
use crate::state::{Outgoing, WorkspaceState};
use crate::theme::{Radius, Role};

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

/// The marker that starts the lines of our side.
const OURS_MARK: &str = "<<<<<<<";

/// The marker that starts the lines of the common ancestor, which a merge written in the style that shows it has.
const BASE_MARK: &str = "|||||||";

/// The marker that parts our side from theirs.
const PART_MARK: &str = "=======";

/// The marker that ends the lines of their side.
const THEIRS_MARK: &str = ">>>>>>>";

/// Which of the sides a conflict is settled with.
#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
    let mut open = Region::default();
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
                ours_name: name.to_owned(),
                ..Region::default()
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
pub fn is_conflict_part(id: Target) -> bool {
    matches!(
        id,
        Target::Search(SearchEvent::ConflictOursAll)
            | Target::Search(SearchEvent::ConflictTheirsAll)
            | Target::Search(SearchEvent::ConflictConclude)
            | Target::Search(SearchEvent::ConflictChoice { .. })
    )
}

/// How the two sides of a conflict lie.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layout {
    /// Ours and theirs side by side, for a pane with room.
    SideBySide,
    /// Ours above theirs, for a narrow pane.
    Stacked,
}

/// The resolver being described: the state it is drawn from and how the sides lie.
///
/// Build it with [`Conflict::new`] and emit it with [`Conflict::build`].
#[derive(Clone)]
pub struct Conflict<'a> {
    /// The state the file and the sides are drawn from.
    state: &'a WorkspaceState,
    /// How the two sides of a conflict lie.
    layout: Layout,
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
    /// The resolver of the file `state` holds the excerpts of, in a room `width` wide, its two sides beside each other when the room allows.
    pub fn new(state: &'a WorkspaceState, width: f32) -> Self {
        Self {
            state,
            layout: if width >= BESIDE {
                Layout::SideBySide
            } else {
                Layout::Stacked
            },
        }
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
        let sides = match self.layout {
            Layout::SideBySide => LABEL + ours.max(theirs).max(1) as f32 * row,
            Layout::Stacked => 2.0 * LABEL + (ours.max(1) + theirs.max(1)) as f32 * row,
        };
        LABEL + sides + BUTTONS + SPACE
    }

    /// Emits a row `height` pixels tall, padded `pad` steps at each end, holding `content` centred across it.
    #[track_caller]
    fn line_row(height: f32, pad: f32, content: impl FnOnce()) {
        rustle::row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(height)
                .padding_xy(theme::sp(pad), 0.0),
            Arrangement::Start,
            Alignment::Center,
            content,
        );
    }

    /// Emits one side of a conflict: what it is called, `name`, and the lines of `text_of` in the code font on a tint of `tint` `row` pixels tall each, the first few of them; beside the other side, sharing the row's width and filling its height, when `beside`, else as wide as the card and as tall as its lines.
    #[track_caller]
    fn side(&self, name: &str, text_of: &str, tint: Role, row: f32, beside: bool) {
        let font = self.state.editor.font_size;
        let more = text_of.lines().count().saturating_sub(MOST_LINES);
        let (own, tinted) = if beside {
            (
                Modifier::new().weight(1.0).fill_max_height(),
                Modifier::new().fill_max_width().weight(1.0),
            )
        } else {
            (
                Modifier::new().fill_max_width(),
                Modifier::new().fill_max_width(),
            )
        };
        rustle::column_arranged(own.clip(), Arrangement::Start, Alignment::Start, || {
            Self::line_row(LABEL, 2.0, || line(name, 11, 1.4, Role::Muted));
            rustle::column_arranged(
                tinted.background(theme::color(tint.alpha(0.12))),
                Arrangement::Start,
                Alignment::Start,
                || {
                    for content in text_of.lines().take(MOST_LINES) {
                        Self::line_row(row, 2.0, || {
                            rustle::basic_text(
                                Modifier::new().weight(1.0),
                                crate::diff_syntax::shown(content),
                                theme::code_style(font, 1.4)
                                    .color(theme::color(Role::Text))
                                    .max_lines(1),
                                TextOverflow::Ellipsis,
                            );
                        });
                    }
                    if more > 0 {
                        Self::line_row(row, 2.0, || {
                            line(format!("… {more} more lines"), 11, 1.4, Role::Subtle);
                        });
                    }
                },
            );
        });
    }

    /// Emits both sides of the conflict `region`, named `ours` and `theirs`, lying as the resolver's layout says, `row` pixels a line.
    #[track_caller]
    fn sides(&self, region: &Region, ours: &str, theirs: &str, row: f32) {
        let beside = self.layout == Layout::SideBySide;
        let both = || {
            self.side(ours, &region.ours, Role::Success, row, beside);
            self.side(theirs, &region.theirs, Role::Warning, row, beside);
        };
        let own = Modifier::new().fill_max_width();
        match self.layout {
            Layout::SideBySide => rustle::row_arranged(
                own.height_intrinsic(rustle::IntrinsicSize::Max)
                    .padding_xy(theme::sp(3), 0.0),
                Arrangement::SpacedBy(theme::sp(2)),
                Alignment::Start,
                both,
            ),
            Layout::Stacked => rustle::column_arranged(
                own.padding_xy(theme::sp(3), 0.0),
                Arrangement::Start,
                Alignment::Start,
                both,
            ),
        }
    }

    /// Emits the card of one conflict, `card`: where it is, both sides, and the buttons that settle it, `row` pixels a line.
    #[track_caller]
    fn card(&self, card: &Card, row: f32) {
        let region = card.region;
        let ours = format!("Ours · {}", name_or(&region.ours_name, "HEAD"));
        let theirs = format!("Theirs · {}", name_or(&region.theirs_name, "incoming"));
        rustle::column_arranged(
            Modifier::new()
                .fill_max_width()
                .height(self.card_height(region, row) - SPACE)
                .outlined(Radius::Lg)
                .clip(),
            Arrangement::Start,
            Alignment::Start,
            || {
                Self::label_bar(format!(
                    "Conflict {} of {} · line {}",
                    card.place + 1,
                    card.count,
                    region.line + 1
                ));
                self.sides(region, &ours, &theirs, row);
                rustle::row_arranged(
                    Modifier::new()
                        .height(BUTTONS)
                        .padding_xy(theme::sp(3), 0.0),
                    Arrangement::SpacedBy(theme::sp(2)),
                    Alignment::Center,
                    || {
                        for choice in Choice::ALL {
                            Button::new(
                                choice.label(),
                                Target::Search(SearchEvent::ConflictChoice {
                                    place: card.place,
                                    choice,
                                }),
                            )
                            .variant(if choice == Choice::Both {
                                Variant::Ghost
                            } else {
                                Variant::Outline
                            })
                            .build();
                        }
                    },
                );
            },
        );
    }

    /// Emits the strip above the conflicts: the file of `resolver`, how many conflicts are `left` once the file is `loaded`, and what can be done with all of them at once.
    #[track_caller]
    fn strip(&self, resolver: &Resolver, left: usize, loaded: bool) {
        let name = resolver.path.rsplit('/').next().unwrap_or(&resolver.path);
        let said = match (loaded, left) {
            (false, _) => "Reading the file".to_owned(),
            (true, 0) => "No conflicts left".to_owned(),
            (true, 1) => "1 conflict left".to_owned(),
            (true, left) => format!("{left} conflicts left"),
        };
        rustle::row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(STRIP)
                .border_bottom(1.0, theme::color(Role::Border))
                .padding_each(0.0, 0.0, 0.0, 1.0)
                .padding_xy(theme::sp(4), 0.0),
            Arrangement::SpacedBy(theme::sp(3)),
            Alignment::Center,
            || {
                rustle::basic_text(
                    Modifier::new(),
                    name,
                    theme::strong(13, Role::Text),
                    TextOverflow::Ellipsis,
                );
                line(
                    said,
                    12,
                    1.4,
                    if left == 0 {
                        Role::Success
                    } else {
                        Role::Warning
                    },
                );
                spacer();
                for (label, id, variant, disabled) in [
                    (
                        "Take all ours",
                        SearchEvent::ConflictOursAll,
                        Variant::Ghost,
                        resolver.ours.is_none(),
                    ),
                    (
                        "Take all theirs",
                        SearchEvent::ConflictTheirsAll,
                        Variant::Ghost,
                        resolver.theirs.is_none(),
                    ),
                    (
                        "Mark as resolved",
                        SearchEvent::ConflictConclude,
                        Variant::Default,
                        !loaded || left > 0,
                    ),
                ] {
                    Button::new(label, Target::Search(id))
                        .variant(variant)
                        .disabled(disabled)
                        .build();
                }
            },
        );
    }

    /// Emits the resolver: its strip, then the conflicts one under another, then the result, which is the file edited in place; nothing when no resolver is open.
    #[track_caller]
    pub fn build(self) {
        let state = self.state;
        let Some(excerpts) = state.excerpts.as_ref() else {
            return;
        };
        let Source::Conflict(resolver) = &excerpts.source else {
            return;
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
        let whole = Block::whole();
        let drawing = Drawing::new(state, None);
        let lines = index.map_or(0, |index| drawing.row_count(index, &whole) + 1);
        let count = regions.len() + 1 + lines;
        rustle::column_arranged(
            Modifier::new().weight(1.0),
            Arrangement::Start,
            Alignment::Start,
            || {
                self.strip(resolver, regions.len(), text_now.is_some());
                rustle::column_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .weight(1.0)
                        .clip()
                        .background(theme::color(Role::Background)),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        rustle::lazy_list_keyed(
                            Modifier::new().fill_max_width().weight(1.0),
                            &excerpts.scroll,
                            count,
                            |item| item,
                            |item| {
                                let after = item.saturating_sub(regions.len());
                                match regions.get(item) {
                                    Some(region) => rustle::column_arranged(
                                        Modifier::new().fill_max_width().padding(theme::sp(3)),
                                        Arrangement::Start,
                                        Alignment::Start,
                                        || {
                                            self.card(
                                                &Card {
                                                    region,
                                                    place: item,
                                                    count: regions.len(),
                                                },
                                                row,
                                            );
                                        },
                                    ),
                                    None if after == 0 => Self::label_bar("Result"),
                                    None => match index {
                                        Some(index) if after < lines => {
                                            drawing.row(index, (usize::MAX, 0), &whole, after - 1);
                                        }
                                        _ => block_gap(),
                                    },
                                }
                            },
                        );
                    },
                );
            },
        );
    }

    /// Emits a bar `LABEL` pixels tall on the surface colour, ruled along its bottom, that says `words`: the head of a conflict's card and the label over the result.
    #[track_caller]
    fn label_bar(words: impl Into<String>) {
        rustle::row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(LABEL)
                .background(theme::color(Role::Surface))
                .border_bottom(1.0, theme::color(Role::Border))
                .padding_each(0.0, 0.0, 0.0, 1.0)
                .padding_xy(theme::sp(3), 0.0),
            Arrangement::Start,
            Alignment::Center,
            || {
                rustle::basic_text(
                    Modifier::new().weight(1.0),
                    words,
                    theme::strong(12, Role::Text),
                    TextOverflow::Ellipsis,
                );
            },
        );
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
    pub(crate) fn conflict_pressed(mut self, id: Target) -> Self {
        match id {
            Target::Search(SearchEvent::ConflictOursAll) => self.conflict_side_taken(true),
            Target::Search(SearchEvent::ConflictTheirsAll) => self.conflict_side_taken(false),
            Target::Search(SearchEvent::ConflictConclude) => return self.conflict_concluded(),
            Target::Search(SearchEvent::ConflictChoice { place, choice }) => {
                self.conflict_settled(place, choice)
            }
            _ => {}
        }
        self
    }
}
