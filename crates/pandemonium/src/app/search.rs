//! Search and replace within one worktree's live documents.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pm_core::Scope;
use pm_text::{Finder, Position, Query};
use pm_ui::{Div, Element, Styled, Theme, h_flex, text, v_flex};

use crate::app::listing::{SEARCH_LIMIT, Search as ListingSearch, SearchBatch, fingerprint};
use crate::app::{App, Wake};
use crate::editor::FileId;
use crate::editor::SearchField;
use crate::excerpts::{Excerpted, Excerpts, OpenExcerpts, excerpts_view};
use crate::input::{Input, hinted_input_view};
use crate::keymap::Action;
use crate::message::{Message, ProjectSearchOption};
use crate::panes::{Item, PaneId};
use crate::prompt::{Answer, Prompt};

/// Height of a search header row.
const ROW_HEIGHT: f32 = 30.0;

/// State of a worktree's search pane.
pub(super) struct ProjectSearch {
    /// Text being looked for.
    pub(super) query: Input,
    /// Text to put in its place.
    pub(super) replacement: Input,
    /// Matching options.
    options: Query,
    /// Invalid regular expression error.
    error: Option<String>,
    /// Live excerpt windows over the matching documents.
    pub(super) excerpts: OpenExcerpts,
    /// Files edited through the pane, kept open even after they stop matching.
    pub(super) held: BTreeSet<FileId>,
    /// File to keep selected after replacing one match.
    preferred: Option<FileId>,
    /// Background results waiting for the window.
    pending: Arc<Mutex<Vec<SearchBatch>>>,
    /// The current query generation.
    wanted: Arc<AtomicU64>,
    /// Whether the current search has completed.
    done: bool,
    /// Whether there are unseen matches past the result limit.
    limited: bool,
}

impl ProjectSearch {
    /// Creates an empty search pane.
    pub(super) fn new() -> Self {
        Self {
            query: Input::default(),
            replacement: Input::default(),
            options: Query::default(),
            error: None,
            excerpts: Rc::new(RefCell::new(Excerpts::default())),
            held: BTreeSet::new(),
            preferred: None,
            pending: Arc::new(Mutex::new(Vec::new())),
            wanted: Arc::new(AtomicU64::new(0)),
            done: true,
            limited: false,
        }
    }

    /// The current complete query.
    fn query(&self) -> Query {
        Query {
            text: self.query.value().to_owned(),
            ..self.options.clone()
        }
    }

    /// How many matches and files are currently shown.
    fn counts(&self) -> (usize, usize) {
        let mut excerpts = self.excerpts.borrow_mut();
        excerpts
            .files_mut()
            .iter_mut()
            .fold((0, 0), |(matches, files), file| {
                let count = file.matches().len();
                (matches + count, files + usize::from(count > 0))
            })
    }

    /// Starts a new background search, replacing the visible results.
    fn search(
        &mut self,
        root: PathBuf,
        snapshots: HashMap<PathBuf, String>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) {
        let generation = self.wanted.fetch_add(1, Ordering::AcqRel) + 1;
        for file in self.excerpts.borrow().files() {
            if file.document.borrow().buffer().is_dirty() {
                self.held.insert(file.file);
            }
        }
        self.excerpts.borrow_mut().set_files(Vec::new());
        self.done = false;
        self.limited = false;
        if let Ok(mut pending) = self.pending.lock() {
            pending.clear();
        }
        let query = self.query();
        let finder = match Finder::new(&query) {
            Ok(finder) => {
                self.error = None;
                finder
            }
            Err(error) => {
                self.error = Some(error);
                self.done = true;
                return;
            }
        };
        if query.text.is_empty() {
            self.done = true;
            return;
        }
        ListingSearch::run_project(
            root,
            snapshots,
            finder,
            generation,
            self.wanted.clone(),
            self.pending.clone(),
            wake,
        );
    }
}

impl Drop for ProjectSearch {
    /// Stops the background walk when its pane closes.
    fn drop(&mut self) {
        self.wanted.fetch_add(1, Ordering::AcqRel);
    }
}

impl App {
    /// Opens or focuses the search pane of the worktree in front.
    pub(super) fn open_project_search(&mut self, seeded: Option<String>) {
        let Some(scope) = self.scope() else {
            return;
        };
        let newly = !self.searches.contains_key(&scope);
        let selected = newly
            .then(|| {
                self.active_file().and_then(|file| {
                    let text = file.borrow().buffer().selected_text();
                    (!text.is_empty() && !text.contains('\n')).then_some(text)
                })
            })
            .flatten();
        self.searches
            .entry(scope)
            .or_insert_with(ProjectSearch::new);
        if let Some(text) = seeded.or(selected)
            && let Some(search) = self.searches.get_mut(&scope)
        {
            search.query.set(&text);
        }
        let item = Item::Search(scope);
        let holder = self.panes.panes().into_iter().find(|pane| {
            self.panes
                .pane(*pane)
                .is_some_and(|pane| pane.items().any(|held| held == item))
        });
        match holder {
            Some(pane) => self.activate_tab(pane, item),
            None => self.show_item(self.panes.focus(), scope, item, false),
        }
        self.project_search_field = Some(SearchField::Query);
        if newly
            || self
                .searches
                .get(&scope)
                .is_some_and(|search| !search.query.is_empty())
        {
            self.run_project_search(scope);
        }
    }

    /// Restarts the search after its query or matching options change.
    pub(super) fn run_project_search(&mut self, scope: Scope) {
        let Some(root) = self.root_of(scope) else {
            return;
        };
        let snapshots = self.editor.search_snapshots(scope);
        let wake = self.waker(Wake::Listing);
        if let Some(search) = self.searches.get_mut(&scope) {
            search.search(root, snapshots, wake);
        }
    }

    /// Takes the background search's completed files into live excerpts.
    pub(super) fn take_project_searches(&mut self) -> bool {
        let scopes = self.searches.keys().copied().collect::<Vec<_>>();
        let mut changed = false;
        for scope in scopes {
            let Some(root) = self.root_of(scope) else {
                continue;
            };
            let Some(search) = self.searches.get(&scope) else {
                continue;
            };
            let batches = search
                .pending
                .lock()
                .map(|mut pending| std::mem::take(&mut *pending))
                .unwrap_or_default();
            let generation = search.wanted.load(Ordering::Acquire);
            let query = search.query();
            let mut stale = false;
            for batch in batches {
                if batch.generation != generation {
                    continue;
                }
                for found in batch.files {
                    let Some(file) = self.editor.open(scope, &root, &found.path, true) else {
                        continue;
                    };
                    let Some(document) = self.editor.get(file) else {
                        continue;
                    };
                    if fingerprint(&document.borrow().buffer().contents()) != found.fingerprint {
                        stale = true;
                        continue;
                    }
                    let name = found
                        .path
                        .strip_prefix(&root)
                        .unwrap_or(&found.path)
                        .display()
                        .to_string();
                    let excerpted =
                        Excerpted::matched(file, document, name, query.clone(), found.ranges);
                    if let Some(search) = self.searches.get_mut(&scope) {
                        search.excerpts.borrow_mut().push_file(excerpted);
                        if search.preferred == Some(file) {
                            search.excerpts.borrow_mut().activate(file);
                        }
                    }
                    changed = true;
                }
                if let Some(search) = self.searches.get_mut(&scope) {
                    search.done |= batch.done;
                    search.limited |= batch.limited;
                    if batch.done {
                        search.preferred = None;
                    }
                }
                changed |= batch.done || batch.limited;
            }
            if stale {
                self.run_project_search(scope);
                changed = true;
            }
        }
        changed
    }

    /// Builds the search header and its shared excerpts view.
    pub(super) fn project_search_content(
        &self,
        theme: &Theme,
        pane: PaneId,
        scope: Scope,
    ) -> Box<dyn Element<Message>> {
        let search = &self.searches[&scope];
        let focused = self.panes.focus() == pane;
        let (count, files) = search.counts();
        let status = if search.limited {
            format!("Showing the first {SEARCH_LIMIT} matches")
        } else {
            format!("{count} matches in {files} files")
        };
        let query = search_field(
            theme,
            pane,
            SearchField::Query,
            &search.query,
            focused && self.project_search_field == Some(SearchField::Query),
            "Search one line at a time",
        );
        let replacement = search_field(
            theme,
            pane,
            SearchField::Replacement,
            &search.replacement,
            focused && self.project_search_field == Some(SearchField::Replacement),
            "Replace",
        );
        Box::new(
            v_flex()
                .w_full()
                .h_full()
                .child(
                    v_flex()
                        .w_full()
                        .px(1)
                        .py(0.5)
                        .gap(0.5)
                        .bg(theme.colors.surface)
                        .child(
                            h_flex()
                                .w_full()
                                .h_px(ROW_HEIGHT)
                                .gap(0.5)
                                .items_center()
                                .child(query)
                                .child(search_toggle(
                                    theme,
                                    pane,
                                    ".*",
                                    search.options.regex,
                                    ProjectSearchOption::Regex,
                                ))
                                .child(search_toggle(
                                    theme,
                                    pane,
                                    "Aa",
                                    search.options.case_sensitive,
                                    ProjectSearchOption::Case,
                                ))
                                .child(search_toggle(
                                    theme,
                                    pane,
                                    "ab",
                                    search.options.whole_word,
                                    ProjectSearchOption::Word,
                                )),
                        )
                        .when_some(search.error.as_deref(), |header, error| {
                            header
                                .child(text(error.to_owned()).text_xs().color(theme.colors.danger))
                        })
                        .child(
                            h_flex()
                                .w_full()
                                .h_px(ROW_HEIGHT)
                                .gap(0.5)
                                .items_center()
                                .child(replacement)
                                .child(search_button(
                                    theme,
                                    pane,
                                    "Replace",
                                    Action::ReplaceMatch,
                                    count > 0,
                                ))
                                .child(search_button(
                                    theme,
                                    pane,
                                    "Replace in File",
                                    Action::ReplaceInFile,
                                    count > 0,
                                ))
                                .child(search_button(
                                    theme,
                                    pane,
                                    "Replace All",
                                    Action::ReplaceAll,
                                    count > 0
                                        && count <= SEARCH_LIMIT
                                        && search.done
                                        && !search.limited
                                        && search.error.is_none(),
                                )),
                        )
                        .when(search.error.is_none(), |header| {
                            header.child(text(status).text_xs().color(theme.colors.text_subtle))
                        }),
                )
                .child(
                    excerpts_view(
                        search.excerpts.clone(),
                        focused && self.project_search_field.is_none(),
                    )
                    .on_select(move |phase, file, anchor, head| {
                        Message::SelectExcerpt(pane, phase, file, anchor, head)
                    })
                    .on_open(move |index| Message::OpenExcerptFile(pane, index)),
                ),
        )
    }

    /// Flips a matching option and reruns the search.
    pub(super) fn toggle_project_search(&mut self, option: ProjectSearchOption) {
        let Some(Item::Search(scope)) = self.active_tab() else {
            return;
        };
        let Some(search) = self.searches.get_mut(&scope) else {
            return;
        };
        match option {
            ProjectSearchOption::Regex => search.options.regex = !search.options.regex,
            ProjectSearchOption::Case => {
                search.options.case_sensitive = !search.options.case_sensitive
            }
            ProjectSearchOption::Word => search.options.whole_word = !search.options.whole_word,
        }
        self.run_project_search(scope);
    }

    /// Moves the result cursor to the next or previous match across files.
    pub(super) fn step_project_match(&mut self, forward: bool) {
        let Some(Item::Search(scope)) = self.active_tab() else {
            return;
        };
        let Some(search) = self.searches.get(&scope) else {
            return;
        };
        let mut excerpts = search.excerpts.borrow_mut();
        let active = excerpts.active();
        let head = active
            .and_then(|file| self.editor.get(file))
            .map(|document| document.borrow().buffer().selection().head);
        let found = excerpts
            .files_mut()
            .iter_mut()
            .flat_map(|file| {
                let id = file.file;
                file.matches()
                    .iter()
                    .cloned()
                    .map(|range| (id, range))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        if found.is_empty() {
            return;
        }
        let at = found
            .iter()
            .position(|(file, range)| {
                Some(*file) == active
                    && head.is_some_and(|head| range.start <= head && head < range.end)
            })
            .or_else(|| {
                found.iter().position(|(file, range)| {
                    Some(*file) == active && head.is_some_and(|head| range.start >= head)
                })
            })
            .unwrap_or(0);
        let next = if forward {
            (at + 1) % found.len()
        } else {
            (at + found.len() - 1) % found.len()
        };
        let (file, range) = &found[next];
        excerpts.activate(*file);
        if let Some(document) = self.editor.get(*file) {
            document
                .borrow_mut()
                .edit(|buffer| buffer.place(range.start, false));
        }
        self.project_search_field = None;
    }

    /// Asks before replacing matches in more than one file.
    pub(super) fn ask_project_replace_all(&mut self) {
        let Some(Item::Search(scope)) = self.active_tab() else {
            return;
        };
        let Some(search) = self.searches.get(&scope) else {
            return;
        };
        if search.error.is_some()
            || search.limited
            || !search.done
            || search.counts().0 > SEARCH_LIMIT
        {
            return;
        }
        let (matches, files) = search.counts();
        if matches == 0 {
            return;
        }
        if files <= 1 {
            self.replace_project(false, true);
            return;
        }
        self.ask_first(Prompt::asking(
            format!("Replace {matches} matches in {files} files?"),
            Vec::new(),
            vec![
                Answer::new("Replace All", Message::ConfirmProjectReplace),
                Answer::cancel(),
            ],
        ));
    }

    /// Replaces the selected match or all matches in the selected file.
    pub(super) fn replace_project(&mut self, all_in_file: bool, all_files: bool) {
        let Some(Item::Search(scope)) = self.active_tab() else {
            return;
        };
        let Some(search) = self.searches.get(&scope) else {
            return;
        };
        if search.error.is_some()
            || (all_files && (search.limited || !search.done || search.counts().0 > SEARCH_LIMIT))
        {
            return;
        }
        let Ok(finder) = Finder::new(&search.query()) else {
            return;
        };
        let replacement = search.replacement.value().to_owned();
        let mut excerpts = search.excerpts.borrow_mut();
        let active = excerpts.active();
        let files = excerpts
            .files_mut()
            .iter_mut()
            .filter(|file| all_files || Some(file.file) == active)
            .map(|file| (file.file, file.document.clone(), file.matches().to_vec()))
            .collect::<Vec<_>>();
        drop(excerpts);
        for (file, document, matches) in files {
            let mut document = document.borrow_mut();
            let buffer = document.buffer();
            let head = buffer.selection().head;
            let selected = if all_files || all_in_file {
                matches
            } else {
                matches
                    .iter()
                    .find(|range| range.start <= head && head <= range.end)
                    .or_else(|| matches.iter().find(|range| range.start >= head))
                    .or_else(|| matches.first())
                    .cloned()
                    .into_iter()
                    .collect()
            };
            let replaced_at = selected.first().map(|range| range.start);
            let edits: Vec<_> = selected
                .into_iter()
                .map(|range| {
                    let line = buffer.line_text(range.start.line);
                    let text = finder.replacement(
                        &line,
                        range.start.column..range.end.column,
                        &replacement,
                    );
                    (range, text)
                })
                .collect();
            if edits.is_empty() {
                continue;
            }
            document.edit(|buffer| {
                buffer.apply_edits(edits);
                if let Some(replaced_at) = replaced_at.filter(|_| !all_in_file && !all_files) {
                    let next = (0..buffer.line_count())
                        .flat_map(|line| {
                            finder
                                .line(&buffer.line_text(line))
                                .into_iter()
                                .map(move |range| Position::new(line, range.start))
                        })
                        .find(|at| *at > replaced_at)
                        .or_else(|| {
                            (0..buffer.line_count())
                                .flat_map(|line| {
                                    finder
                                        .line(&buffer.line_text(line))
                                        .into_iter()
                                        .map(move |range| Position::new(line, range.start))
                                })
                                .next()
                        });
                    if let Some(next) = next {
                        buffer.place(next, false);
                    }
                }
            });
            if let Some(search) = self.searches.get_mut(&scope) {
                search.held.insert(file);
                if !all_in_file && !all_files {
                    search.preferred = Some(file);
                }
            }
        }
        self.project_search_field = None;
        self.run_project_search(scope);
    }
}

/// Draws one field of the project search header.
fn search_field(
    theme: &Theme,
    pane: PaneId,
    which: SearchField,
    value: &Input,
    focused: bool,
    placeholder: &str,
) -> Div<Message> {
    hinted_input_view(
        theme,
        value,
        focused,
        focused,
        placeholder,
        move |phase, anchor, head| Message::WriteProjectSearch(pane, which, phase, anchor, head),
        Message::ShowInputMenu,
    )
    .flex_1()
    .h_px(ROW_HEIGHT - 6.0)
    .border_1(if focused {
        theme.colors.border_focused
    } else {
        theme.colors.border
    })
}

/// Draws one matching option switch.
fn search_toggle(
    theme: &Theme,
    pane: PaneId,
    label: &str,
    on: bool,
    option: ProjectSearchOption,
) -> Div<Message> {
    h_flex()
        .h_px(ROW_HEIGHT - 8.0)
        .px(0.75)
        .items_center()
        .rounded(theme.radius.md)
        .when(on, |view| view.bg(theme.colors.accent))
        .on_click(Message::ToggleProjectSearch(pane, option))
        .child(text(label.to_owned()).text_xs().color(if on {
            theme.colors.text_on_accent
        } else {
            theme.colors.text_muted
        }))
}

/// Draws a replace command button.
fn search_button(
    theme: &Theme,
    pane: PaneId,
    label: &str,
    action: Action,
    enabled: bool,
) -> Div<Message> {
    h_flex()
        .h_px(ROW_HEIGHT - 8.0)
        .px(0.75)
        .items_center()
        .rounded(theme.radius.md)
        .when(enabled, |view| {
            view.on_click(Message::PaneAction(pane, action))
        })
        .child(text(label.to_owned()).text_xs().color(if enabled {
            theme.colors.text_muted
        } else {
            theme.colors.text_subtle
        }))
}
