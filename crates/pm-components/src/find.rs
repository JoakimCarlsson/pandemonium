//! Finding and replacing text in the file shown: the bar over its top right corner, what it looks for, and the matches it steps through.

use rustle::{Flex, flex_column, flex_row};
use std::ops::Range;

use pm_editor::{Document, Query};
use pm_field::{Edit, Field};
use rustle::{BoxAlignment, CursorIcon, Modifier};

use crate::button::{Button, Variant};
use crate::input::Input;
use crate::kit::{
    FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Styled, Type, label_with, sp,
};
use crate::size::Size;
use crate::state::WorkspaceState;

/// The field the text looked for is typed in.
pub const FIND_FIELD: FocusId = FocusId(7_402_100);

/// The field the replacement is typed in.
pub const REPLACE_FIELD: FocusId = FocusId(7_402_101);

/// Selects the next match.
pub const FIND_NEXT: FocusId = FocusId(7_402_102);

/// Selects the match before.
pub const FIND_PREVIOUS: FocusId = FocusId(7_402_103);

/// Closes the bar.
pub const FIND_CLOSE: FocusId = FocusId(7_402_104);

/// Switches matching case on and off.
pub const FIND_CASE: FocusId = FocusId(7_402_105);

/// Switches matching whole words on and off.
pub const FIND_WORD: FocusId = FocusId(7_402_106);

/// Switches reading the text as a regular expression on and off.
pub const FIND_REGEX: FocusId = FocusId(7_402_107);

/// Replaces the selected match.
pub const REPLACE_ONE: FocusId = FocusId(7_402_108);

/// Replaces every match.
pub const REPLACE_ALL: FocusId = FocusId(7_402_109);

/// The bar itself, which keeps a press on it from counting as outside.
pub const FIND_BAR: FocusId = FocusId(7_402_110);

/// Shows and hides the row for replacing.
pub const FIND_TOGGLE_REPLACE: FocusId = FocusId(7_402_111);

/// Limits finding to the lines that were selected, and lifts the limit again.
pub const FIND_IN_SELECTION: FocusId = FocusId(7_402_112);

/// The shadow under the bar: each layer's `(x, y, blur, spread, alpha)`.
pub(crate) const SHADOW: [(f32, f32, f32, f32, f32); 1] = [(0.0, 8.0, 28.0, 0.0, 0.3)];

/// The stretches a query finds, or why it cannot be used.
type Matches = Result<Vec<Range<usize>>, String>;

/// The bar for finding and replacing in the file shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// What is looked for.
    pub query: Field,
    /// What replaces it.
    pub replacement: Field,
    /// Whether the row for replacing shows.
    pub replacing: bool,
    /// Whether upper and lower case must match.
    pub case: bool,
    /// Whether only whole words match.
    pub word: bool,
    /// Whether the text is a regular expression.
    pub regex: bool,
    /// Whether finding is limited to the stretch the file shown keeps for it.
    pub scoped: bool,
}

impl Finding {
    /// The stretch of `document` finding is limited to, when it is limited.
    pub fn scope(&self, document: &Document) -> Option<Range<usize>> {
        self.scoped.then(|| document.find_scope()).flatten()
    }

    /// What the bar looks for; case matters when it is switched on, or with `smartcase` when what is typed holds a capital letter.
    pub fn query(&self, smartcase: bool) -> Query {
        let capital = self.query.text().chars().any(char::is_uppercase);
        Query {
            text: self.query.text().to_owned(),
            case: self.case || (smartcase && capital),
            word: self.word,
            regex: self.regex,
        }
    }
}

/// Whether `id` is one of the fields or buttons of the bar.
pub fn is_find_part(id: FocusId) -> bool {
    (FIND_FIELD.0..=FIND_IN_SELECTION.0).contains(&id.0)
}

impl WorkspaceState {
    /// The matches of the bar in the file shown, or why its query cannot be used.
    fn found(&self) -> Option<(Matches, Query)> {
        let finding = self.finding.as_ref()?;
        let document = self.shown_document()?;
        let query = finding.query(self.editor.smartcase);
        Some((
            document.find_all_within(&query, finding.scope(document)),
            query,
        ))
    }

    /// Opens the bar, with the row for replacing when `replacing`, looking for what is selected in the file shown when that is part of one line.
    pub fn find_opened(mut self, replacing: bool) -> Self {
        let selected = self
            .shown_document()
            .map(|document| document.field.selected().to_owned())
            .filter(|selected| !selected.is_empty() && !selected.contains('\n'))
            .filter(|_| self.editor.seed_search_from_selection);
        let mut finding = self.finding.take().unwrap_or(Finding {
            query: Field::new(""),
            replacement: Field::new(""),
            replacing,
            case: false,
            word: false,
            regex: false,
            scoped: false,
        });
        finding.replacing |= replacing;
        if let Some(selected) = selected {
            finding.query = Field::new(selected);
        }
        finding.query.apply(Edit::SelectAll);
        let mode = self.editor.auto_find_in_selection;
        if let Some(document) = self.shown_text() {
            let selected = document.field.selected().to_owned();
            if mode.limits_to(&selected) {
                document.set_find_scope(document.selection_scope());
            }
            finding.scoped = document.find_scope().is_some();
        }
        self.finding = Some(finding);
        self
    }

    /// Limits finding to the stretch selected in the file shown when it is not limited, opening the bar when it is closed, and lifts the limit when it is; nothing is limited while nothing is selected.
    pub fn find_scope_toggled(mut self) -> Self {
        if self.finding.is_none() {
            self = self.find_opened(false);
            if self.finding.as_ref().is_some_and(|finding| finding.scoped) {
                return self.select_first_match();
            }
        }
        let scoped = self.finding.as_ref().is_some_and(|finding| finding.scoped);
        let limited = self.shown_text().map(|document| {
            if scoped {
                document.set_find_scope(None);
            } else {
                document.set_find_scope(document.selection_scope());
            }
            document.find_scope().is_some()
        });
        if let (Some(limited), Some(finding)) = (limited, self.finding.as_mut()) {
            finding.scoped = limited;
        }
        self.select_first_match()
    }

    /// Whether the bar is open.
    pub fn find_open(&self) -> bool {
        self.finding.is_some()
    }

    /// Closes the bar.
    pub fn find_closed(mut self) -> Self {
        self.finding = None;
        if let Some(document) = self.shown_text() {
            document.set_find_scope(None);
        }
        self
    }

    /// Selects the match after the selection of the file shown, or the one before it when `back`.
    pub fn find_stepped(mut self, back: bool) -> Self {
        let Some((Ok(matches), _)) = self.found() else {
            return self;
        };
        let edit = self.shown_document().and_then(|document| {
            document.next_match_edit_with(&matches, back, self.editor.search_wrap)
        });
        if let Some(edit) = edit {
            self.file_edited(edit);
        }
        self
    }

    /// The state after the text looked for was changed by `edit`: the first match from the selection on is selected.
    pub fn find_typed(mut self, edit: Edit) -> Self {
        let Some(finding) = self.finding.as_mut() else {
            return self;
        };
        finding.query.apply(edit);
        self.select_first_match()
    }

    /// Selects the first match from where the selection of the file shown starts, going round to the first.
    fn select_first_match(mut self) -> Self {
        let Some((Ok(matches), _)) = self.found() else {
            return self;
        };
        let edit = self
            .shown_document()
            .and_then(|document| document.first_match_edit(&matches));
        if let Some(edit) = edit {
            self.file_edited(edit);
        }
        self
    }

    /// The state after the replacement was changed by `edit`.
    pub fn replacement_typed(mut self, edit: Edit) -> Self {
        if let Some(finding) = self.finding.as_mut() {
            finding.replacement.apply(edit);
        }
        self
    }

    /// Switches one of the ways of matching, the one `which` names, on or off, and selects the first match from the selection on.
    pub fn find_toggled(mut self, which: FocusId) -> Self {
        if let Some(finding) = self.finding.as_mut() {
            match which {
                FIND_CASE => finding.case = !finding.case,
                FIND_WORD => finding.word = !finding.word,
                FIND_REGEX => finding.regex = !finding.regex,
                FIND_TOGGLE_REPLACE => finding.replacing = !finding.replacing,
                FIND_IN_SELECTION => return self.find_scope_toggled(),
                _ => {}
            }
        }
        self.select_first_match()
    }

    /// Replaces the selected match, or selects the next one when none is selected, and then selects the next.
    pub fn replaced_one(mut self) -> Self {
        let Some((Ok(matches), query)) = self.found() else {
            return self;
        };
        let replacement = self
            .finding
            .as_ref()
            .map(|finding| finding.replacement.text().to_owned())
            .unwrap_or_default();
        let read_only = self
            .shown_document()
            .is_none_or(|document| document.field.is_read_only());
        let edit = self
            .shown_document()
            .and_then(|document| document.replace_edit(&query, &matches, &replacement))
            .filter(|_| !read_only);
        if let Some(edit) = edit {
            self.file_edited_inside_scope(edit);
        }
        self.find_stepped(false)
    }

    /// Replaces every match in one change.
    pub fn replaced_all(mut self) -> Self {
        let Some((Ok(matches), query)) = self.found() else {
            return self;
        };
        let replacement = self
            .finding
            .as_ref()
            .map(|finding| finding.replacement.text().to_owned())
            .unwrap_or_default();
        let read_only = self
            .shown_document()
            .is_none_or(|document| document.field.is_read_only());
        let edit = self
            .shown_document()
            .and_then(|document| document.replace_all_edit(&query, &matches, &replacement))
            .filter(|_| !read_only);
        if let Some(edit) = edit {
            self.file_edited_inside_scope(edit);
        }
        self
    }

    /// Makes `edit`, a replacement of a stretch inside the limit of finding, to the file shown, and has the limit take in the whole of what was written, which it cannot tell from the text alone where the replacement ends where the limit does.
    fn file_edited_inside_scope(&mut self, edit: Edit) {
        let scope = self
            .shown_document()
            .zip(self.finding.as_ref())
            .and_then(|(document, finding)| finding.scope(document));
        let grown = match (&edit, &scope) {
            (Edit::Replace(range, written, _), Some(scope))
                if scope.start <= range.start && range.end <= scope.end =>
            {
                Some(scope.start..scope.end + written.len() - range.len())
            }
            _ => None,
        };
        self.file_edited(edit);
        if let (Some(grown), Some(document)) = (grown, self.shown_text()) {
            document.set_find_scope(Some(grown));
        }
    }

    /// Carries out what pressing the part `id` of the bar does.
    pub(crate) fn find_pressed(self, id: FocusId) -> Self {
        match id {
            FIND_NEXT => self.find_stepped(false),
            FIND_PREVIOUS => self.find_stepped(true),
            FIND_CLOSE => self.find_closed(),
            FIND_CASE | FIND_WORD | FIND_REGEX | FIND_TOGGLE_REPLACE | FIND_IN_SELECTION => {
                self.find_toggled(id)
            }
            REPLACE_ONE => self.replaced_one(),
            REPLACE_ALL => self.replaced_all(),
            _ => self,
        }
    }

    /// The matches the bar finds in the file shown, to mark them, and which one is selected.
    pub fn find_marks(&self) -> (Vec<Range<usize>>, Option<usize>) {
        let Some((Ok(matches), _)) = self.found() else {
            return (Vec::new(), None);
        };
        let selected = self
            .shown_document()
            .and_then(|document| document.selected_match(&matches));
        (matches, selected)
    }
}

/// Emits a switch of the bar, `label` in the font for code, lit when `on`, answering to `id`.
#[track_caller]
pub(crate) fn switch(label: &str, id: FocusId, on: bool) {
    let looks = if on {
        Looks::new().idle(Role::SurfaceActive)
    } else {
        Looks::new()
    }
    .hover(Role::SurfaceHover)
    .radius(Radius::Sm);
    let mut own = Modifier::new().w(6.5).h(6).looks_of(looks, id);
    if on {
        own = own.stroke(Role::Accent, Radius::Sm);
    }
    let own = own.cursor(CursorIcon::Pointer).target(id);
    let kind = Type::new(11)
        .ratio(1.2)
        .mono()
        .tone(if on { Role::Text } else { Role::Subtle });
    flex_row(own, Flex::new().items_center().justify_center(), || {
        crate::kit::label(label, kind);
    });
}

/// What the bar says of the matches: which is selected of how many, that there are none, or why the query cannot be used.
fn tally(found: &Matches, selected: Option<usize>, empty: bool) -> (String, Role) {
    match found {
        Err(problem) => (problem.clone(), Role::Danger),
        Ok(_) if empty => (String::new(), Role::Subtle),
        Ok(matches) if matches.is_empty() => ("No results".to_owned(), Role::Danger),
        Ok(matches) => match selected {
            Some(index) => (format!("{} of {}", index + 1, matches.len()), Role::Muted),
            None => (format!("{} found", matches.len()), Role::Muted),
        },
    }
}

/// Emits the bar for finding and replacing in `document`, over the top right corner of the file.
#[track_caller]
pub fn find_bar(finding: &Finding, document: &Document, smartcase: bool) {
    let query = finding.query(smartcase);
    let found = document.find_all_within(&query, finding.scope(document));
    let selected = found
        .as_ref()
        .ok()
        .and_then(|matches| document.selected_match(matches));
    let (said, tone) = tally(&found, selected, query.text.is_empty());
    let toggle = if finding.replacing {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let read_only = document.field.is_read_only();
    flex_column(
        Modifier::new()
            .unbounded_width()
            .placed(BoxAlignment::TopEnd)
            .at(-sp(6), sp(2))
            .z_index(40)
            .shadows(&SHADOW, Radius::Md)
            .fill_rounded(Role::Surface, Radius::Md)
            .stroke(Role::Border, Radius::Md)
            .target(FIND_BAR)
            .pad(1),
        Flex::new().gap(sp(1)),
        || {
            flex_row(
                Modifier::new(),
                Flex::new().items_center().gap(sp(1)),
                || {
                    Button::icon(toggle, FIND_TOGGLE_REPLACE).build();
                    Input::new(&finding.query, FIND_FIELD)
                        .size(Size::Sm)
                        .placeholder("Find")
                        .invalid(found.is_err())
                        .modifier(Modifier::new().width(194.0))
                        .build();
                    switch("Aa", FIND_CASE, finding.case);
                    switch("ab", FIND_WORD, finding.word);
                    switch(".*", FIND_REGEX, finding.regex);
                    switch("sel", FIND_IN_SELECTION, finding.scoped);
                    label_with(
                        Modifier::new().keep().width(84.0).pad_x(1),
                        said,
                        Type::new(11).ratio(1.4).tone(tone).truncate(),
                    );
                    Button::icon(IconName::ArrowUp, FIND_PREVIOUS).build();
                    Button::icon(IconName::ArrowDown, FIND_NEXT).build();
                    Button::icon(IconName::Close, FIND_CLOSE).build();
                },
            );
            if finding.replacing {
                flex_row(
                    Modifier::new(),
                    Flex::new().items_center().gap(sp(1)),
                    || {
                        rustle::spacer(Modifier::new().w(7));
                        Input::new(&finding.replacement, REPLACE_FIELD)
                            .size(Size::Sm)
                            .placeholder("Replace")
                            .modifier(Modifier::new().width(194.0))
                            .build();
                        Button::new("Replace", REPLACE_ONE)
                            .variant(Variant::Ghost)
                            .size(Size::Sm)
                            .disabled(read_only)
                            .build();
                        Button::new("Replace all", REPLACE_ALL)
                            .variant(Variant::Ghost)
                            .size(Size::Sm)
                            .disabled(read_only)
                            .build();
                    },
                );
            }
        },
    );
}
