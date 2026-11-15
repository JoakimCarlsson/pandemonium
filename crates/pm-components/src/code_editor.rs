//! A code editor: a document's lines in the monospace face with their
//! numbers, coloured by its syntax, with carets and selections drawn, edited
//! with the keys (vim's when asked), with a bar to find and replace, a bar
//! to go to a line and a status line under it all. Beside the numbers it
//! marks breakpoints and the lines changed since the last commit; after a
//! line it shows the values a paused debugger gives; the blocks the view is
//! inside stay at its top, and a minimap with marks runs down its side.
//! When a language server reads the file it is kept told of each change, and
//! what it says shows in and over the text: names coloured by what they are,
//! hints inside the lines, lenses after them, the places the name at the
//! caret is used, a lightbulb where changes are offered, colour swatches,
//! completions, hover cards and signatures. Selected text can be dragged to
//! another place, and the caret moves along the screen through right-to-left text.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use pm_editor::{
    CursorShape, Document, FIX_ALL, LineChange, ModalView, ORGANIZE_IMPORTS, ServerAsk,
};
use pm_text::{Edit, Motion};
use pm_vim::{Command as VimCommand, Effect, Shape, Vim};
use rustle::layout_text;
use rustle::prelude::*;
use rustle::{ItemAlign, KeyEvent, PointerButton, PointerPhase};

use crate::app_theme::{ExtendedColors, extended_colors};
use crate::code_assist::{Assist, CodeEditorPopup, Grid, assist_layer, colour_of};
use crate::code_bars::{Bars, CodeEditorBar, FindBar, find_bar, go_to_bar};
use crate::code_keys::{Command, command, keystroke};
use crate::code_look::CodeLook;
use crate::code_overview::{Marks, gutter_colour, overview};
use crate::code_sticky::sticky_lines;
use crate::code_text::{byte_at_column, column_of, digits, expand_tabs, kind_colour, runs};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::{ColorScheme, color_scheme};

/// How a code editor behaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CodeEditorVariant {
    /// The text can be changed.
    #[default]
    Editor,
    /// The text can be read, selected and copied, not changed.
    Viewer,
}

/// A code editor over a shared document, with a variant and a size.
#[derive(Clone)]
pub struct CodeEditor {
    /// The document it shows and edits.
    document: Rc<RefCell<Document>>,
    /// How it behaves.
    variant: CodeEditorVariant,
    /// Its size, which sets the type size.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// Whether keys go through vim first.
    vim: bool,
    /// What its status line names the language as.
    language: Option<String>,
    /// What learns the text changed.
    on_change: Option<Rc<dyn Fn()>>,
    /// What saves the document.
    on_save: Option<Rc<dyn Fn()>>,
    /// What a press on the gutter of numbers does, given the line counted from 1.
    on_gutter: Option<Rc<dyn Fn(usize)>>,
    /// The lines, counted from 0, that hold a breakpoint.
    breakpoints: Rc<Vec<usize>>,
    /// What a paused debugger shows after lines, by line counted from 0.
    inline_values: Rc<Vec<(usize, String)>>,
    /// Whether the minimap with its marks runs down the side.
    minimap: bool,
    /// Whether the lines opening the blocks in view stay at the top.
    sticky: bool,
    /// The bar it opens with over the text.
    opening: Option<CodeEditorBar>,
    /// The number the first line is shown with, for an excerpt of a longer file.
    first_line: usize,
    /// Whether the status line shows under the text.
    status: bool,
    /// What the language server's popups need.
    assist: Assist,
    /// The popup it opens with over the text.
    popup: Option<CodeEditorPopup>,
}

impl CodeEditor {
    /// An editor over `document`.
    pub fn new(document: Rc<RefCell<Document>>) -> Self {
        Self {
            document,
            variant: CodeEditorVariant::Editor,
            size: Size::Sm,
            modifier: Modifier::new(),
            vim: false,
            language: None,
            on_change: None,
            on_save: None,
            on_gutter: None,
            breakpoints: Rc::new(Vec::new()),
            inline_values: Rc::new(Vec::new()),
            minimap: true,
            sticky: true,
            opening: None,
            first_line: 1,
            status: true,
            assist: Assist::default(),
            popup: None,
        }
    }

    /// The editor numbering its first line `first_line`, as an excerpt of a longer file.
    pub fn first_line(mut self, first_line: usize) -> Self {
        self.first_line = first_line.max(1);
        self
    }

    /// The editor with its status line shown when `status`.
    pub fn status_line(mut self, status: bool) -> Self {
        self.status = status;
        self
    }

    /// The editor marking a breakpoint beside each of `lines`, counted from 0.
    pub fn breakpoints(mut self, lines: Vec<usize>) -> Self {
        self.breakpoints = Rc::new(lines);
        self
    }

    /// The editor showing each text of `values` after its line, counted
    /// from 0, as a paused debugger gives them.
    pub fn inline_values(mut self, values: Vec<(usize, String)>) -> Self {
        self.inline_values = Rc::new(values);
        self
    }

    /// The editor with a minimap and its marks down the side when `minimap`.
    pub fn minimap(mut self, minimap: bool) -> Self {
        self.minimap = minimap;
        self
    }

    /// The editor keeping the lines that open the blocks in view at its top when `sticky`.
    pub fn sticky(mut self, sticky: bool) -> Self {
        self.sticky = sticky;
        self
    }

    /// The editor opening with `bar` over its text.
    pub fn opening(mut self, bar: Option<CodeEditorBar>) -> Self {
        self.opening = bar;
        self
    }

    /// The editor in `variant`.
    pub fn variant(mut self, variant: CodeEditorVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The editor asking the language server reading its file through
    /// `on_ask`: told of each change and asked for what it says.
    pub fn on_ask(mut self, on_ask: impl Fn(ServerAsk) + 'static) -> Self {
        self.assist.on_ask = Some(Rc::new(on_ask));
        self
    }

    /// The editor offering `titles`, the changes the language server offers
    /// at the caret, in its lightbulb's menu.
    pub fn actions(mut self, titles: Vec<String>) -> Self {
        self.assist.actions = Rc::new(titles);
        self
    }

    /// The editor opening with `popup` over its text.
    pub fn popup(mut self, popup: Option<CodeEditorPopup>) -> Self {
        self.popup = popup;
        self
    }

    /// The editor showing the field that renames the name held in `range`,
    /// starting from `name`, while `rename` is given.
    pub fn rename(mut self, rename: Option<(Range<usize>, String)>) -> Self {
        self.assist.rename = rename;
        self
    }

    /// The editor calling `on_dismiss` when the rename field or the popup it
    /// opened with closes.
    pub fn on_dismiss(mut self, on_dismiss: impl Fn() + 'static) -> Self {
        self.assist.on_dismiss = Some(Rc::new(on_dismiss));
        self
    }

    /// The editor at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The editor laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The editor taking vim's keys when `vim`.
    pub fn vim(mut self, vim: bool) -> Self {
        self.vim = vim;
        self
    }

    /// The editor naming `language` in its status line.
    pub fn language(mut self, language: Option<String>) -> Self {
        self.language = language;
        self
    }

    /// The editor calling `on_change` each time the text changes.
    pub fn on_change(mut self, on_change: impl Fn() + 'static) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }

    /// The editor calling `on_save` when saving is asked for.
    pub fn on_save(mut self, on_save: impl Fn() + 'static) -> Self {
        self.on_save = Some(Rc::new(on_save));
        self
    }

    /// The editor calling `on_gutter` with the line, counted from 1, whose
    /// number is pressed, such as to set a breakpoint there.
    pub fn on_gutter(mut self, on_gutter: impl Fn(usize) + 'static) -> Self {
        self.on_gutter = Some(Rc::new(on_gutter));
        self
    }

    /// The text style the code is set in.
    pub fn style(&self) -> TextStyle {
        let code = crate::code_font();
        let font_size = code.sized(match self.size {
            Size::Xs => 11.0,
            Size::Sm => 12.5,
            Size::Md => 13.5,
            Size::Lg => 15.0,
        });
        TextStyle {
            color: color_scheme().on_surface,
            font_size,
            line_height: code.line_for(font_size, 1.6),
            font_family: code.family,
            max_lines: Some(1),
            ligatures: crate::code_look().ligatures,
            ..TextStyle::default()
        }
    }

    /// Emits the editor.
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let style = self.style();
        let cell = cell_size(&style);
        let revision = remember(|| 0_u64);
        let _ = revision.get();
        let engine = remember(|| Rc::new(RefCell::new(Vim::default()))).get();
        if self.variant == CodeEditorVariant::Viewer {
            let mut document = self.document.borrow_mut();
            if !document.field.is_read_only() {
                let text = document.field.text().to_owned();
                document.field = pm_text::Field::new(text).multiline(true).read_only(true);
            }
        }
        let handle = Handle {
            document: self.document.clone(),
            revision,
            bars: {
                let opening = self.opening.clone();
                remember(move || Bars::opening(opening.as_ref()))
            },
            list: remember_lazy_list_state(),
            focus: remember_focus(),
            line_height: cell.1,
            on_change: self.on_change.clone(),
            on_save: self.on_save.clone(),
            on_gutter: self.on_gutter.clone(),
            first_line: self.first_line,
            code: crate::code_look(),
            assist: self.assist.clone(),
            popup: {
                let opening = self.popup;
                remember(move || opening)
            },
            hovered: remember(|| None),
        };
        if let Some(opening) = self.popup
            && handle.popup.get().is_none()
        {
            handle.popup.set(Some(opening));
        }
        let status = self.status;
        let revealed = remember(|| false);
        if !revealed.get() {
            revealed.set(true);
            handle.reveal_caret();
        }
        let vim = self.vim;
        let language = self.language.clone();
        let code = handle.code;
        let extras = Extras {
            breakpoints: self.breakpoints.clone(),
            inline_values: self.inline_values.clone(),
            minimap: self.minimap && code.minimap,
            sticky: self.sticky && code.sticky,
            first_line: self.first_line,
            code,
        };
        column_with(
            self.modifier.background(scheme.background).clip(),
            move || {
                let bars = handle.bars.get();
                if let Some(find) = &bars.find {
                    find_bar(&handle, find);
                }
                if let Some(typed) = &bars.go_to {
                    go_to_bar(&handle, typed);
                }
                text_area(
                    &handle,
                    &engine,
                    vim,
                    &extras,
                    (style, cell, scheme, extended),
                );
                if status {
                    status_line(&handle, vim, language.as_deref());
                }
            },
        );
    }
}

/// What the parts of an editor share to change its document and redraw it.
#[derive(Clone)]
pub(crate) struct Handle {
    /// The document.
    pub(crate) document: Rc<RefCell<Document>>,
    /// Bumped to draw the editor again.
    revision: State<u64>,
    /// Which bars are open over the text.
    pub(crate) bars: State<Bars>,
    /// The scrolled list of lines.
    pub(crate) list: LazyListState,
    /// The text area's focus.
    focus: Focus,
    /// How tall a line is.
    pub(crate) line_height: f32,
    /// What learns the text changed.
    on_change: Option<Rc<dyn Fn()>>,
    /// What saves the document.
    on_save: Option<Rc<dyn Fn()>>,
    /// What a press on the gutter of numbers does, given the line counted from 1.
    on_gutter: Option<Rc<dyn Fn(usize)>>,
    /// The number the first line is shown with.
    first_line: usize,
    /// How the editor settings say the editor looks and behaves, as they stood when it was last drawn.
    code: CodeLook,
    /// What the language server's popups need.
    pub(crate) assist: Assist,
    /// The popup open over the text, when one is.
    pub(crate) popup: State<Option<CodeEditorPopup>>,
    /// The byte the pointer rests on that a hover card was asked for last.
    hovered: State<Option<usize>>,
}

impl Handle {
    /// Makes `edits` in order, tells the language server what they changed
    /// and asks it what they call for, then scrolls the caret into view and redraws.
    pub(crate) fn apply(&self, edits: impl IntoIterator<Item = Edit>) {
        let (changed, moved, asks) = {
            let mut document = self.document.borrow_mut();
            let before = document.field.revision();
            let caret = document.field.caret();
            let mut asks = Vec::new();
            for edit in edits {
                if let Some(edited) = document.edit(edit) {
                    asks.extend(server_asks(&document, edited));
                }
            }
            let moved = document.field.caret() != caret;
            (document.field.revision() != before, moved, asks)
        };
        for ask in asks {
            self.ask(ask);
        }
        if moved {
            self.ask_at_caret();
        }
        self.settle(changed);
    }

    /// Asks the language server `ask`, when the file is read by one.
    pub(crate) fn ask(&self, ask: ServerAsk) {
        if let Some(on_ask) = &self.assist.on_ask {
            on_ask(ask);
        }
    }

    /// Whether the language server reads the file.
    fn served(&self) -> bool {
        self.assist.on_ask.is_some() && self.document.borrow().synced
    }

    /// Asks the language server, after the caret moved, where the name at
    /// it is used and what changes it offers there, for the lightbulb.
    fn ask_at_caret(&self) {
        if !self.served() {
            return;
        }
        let (version, position) = {
            let document = self.document.borrow();
            (
                document.version,
                document.position_of(document.field.caret()),
            )
        };
        self.ask(ServerAsk::Highlights(version, position));
        self.ask(ServerAsk::CodeActions(
            pm_editor::Span::new(position, position),
            None,
        ));
    }

    /// Closes the popups open over the text and tells the window, which
    /// closes the ones it opened.
    pub(crate) fn dismiss(&self) {
        self.popup.set(None);
        if let Some(on_dismiss) = &self.assist.on_dismiss {
            on_dismiss();
        }
        self.focus_text();
        self.redraw();
    }

    /// Writes the completion at `row` of those shown, or the one picked, and closes the list.
    pub(crate) fn accept_completion(&self, row: Option<usize>) {
        let edits = {
            let mut document = self.document.borrow_mut();
            let edits = document.completion_edits(row);
            document.completion = None;
            edits
        };
        if let Some(edits) = edits {
            self.apply(edits);
        }
        self.focus_text();
        self.redraw();
    }

    /// Takes a key while the list of completions is open: the arrows move
    /// the pick, Enter and Tab write it and Escape closes the list; says
    /// whether the key was taken.
    fn completion_key(&self, event: &KeyEvent) -> bool {
        if !event.pressed || self.document.borrow().completion.is_none() {
            return false;
        }
        let modifiers = event.modifiers;
        if modifiers.ctrl || modifiers.alt || modifiers.meta || modifiers.shift {
            return false;
        }
        match event.key {
            rustle::Key::ArrowDown => self.document.borrow_mut().move_completion(1),
            rustle::Key::ArrowUp => self.document.borrow_mut().move_completion(-1),
            rustle::Key::PageDown => self.document.borrow_mut().move_completion(8),
            rustle::Key::PageUp => self.document.borrow_mut().move_completion(-8),
            rustle::Key::Enter | rustle::Key::Tab => {
                let ready = self
                    .document
                    .borrow()
                    .completion
                    .as_ref()
                    .is_some_and(|completion| !completion.shown.is_empty());
                if !ready {
                    return false;
                }
                self.accept_completion(None);
                return true;
            }
            rustle::Key::Escape => self.document.borrow_mut().completion = None,
            _ => return false,
        }
        let resolve = self.document.borrow_mut().completion_to_resolve();
        if let Some(item) = resolve {
            self.ask(ServerAsk::Resolve(item));
        }
        self.redraw();
        true
    }

    /// Takes the left and right arrows while the caret is in right-to-left
    /// text, moving it along the screen rather than through the text; says
    /// whether the key was taken.
    fn right_to_left_key(&self, event: &KeyEvent) -> bool {
        let right = match event.key {
            rustle::Key::ArrowRight => true,
            rustle::Key::ArrowLeft => false,
            _ => return false,
        };
        let modifiers = event.modifiers;
        if !event.pressed || modifiers.ctrl || modifiers.alt || modifiers.meta {
            return false;
        }
        let in_reversed = {
            let document = self.document.borrow();
            let caret = document.field.caret();
            let range = document.line(document.line_of(caret));
            let text = &document.field.text()[range.clone()];
            let at = caret - range.start;
            reversed_runs(text).iter().any(|run| {
                run.start < at && at < run.end
                    || run.start == at && !right
                    || run.end == at && right
            })
        };
        if !in_reversed {
            return false;
        }
        let motion = if right { Motion::Left } else { Motion::Right };
        self.apply([Edit::Move(motion, modifiers.shift)]);
        true
    }

    /// Asks the language server what it says of the byte `offset`, which
    /// the pointer rests on, unless it was asked of that word last.
    fn hover_at(&self, offset: Option<usize>) {
        if !self.served() {
            return;
        }
        let word = offset.map(|offset| self.document.borrow().field.word_range_at(offset).start);
        if word == self.hovered.get() {
            return;
        }
        self.hovered.set(word);
        let asked = {
            let mut document = self.document.borrow_mut();
            match offset {
                Some(offset) if !document.field.word_range_at(offset).is_empty() => {
                    document.hover = Some(pm_editor::HoverCard {
                        offset: document.field.word_range_at(offset).start,
                        at: (0.0, 0.0),
                        text: None,
                        diagnostics: document.diagnostics_at(offset),
                        scroll: 0.0,
                    });
                    Some((document.version, document.position_of(offset)))
                }
                _ => {
                    document.hover = None;
                    None
                }
            }
        };
        if let Some((version, position)) = asked {
            self.ask(ServerAsk::Hover(version, position));
        }
        self.redraw();
    }

    /// Tells what the text changed when `changed`, scrolls the caret into view and redraws.
    fn settle(&self, changed: bool) {
        if changed && let Some(on_change) = &self.on_change {
            on_change();
        }
        self.reveal_caret();
        self.redraw();
    }

    /// Draws the editor again.
    pub(crate) fn redraw(&self) {
        self.revision.update(|value| *value += 1);
    }

    /// Gives the text area the keys again.
    pub(crate) fn focus_text(&self) {
        self.focus.request_focus();
    }

    /// Asks for the document to be saved.
    fn save(&self) {
        if let Some(on_save) = &self.on_save {
            on_save();
        }
    }

    /// How many whole lines the text area has room for.
    fn rows(&self) -> usize {
        (self.list.scroll().viewport_length() / self.line_height).floor() as usize
    }

    /// The first line in view.
    fn top(&self) -> usize {
        (self.list.scroll().offset() / self.line_height).floor() as usize
    }

    /// Scrolls so `line`, counted from 0, is the first in view, and puts the caret on it.
    pub(crate) fn jump_to_line(&self, line: usize) {
        let start = self.document.borrow().line(line).start;
        self.list.scroll().jump_to(line as f32 * self.line_height);
        self.apply([Edit::Place(start, false)]);
    }

    /// Scrolls so the line drawn at height `y` of a minimap drawing each
    /// line `pitch` tall, from the first line `first` gives, is in the middle of the view.
    pub(crate) fn scroll_minimap_to(
        &self,
        y: f32,
        pitch: f32,
        first: fn(usize, f32, f32, f32) -> f32,
    ) {
        let scroll = self.list.scroll();
        let count = self.document.borrow().lines().max(1);
        let rows = scroll.viewport_length() / self.line_height;
        let height = scroll.viewport_length();
        let top = scroll.offset() / self.line_height;
        let line = first(count, top, rows, height) + y / pitch;
        let target = (line - rows / 2.0).max(0.0) * self.line_height;
        scroll.jump_to(target.min(scroll.max_offset().max(0.0)));
        self.redraw();
    }

    /// Scrolls so the line the caret is on is in view.
    fn reveal_caret(&self) {
        let line = {
            let document = self.document.borrow();
            document.line_of(document.field.caret())
        };
        let scroll = self.list.scroll();
        let top = line as f32 * self.line_height;
        let bottom = top + self.line_height;
        let viewport = scroll.viewport_length();
        if viewport <= 0.0 {
            self.list.scroll_to_item(line, ItemAlign::Center);
        } else if top < scroll.offset() {
            scroll.jump_to(top);
        } else if bottom > scroll.offset() + viewport {
            scroll.jump_to(bottom - viewport);
        }
    }

    /// Opens the find bar, replacing too when `replacing`, looking for the selected text when there is any.
    fn open_find(&self, replacing: bool) {
        let selected = self.document.borrow().field.selected().to_owned();
        self.bars.update(|bars| {
            let mut find = bars.find.take().unwrap_or_default();
            if !selected.is_empty() && !selected.contains('\n') {
                find.query.text = selected;
            }
            find.replacing = replacing;
            bars.find = Some(find);
        });
    }

    /// Carries out `command`; returns whether it was taken.
    fn run(&self, command: Command) -> bool {
        let edits: Vec<Edit> = {
            let document = self.document.borrow();
            match command {
                Command::Type(text) => {
                    drop(document);
                    self.type_text(&text);
                    return true;
                }
                Command::Newline => document.newline_edits(),
                Command::OpenLine(above) => vec![document.open_line_edit(above)],
                Command::Indent => {
                    let selection = document.field.selection();
                    let spans_lines =
                        selection.is_some_and(|range| document.field.text()[range].contains('\n'));
                    if spans_lines {
                        document.indent_lines_edit(false).into_iter().collect()
                    } else {
                        vec![Edit::Paste(document.editing.unit(1))]
                    }
                }
                Command::Outdent => document.indent_lines_edit(true).into_iter().collect(),
                Command::Backspace => vec![document.backspace_edit().unwrap_or(Edit::Backspace)],
                Command::BackspaceWord => vec![Edit::BackspaceWord],
                Command::Delete => vec![Edit::Delete],
                Command::DeleteWord => vec![Edit::DeleteWord],
                Command::Move(motion, extend) => vec![Edit::Move(motion, extend)],
                Command::Home(extend) => vec![document.home_edit(extend)],
                Command::Page(down, extend) => {
                    let motion = if down {
                        Motion::LineDown
                    } else {
                        Motion::LineUp
                    };
                    vec![Edit::Move(motion, extend); self.rows().max(1)]
                }
                Command::SelectAll => vec![Edit::SelectAll],
                Command::SelectLine => vec![document.select_line_edit()],
                Command::Undo => vec![Edit::Undo],
                Command::Redo => vec![Edit::Redo],
                Command::Copy => {
                    copy(&document.field.selected_all());
                    return true;
                }
                Command::Cut => {
                    if document.field.selection().is_none() {
                        return true;
                    }
                    copy(&document.field.selected_all());
                    vec![Edit::Backspace]
                }
                Command::Paste => match rustle::clipboard().and_then(|board| board.text()) {
                    Some(text) => vec![Edit::Paste(text)],
                    None => return true,
                },
                Command::Save => {
                    drop(document);
                    self.save();
                    return true;
                }
                Command::Find | Command::Replace => {
                    drop(document);
                    self.open_find(command == Command::Replace);
                    self.redraw();
                    return true;
                }
                Command::GoToLine => {
                    self.bars.update(|bars| bars.go_to = Some(String::new()));
                    return true;
                }
                Command::ToggleComment => document.toggle_comment_edit().into_iter().collect(),
                Command::MoveLines(down) => document.move_lines_edit(down).into_iter().collect(),
                Command::DuplicateLines(down) => {
                    document.duplicate_lines_edit(down).into_iter().collect()
                }
                Command::DeleteLines => document.delete_lines_edit().into_iter().collect(),
                Command::JoinLines => document.join_lines_edit().into_iter().collect(),
                Command::MatchingBracket => document.matching_bracket_edit().into_iter().collect(),
                Command::GrowSelection | Command::ShrinkSelection => {
                    drop(document);
                    let edit = {
                        let mut document = self.document.borrow_mut();
                        if command == Command::GrowSelection {
                            document.grow_selection_edit()
                        } else {
                            document.shrink_selection_edit()
                        }
                    };
                    self.apply(edit);
                    return true;
                }
                Command::Escape => {
                    let bars = self.bars.get();
                    if bars.find.is_some() || bars.go_to.is_some() {
                        self.bars.set(Bars::default());
                        return true;
                    }
                    let open = document.hover.is_some() || document.signature.is_some();
                    drop(document);
                    if open {
                        let mut document = self.document.borrow_mut();
                        document.hover = None;
                        document.signature = None;
                        drop(document);
                        self.redraw();
                        return true;
                    }
                    if self.popup.get().is_some() || self.assist.rename.is_some() {
                        self.dismiss();
                        return true;
                    }
                    vec![Edit::OnlySelection]
                }
                command => {
                    drop(document);
                    return self.serve(command);
                }
            }
        };
        self.apply(edits);
        true
    }

    /// Carries out `command`, one the language server answers; returns
    /// whether it was taken, which it is not while no server reads the file.
    fn serve(&self, command: Command) -> bool {
        if !self.served() {
            return false;
        }
        let (version, caret, selection) = {
            let document = self.document.borrow();
            let caret = document.position_of(document.field.caret());
            let selection = document.field.selection().map(|range| {
                pm_editor::Span::new(
                    document.position_of(range.start),
                    document.position_of(range.end),
                )
            });
            (document.version, caret, selection)
        };
        let whole = selection.unwrap_or(pm_editor::Span::new(caret, caret));
        match command {
            Command::Complete => {
                let (version, position) = self.document.borrow_mut().open_completion((0.0, 0.0));
                self.ask(ServerAsk::Complete(version, position, None));
            }
            Command::Seek(seek) => self.ask(ServerAsk::Seek(caret, seek)),
            Command::Rename => self.ask(ServerAsk::PrepareRename(caret)),
            Command::CodeActions => {
                self.ask(ServerAsk::CodeActions(whole, None));
                self.popup.set(Some(CodeEditorPopup::Actions));
            }
            Command::Hover => {
                let offset = self.document.borrow().field.caret();
                self.hovered.set(None);
                self.hover_at(Some(offset));
            }
            Command::Signature => self.ask(ServerAsk::Signature(version, caret)),
            Command::OrganizeImports => {
                let span = self.document.borrow().whole_span();
                self.ask(ServerAsk::CodeActions(
                    span,
                    Some(vec![ORGANIZE_IMPORTS.to_owned()]),
                ));
            }
            Command::FixAll => {
                let span = self.document.borrow().whole_span();
                self.ask(ServerAsk::CodeActions(span, Some(vec![FIX_ALL.to_owned()])));
            }
            Command::Format => self.ask(ServerAsk::Format(selection)),
            _ => return false,
        }
        self.redraw();
        true
    }

    /// Types `text`: one character with its closing pair and the outdent
    /// it calls for, more as they are.
    fn type_text(&self, text: &str) {
        let mut characters = text.chars();
        let (Some(character), None) = (characters.next(), characters.next()) else {
            self.apply([Edit::Paste(text.to_owned())]);
            return;
        };
        let typed = {
            let document = self.document.borrow();
            document
                .typed_edit(character)
                .unwrap_or(Edit::Insert(character))
        };
        self.apply([typed]);
        let outdent = self.document.borrow().outdent_edits(character);
        if !outdent.is_empty() {
            self.apply(outdent);
        }
    }

    /// Hands `event` to vim; returns whether vim took it, or swallowed a
    /// character it does not bind outside typing.
    fn vim_key(&self, engine: &RefCell<Vim>, event: &KeyEvent) -> bool {
        let Some(stroke) = keystroke(event) else {
            return false;
        };
        let code = self.code;
        let view = ModalView {
            top: self.top(),
            rows: self.rows().max(1),
            margin: code.scroll_margin,
            wrap: code.wrap_guide.unwrap_or(80),
            auto_close: code.auto_close,
            auto_indent: code.auto_indent,
            replay_typing: code.replay_typing,
            syntax_objects: code.syntax_objects,
        };
        let press = {
            let mut document = self.document.borrow_mut();
            document.vim_press(&mut engine.borrow_mut(), stroke, view, &mut VimClipboard)
        };
        if !press.outcome.handled {
            let typing = self.document.borrow().vim_mode().is_typing();
            return !typing && !event.modifiers.ctrl && matches!(stroke.key, pm_vim::Key::Char(_));
        }
        let changed = {
            let mut document = self.document.borrow_mut();
            let before = document.field.revision();
            for edit in press.edits {
                document.edit(edit);
            }
            if let Some(depth) = press.squash {
                document.field.squash_undo_since(depth);
            }
            document.field.revision() != before
        };
        for effect in press.outcome.effects {
            if effect == Effect::Command(VimCommand::Save) {
                self.save();
            }
        }
        self.settle(changed);
        true
    }

    /// The line, counted from 0, at height `y` of the text area, kept inside the text.
    fn line_at(&self, y: f32) -> usize {
        let line = ((y + self.list.scroll().offset()) / self.line_height)
            .floor()
            .max(0.0) as usize;
        line.min(self.document.borrow().lines().saturating_sub(1))
    }

    /// Calls what a press on the gutter does with the line at height `y`,
    /// and says whether anything was called.
    fn press_gutter(&self, y: f32) -> bool {
        let Some(on_gutter) = &self.on_gutter else {
            return false;
        };
        on_gutter(self.line_at(y) + self.first_line);
        true
    }

    /// Places the caret at the point `x`, `y` of the text area, measured
    /// past the gutter `gutter` wide, as a press counted `clicks` does, or
    /// extends the selection to it while dragging or when `extend`.
    fn press_at(&self, x: f32, y: f32, gutter: f32, cell: f32, clicks: u32, extend: bool) {
        let edit = {
            let line = self.line_at(y);
            let document = self.document.borrow();
            let range = document.line(line);
            let byte = self.byte_at(x, y, gutter, cell);
            if clicks == 1 && !extend && document.picks_up_at(byte) {
                drop(document);
                self.document.borrow_mut().drag_started(byte);
                self.redraw();
                return;
            }
            match clicks {
                2 if !extend => {
                    let word = document.field.word_range_at(byte);
                    Edit::Select(word.start, word.end)
                }
                3 if !extend => {
                    let end = if line + 1 < document.lines() {
                        document.line(line + 1).start
                    } else {
                        range.end
                    };
                    Edit::Select(range.start, end)
                }
                _ => Edit::Place(byte, extend),
            }
        };
        self.apply([edit]);
    }

    /// The byte of the text drawn at the point `x`, `y` of the text area,
    /// measured past the gutter `gutter` wide with characters `cell` wide,
    /// past the hints drawn inside the line and through right-to-left text
    /// as it is drawn.
    fn byte_at(&self, x: f32, y: f32, gutter: f32, cell: f32) -> usize {
        let line = self.line_at(y);
        let document = self.document.borrow();
        let range = document.line(line);
        let text = &document.field.text()[range.clone()];
        let inserts = inserts_on(&document, line, &range);
        let mut column = ((x - gutter) / cell).max(0.0);
        let mut shifted = 0.0;
        for (at, width) in &inserts {
            if column < column_of(text, *at) as f32 + shifted {
                break;
            }
            shifted += *width as f32;
        }
        column = (column - shifted).max(0.0);
        let mut byte = byte_at_column(text, column);
        for run in reversed_runs(text) {
            let (left, right) = (column_of(text, run.start), column_of(text, run.end));
            if (left as f32) <= column && column < right as f32 {
                let mirrored = (left + right) as f32 - column;
                byte = byte_at_column(text, mirrored);
            }
        }
        range.start + byte
    }

    /// Carries what a press on the text area at `x`, `y` does besides
    /// placing the caret: a lens after the line pressed, a colour swatch
    /// opening the picker; says whether it was one of them.
    fn press_extra(&self, x: f32, y: f32, gutter: f32, cell: (f32, f32)) -> bool {
        let line = self.line_at(y);
        let (lens, swatch) = {
            let document = self.document.borrow();
            let range = document.line(line);
            let text = &document.field.text()[range.clone()];
            let inserts = inserts_on(&document, line, &range);
            let width: usize = inserts.iter().map(|(_, width)| width).sum();
            let end = gutter + (column_of(text, text.len()) + width + 2) as f32 * cell.0;
            let lens = (x >= end).then(|| {
                let mut left = end;
                document.lenses_on(line).iter().find_map(|lens| {
                    let right = left + (lens.title.chars().count() + 3) as f32 * cell.0;
                    let hit = lens.pressable && x < right;
                    left = right;
                    hit.then_some(lens.index)
                })
            });
            let mut shifted = 0;
            let swatch = document.colors_on(line).into_iter().find_map(|hint| {
                let at = hint.offset - range.start;
                let shift_before = inserts
                    .iter()
                    .filter(|(place, _)| *place < at)
                    .map(|(_, width)| width)
                    .sum::<usize>();
                shifted = shift_before;
                let left = gutter + (column_of(text, at) + shifted) as f32 * cell.0;
                (x >= left && x < left + cell.0 * 1.5)
                    .then(|| hint.swatch.map(|swatch| swatch.index))
            });
            (lens.flatten(), swatch.flatten())
        };
        if let Some(index) = lens {
            self.ask(ServerAsk::PressLens(index));
            return true;
        }
        if let Some(index) = swatch {
            self.popup.set(Some(CodeEditorPopup::Colour(index)));
            self.redraw();
            return true;
        }
        false
    }

    /// Opens the lightbulb's menu when a press at height `y` of the gutter,
    /// `x` across, falls on the lightbulb; says whether it did.
    fn press_bulb(&self, x: f32, y: f32, cell: f32) -> bool {
        let line = self.line_at(y);
        let on_bulb = self.document.borrow().bulb_line(false) == Some(line);
        if !on_bulb || x < cell * 1.6 || x > cell * 3.0 {
            return false;
        }
        self.popup.set(Some(CodeEditorPopup::Actions));
        self.redraw();
        true
    }

    /// Follows the pointer dragging selected text over the point `x`, `y`;
    /// says whether text is being dragged.
    fn drag_text_over(&self, x: f32, y: f32, gutter: f32, cell: f32) -> bool {
        if !self.document.borrow().is_dragging() {
            return false;
        }
        let byte = self.byte_at(x, y, gutter, cell);
        self.document.borrow_mut().drag_over(byte);
        self.redraw();
        true
    }

    /// Puts down the text being dragged where it is over, copied when
    /// `copy`; says whether text was being dragged.
    fn drop_text(&self, copy: bool) -> bool {
        if !self.document.borrow().is_dragging() {
            return false;
        }
        let edit = self.document.borrow_mut().drag_dropped(copy);
        match edit {
            Some(edit) => self.apply([edit]),
            None => {
                self.document.borrow_mut().drag_cancelled();
                self.redraw();
            }
        }
        true
    }
}

/// What the language server is to be told and asked after an edit of
/// `document` that called for `edited`.
fn server_asks(document: &Document, edited: pm_editor::Edited) -> Vec<ServerAsk> {
    let caret = document.position_of(document.field.caret());
    let mut asks = Vec::new();
    if let Some((version, change)) = edited.change {
        asks.push(ServerAsk::Changed(version, change));
    }
    if let Some(trigger) = edited.ask {
        asks.push(ServerAsk::Complete(document.version, caret, trigger));
    }
    if edited.signature {
        asks.push(ServerAsk::Signature(document.version, caret));
    }
    asks
}

/// What is drawn inside line `line`, whose stretch of the text is `range`,
/// that is not its text: each hint and colour swatch as the byte of the line
/// it stands before and how many characters wide it is drawn, in order.
fn inserts_on(document: &Document, line: usize, range: &Range<usize>) -> Vec<(usize, usize)> {
    let mut inserts: Vec<(usize, usize)> = document
        .hints_on(line)
        .iter()
        .map(|hint| {
            (
                hint.offset.saturating_sub(range.start),
                hint_width(&hint.label, hint.pad_left, hint.pad_right),
            )
        })
        .chain(
            document
                .colors_on(line)
                .into_iter()
                .map(|hint| (hint.offset.saturating_sub(range.start), SWATCH_WIDTH)),
        )
        .collect();
    inserts.sort_by_key(|(at, _)| *at);
    inserts
}

/// How many characters a colour swatch takes inside a line.
const SWATCH_WIDTH: usize = 2;

/// How many columns apart the indent guides stand.
const INDENT_STEP: usize = 4;

/// How many characters a hint reading `label` takes, with a space before
/// it when `pad_left` and after it when `pad_right`.
fn hint_width(label: &str, pad_left: bool, pad_right: bool) -> usize {
    label.chars().count() + usize::from(pad_left) + usize::from(pad_right)
}

/// Whether `character` is written right to left.
fn is_right_to_left(character: char) -> bool {
    matches!(character as u32,
        0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// The stretches of `text`, as byte ranges, written right to left: runs of
/// right-to-left characters with the spaces and marks between them.
pub(crate) fn reversed_runs(text: &str) -> Vec<Range<usize>> {
    let mut runs: Vec<Range<usize>> = Vec::new();
    let mut open: Option<Range<usize>> = None;
    for (at, character) in text.char_indices() {
        let end = at + character.len_utf8();
        if is_right_to_left(character) {
            open = Some(open.map_or(at..end, |run| run.start..end));
        } else if character == ' ' && open.is_some() {
            continue;
        } else if let Some(run) = open.take() {
            runs.push(run);
        }
    }
    runs.extend(open);
    runs
}

/// The system clipboard as vim reads and writes it.
struct VimClipboard;

impl pm_vim::Clipboard for VimClipboard {
    /// What the system clipboard holds.
    fn read(&mut self) -> Option<String> {
        rustle::clipboard().and_then(|board| board.text())
    }

    /// Puts `text` on the system clipboard.
    fn write(&mut self, text: String) {
        copy(&text);
    }
}

/// Puts `text` on the system clipboard, unless it is empty.
fn copy(text: &str) {
    if let (false, Some(board)) = (text.is_empty(), rustle::clipboard()) {
        board.set_text(text);
    }
}

/// What an editor draws besides the text: breakpoints, a debugger's values,
/// the minimap and the sticky lines.
#[derive(Clone)]
struct Extras {
    /// The lines, counted from 0, that hold a breakpoint.
    breakpoints: Rc<Vec<usize>>,
    /// What a paused debugger shows after lines, by line counted from 0.
    inline_values: Rc<Vec<(usize, String)>>,
    /// Whether the minimap with its marks runs down the side.
    minimap: bool,
    /// Whether the lines opening the blocks in view stay at the top.
    sticky: bool,
    /// The number the first line is shown with.
    first_line: usize,
    /// How the editor settings say the editor looks and behaves.
    code: CodeLook,
}

impl Line {
    /// The line at `index` of `document` with its colours, gutter marks and
    /// the values `extras` gives it, and nothing selected, found or carets.
    fn of(
        document: &Document,
        index: usize,
        painted: Option<&Vec<(Range<usize>, pm_treesitter::Kind)>>,
        extras: &Extras,
    ) -> Self {
        let range = if index < document.lines() {
            document.line(index)
        } else {
            0..0
        };
        let text = document.field.text()[range].to_owned();
        let spans = painted
            .cloned()
            .unwrap_or_else(|| document.line_highlights(index));
        let line_range = if index < document.lines() {
            document.line(index)
        } else {
            0..0
        };
        let (hints, lenses) = if index < document.lines() {
            let mut hints: Vec<Inset> = document
                .hints_on(index)
                .iter()
                .map(|hint| Inset {
                    at: hint.offset.saturating_sub(line_range.start),
                    label: format!(
                        "{}{}{}",
                        if hint.pad_left { " " } else { "" },
                        hint.label,
                        if hint.pad_right { " " } else { "" }
                    ),
                    swatch: None,
                })
                .chain(document.colors_on(index).into_iter().map(|hint| Inset {
                    at: hint.offset.saturating_sub(line_range.start),
                    label: " ".repeat(SWATCH_WIDTH),
                    swatch: hint.swatch.map(|swatch| swatch.color),
                }))
                .collect();
            if !extras.code.inlay_hints {
                hints.retain(|inset| inset.swatch.is_some());
            }
            hints.sort_by_key(|inset| inset.at);
            let lenses = document
                .lenses_on(index)
                .iter()
                .map(|lens| lens.title.clone())
                .filter(|_| extras.code.code_lens)
                .collect();
            (hints, lenses)
        } else {
            (Vec::new(), Vec::new())
        };
        Line {
            insets: hints,
            lenses,
            occurrences: Vec::new(),
            bulb: false,
            drop_mark: None,
            number: index + extras.first_line,
            runs: runs(text.len(), &spans),
            selected: Vec::new(),
            found: Vec::new(),
            carets: Vec::new(),
            current: false,
            breakpoint: extras.breakpoints.contains(&index),
            change: document
                .line_change(index)
                .filter(|_| extras.code.git_gutter),
            value: extras
                .inline_values
                .iter()
                .find(|(line, _)| *line == index)
                .map(|(_, text)| text.clone()),
            text,
        }
    }
}

/// Emits the scrolled lines with their gutter, the sticky lines over them
/// and the minimap beside them, taking keys while focused and placing the
/// caret where pressed.
fn text_area(
    handle: &Handle,
    engine: &Rc<RefCell<Vim>>,
    vim: bool,
    extras: &Extras,
    looks: (TextStyle, (f32, f32), ColorScheme, ExtendedColors),
) {
    let (style, cell, scheme, extended) = looks;
    let document = handle.document.borrow();
    let count = document.lines().max(1);
    let gutter = if extras.code.line_numbers {
        (digits(count + extras.first_line - 1) as f32 + 4.5) * cell.0
    } else {
        3.5 * cell.0
    };
    let top = handle.top();
    let window = top..(top + handle.rows() + 2).min(count);
    let painted = Rc::new((window.start, document.highlights(window)));
    let selections: Rc<Vec<Range<usize>>> = Rc::new(
        document
            .field
            .selections()
            .into_iter()
            .map(|(anchor, caret)| anchor.min(caret)..anchor.max(caret))
            .collect(),
    );
    let carets: Rc<Vec<usize>> = Rc::new(
        document
            .field
            .selections()
            .into_iter()
            .map(|(_, caret)| caret)
            .collect(),
    );
    let found = Rc::new(found_matches(&handle.bars.get(), &document));
    let caret_line = document.line_of(document.field.caret());
    let occurrences: Rc<Vec<Range<usize>>> = Rc::new(document.occurrences().to_vec());
    let bulb = document.bulb_line(false);
    let drop_mark = document.drop_mark();
    let scrolled = handle.list.scroll().offset();
    let marks = Marks {
        found: found
            .iter()
            .map(|range| document.line_of(range.start))
            .collect(),
        breakpoints: extras.breakpoints.as_ref().clone(),
        caret: caret_line,
    };
    let held = if extras.sticky {
        sticky_lines(&document, top)
    } else {
        Vec::new()
    };
    let shape = match (vim, extras.code.cursor) {
        (true, _) => document.vim_shape(),
        (false, CursorShape::Bar) => Shape::Typing,
        (false, CursorShape::Block) => Shape::Block,
        (false, CursorShape::Underline) => Shape::Underline,
    };
    drop(document);
    let focused = handle.focus.is_focused();
    let look = Look {
        style,
        cell,
        gutter,
        scheme,
        extended,
        focused,
        shape,
        code: extras.code,
    };
    let keys = {
        let handle = handle.clone();
        let engine = engine.clone();
        move |event: &KeyEvent| {
            if handle.completion_key(event) {
                return true;
            }
            if vim && handle.vim_key(&engine, event) {
                return true;
            }
            if handle.right_to_left_key(event) {
                return true;
            }
            command(event).is_some_and(|command| handle.run(command))
        }
    };
    let pointer = {
        let handle = handle.clone();
        move |event: &rustle::PointerEvent| {
            let (x, y) = (event.position.x, event.position.y);
            match event.phase {
                PointerPhase::Press if x < gutter && handle.press_bulb(x, y, cell.0) => {}
                PointerPhase::Press if x < gutter && handle.press_gutter(y) => {}
                PointerPhase::Press if handle.press_extra(x, y, gutter, cell) => {}
                PointerPhase::Press => {
                    handle.focus_text();
                    handle.press_at(
                        x,
                        y,
                        gutter,
                        cell.0,
                        event.click_count,
                        event.modifiers.shift,
                    );
                }
                PointerPhase::Drag if handle.drag_text_over(x, y, gutter, cell.0) => {}
                PointerPhase::Drag => handle.press_at(x, y, gutter, cell.0, 1, true),
                PointerPhase::Release => {
                    handle.drop_text(event.modifiers.ctrl || event.modifiers.alt);
                }
                _ => {}
            }
        }
    };
    let list = handle.list.clone();
    let document = handle.document.clone();
    let extras = extras.clone();
    let minimap = extras.minimap;
    let sticky_handle = handle.clone();
    let overview_handle = handle.clone();
    let area_handle = handle.clone();
    let hover_handle = handle.clone();
    let hover = move |at: Option<rustle::Offset>| match at {
        Some(at) if at.x >= gutter => {
            let byte = hover_handle.byte_at(at.x, at.y, gutter, cell.0);
            hover_handle.hover_at(Some(byte));
        }
        _ => hover_handle.hover_at(None),
    };
    let assist_handle = handle.clone();
    let grid = Grid {
        cell,
        gutter,
        scrolled,
    };
    row_with(Modifier::new().fill_max_width().weight(1.0), move || {
        let document = document.clone();
        let list = list.clone();
        let painted = painted.clone();
        let selections = selections.clone();
        let carets = carets.clone();
        let found = found.clone();
        let extras = extras.clone();
        let held = held.clone();
        let sticky_handle = sticky_handle.clone();
        let area_handle = area_handle.clone();
        let assist_handle = assist_handle.clone();
        let occurrences = occurrences.clone();
        stack_with(
            Modifier::new()
                .fill_max_height()
                .weight(1.0)
                .focusable(&area_handle.focus, keys.clone())
                .on_pointer_button(PointerButton::Primary, pointer.clone())
                .on_pointer_move(hover.clone())
                .cursor(CursorIcon::Text),
            move || {
                let lines_document = document.clone();
                let extras_inner = extras.clone();
                let painted = painted.clone();
                let selections = selections.clone();
                let carets = carets.clone();
                let found = found.clone();
                let occurrences = occurrences.clone();
                lazy_list(
                    Modifier::new().fill_max_size(),
                    &list,
                    count,
                    move |index| {
                        let document = lines_document.borrow();
                        let cached = (painted.0..painted.0 + painted.1.len())
                            .contains(&index)
                            .then(|| &painted.1[index - painted.0]);
                        let mut line = Line::of(&document, index, cached, &extras_inner);
                        let range = if index < document.lines() {
                            document.line(index)
                        } else {
                            0..0
                        };
                        line.selected = within(&selections, &range);
                        line.found = within(&found, &range);
                        if extras_inner.code.occurrences {
                            line.occurrences = within(&occurrences, &range);
                        }
                        line.bulb = bulb == Some(index);
                        line.drop_mark = drop_mark
                            .filter(|mark| range.contains(mark) || *mark == range.end)
                            .map(|mark| mark - range.start);
                        line.carets = carets
                            .iter()
                            .filter(|caret| range.contains(caret) || **caret == range.end)
                            .map(|caret| caret - range.start)
                            .collect();
                        line.current = index == caret_line;
                        if extras_inner.code.relative_line_numbers && !line.current {
                            line.number = index.abs_diff(caret_line);
                        }
                        drop(document);
                        canvas(
                            Modifier::new().fill_max_width().height(cell.1),
                            move |canvas| line.paint(canvas, &look),
                        );
                    },
                );
                if !held.is_empty() {
                    sticky_header(&sticky_handle, &held, &extras, look);
                }
                assist_layer(&assist_handle, grid, assist_handle.popup.get());
            },
        );
        if minimap {
            overview(&overview_handle, marks.clone(), scheme, extended);
        }
    });
}

/// Emits the lines `held`, counted from 0, at the top of the text area over
/// the lines scrolled under them; a press on one scrolls to it.
fn sticky_header(handle: &Handle, held: &[usize], extras: &Extras, look: Look) {
    let scheme = look.scheme;
    let lines: Vec<Line> = {
        let document = handle.document.borrow();
        held.iter()
            .map(|index| Line::of(&document, *index, None, extras))
            .collect()
    };
    let starts = held.to_vec();
    column_with(
        Modifier::new()
            .fill_max_width()
            .background(scheme.background)
            .border_bottom_hairline(scheme.outline_variant),
        move || {
            for (line, index) in lines.iter().zip(starts.iter().copied()) {
                let line = Line {
                    value: None,
                    lenses: Vec::new(),
                    ..line.clone()
                };
                let handle = handle.clone();
                canvas(
                    Modifier::new()
                        .fill_max_width()
                        .height(look.cell.1)
                        .clickable_plain(move || handle.jump_to_line(index))
                        .cursor(CursorIcon::Default),
                    move |canvas| line.paint(canvas, &look),
                );
            }
        },
    );
}

/// The stretches the find bar's query finds, when it is open and finds any.
fn found_matches(bars: &Bars, document: &Document) -> Vec<Range<usize>> {
    bars.find
        .as_ref()
        .and_then(|find: &FindBar| document.find_all(&find.query).ok())
        .unwrap_or_default()
}

/// The parts of `ranges`, sorted by start, that fall on the line at
/// `line`, as offsets into the line; a part running past the line's end
/// reaches one byte past it.
fn within(ranges: &[Range<usize>], line: &Range<usize>) -> Vec<Range<usize>> {
    ranges
        .iter()
        .filter(|range| range.start <= line.end && range.end > line.start)
        .filter(|range| !range.is_empty())
        .map(|range| {
            let start = range.start.max(line.start) - line.start;
            let end = range.end.min(line.end + 1) - line.start;
            start..end
        })
        .collect()
}

/// How lines are painted, the same for each.
#[derive(Clone, Copy)]
struct Look {
    /// The text style.
    style: TextStyle,
    /// The width and height of a character.
    cell: (f32, f32),
    /// How wide the gutter of numbers is.
    gutter: f32,
    /// The Material colours.
    scheme: ColorScheme,
    /// The colours Material has no role for.
    extended: ExtendedColors,
    /// Whether the editor holds the keys, which draws the carets.
    focused: bool,
    /// How carets are drawn.
    shape: Shape,
    /// How the editor settings say the editor looks.
    code: CodeLook,
}

/// Something drawn inside a line that is not its text: a hint or a colour swatch.
#[derive(Clone, Debug)]
struct Inset {
    /// The byte of the line it stands before.
    at: usize,
    /// What it reads, with its spaces; blanks the width of a swatch.
    label: String,
    /// The colour it shows as a square, when it is a swatch.
    swatch: Option<[u8; 4]>,
}

/// One line as it is painted.
#[derive(Clone)]
struct Line {
    /// The hints and swatches drawn inside it, in order.
    insets: Vec<Inset>,
    /// What the language server's lenses say of it, drawn after it.
    lenses: Vec<String>,
    /// The stretches where the name at the caret is used on it.
    occurrences: Vec<Range<usize>>,
    /// Whether the lightbulb stands beside it.
    bulb: bool,
    /// Where dragged text would be put down on it, as an offset into it.
    drop_mark: Option<usize>,
    /// Its number, counted from 1.
    number: usize,
    /// Its text, without the line break.
    text: String,
    /// The runs of one highlight its text is painted in.
    runs: Vec<(Range<usize>, Option<pm_treesitter::Kind>)>,
    /// The selected stretches on it.
    selected: Vec<Range<usize>>,
    /// The stretches the find bar finds on it.
    found: Vec<Range<usize>>,
    /// The carets on it, as offsets into it.
    carets: Vec<usize>,
    /// Whether the main caret is on it.
    current: bool,
    /// Whether a breakpoint is set on it.
    breakpoint: bool,
    /// How it differs from the last commit, when it does.
    change: Option<LineChange>,
    /// What a paused debugger shows after it.
    value: Option<String>,
}

impl Line {
    /// How many characters of hints and swatches stand before the byte `at`
    /// of the line, or at it as well when `at_too`.
    fn shift_before(&self, at: usize, at_too: bool) -> usize {
        self.insets
            .iter()
            .filter(|inset| inset.at < at || at_too && inset.at == at)
            .map(|inset| inset.label.chars().count())
            .sum()
    }

    /// The column the byte `byte` of the line is drawn at, counting the
    /// hints before it and mirrored inside right-to-left text.
    fn column_at(&self, byte: usize) -> usize {
        let byte = byte.min(self.text.len());
        let mut column = column_of(&self.text, byte);
        for run in reversed_runs(&self.text) {
            if run.start < byte && byte < run.end {
                let (left, right) = (
                    column_of(&self.text, run.start),
                    column_of(&self.text, run.end),
                );
                column = left + right - column;
            }
        }
        column + self.shift_before(byte, false)
    }

    /// Paints the line onto `canvas`: the current line's band, the number,
    /// the lightbulb, matches, uses of the name at the caret, selections,
    /// text with its hints and swatches, lenses, values and carets.
    fn paint(&self, canvas: &mut Canvas, look: &Look) {
        let (width, height) = look.cell;
        let scheme = look.scheme;
        let extended = look.extended;
        let start = look.gutter;
        if self.current && look.code.current_line {
            canvas.rect(0.0, 0.0, 20_000.0, height, faded(scheme.on_surface, 10));
        }
        if look.code.line_numbers {
            self.paint_number(canvas, look);
        }
        self.paint_guides(canvas, look);
        if self.breakpoint {
            canvas.circle(
                width,
                height / 2.0,
                (height * 0.22).min(width * 0.6),
                scheme.error,
            );
        }
        if self.bulb {
            let radius = (height * 0.2).min(width * 0.5);
            canvas.circle(width * 2.3, height * 0.42, radius, extended.warning);
            canvas.rect(
                width * 2.3 - radius * 0.5,
                height * 0.42 + radius * 0.8,
                radius,
                radius * 0.7,
                extended.warning,
            );
        }
        match self.change {
            Some(LineChange::DeletedBefore) => {
                canvas.rect(look.gutter - width, 0.0, width * 0.6, 2.0, scheme.error)
            }
            Some(change) => canvas.rect(
                look.gutter - width,
                0.0,
                3.0,
                height,
                gutter_colour(change, &scheme, &extended),
            ),
            None => {}
        }
        let end_column =
            column_of(&self.text, self.text.len()) + self.shift_before(self.text.len(), true);
        let x_of = |byte: usize| start + self.column_at(byte) as f32 * width;
        let span_x = |range: &Range<usize>| {
            let left = x_of(range.start);
            let right = if range.end > self.text.len() {
                start + end_column as f32 * width + width * 0.6
            } else {
                x_of(range.end)
            };
            (left.min(right), (right - left).abs().max(2.0))
        };
        for range in &self.found {
            let (left, span) = span_x(range);
            canvas.rect(left, 0.0, span, height, faded(extended.warning, 70));
        }
        for range in &self.occurrences {
            let (left, span) = span_x(range);
            canvas.rect(left, 1.0, span, height - 2.0, faded(scheme.on_surface, 28));
        }
        for range in &self.selected {
            let (left, span) = span_x(range);
            canvas.rect(left, 0.0, span, height, faded(scheme.primary, 80));
        }
        let reversed = reversed_runs(&self.text);
        for (range, kind) in &self.runs {
            let colour = kind.map_or(scheme.on_surface, |kind| {
                kind_colour(kind, &scheme, &extended)
            });
            for piece_range in self.pieces(range, &reversed) {
                let piece = &self.text[piece_range.clone()];
                if piece.trim().is_empty() {
                    continue;
                }
                let column = column_of(&self.text, piece_range.start);
                let style = TextStyle {
                    color: colour,
                    ..look.style
                };
                let left = if reversed
                    .iter()
                    .any(|run| run.start <= piece_range.start && piece_range.end <= run.end)
                {
                    let run = reversed
                        .iter()
                        .find(|run| run.start <= piece_range.start && piece_range.end <= run.end)
                        .cloned()
                        .unwrap_or(piece_range.clone());
                    let (left, right) = (
                        column_of(&self.text, run.start),
                        column_of(&self.text, run.end),
                    );
                    let end = column_of(&self.text, piece_range.end);
                    left + right - end
                } else {
                    column
                };
                canvas.draw_text(
                    start + (left + self.shift_before(piece_range.start, true)) as f32 * width,
                    0.0,
                    &expand_tabs(piece, column),
                    &style,
                    None,
                );
            }
        }
        for inset in &self.insets {
            let left = start
                + (column_of(&self.text, inset.at.min(self.text.len()))
                    + self.shift_before(inset.at, false)) as f32
                    * width;
            match inset.swatch {
                Some(colour) => {
                    let side = (height * 0.55).min(width * 1.4);
                    canvas.rect(
                        left + width * 0.2,
                        (height - side) / 2.0,
                        side,
                        side,
                        scheme.outline_variant,
                    );
                    canvas.rect(
                        left + width * 0.2 + 1.0,
                        (height - side) / 2.0 + 1.0,
                        side - 2.0,
                        side - 2.0,
                        colour_of(colour),
                    );
                }
                None => {
                    let span = inset.label.chars().count() as f32 * width;
                    canvas.rect(left, 2.0, span, height - 4.0, faded(scheme.on_surface, 14));
                    let style = TextStyle {
                        color: scheme.on_surface_variant,
                        ..look.style
                    };
                    canvas.draw_text(left, 0.0, &inset.label, &style, None);
                }
            }
        }
        let mut after = start + (end_column + 2) as f32 * width;
        if let Some(value) = &self.value {
            let value_style = TextStyle {
                color: scheme.on_surface_variant,
                ..look.style
            };
            canvas.rect(
                after - width * 0.5,
                2.0,
                (value.chars().count() as f32 + 1.0) * width,
                height - 4.0,
                faded(scheme.primary, 30),
            );
            canvas.draw_text(after, 0.0, value, &value_style, None);
            after += (value.chars().count() + 3) as f32 * width;
        }
        if !self.lenses.is_empty() {
            let lens_style = TextStyle {
                color: scheme.on_surface_variant,
                ..look.style
            };
            let text = self.lenses.join(" | ");
            canvas.draw_text(after, 0.0, &text, &lens_style, None);
        }
        if let Some(mark) = self.drop_mark {
            canvas.rect(x_of(mark), 0.0, 2.0, height, scheme.primary);
        }
        if look.focused {
            for caret in &self.carets {
                let x = if *caret >= self.text.len() {
                    start
                        + column_of(&self.text, self.text.len()) as f32 * width
                        + self.shift_before(self.text.len(), false) as f32 * width
                } else {
                    x_of(*caret)
                };
                match look.shape {
                    Shape::Typing => canvas.rect(x, 1.0, 2.0, height - 2.0, scheme.on_surface),
                    Shape::Block => {
                        canvas.rect(x, 0.0, width, height, faded(scheme.on_surface, 140))
                    }
                    Shape::Underline => canvas.rect(x, height - 2.0, width, 2.0, scheme.on_surface),
                }
            }
        }
    }

    /// Paints the line's number at the right of the gutter, brighter on the caret's line.
    fn paint_number(&self, canvas: &mut Canvas, look: &Look) {
        let width = look.cell.0;
        let number = self.number.to_string();
        let number_x = look.gutter - (number.len() as f32 + 1.5) * width;
        let number_style = TextStyle {
            color: if self.current {
                look.scheme.on_surface
            } else {
                look.scheme.on_surface_variant
            },
            ..look.style
        };
        canvas.draw_text(number_x, 0.0, &number, &number_style, None);
    }

    /// Paints the upright lines the settings ask for: one at each level of
    /// the line's leading indentation, every four columns, and one at the
    /// column where lines should end.
    fn paint_guides(&self, canvas: &mut Canvas, look: &Look) {
        let (width, height) = look.cell;
        let colour = look.scheme.outline_variant;
        if look.code.indent_guides {
            let indent = column_of(&self.text, self.text.len() - self.text.trim_start().len());
            for column in (0..indent).step_by(INDENT_STEP) {
                canvas.rect(
                    look.gutter + column as f32 * width,
                    0.0,
                    1.0,
                    height,
                    colour,
                );
            }
        }
        if let Some(column) = look.code.wrap_guide {
            canvas.rect(
                look.gutter + column as f32 * width,
                0.0,
                1.0,
                height,
                colour,
            );
        }
    }

    /// The parts of `range` of the line cut where a hint or swatch stands
    /// and where right-to-left text in `reversed` starts or ends, so each
    /// is drawn at its own place.
    fn pieces(&self, range: &Range<usize>, reversed: &[Range<usize>]) -> Vec<Range<usize>> {
        let mut cuts: Vec<usize> = self
            .insets
            .iter()
            .map(|inset| inset.at)
            .chain(reversed.iter().flat_map(|run| [run.start, run.end]))
            .filter(|at| range.start < *at && *at < range.end)
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        let mut pieces = Vec::new();
        let mut from = range.start;
        for cut in cuts {
            pieces.push(from..cut);
            from = cut;
        }
        pieces.push(from..range.end);
        pieces
    }
}

/// `colour` with its opacity set to `alpha` of 255.
fn faded(colour: Color, alpha: u8) -> Color {
    Color { a: alpha, ..colour }
}

/// Emits the line under the editor: vim's mode and pending keys, where the
/// caret is, how the file is indented and its language.
fn status_line(handle: &Handle, vim: bool, language: Option<&str>) {
    let scheme = color_scheme();
    let document = handle.document.borrow();
    let caret = document.field.caret();
    let line = document.line_of(caret);
    let start = document.line(line).start;
    let column = document.field.text()[start..caret.max(start)]
        .chars()
        .count()
        + 1;
    let indent = if document.editing.tabs {
        "Tabs".to_owned()
    } else {
        format!("Spaces: {}", document.editing.indent)
    };
    let mode = vim.then(|| {
        let pending = document.vim_pending().unwrap_or_default();
        format!("{} {pending}", document.vim_mode().label())
    });
    let read_only = document.field.is_read_only();
    drop(document);
    let language = language.unwrap_or("Plain text").to_owned();
    spacer(
        Modifier::new()
            .fill_max_width()
            .height(1.0)
            .background(scheme.outline_variant),
    );
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(24.0)
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(crate::spacing(16.0)),
        Alignment::Center,
        move || {
            if let Some(mode) = &mode {
                Label::new(mode.trim().to_owned())
                    .variant(LabelVariant::Code)
                    .size(Size::Xs)
                    .show();
            }
            if read_only {
                Label::new("Read only")
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .show();
            }
            spacer(Modifier::new().weight(1.0));
            for part in [
                format!("Ln {}, Col {column}", line + 1),
                indent.clone(),
                language.clone(),
            ] {
                Label::new(part)
                    .variant(LabelVariant::Muted)
                    .size(Size::Xs)
                    .show();
            }
        },
    );
}

/// The width and height of one character in `style`, measured on a run of characters.
fn cell_size(style: &TextStyle) -> (f32, f32) {
    let sample = "0".repeat(64);
    let width = layout_text(&sample, style, None).size().width / 64.0;
    (width.max(1.0), style.line_height.max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Right-to-left runs take in the spaces between their words, not those
    /// at their ends.
    #[test]
    fn right_to_left_runs() {
        let text = "a = \"שלום עולם\" b";
        let runs = reversed_runs(text);
        assert_eq!(runs.len(), 1);
        assert_eq!(&text[runs[0].clone()], "שלום עולם");
        assert!(reversed_runs("plain text").is_empty());
    }

    /// Ranges are cut to a line and measured from its start, reaching past
    /// its end when they run on.
    #[test]
    fn ranges_fall_within_lines() {
        let line = 10..20;
        let inside: Vec<Range<usize>> = std::iter::once(12..15).collect();
        let across: Vec<Range<usize>> = std::iter::once(5..30).collect();
        assert_eq!(
            within(&inside, &line),
            std::iter::once(2..5).collect::<Vec<_>>()
        );
        assert_eq!(
            within(&across, &line),
            std::iter::once(0..11).collect::<Vec<_>>()
        );
        assert!(within(&[0..5, 25..30], &line).is_empty());
    }
}
