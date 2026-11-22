//! Searching the text of every file of the project from the review panel: the box the search is typed in, the ways of matching and the hits, grouped by file.

use crate::areas::SearchEvent;
use crate::events;
use crate::icons;
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, TextOverflow};

use std::collections::HashSet;
use std::ops::Range;

use crate::theme::Role;
use pm_editor::Query;
use pm_protocol::Location;
use rustle::Modifier;
use rustle::TextFieldState;

use crate::file_icon::file_icon;
use crate::find::switch;
use crate::input::Input;
use crate::parts::{glyph, line};
use crate::size::Size;
use crate::state::{Outgoing, ReviewTab, WorkspaceState};

/// How many characters of a line are kept before the match, so that the match shows in a narrow panel.
const LEAD: usize = 24;

/// The hits in one file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    /// The file, from the root of the project.
    pub path: String,
    /// Where in the file, in the order of the file.
    pub hits: Vec<Location>,
}

/// What was asked of the server last: the session, the text, whether case matters and whether the text is a regular expression.
type Asked = (usize, String, bool, bool);

/// The search of the review panel and what it found.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectSearch {
    /// What is looked for.
    pub query: TextFieldState,
    /// Whether upper and lower case must match.
    pub case: bool,
    /// Whether the text is a regular expression.
    pub regex: bool,
    /// What the hits shown answer, and nothing before the first search.
    asked: Option<Asked>,
    /// Whether the server has yet to answer.
    waiting: bool,
    /// Why the search could not be made, when it could not.
    problem: Option<String>,
    /// The hits, by file.
    groups: Vec<Group>,
    /// The files whose hits are folded away.
    folded: HashSet<String>,
    /// Where the list of hits is scrolled.
    list: rustle::LazyListState,
}

/// One row of the list of hits.
#[derive(Clone, Copy)]
enum Row<'a> {
    /// The row naming the file at this place, folded away when the flag says so.
    File(usize, &'a Group, bool),
    /// The hit with this number, counted over every file, at this place.
    Hit(usize, &'a Location),
}

impl Default for ProjectSearch {
    /// An empty search that has asked for nothing.
    fn default() -> Self {
        Self {
            query: TextFieldState::new(""),
            case: false,
            regex: false,
            asked: None,
            waiting: false,
            problem: None,
            groups: Vec::new(),
            folded: HashSet::new(),
            list: rustle::LazyListState::default(),
        }
    }
}

impl ProjectSearch {
    /// The hits, by file.
    pub(crate) fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// The rows of the list of hits, in order: each file, followed by its hits unless they are folded away.
    fn rows(&self) -> Vec<Row<'_>> {
        let mut rows = Vec::new();
        let mut index = 0;
        for (place, group) in self.groups.iter().enumerate() {
            let folded = self.folded.contains(&group.path);
            rows.push(Row::File(place, group, folded));
            for location in &group.hits {
                if !folded {
                    rows.push(Row::Hit(index, location));
                }
                index += 1;
            }
        }
        rows
    }

    /// How many hits there are in all.
    fn total(&self) -> usize {
        self.groups.iter().map(|group| group.hits.len()).sum()
    }

    /// The hit at `index` among all of them, in the order they are listed.
    fn hit(&self, index: usize) -> Option<&Location> {
        self.groups
            .iter()
            .flat_map(|group| group.hits.iter())
            .nth(index)
    }

    /// What is looked for, as the server is asked and as the hits are marked.
    fn matcher(&self) -> Query {
        Query {
            text: self.query.text().to_owned(),
            case: self.case,
            word: false,
            regex: self.regex,
        }
    }
}

/// Whether `id` is a field, button or row of the search.
pub fn is_search_part(id: Target) -> bool {
    matches!(
        id,
        Target::Search(SearchEvent::ProjectSearchField)
            | Target::Search(SearchEvent::ProjectSearchGo)
            | Target::Search(SearchEvent::ProjectSearchCase)
            | Target::Search(SearchEvent::ProjectSearchRegex)
            | Target::Search(SearchEvent::ProjectSearchClear)
            | Target::Search(SearchEvent::ReviewSearch)
            | Target::Search(SearchEvent::ProjectSearchExcerpts)
            | Target::Search(SearchEvent::SearchGroup(_))
            | Target::Search(SearchEvent::SearchHit(_))
    )
}

/// The hits of `locations` gathered by file, files and hits in the order they came.
fn grouped(locations: Vec<Location>) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for location in locations {
        match groups.last_mut() {
            Some(group) if group.path == location.path => group.hits.push(location),
            _ => groups.push(Group {
                path: location.path.clone(),
                hits: vec![location],
            }),
        }
    }
    groups
}

impl WorkspaceState {
    /// Shows the search in the review panel, looking for what is selected in the file shown when that is part of one line, and asks again when the box holds something that was not searched for yet.
    pub fn project_search_opened(mut self) -> Self {
        let selected = self.selection_to_seek();
        self.review = true;
        self.show_review_tab(ReviewTab::Search);
        if let Some(selected) = selected {
            self.project_search.query = TextFieldState::new(selected);
        }
        self.project_search.query.select_all();
        self.project_search_typed()
    }

    /// The state after the reader changed the text looked for: the server is asked for its hits, or the list is emptied when nothing is typed.
    pub fn project_search_typed(mut self) -> Self {
        let session = self.review_index();
        let search = &mut self.project_search;
        let text = search.query.text().to_owned();
        let Some(session) = session.filter(|_| !text.is_empty()) else {
            search.asked = None;
            search.waiting = false;
            search.problem = None;
            search.groups.clear();
            return self;
        };
        let asked = (session, text.clone(), search.case, search.regex);
        if search.asked.as_ref() == Some(&asked) {
            return self;
        }
        search.asked = Some(asked);
        search.waiting = true;
        search.problem = None;
        self.outbox.push(Outgoing::SearchProject {
            session,
            query: text,
            case: self.project_search.case,
            regex: self.project_search.regex,
        });
        self
    }

    /// The state after the server answered a search of the project of the session at `session` for `query`, with `hits` or with why it could not.
    pub fn with_project_search(
        mut self,
        session: usize,
        query: &str,
        case: bool,
        regex: bool,
        hits: Result<Vec<Location>, String>,
    ) -> Self {
        let search = match self.search_stash.get_mut(&session) {
            Some(kept) => kept,
            None => &mut self.project_search,
        };
        let answers = search.asked.as_ref() == Some(&(session, query.to_owned(), case, regex));
        if !answers {
            return self;
        }
        search.waiting = false;
        match hits {
            Ok(locations) => {
                search.problem = None;
                search.groups = grouped(locations);
                search
                    .folded
                    .retain(|path| search.groups.iter().any(|group| &group.path == path));
            }
            Err(problem) => {
                search.problem = Some(problem);
                search.groups.clear();
            }
        }
        search.list.scroll().jump_to(0.0);
        self
    }

    /// Carries out what pressing the part `id` of the search does.
    pub(crate) fn project_search_pressed(mut self, id: Target) -> Self {
        let Target::Search(event) = id else {
            return self;
        };
        match event {
            SearchEvent::ReviewSearch => self.project_search_opened(),
            SearchEvent::ProjectSearchExcerpts => self.excerpts_opened(),
            SearchEvent::ProjectSearchGo => {
                self.project_search.asked = None;
                self.project_search_typed()
            }
            SearchEvent::ProjectSearchCase => {
                self.project_search.case ^= true;
                self.project_search_typed()
            }
            SearchEvent::ProjectSearchRegex => {
                self.project_search.regex ^= true;
                self.project_search_typed()
            }
            SearchEvent::ProjectSearchClear => {
                self.project_search.query = TextFieldState::new("");
                self.project_search_typed()
            }
            SearchEvent::SearchGroup(place) => {
                if let Some(path) = self
                    .project_search
                    .groups
                    .get(place)
                    .map(|g| g.path.clone())
                    && !self.project_search.folded.remove(&path)
                {
                    self.project_search.folded.insert(path);
                }
                self
            }
            SearchEvent::SearchHit(offset) => {
                let location = self.project_search.hit(offset).cloned();
                match (self.review_index(), location) {
                    (Some(session), Some(location)) => self.go_to_location(session, location),
                    _ => self,
                }
            }
            _ => self,
        }
    }
}

/// The text of `line` cut to show the match at `found`: the part before it, the match, and the part after it, with an ellipsis when the start was left out.
fn windowed(line: &str, found: Option<Range<usize>>) -> (String, String, String) {
    let line = line.trim_end();
    let trimmed = line.trim_start();
    let skipped = line.len() - trimmed.len();
    let Some(found) = found else {
        return (trimmed.to_owned(), String::new(), String::new());
    };
    let start = found.start.saturating_sub(skipped);
    let end = found.end.saturating_sub(skipped).min(trimmed.len());
    if start >= end || !trimmed.is_char_boundary(start) || !trimmed.is_char_boundary(end) {
        return (trimmed.to_owned(), String::new(), String::new());
    }
    let before = &trimmed[..start];
    let kept = before.chars().count().saturating_sub(LEAD);
    let cut = before.char_indices().nth(kept).map_or(0, |(at, _)| at);
    let lead = if cut > 0 {
        format!("…{}", &before[cut..])
    } else {
        before.to_owned()
    };
    (
        lead,
        trimmed[start..end].to_owned(),
        trimmed[end..].to_owned(),
    )
}

/// A row of the list across the panel, lit under the pointer, made the control `id` that sends its press.
#[track_caller]
fn pressed_row(id: Target) -> Modifier {
    let own = Modifier::new()
        .fill_max_width()
        .height(theme::sp(6))
        .indicated(
            theme::InteractionTones::new().hover(Role::SurfaceHover),
            theme::Focused::Control(crate::focus::is_focused(id)),
        );
    events::pressed(own, id)
}

/// Emits the row for one hit: the line it is on, with the match marked, and the number of the line.
fn hit_row(index: usize, location: &Location, matcher: &Query) {
    let text = location.line.as_deref().unwrap_or_default();
    let (before, found, after) = windowed(text, matcher.first_in(text));
    let modifier = pressed_row(Target::Search(SearchEvent::SearchHit(index))).padding_each(
        theme::sp(9),
        0.0,
        theme::sp(3),
        0.0,
    );
    let code = theme::mono_style(12, 1.4);
    rustle::row_arranged(
        modifier,
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            rustle::row_arranged(
                Modifier::new().weight(1.0).clip(),
                Arrangement::Start,
                Alignment::Center,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        before,
                        code.color(theme::color(Role::Muted)),
                        TextOverflow::Ellipsis,
                    );
                    if !found.is_empty() {
                        rustle::basic_text(
                            Modifier::new().background(theme::color(Role::SurfaceActive)),
                            found,
                            code.color(theme::color(Role::Text)).max_lines(1),
                            TextOverflow::Ellipsis,
                        );
                    }
                    rustle::basic_text(
                        Modifier::new().weight(1.0),
                        after,
                        code.color(theme::color(Role::Muted)).max_lines(1),
                        TextOverflow::Ellipsis,
                    );
                },
            );
            rustle::basic_text(
                Modifier::new(),
                (location.span.start.line + 1).to_string(),
                theme::mono_style(11, 1.4).color(theme::color(Role::Subtle)),
                TextOverflow::Ellipsis,
            );
        },
    );
}

/// Emits the row that names a file and folds its hits.
fn group_row(place: usize, group: &Group, folded: bool) {
    let name = group.path.rsplit('/').next().unwrap_or(&group.path);
    let folder = group.path.strip_suffix(name).unwrap_or_default();
    let modifier =
        pressed_row(Target::Search(SearchEvent::SearchGroup(place))).padding_xy(theme::sp(3), 0.0);
    let chevron = icons::chevron(!folded);
    rustle::row_arranged(
        modifier,
        Arrangement::SpacedBy(theme::sp(1.5)),
        Alignment::Center,
        || {
            glyph(chevron, 3.5, Role::Muted);
            file_icon(name, 4.0);
            line(name, 13, 1.4, Role::Text);
            rustle::basic_text(
                Modifier::new().weight(1.0),
                folder,
                theme::line(12, Role::Subtle),
                TextOverflow::Ellipsis,
            );
            line(group.hits.len().to_string(), 11, 1.4, Role::Muted);
        },
    );
}

/// What the search says under its box: how much it found, that it is looking, or why it cannot.
fn tally(search: &ProjectSearch, has_project: bool) -> (String, Role) {
    if !has_project {
        return (
            "Open a project to search its files".to_owned(),
            Role::Subtle,
        );
    }
    if let Some(problem) = &search.problem {
        return (problem.clone(), Role::Danger);
    }
    if search.query.text().is_empty() {
        return (
            "Type to search the text of every file in the project".to_owned(),
            Role::Subtle,
        );
    }
    if search.waiting {
        return ("Searching…".to_owned(), Role::Muted);
    }
    match (search.total(), search.groups.len()) {
        (0, _) => ("No results".to_owned(), Role::Danger),
        (1, _) => ("1 result in 1 file".to_owned(), Role::Muted),
        (hits, 1) => (format!("{hits} results in 1 file"), Role::Muted),
        (hits, files) => (format!("{hits} results in {files} files"), Role::Muted),
    }
}

/// Emits the search in the review panel: its box, its switches, what it says, and the hits.
#[track_caller]
pub fn panel(state: &WorkspaceState) {
    let search = &state.project_search;
    let (said, tone) = tally(search, state.review_index().is_some());
    let matcher = search.matcher();
    rustle::column_arranged(
        Modifier::new().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .chain(crate::focus::keeps_keyboard_in(Target::Search(
                        SearchEvent::ProjectSearchField,
                    )))
                    .padding_xy(theme::sp(3), theme::sp(2)),
                Arrangement::SpacedBy(theme::sp(1.5)),
                Alignment::Start,
                || {
                    rustle::row_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(theme::sp(1)),
                        Alignment::Center,
                        || {
                            Input::new(
                                &search.query,
                                Target::Search(SearchEvent::ProjectSearchField),
                            )
                            .size(Size::Sm)
                            .placeholder("Search")
                            .invalid(search.problem.is_some())
                            .modifier(Modifier::new().weight(1.0))
                            .submits(Target::Search(SearchEvent::ProjectSearchGo))
                            .build();
                            if !search.query.text().is_empty() {
                                crate::button::Button::icon(
                                    phosphor::X,
                                    Target::Search(SearchEvent::ProjectSearchClear),
                                )
                                .build();
                            }
                            switch(
                                "Aa",
                                Target::Search(SearchEvent::ProjectSearchCase),
                                search.case,
                            );
                            switch(
                                ".*",
                                Target::Search(SearchEvent::ProjectSearchRegex),
                                search.regex,
                            );
                        },
                    );
                    rustle::row_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(theme::sp(1)),
                        Alignment::Center,
                        || {
                            rustle::basic_text(
                                Modifier::new().weight(1.0),
                                said.as_str(),
                                theme::line(12, tone),
                                TextOverflow::Ellipsis,
                            );
                            if search.total() > 0 {
                                crate::button::Button::icon(
                                    phosphor::CODE,
                                    Target::Search(SearchEvent::ProjectSearchExcerpts),
                                )
                                .tooltip("Open Results in Editor")
                                .build();
                            }
                        },
                    );
                },
            );
            let rows = search.rows();
            rustle::lazy_list_with(
                Modifier::new().fill_max_width().weight(1.0).clip(),
                &search.list,
                rustle::LazyContent::new().padding(theme::sp(1)),
                rows.len(),
                |at| match rows[at] {
                    Row::File(place, _, _) => (place, None),
                    Row::Hit(index, _) => (usize::MAX, Some(index)),
                },
                |at| match rows[at] {
                    Row::File(place, group, folded) => group_row(place, group, folded),
                    Row::Hit(index, location) => hit_row(index, location, &matcher),
                },
            );
        },
    );
}
