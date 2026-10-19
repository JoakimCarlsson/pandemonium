//! Finding and replacing text in the file shown: the bar over its top right corner, what it looks for, and the matches it steps through.

use std::ops::Range;

use pm_editor::{Document, Query};
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, ShadowLayer, absolute, bg, border, border_color, cursor_pointer, flex, flex_col,
    flex_none, font_mono, gap, h, items_center, justify_center, leading_ratio, p, px, right,
    rounded, shadow_layers, text_color, text_px, top, w, w_px, z,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node, text};

use crate::button::{Button, Variant};
use crate::input::Input;
use crate::parts::line;
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

/// The shadow under the bar.
const SHADOW: [ShadowLayer; 1] = [ShadowLayer::new(0.0, 8.0, 28.0, 0.0, 0.3, false)];

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
}

impl Finding {
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
    (FIND_FIELD.0..=FIND_TOGGLE_REPLACE.0).contains(&id.0)
}

impl WorkspaceState {
    /// The matches of the bar in the file shown, or why its query cannot be used.
    fn found(&self) -> Option<(Matches, Query)> {
        let finding = self.finding.as_ref()?;
        let document = self.shown_document()?;
        let query = finding.query(self.editor.smartcase);
        Some((document.find_all(&query), query))
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
        });
        finding.replacing |= replacing;
        if let Some(selected) = selected {
            finding.query = Field::new(selected);
        }
        finding.query.apply(Edit::SelectAll);
        self.finding = Some(finding);
        self
    }

    /// Whether the bar is open.
    pub fn find_open(&self) -> bool {
        self.finding.is_some()
    }

    /// Closes the bar.
    pub fn find_closed(mut self) -> Self {
        self.finding = None;
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
            self.file_edited(edit);
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
            self.file_edited(edit);
        }
        self
    }

    /// Carries out what pressing the part `id` of the bar does.
    pub(crate) fn find_pressed(self, id: FocusId) -> Self {
        match id {
            FIND_NEXT => self.find_stepped(false),
            FIND_PREVIOUS => self.find_stepped(true),
            FIND_CLOSE => self.find_closed(),
            FIND_CASE | FIND_WORD | FIND_REGEX | FIND_TOGGLE_REPLACE => self.find_toggled(id),
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

/// A switch of the bar, `label` in the font for code, lit when `on`.
pub(crate) fn switch(label: &str, id: FocusId, on: bool) -> Node {
    let mut held = node()
        .styles([
            flex(),
            items_center(),
            justify_center(),
            flex_none(),
            w(6.5),
            h(6),
            rounded(Radius::Sm),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .focusable(id)
        .child(text(label).styles([
            text_px(11),
            leading_ratio(1.2),
            font_mono(),
            text_color(if on { Role::Text } else { Role::Subtle }),
        ]));
    if on {
        held = held.styles([
            bg(Role::SurfaceActive),
            border(),
            border_color(Role::Accent),
        ]);
    }
    held
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

/// The bar for finding and replacing in `document`, over the top right corner of the file.
pub fn find_bar(finding: &Finding, document: &Document, smartcase: bool) -> Node {
    let query = finding.query(smartcase);
    let found = document.find_all(&query);
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
    let find_row = node()
        .styles([flex(), items_center(), gap(1)])
        .child(Button::icon(toggle, FIND_TOGGLE_REPLACE).build())
        .child(
            node().styles([w_px(220.0), flex_none()]).child(
                Input::new(&finding.query, FIND_FIELD)
                    .size(Size::Sm)
                    .placeholder("Find")
                    .invalid(found.is_err())
                    .build(),
            ),
        )
        .child(switch("Aa", FIND_CASE, finding.case))
        .child(switch("ab", FIND_WORD, finding.word))
        .child(switch(".*", FIND_REGEX, finding.regex))
        .child(
            node()
                .styles([w_px(84.0), flex_none(), px(1)])
                .child(line(said, 11, 1.4, tone)),
        )
        .child(Button::icon(IconName::ArrowUp, FIND_PREVIOUS).build())
        .child(Button::icon(IconName::ArrowDown, FIND_NEXT).build())
        .child(Button::icon(IconName::Close, FIND_CLOSE).build());
    let mut bar = node()
        .styles([
            absolute(),
            top(2),
            right(6),
            z(40),
            flex(),
            flex_col(),
            gap(1),
            p(1),
            rounded(Radius::Md),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
            shadow_layers(&SHADOW),
        ])
        .focusable(FIND_BAR)
        .child(find_row);
    if finding.replacing {
        bar = bar.child(
            node()
                .styles([flex(), items_center(), gap(1)])
                .child(node().styles([w(7), flex_none()]))
                .child(
                    node().styles([w_px(220.0), flex_none()]).child(
                        Input::new(&finding.replacement, REPLACE_FIELD)
                            .size(Size::Sm)
                            .placeholder("Replace")
                            .build(),
                    ),
                )
                .child(
                    Button::new("Replace", REPLACE_ONE)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .disabled(document.field.is_read_only())
                        .build(),
                )
                .child(
                    Button::new("Replace all", REPLACE_ALL)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .disabled(document.field.is_read_only())
                        .build(),
                ),
        );
    }
    bar
}
