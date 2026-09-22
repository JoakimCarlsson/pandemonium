//! Filling the picker, and acting on what was picked.
//!
//! What every list offers is gathered here, because it is the window that
//! knows it: which projects are open, what is in their worktrees, which
//! commands apply where the keyboard is. [`crate::picker`] is the list
//! itself, which knows none of that and only narrows what it was given.

use std::path::{Path, PathBuf};

use pm_core::ProjectId;
use pm_text::Position;

use crate::app::App;
use crate::app::places::Place;
use crate::keymap::Action;
use crate::panes::Item;
use crate::picker::{Choice, Kind, Picker, Row};

/// Most results a project-wide search gathers before it stops looking.
const SEARCH_LIMIT: usize = 500;

/// Longest a line of context beside a search result is drawn.
const CONTEXT: usize = 120;

impl App {
    /// Opens the picker of `kind`, gathering what it offers.
    pub(super) fn open_picker(&mut self, kind: Kind) {
        let seeded = match kind {
            Kind::Search => self
                .with_buffer(pm_text::Buffer::selected_text)
                .unwrap_or_default(),
            _ => String::new(),
        };
        let rows = self.rows_for(kind, &seeded);
        self.open_picker_with(kind, rows, seeded);
    }

    /// Opens the picker of `kind` over `rows`, with `seeded` already typed.
    pub(super) fn open_picker_with(&mut self, kind: Kind, rows: Vec<Row>, seeded: String) {
        self.picker = Some(Picker::new(kind, rows, &seeded));
        self.completions = None;
        self.hint = None;
    }

    /// Opens the prompt `action` asks a line of text for.
    pub(super) fn open_prompt(&mut self, action: Action) {
        let kind = match action {
            Action::Rename => Kind::Rename,
            _ => Kind::Line,
        };
        let seeded = match kind {
            Kind::Rename => self
                .with_buffer(|buffer| {
                    let word = buffer.word_at(buffer.selection().head);
                    buffer.text_in(word)
                })
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.open_picker_with(kind, Vec::new(), seeded);
    }

    /// Puts the picker away, saying whether one was open.
    pub(super) fn dismiss_picker(&mut self) -> bool {
        self.path_target = None;
        self.picker.take().is_some()
    }

    /// Narrows the picker to what has been typed, re-gathering when it must.
    pub(super) fn refilter_picker(&mut self) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        if !picker.kind().is_queried() {
            return;
        }
        let (kind, query) = (picker.kind(), picker.field().value().to_owned());
        let rows = self.rows_for(kind, &query);
        if let Some(picker) = self.picker.as_mut() {
            picker.refill(rows);
        }
    }

    /// Takes what the picker has selected, and puts the picker away.
    pub(super) fn confirm_picker(&mut self) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        let kind = picker.kind();
        let typed = picker.field().value().to_owned();
        let chosen = picker.chosen().cloned();

        self.picker = None;
        match (kind, chosen) {
            (Kind::Line, _) => self.go_to_typed_line(&typed),
            (Kind::Rename, _) => self.rename_to(typed),
            (Kind::NewFile | Kind::NewFolder, _) => self.make_path(kind, &typed),
            (Kind::RenamePath, _) => self.rename_path(&typed),
            (_, Some(choice)) => self.take(choice),
            (_, None) => {}
        }
    }

    /// Chooses the `place`-th row shown, and puts the picker away.
    pub(super) fn choose_picker(&mut self, place: usize) {
        if let Some(picker) = self.picker.as_mut() {
            picker.select(place);
        }
        self.confirm_picker();
    }

    /// Carries out what one row of the picker stood for.
    fn take(&mut self, choice: Choice) {
        match choice {
            Choice::Act(action) => self.act(action),
            Choice::Open(project, path) => {
                self.jump_to(&Place {
                    project,
                    path,
                    position: Position::default(),
                });
            }
            Choice::OpenAt(project, path, position) => {
                self.jump_to(&Place {
                    project,
                    path,
                    position,
                });
            }
            Choice::Project(project) => {
                self.open.activate(project);
                self.store();
            }
        }
    }

    /// Goes to the line, and the column, a go-to-line prompt was given.
    fn go_to_typed_line(&mut self, typed: &str) {
        let mut parts = typed.split(&[':', ','][..]).map(str::trim);
        let Some(line) = parts.next().and_then(|line| line.parse::<usize>().ok()) else {
            return;
        };
        let column = parts
            .next()
            .and_then(|column| column.parse::<usize>().ok())
            .unwrap_or(1);
        let at = Position::new(line.saturating_sub(1), column.saturating_sub(1));
        if let Some(from) = self.here() {
            self.trail.jumped(from);
        }
        self.place_cursor(at, false);
    }

    /// Asks the server to rename the symbol under the cursor to `name`.
    fn rename_to(&mut self, name: String) {
        if name.is_empty() {
            return;
        }
        self.ask(pm_text::Request::Rename(name));
    }

    /// What a picker of `kind` offers, given what has been typed so far.
    fn rows_for(&self, kind: Kind, query: &str) -> Vec<Row> {
        match kind {
            Kind::Commands => self.command_rows(),
            Kind::Files => self.file_rows(),
            Kind::Projects => self.project_rows(),
            Kind::Problems => self.problem_rows(),
            Kind::References => Vec::new(),
            Kind::Search => self.search_rows(query),
            Kind::Symbols
            | Kind::Line
            | Kind::Rename
            | Kind::NewFile
            | Kind::NewFolder
            | Kind::RenamePath => Vec::new(),
        }
    }

    /// Every command the window can carry out, with the chords it answers to.
    fn command_rows(&self) -> Vec<Row> {
        let context = self.context();
        let has_buffer = self.active_file().is_some();

        Action::all()
            .map(|action| Row {
                label: action.title().to_owned(),
                detail: self
                    .resolver
                    .keymap()
                    .sequence_for(action, &context)
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                choice: Choice::Act(action),
                enabled: has_buffer || !action.needs_buffer(),
            })
            .collect()
    }

    /// Every file of every open project.
    fn file_rows(&self) -> Vec<Row> {
        self.open
            .iter()
            .flat_map(|project| {
                let (id, root) = (project.id(), project.root().to_path_buf());
                pm_core::walk(&root).into_iter().map(move |path| {
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    Row {
                        label: name,
                        detail: relative(&root, &path),
                        choice: Choice::Open(id, path),
                        enabled: true,
                    }
                })
            })
            .collect()
    }

    /// The projects the window holds open.
    fn project_rows(&self) -> Vec<Row> {
        self.open
            .iter()
            .map(|project| Row {
                label: project.name().to_owned(),
                detail: project.branch().to_owned(),
                choice: Choice::Project(project.id()),
                enabled: true,
            })
            .collect()
    }

    /// Every error and warning a server has reported in an open file.
    fn problem_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        for (project, path, file) in self.open_files() {
            let Some(document) = self.editor.get(file) else {
                continue;
            };
            let document = document.borrow();
            for found in document.buffer().diagnostics() {
                rows.push(Row {
                    label: found.message.lines().next().unwrap_or_default().to_owned(),
                    detail: format!(
                        "{}:{}",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        found.range.start.line + 1
                    ),
                    choice: Choice::OpenAt(project, path.clone(), found.range.start),
                    enabled: true,
                });
            }
        }
        rows
    }

    /// Every place `query` appears in the worktrees of the open projects.
    fn search_rows(&self, query: &str) -> Vec<Row> {
        if query.len() < 2 {
            return Vec::new();
        }
        let needle = query.to_lowercase().chars().collect::<Vec<_>>();
        let mut rows = Vec::new();

        for project in self.open.iter() {
            let (id, root) = (project.id(), project.root().to_path_buf());
            for path in pm_core::walk(&root) {
                if rows.len() >= SEARCH_LIMIT {
                    return rows;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for (line, content) in text.lines().enumerate() {
                    let Some(column) = found_at(content, &needle) else {
                        continue;
                    };
                    rows.push(Row {
                        label: content.trim().chars().take(CONTEXT).collect(),
                        detail: format!("{}:{}", relative(&root, &path), line + 1),
                        choice: Choice::OpenAt(id, path.clone(), Position::new(line, column)),
                        enabled: true,
                    });
                    if rows.len() >= SEARCH_LIMIT {
                        return rows;
                    }
                }
            }
        }
        rows
    }

    /// Every file the window has open, with the project and path it belongs to.
    fn open_files(&self) -> Vec<(ProjectId, PathBuf, crate::editor::FileId)> {
        self.panes
            .held()
            .into_iter()
            .filter_map(Item::file)
            .filter_map(|file| Some((self.editor.project_of(file)?, self.editor.path(file)?, file)))
            .collect()
    }
}

/// Which character of `line` the lower-cased `needle` first appears at.
///
/// The comparison is made over characters rather than over bytes, because
/// lower-casing a line can change how many bytes it takes: an offset into
/// the folded copy is not an offset into the line it came from.
fn found_at(line: &str, needle: &[char]) -> Option<usize> {
    let folded = line.to_lowercase().chars().collect::<Vec<_>>();
    if needle.is_empty() || needle.len() > folded.len() {
        return None;
    }
    (0..=folded.len() - needle.len())
        .find(|start| folded[*start..start + needle.len()] == *needle)
        .filter(|start| *start <= line.chars().count())
}

/// `path` written from `root` down.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
