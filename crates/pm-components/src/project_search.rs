//! Searching the text of every file of the project from the review panel: the box the search is typed in, the ways of matching and the hits, grouped by file.

use std::collections::HashSet;
use std::ops::Range;

use pm_editor::Query;
use pm_icon::IconName;
use pm_input::FocusId;
use pm_protocol::Location;
use pm_style::{
    Role, bg, cursor_pointer, flex, flex_1, flex_col, flex_none, font_mono, gap, h, items_center,
    min_h, min_w_px, overflow_hidden, pl, pr, px, py, truncate,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::file_icon::file_icon;
use crate::find::switch;
use crate::input::{Input, Size as InputSize};
use crate::parts::{glyph, line};
use crate::state::{Outgoing, ReviewTab, TREE_SCROLL, WorkspaceState};

/// The field the text looked for in the project is typed in.
pub const PROJECT_SEARCH_FIELD: FocusId = FocusId(7_406_000);

/// Searches again for what is typed.
pub const PROJECT_SEARCH_GO: FocusId = FocusId(7_406_001);

/// Switches matching case on and off.
pub const PROJECT_SEARCH_CASE: FocusId = FocusId(7_406_002);

/// Switches reading the text as a regular expression on and off.
pub const PROJECT_SEARCH_REGEX: FocusId = FocusId(7_406_003);

/// Empties the box and the hits.
pub const PROJECT_SEARCH_CLEAR: FocusId = FocusId(7_406_004);

/// Shows the search in the review panel.
pub const REVIEW_SEARCH: FocusId = FocusId(7_406_005);

/// Opens the hits as excerpts in the editor.
pub const PROJECT_SEARCH_EXCERPTS: FocusId = FocusId(7_406_006);

/// The first of the rows that fold a file's hits, each following it by the position of the file in the list.
pub const SEARCH_GROUP_BASE: u64 = 7_407_000;

/// The first of the rows that open a hit, each following it by the position of the hit among all of them.
pub const SEARCH_HIT_BASE: u64 = 7_408_000;

/// One past the last row that opens a hit.
const SEARCH_HIT_END: u64 = 7_409_000;

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
    pub query: Field,
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
}

impl Default for ProjectSearch {
    /// An empty search that has asked for nothing.
    fn default() -> Self {
        Self {
            query: Field::new(""),
            case: false,
            regex: false,
            asked: None,
            waiting: false,
            problem: None,
            groups: Vec::new(),
            folded: HashSet::new(),
        }
    }
}

impl ProjectSearch {
    /// The hits, by file.
    pub(crate) fn groups(&self) -> &[Group] {
        &self.groups
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
pub fn is_search_part(id: FocusId) -> bool {
    (PROJECT_SEARCH_FIELD.0..=PROJECT_SEARCH_EXCERPTS.0).contains(&id.0)
        || (SEARCH_GROUP_BASE..SEARCH_HIT_END).contains(&id.0)
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
        let selected = self
            .shown_document()
            .map(|document| document.field.selected().to_owned())
            .filter(|selected| !selected.is_empty() && !selected.contains('\n'))
            .filter(|_| self.editor.seed_search_from_selection);
        self.review = true;
        self.show_review_tab(ReviewTab::Search);
        if let Some(selected) = selected {
            self.project_search.query = Field::new(selected);
        }
        self.project_search.query.apply(Edit::SelectAll);
        self.project_searched()
    }

    /// The state after the text looked for was changed by `edit`.
    pub fn project_search_typed(mut self, edit: Edit) -> Self {
        self.project_search.query.apply(edit);
        self.project_searched()
    }

    /// Asks the server for the hits of what is typed, or empties the list when nothing is.
    fn project_searched(mut self) -> Self {
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
        let search = &mut self.project_search;
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
        self.tree_scroll = 0.0;
        self
    }

    /// Carries out what pressing the part `id` of the search does.
    pub(crate) fn project_search_pressed(mut self, id: FocusId) -> Self {
        match id {
            REVIEW_SEARCH => return self.project_search_opened(),
            PROJECT_SEARCH_EXCERPTS => return self.excerpts_opened(),
            PROJECT_SEARCH_GO => {
                self.project_search.asked = None;
                return self.project_searched();
            }
            PROJECT_SEARCH_CASE => {
                self.project_search.case ^= true;
                return self.project_searched();
            }
            PROJECT_SEARCH_REGEX => {
                self.project_search.regex ^= true;
                return self.project_searched();
            }
            PROJECT_SEARCH_CLEAR => {
                self.project_search.query = Field::new("");
                return self.project_searched();
            }
            _ => {}
        }
        if (SEARCH_GROUP_BASE..SEARCH_HIT_BASE).contains(&id.0) {
            let place = (id.0 - SEARCH_GROUP_BASE) as usize;
            if let Some(path) = self
                .project_search
                .groups
                .get(place)
                .map(|g| g.path.clone())
                && !self.project_search.folded.remove(&path)
            {
                self.project_search.folded.insert(path);
            }
            return self;
        }
        let Some(offset) = id.0.checked_sub(SEARCH_HIT_BASE) else {
            return self;
        };
        let location = self.project_search.hit(offset as usize).cloned();
        match (self.review_index(), location) {
            (Some(session), Some(location)) => self.go_to_location(session, location),
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

/// The row for one hit: the line it is on, with the match marked, and the number of the line.
fn hit_row(index: usize, location: &Location, matcher: &Query) -> Node {
    let text = location.line.as_deref().unwrap_or_default();
    let (before, found, after) = windowed(text, matcher.first_in(text));
    let mut words = node()
        .styles([
            flex(),
            items_center(),
            flex_1(),
            min_w_px(0),
            overflow_hidden(),
        ])
        .child(line(before, 12, 1.4, Role::Muted).styles([font_mono(), flex_none()]));
    if !found.is_empty() {
        words = words.child(
            node()
                .styles([flex_none(), bg(Role::SurfaceActive)])
                .child(line(found, 12, 1.4, Role::Text).styles([font_mono()])),
        );
    }
    words = words.child(line(after, 12, 1.4, Role::Muted).styles([font_mono(), truncate()]));
    node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(6),
            pl(9),
            pr(3),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .child(words)
        .child(
            line(
                (location.span.start.line + 1).to_string(),
                11,
                1.4,
                Role::Subtle,
            )
            .styles([font_mono(), flex_none()]),
        )
        .focusable(FocusId(SEARCH_HIT_BASE + index as u64))
}

/// The row that names a file and folds its hits.
fn group_row(place: usize, group: &Group, folded: bool) -> Node {
    let name = group.path.rsplit('/').next().unwrap_or(&group.path);
    let folder = group.path.strip_suffix(name).unwrap_or_default();
    let chevron = if folded {
        IconName::ChevronRight
    } else {
        IconName::ChevronDown
    };
    node()
        .styles([
            flex(),
            items_center(),
            gap(1.5),
            h(6),
            px(3),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .child(glyph(chevron, 3.5, Role::Muted))
        .child(file_icon(name, 4.0))
        .child(line(name, 13, 1.4, Role::Text).styles([flex_none()]))
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(folder, 12, 1.4, Role::Subtle).styles([truncate()])),
        )
        .child(line(group.hits.len().to_string(), 11, 1.4, Role::Muted).styles([flex_none()]))
        .focusable(FocusId(SEARCH_GROUP_BASE + place as u64))
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

/// The search in the review panel: its box, its switches, what it says, and the hits.
pub fn panel(state: &WorkspaceState) -> Node {
    let search = &state.project_search;
    let (said, tone) = tally(search, state.review_index().is_some());
    let matcher = search.matcher();
    let mut rows = Vec::new();
    let mut index = 0;
    for (place, group) in search.groups.iter().enumerate() {
        let folded = search.folded.contains(&group.path);
        rows.push(group_row(place, group, folded));
        for location in &group.hits {
            if !folded {
                rows.push(hit_row(index, location, &matcher));
            }
            index += 1;
        }
    }
    let list = node()
        .styles([
            flex(),
            flex_col(),
            flex_1(),
            min_h(0),
            overflow_hidden(),
            py(1),
        ])
        .scroll_from_start(TREE_SCROLL, state.tree_scroll)
        .children(rows);
    let clear = (!search.query.text().is_empty()).then(|| {
        crate::button::Button::icon(IconName::Close, PROJECT_SEARCH_CLEAR)
            .variant(crate::button::Variant::Ghost)
            .size(crate::button::Size::Icon)
            .build()
    });
    node()
        .styles([flex(), flex_col(), flex_1(), min_h(0)])
        .child(
            node()
                .styles([flex(), flex_col(), gap(1.5), px(3), py(2), flex_none()])
                .child(
                    node()
                        .styles([flex(), items_center(), gap(1)])
                        .child(
                            node().styles([flex_1(), min_w_px(0)]).child(
                                Input::new(&search.query, PROJECT_SEARCH_FIELD)
                                    .size(InputSize::Sm)
                                    .placeholder("Search")
                                    .invalid(search.problem.is_some())
                                    .build(),
                            ),
                        )
                        .children(clear)
                        .child(switch("Aa", PROJECT_SEARCH_CASE, search.case))
                        .child(switch(".*", PROJECT_SEARCH_REGEX, search.regex)),
                )
                .child(
                    node()
                        .styles([flex(), items_center(), gap(1)])
                        .child(
                            node()
                                .styles([flex_1(), min_w_px(0)])
                                .child(line(said, 12, 1.4, tone)),
                        )
                        .children((search.total() > 0).then(|| {
                            crate::button::Button::icon(IconName::FileCode, PROJECT_SEARCH_EXCERPTS)
                                .variant(crate::button::Variant::Ghost)
                                .size(crate::button::Size::Icon)
                                .build()
                                .tooltip("Open Results in Editor")
                        })),
                ),
        )
        .child(list)
}
