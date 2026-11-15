//! The prompts written while the agent was on a turn, listed over the
//! composer: each with its text and attached files, a button that hands it
//! over in place of the turn, one that edits it and one that removes it.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, Modifiers};

use crate::card::{Card, CardVariant};
use crate::icon_button::IconButton;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::text_input::TextInput;
use rustle::color_scheme;

/// One waiting prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Waiting {
    /// The number it goes by within its session.
    id: u64,
    /// What the reader wrote.
    text: String,
    /// The names of the files attached.
    files: Vec<String>,
}

/// How the list of queued prompts is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueuedVariant {
    /// An outlined card over the composer.
    #[default]
    Card,
    /// Rows only, inside another surface.
    Inline,
}

/// The callbacks of the list.
#[derive(Clone, Default)]
struct Callbacks {
    /// Hands the prompt numbered so over in place of the turn.
    on_hurry: Option<Rc<dyn Fn(u64)>>,
    /// Removes the prompt numbered so.
    on_remove: Option<Rc<dyn Fn(u64)>>,
    /// Holds the queue while a prompt is edited, or lets it go on.
    on_hold: Option<Rc<dyn Fn(bool)>>,
    /// Makes the prompt numbered so say the text.
    on_edit: Option<Rc<dyn Fn(u64, String)>>,
}

/// The list of queued prompts with a variant and a size.
#[derive(Clone)]
pub struct QueuedList {
    /// The waiting prompts, oldest first.
    waiting: Vec<Waiting>,
    /// How it is drawn.
    variant: QueuedVariant,
    /// Its size.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What its buttons do.
    callbacks: Callbacks,
}

impl QueuedList {
    /// An empty list.
    pub fn new() -> Self {
        Self {
            waiting: Vec::new(),
            variant: QueuedVariant::Card,
            size: Size::Sm,
            modifier: Modifier::new(),
            callbacks: Callbacks::default(),
        }
    }

    /// The list with the prompt numbered `id` saying `text` with `files` attached added.
    pub fn item(mut self, id: u64, text: impl Into<String>, files: Vec<String>) -> Self {
        self.waiting.push(Waiting {
            id,
            text: text.into(),
            files,
        });
        self
    }

    /// The list in `variant`.
    pub fn variant(mut self, variant: QueuedVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The list at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The list laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The list calling `on_hurry` with a prompt's number to hand it over in place of the turn.
    pub fn on_hurry(mut self, on_hurry: impl Fn(u64) + 'static) -> Self {
        self.callbacks.on_hurry = Some(Rc::new(on_hurry));
        self
    }

    /// The list calling `on_remove` with a prompt's number to drop it.
    pub fn on_remove(mut self, on_remove: impl Fn(u64) + 'static) -> Self {
        self.callbacks.on_remove = Some(Rc::new(on_remove));
        self
    }

    /// The list calling `on_hold` with true when editing a prompt starts and false when it is given up.
    pub fn on_hold(mut self, on_hold: impl Fn(bool) + 'static) -> Self {
        self.callbacks.on_hold = Some(Rc::new(on_hold));
        self
    }

    /// The list calling `on_edit` with a prompt's number and its new text when an edit is saved.
    pub fn on_edit(mut self, on_edit: impl Fn(u64, String) + 'static) -> Self {
        self.callbacks.on_edit = Some(Rc::new(on_edit));
        self
    }

    /// Emits the list, or nothing while no prompt waits.
    #[track_caller]
    pub fn show(self) {
        if self.waiting.is_empty() {
            return;
        }
        let editing = remember(|| None::<u64>);
        let waiting = self.waiting;
        let callbacks = self.callbacks;
        let size = self.size;
        let rows = move || {
            Label::new(format!("Queued · {}", waiting.len()))
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .show();
            for prompt in waiting {
                let callbacks = callbacks.clone();
                let editing = editing.clone();
                key(prompt.id, move || {
                    waiting_row(prompt, &callbacks, &editing, size)
                });
            }
        };
        match self.variant {
            QueuedVariant::Inline => column_arranged(
                self.modifier,
                Arrangement::SpacedBy(crate::spacing(4.0)),
                Alignment::Start,
                rows,
            ),
            QueuedVariant::Card => Card::new()
                .variant(CardVariant::Outlined)
                .size(Size::Sm)
                .modifier(self.modifier)
                .show(move || {
                    column_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(crate::spacing(4.0)),
                        Alignment::Start,
                        rows,
                    )
                }),
        }
    }
}

impl Default for QueuedList {
    /// The same as [`QueuedList::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// Emits the row of `prompt` at `size`: its text, or a field editing it
/// while `editing` names it, and the buttons calling `callbacks`.
#[track_caller]
fn waiting_row(prompt: Waiting, callbacks: &Callbacks, editing: &State<Option<u64>>, size: Size) {
    let scheme = color_scheme();
    let id = prompt.id;
    let edited = editing.get() == Some(id);
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(crate::spacing(4.0)),
        Alignment::Center,
        || {
            icon(phosphor::CLOCK, 12.0, scheme.on_surface_variant);
            if edited {
                let save = callbacks.on_edit.clone();
                let hold = callbacks.on_hold.clone();
                let done = editing.clone();
                let cancel = editing.clone();
                TextInput::new(prompt.text.clone())
                    .autofocus()
                    .size(size)
                    .modifier(
                        Modifier::new()
                            .weight(1.0)
                            .on_preview_key_event(move |event| {
                                let escape =
                                    event.pressed && event.is(Key::Escape, Modifiers::NONE);
                                if escape {
                                    cancel.set(None);
                                    if let Some(hold) = &hold {
                                        hold(false);
                                    }
                                }
                                escape
                            }),
                    )
                    .on_submit(move |text| {
                        if text.trim().is_empty() {
                            return;
                        }
                        done.set(None);
                        if let Some(save) = &save {
                            save(id, text);
                        }
                    })
                    .show();
                return;
            }
            let mut line = prompt.text.lines().next().unwrap_or_default().to_owned();
            if prompt.text.lines().nth(1).is_some() {
                line.push('…');
            }
            if !prompt.files.is_empty() {
                line.push_str(&format!(" · {} attached", prompt.files.len()));
            }
            let mut whole = prompt.text.clone();
            for file in &prompt.files {
                whole.push_str(&format!("\n📎 {file}"));
            }
            row_with(Modifier::new().weight(1.0).clip(), || {
                crate::tip::tip(&whole, || Label::new(line).size(size).show());
            });
            if let Some(hurry) = callbacks.on_hurry.clone() {
                IconButton::new(phosphor::ARROW_UP)
                    .tooltip("Send now, stopping the turn")
                    .size(Size::Xs)
                    .tooltip("Send next")
                    .on_click(move || hurry(id))
                    .show();
            }
            if callbacks.on_edit.is_some() {
                let hold = callbacks.on_hold.clone();
                let start = editing.clone();
                IconButton::new(phosphor::PENCIL_SIMPLE)
                    .tooltip("Edit")
                    .size(Size::Xs)
                    .tooltip("Edit")
                    .on_click(move || {
                        start.set(Some(id));
                        if let Some(hold) = &hold {
                            hold(true);
                        }
                    })
                    .show();
            }
            if let Some(remove) = callbacks.on_remove.clone() {
                IconButton::new(phosphor::TRASH)
                    .tooltip("Remove")
                    .size(Size::Xs)
                    .tooltip("Remove")
                    .on_click(move || remove(id))
                    .show();
            }
        },
    );
}
