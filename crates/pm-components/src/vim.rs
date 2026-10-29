//! Modal editing in the window: the keys of the reader carried out by vim's engine on the file shown, the effects it asks of the window, and what its status bar says.

use pm_editor::{ModalView, VimClipboard};
use pm_input::{Key, KeyEvent};
use pm_vim::{Clipboard, Command, Effect, Keystroke, Placement, Vim};

use crate::state::WorkspaceState;

/// What the status bar of modal editing says.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VimStatus {
    /// The mode, as vim names it: `NORMAL`, `INSERT`, `VISUAL LINE`.
    pub mode: &'static str,
    /// The keys gathered towards a command, or the line being typed after `:` or `/`.
    pub pending: Option<String>,
    /// The register being recorded into with `q`, when one is.
    pub recording: Option<char>,
}

/// The keystroke of modal editing that `event` is, or nothing for a key it has no use for.
pub fn keystroke(event: &KeyEvent) -> Option<Keystroke> {
    let modifiers = event.modifiers;
    if modifiers.logo || (modifiers.alt && !modifiers.control) {
        return None;
    }
    let key = match event.key {
        Key::Character(ch) => pm_vim::Key::Char(ch),
        Key::Space => pm_vim::Key::Char(' '),
        Key::Escape => pm_vim::Key::Escape,
        Key::Enter => pm_vim::Key::Enter,
        Key::Backspace => pm_vim::Key::Backspace,
        Key::Delete => pm_vim::Key::Delete,
        Key::Tab => pm_vim::Key::Tab,
        Key::ArrowLeft => pm_vim::Key::Left,
        Key::ArrowRight => pm_vim::Key::Right,
        Key::ArrowUp => pm_vim::Key::Up,
        Key::ArrowDown => pm_vim::Key::Down,
        Key::Home => pm_vim::Key::Home,
        Key::End => pm_vim::Key::End,
        Key::PageUp => pm_vim::Key::PageUp,
        Key::PageDown => pm_vim::Key::PageDown,
        Key::Function(_) => return None,
    };
    let ctrl = modifiers.control;
    let key = match key {
        pm_vim::Key::Char(ch) if ctrl => pm_vim::Key::Char(ch.to_ascii_lowercase()),
        key => key,
    };
    Some(Keystroke {
        key,
        ctrl,
        shift: modifiers.shift && !matches!(key, pm_vim::Key::Char(_)),
    })
}

/// A binding written as `keys => Action`, with `@ context` after when it holds only in one, as its keys, its action and its context; the context is normal mode when none is written.
pub fn read_binding(written: &str) -> Option<(&str, &str, &str)> {
    let (keys, rest) = written.split_once("=>")?;
    let (action, context) = rest.split_once('@').unwrap_or((rest, "normal"));
    let (keys, action, context) = (keys.trim(), action.trim(), context.trim());
    (!keys.is_empty() && !action.is_empty() && !context.is_empty())
        .then_some((keys, action, context))
}

/// Whether `written` is a binding the engine can read.
pub fn binding_reads(written: &str) -> bool {
    read_binding(written)
        .is_some_and(|(keys, action, context)| Vim::default().bind(keys, action, context).is_ok())
}

impl WorkspaceState {
    /// Puts the settings of modal editing to work: the reader's bindings over vim's own, how much of the clipboard the unnamed register shares, and, when it is off, every open file out of its modes.
    pub fn apply_vim(&mut self) {
        self.vim.reset_bindings();
        for written in &self.editor.vim_bindings {
            if let Some((keys, action, context)) = read_binding(written) {
                let _ = self.vim.bind(keys, action, context);
            }
        }
        let sharing: VimClipboard = self.editor.vim_clipboard;
        self.vim.share_clipboard(sharing.into());
        if !self.editor.vim_mode {
            for (_, viewer) in self.every_viewer() {
                if let crate::open_files::Contents::Text(document) = &mut viewer.contents {
                    document.vim_reset();
                }
            }
        }
    }

    /// Whether files are edited with vim's keys.
    pub fn vim_on(&self) -> bool {
        self.editor.vim_mode
    }

    /// What the status bar of modal editing says of the file shown, while modal editing is on and a file with text is shown.
    pub fn vim_status(&self) -> Option<VimStatus> {
        let document = self.shown_document().filter(|_| self.editor.vim_mode)?;
        Some(VimStatus {
            mode: document.vim_mode().label(),
            pending: document.vim_pending(),
            recording: self.vim.recording(),
        })
    }

    /// Whether the file shown is in a mode that types text in, where keys the engine does not claim are typed as they are.
    pub fn vim_typing(&self) -> bool {
        self.shown_document()
            .is_some_and(|document| document.vim_mode().is_typing())
    }

    /// Carries `key` out on the file shown when modal editing is on, the view being `height` logical pixels tall; the commands it asks of the window come back, or nothing when the key is not one modal editing takes.
    pub fn vim_key(
        &mut self,
        key: Keystroke,
        height: f32,
        clipboard: &mut dyn Clipboard,
    ) -> Option<Vec<Command>> {
        if !self.editor.vim_mode {
            return None;
        }
        let row = self.editor.row_height();
        let rows = ((height / row.max(1.0)).floor() as usize).max(1);
        let view = ModalView {
            top: self
                .shown_viewer()
                .map_or(0, |viewer| (viewer.scroll / row.max(1.0)) as usize),
            rows,
            margin: self.editor.vertical_scroll_margin,
            wrap: self.editor.preferred_line_length,
        };
        let mut vim = std::mem::take(&mut self.vim);
        let pressed = self
            .shown_text()
            .map(|document| document.vim_press(&mut vim, key, view, clipboard));
        self.vim = vim;
        let pressed = pressed.filter(|pressed| pressed.outcome.handled)?;
        if pressed
            .outcome
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::Jumped(_)))
        {
            self.remember_spot();
        }
        self.file_edits(pressed.edits);
        if let Some(depth) = pressed.squash
            && let Some(document) = self.shown_text()
        {
            document.field.squash_undo_since(depth);
        }
        let mut commands = Vec::new();
        for effect in pressed.outcome.effects {
            match effect {
                Effect::Command(command) => commands.push(command),
                Effect::Scroll(placement) => self.vim_scrolled_to(placement, rows),
                Effect::ScrollBy(lines) => self.vim_scrolled_by(lines),
                Effect::ScrollColumns(_) | Effect::Jumped(_) => {}
            }
        }
        if !self.vim_typing() {
            self.dismiss_typing_popups();
        }
        Some(commands)
    }

    /// Closes what hangs over the caret while it is being typed at, which leaving insert mode ends.
    fn dismiss_typing_popups(&mut self) {
        if let Some(document) = self.shown_text() {
            document.completion = None;
            document.hover = None;
            document.signature = None;
        }
    }

    /// Scrolls the view so that the line of the caret stands at `placement` of it.
    fn vim_scrolled_to(&mut self, placement: Placement, rows: usize) {
        let row = self.editor.row_height();
        let columns = self.wrap_columns();
        let Some(index) = self.git.shown else {
            return;
        };
        let Some(viewer) = self.git.viewers.get_mut(index) else {
            return;
        };
        let crate::open_files::Contents::Text(document) = &viewer.contents else {
            return;
        };
        let line = document.line_of(document.field.caret());
        let at = document.wrapped_row_of_line(line, columns) as f32;
        let rows = rows as f32;
        viewer.scroll = match placement {
            Placement::Top => at * row,
            Placement::Center => (at - (rows / 2.0).floor()) * row,
            Placement::Bottom => (at - (rows - 1.0)) * row,
        }
        .max(0.0);
    }

    /// Scrolls the view by `lines`, down when positive.
    fn vim_scrolled_by(&mut self, lines: isize) {
        let row = self.editor.row_height();
        if let Some(viewer) = self
            .git
            .shown
            .and_then(|index| self.git.viewers.get_mut(index))
        {
            viewer.scroll = (viewer.scroll + lines as f32 * row).max(0.0);
        }
    }
}
