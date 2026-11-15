//! A changed file in a list: a box that stages it, its name and folder, how
//! many lines it adds and removes, its status letter, and buttons that show
//! under the pointer; picked rows are highlighted.

use std::rc::Rc;

use rustle::Modifiers;
use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::checkbox::{CheckState, Checkbox};
use crate::icon_button::IconButton;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a changed file reads, which colours its status letter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChangeRowVariant {
    /// A tracked file changed: the warning tone.
    #[default]
    Modified,
    /// A new or untracked file: the success tone.
    Added,
    /// A deleted file: the danger tone, its name muted.
    Deleted,
    /// A file changed on both sides of a merge: the danger tone.
    Conflicted,
}

/// A button shown at the end of a row under the pointer.
#[derive(Clone)]
struct RowAction {
    /// Its icon.
    icon: Icon,
    /// What a click does.
    on_click: Rc<dyn Fn()>,
}

/// A changed file row with a variant and a size.
#[derive(Clone)]
pub struct ChangeRow {
    /// The file's name.
    name: String,
    /// The folder it is in, muted after the name.
    folder: String,
    /// Its status letter.
    status: char,
    /// Lines added and removed.
    tally: (usize, usize),
    /// Whether its box is ticked.
    checked: bool,
    /// Whether it is picked.
    selected: bool,
    /// How it reads.
    variant: ChangeRowVariant,
    /// Its height.
    size: Size,
    /// How far its content is indented.
    indent: f32,
    /// The buttons it shows under the pointer.
    actions: Vec<RowAction>,
    /// What a click does, given the modifier keys held.
    on_click: Option<Rc<dyn Fn(Modifiers)>>,
    /// What a double click does.
    on_open: Option<Rc<dyn Fn()>>,
    /// What ticking its box does.
    on_toggle: Option<Rc<dyn Fn()>>,
    /// What a secondary click does.
    on_menu: Option<Rc<dyn Fn()>>,
}

impl ChangeRow {
    /// A row for the file `name` in `folder`, marked with `status`.
    pub fn new(name: impl Into<String>, folder: impl Into<String>, status: char) -> Self {
        Self {
            name: name.into(),
            folder: folder.into(),
            status,
            tally: (0, 0),
            checked: false,
            selected: false,
            variant: ChangeRowVariant::Modified,
            size: Size::Sm,
            indent: 0.0,
            actions: Vec::new(),
            on_click: None,
            on_open: None,
            on_toggle: None,
            on_menu: None,
        }
    }

    /// The row saying it adds `added` lines and removes `removed`.
    pub fn tally(mut self, added: usize, removed: usize) -> Self {
        self.tally = (added, removed);
        self
    }

    /// The row with its box ticked when `checked`.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// The row highlighted when `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The row in `variant`.
    pub fn variant(mut self, variant: ChangeRowVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The row at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The row with its content indented by `indent`.
    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent;
        self
    }

    /// The row showing a button with `icon` under the pointer that calls `on_click`.
    pub fn action(mut self, icon: Icon, on_click: impl Fn() + 'static) -> Self {
        self.actions.push(RowAction {
            icon,
            on_click: Rc::new(on_click),
        });
        self
    }

    /// The row calling `on_click` with the modifier keys held when clicked.
    pub fn on_click(mut self, on_click: impl Fn(Modifiers) + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// The row calling `on_open` when double clicked.
    pub fn on_open(mut self, on_open: impl Fn() + 'static) -> Self {
        self.on_open = Some(Rc::new(on_open));
        self
    }

    /// The row calling `on_toggle` when its box is clicked.
    pub fn on_toggle(mut self, on_toggle: impl Fn() + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }

    /// The row calling `on_menu` on a secondary click.
    pub fn on_menu(mut self, on_menu: impl Fn() + 'static) -> Self {
        self.on_menu = Some(Rc::new(on_menu));
        self
    }

    /// Emits the row.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let hovered = remember(|| false);
        let height = match self.size {
            Size::Xs => 20.0,
            Size::Sm => 24.0,
            Size::Md => 28.0,
            Size::Lg => 32.0,
        };
        let fill = if self.selected {
            scheme.secondary_container
        } else if hovered.get() {
            scheme.surface_container_high
        } else {
            Color::TRANSPARENT
        };
        let tone = match self.variant {
            ChangeRowVariant::Modified => extended.warning,
            ChangeRowVariant::Added => extended.success,
            ChangeRowVariant::Deleted | ChangeRowVariant::Conflicted => scheme.error,
        };
        let mut modifier = Modifier::new()
            .fill_max_width()
            .height(height)
            .background_rounded(fill, 4.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            })
            .cursor(CursorIcon::Pointer);
        if self.on_click.is_some() || self.on_open.is_some() {
            let (on_click, on_open) = (self.on_click.clone(), self.on_open.clone());
            modifier = modifier.on_press(move |press| {
                if let Some(on_click) = &on_click {
                    on_click(press.modifiers);
                }
                if let (Some(on_open), 2) = (&on_open, press.click_count) {
                    on_open();
                }
            });
        }
        if let Some(on_menu) = self.on_menu.clone() {
            modifier = modifier.on_context_menu(move |_| on_menu());
        }
        let showing_actions = hovered.get() && !self.actions.is_empty();
        row_arranged(
            modifier.padding_xy(4.0, 0.0),
            Arrangement::SpacedBy(6.0),
            Alignment::Center,
            move || {
                if self.indent > 0.0 {
                    spacer(Modifier::new().width(self.indent));
                }
                let toggle = self.on_toggle.clone();
                Checkbox::new(if self.checked {
                    CheckState::On
                } else {
                    CheckState::Off
                })
                .size(Size::Xs)
                .on_toggle(move || {
                    if let Some(toggle) = &toggle {
                        toggle();
                    }
                })
                .show();
                row_arranged(
                    Modifier::new().weight(1.0).clip(),
                    Arrangement::SpacedBy(6.0),
                    Alignment::Center,
                    || {
                        Label::new(self.name.clone())
                            .size(Size::Xs)
                            .colour(if self.variant == ChangeRowVariant::Deleted {
                                scheme.on_surface_variant
                            } else {
                                scheme.on_surface
                            })
                            .show();
                        if !self.folder.is_empty() {
                            Label::new(self.folder.clone())
                                .variant(LabelVariant::Subtle)
                                .size(Size::Xs)
                                .show();
                        }
                    },
                );
                if showing_actions {
                    for (place, action) in self.actions.iter().enumerate() {
                        let on_click = action.on_click.clone();
                        let glyph = action.icon.clone();
                        key(place, || {
                            IconButton::new(glyph)
                                .size(Size::Xs)
                                .on_click(move || on_click())
                                .show();
                        });
                    }
                } else {
                    let (added, removed) = self.tally;
                    if added > 0 {
                        Label::new(format!("+{added}"))
                            .colour(extended.success)
                            .size(Size::Xs)
                            .show();
                    }
                    if removed > 0 {
                        Label::new(format!("-{removed}"))
                            .colour(scheme.error)
                            .size(Size::Xs)
                            .show();
                    }
                }
                Label::new(self.status.to_string())
                    .colour(tone)
                    .size(Size::Xs)
                    .modifier(Modifier::new().width(10.0))
                    .show();
            },
        );
    }
}
