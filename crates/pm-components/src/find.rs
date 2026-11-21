//! Finding and replacing text in the file shown: the bar over its top right corner, what it looks for, and the matches it steps through.

use crate::areas::EditorEvent;
use crate::button;
use crate::events;
use crate::icons;
use crate::theme::Frame;
use crate::theme::Indicated;
use crate::theme::ShadowLayers;
use rustle::TextOverflow;

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, row_arranged};
use std::ops::Range;

use pm_editor::Edit;
use pm_editor::{Document, Query};
use rustle::TextFieldState;
use rustle::{BoxAlignment, Modifier};

use crate::button::{Button, Variant};
use crate::events::Interactive;
use crate::input::Input;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::theme::{Radius, Role, sp};

/// The shadow under the bar: each layer's `(x, y, blur, spread, alpha)`.
pub(crate) const SHADOW: [rustle::Shadow; 1] = [crate::theme::box_shadow(0.0, 8.0, 28.0, 0.0, 0.3)];

/// The stretches a query finds, or why it cannot be used.
type Matches = Result<Vec<Range<usize>>, String>;

/// The bar for finding and replacing in the file shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// What is looked for.
    pub query: TextFieldState,
    /// What replaces it.
    pub replacement: TextFieldState,
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
pub fn is_find_part(id: Target) -> bool {
    matches!(
        id,
        Target::Editor(EditorEvent::FindField)
            | Target::Editor(EditorEvent::ReplaceField)
            | Target::Editor(EditorEvent::FindNext)
            | Target::Editor(EditorEvent::FindPrevious)
            | Target::Editor(EditorEvent::FindClose)
            | Target::Editor(EditorEvent::FindCase)
            | Target::Editor(EditorEvent::FindWord)
            | Target::Editor(EditorEvent::FindRegex)
            | Target::Editor(EditorEvent::ReplaceOne)
            | Target::Editor(EditorEvent::ReplaceAll)
            | Target::Editor(EditorEvent::FindBar)
            | Target::Editor(EditorEvent::FindToggleReplace)
            | Target::Editor(EditorEvent::FindInSelection)
    )
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

    /// What is selected in the file shown, when that is part of one line and
    /// a search is set to start from the selection: what a search opened
    /// looks for.
    pub(crate) fn selection_to_seek(&self) -> Option<String> {
        self.shown_document()
            .map(|document| document.field.selected().to_owned())
            .filter(|selected| !selected.is_empty() && !selected.contains('\n'))
            .filter(|_| self.editor.seed_search_from_selection)
    }

    /// Opens the bar, with the row for replacing when `replacing`, looking for what is selected in the file shown when that is part of one line.
    pub fn find_opened(mut self, replacing: bool) -> Self {
        let selected = self.selection_to_seek();
        let mut finding = self.finding.take().unwrap_or(Finding {
            query: TextFieldState::new(""),
            replacement: TextFieldState::new(""),
            replacing,
            case: false,
            word: false,
            regex: false,
            scoped: false,
        });
        finding.replacing |= replacing;
        if let Some(selected) = selected {
            finding.query = TextFieldState::new(selected);
        }
        finding.query.select_all();
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
    pub fn find_stepped(self, back: bool) -> Self {
        let wrap = self.editor.search_wrap;
        self.match_selected(|document, matches| document.next_match_edit_with(matches, back, wrap))
    }

    /// Makes the edit `pick` chooses from the matches of the bar in the file shown, when the query can be used and it chooses one.
    fn match_selected(
        mut self,
        pick: impl FnOnce(&Document, &[Range<usize>]) -> Option<Edit>,
    ) -> Self {
        if let Some((Ok(matches), _)) = self.found() {
            self.edit_shown(|document| pick(document, &matches));
        }
        self
    }

    /// The state after the reader changed the text looked for: the first match from the selection on is selected.
    pub fn find_typed(self) -> Self {
        if self.finding.is_none() {
            return self;
        }
        self.select_first_match()
    }

    /// Selects the first match from where the selection of the file shown starts, going round to the first.
    fn select_first_match(self) -> Self {
        self.match_selected(|document, matches| document.first_match_edit(matches))
    }

    /// Switches one of the ways of matching, the one `which` names, on or off, and selects the first match from the selection on.
    pub fn find_toggled(mut self, which: Target) -> Self {
        if let Some(finding) = self.finding.as_mut() {
            match which {
                Target::Editor(EditorEvent::FindCase) => finding.case = !finding.case,
                Target::Editor(EditorEvent::FindWord) => finding.word = !finding.word,
                Target::Editor(EditorEvent::FindRegex) => finding.regex = !finding.regex,
                Target::Editor(EditorEvent::FindToggleReplace) => {
                    finding.replacing = !finding.replacing
                }
                Target::Editor(EditorEvent::FindInSelection) => return self.find_scope_toggled(),
                _ => {}
            }
        }
        self.select_first_match()
    }

    /// The edit that replaces the selected match, or every match when `all`, by what the bar replaces with; nothing when the file shown is read only or the query cannot be used.
    fn replacement(&self, all: bool) -> Option<Edit> {
        let Some((Ok(matches), query)) = self.found() else {
            return None;
        };
        let replacement = self
            .finding
            .as_ref()
            .map(|finding| finding.replacement.text().to_owned())
            .unwrap_or_default();
        let document = self
            .shown_document()
            .filter(|document| !document.field.is_read_only())?;
        if all {
            document.replace_all_edit(&query, &matches, &replacement)
        } else {
            document.replace_edit(&query, &matches, &replacement)
        }
    }

    /// Replaces the selected match, or selects the next one when none is selected, and then selects the next.
    pub fn replaced_one(mut self) -> Self {
        if !matches!(self.found(), Some((Ok(_), _))) {
            return self;
        }
        if let Some(edit) = self.replacement(false) {
            self.file_edited_inside_scope(edit);
        }
        self.find_stepped(false)
    }

    /// Replaces every match in one change.
    pub fn replaced_all(mut self) -> Self {
        if let Some(edit) = self.replacement(true) {
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
    pub(crate) fn find_pressed(self, id: Target) -> Self {
        match id {
            Target::Editor(EditorEvent::FindNext) => self.find_stepped(false),
            Target::Editor(EditorEvent::FindPrevious) => self.find_stepped(true),
            Target::Editor(EditorEvent::FindClose) => self.find_closed(),
            Target::Editor(EditorEvent::FindCase)
            | Target::Editor(EditorEvent::FindWord)
            | Target::Editor(EditorEvent::FindRegex)
            | Target::Editor(EditorEvent::FindToggleReplace)
            | Target::Editor(EditorEvent::FindInSelection) => self.find_toggled(id),
            Target::Editor(EditorEvent::ReplaceOne) => self.replaced_one(),
            Target::Editor(EditorEvent::ReplaceAll) => self.replaced_all(),
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
pub(crate) fn switch(label: &str, id: Target, on: bool) {
    let looks = if on {
        theme::InteractionTones::new().idle(Role::SurfaceActive)
    } else {
        theme::InteractionTones::new()
    }
    .hover(Role::SurfaceHover)
    .radius(Radius::Sm);
    let mut own = Modifier::new()
        .width(theme::sp(6.5))
        .height(theme::sp(6))
        .indicated(looks, theme::Focused::Control(crate::focus::is_focused(id)));
    if on {
        own = own
            .border(1.0, theme::color(Role::Accent), Radius::Sm.px())
            .padding(1.0);
    }
    let own = events::pressed(own, id);
    let kind =
        theme::mono_style(11, 1.2).color(theme::color(if on { Role::Text } else { Role::Subtle }));
    row_arranged(own, Arrangement::Center, Alignment::Center, || {
        rustle::basic_text(Modifier::new(), label, kind, TextOverflow::Ellipsis);
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
    let toggle = icons::chevron(finding.replacing);
    let read_only = document.field.is_read_only();
    rustle::column_arranged(
        Modifier::new()
            .wrap_content_width(BoxAlignment::TopEnd, true)
            .align(BoxAlignment::TopEnd)
            .offset(-sp(6), sp(2))
            .z_index(40)
            .shadow_layers(&SHADOW, Radius::Md.px())
            .panel(Radius::Md)
            .control(
                Target::Editor(EditorEvent::FindBar),
                send(Event::Editor(EditorEvent::FindBar)),
            )
            .padding(theme::sp(1)),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(theme::sp(1)),
                Alignment::Center,
                || {
                    Button::icon(toggle, Target::Editor(EditorEvent::FindToggleReplace)).build();
                    Input::new(&finding.query, Target::Editor(EditorEvent::FindField))
                        .size(Size::Sm)
                        .placeholder("Find")
                        .invalid(found.is_err())
                        .modifier(Modifier::new().width(194.0))
                        .build();
                    switch("Aa", Target::Editor(EditorEvent::FindCase), finding.case);
                    switch("ab", Target::Editor(EditorEvent::FindWord), finding.word);
                    switch(".*", Target::Editor(EditorEvent::FindRegex), finding.regex);
                    switch(
                        "sel",
                        Target::Editor(EditorEvent::FindInSelection),
                        finding.scoped,
                    );
                    rustle::basic_text(
                        Modifier::new().width(84.0).padding_xy(theme::sp(1), 0.0),
                        said,
                        theme::line(11, tone),
                        TextOverflow::Ellipsis,
                    );
                    Button::icon(
                        phosphor::ARROW_UP,
                        Target::Editor(EditorEvent::FindPrevious),
                    )
                    .build();
                    Button::icon(phosphor::ARROW_DOWN, Target::Editor(EditorEvent::FindNext))
                        .build();
                    Button::icon(phosphor::X, Target::Editor(EditorEvent::FindClose)).build();
                },
            );
            if finding.replacing {
                rustle::row_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(theme::sp(1)),
                    Alignment::Center,
                    || {
                        rustle::spacer(Modifier::new().width(theme::sp(7)));
                        Input::new(
                            &finding.replacement,
                            Target::Editor(EditorEvent::ReplaceField),
                        )
                        .size(Size::Sm)
                        .placeholder("Replace")
                        .modifier(Modifier::new().width(194.0))
                        .build();
                        for (label, event) in [
                            ("Replace", EditorEvent::ReplaceOne),
                            ("Replace all", EditorEvent::ReplaceAll),
                        ] {
                            rustle::key(label, || {
                                button::small_button(label, Target::Editor(event), Variant::Ghost)
                                    .disabled(read_only)
                                    .build();
                            });
                        }
                    },
                );
            }
        },
    );
}
