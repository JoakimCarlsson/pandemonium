//! A row of a file tree: indented by depth, a caret for a folder, the
//! entry's icon, its name tinted by its change mark, and the mark at the end;
//! it can be dragged onto a folder, takes files dropped from other
//! applications, and opens a menu on a secondary click. In its editing size
//! it holds a box the entry's name is typed in.

use std::path::PathBuf;
use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, Modifiers};

use crate::app_theme::{ExtendedColors, extended_colors};
use crate::file_icon::{FileIcon, FileIconVariant};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::text_input::TextInput;
use rustle::{ColorScheme, color_scheme};

/// How far each level of a tree is indented.
const INDENT: f32 = 12.0;

/// How a tree row reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeItemVariant {
    /// The name in body text.
    #[default]
    Plain,
    /// The name muted, for an entry git ignores.
    Dimmed,
}

/// The box a tree row types a name in.
#[derive(Clone)]
struct Editor {
    /// The name as typed.
    text: String,
    /// What typing does.
    on_change: Rc<dyn Fn(String)>,
    /// What Enter does.
    on_submit: Rc<dyn Fn()>,
    /// What Escape does.
    on_cancel: Rc<dyn Fn()>,
}

/// A file tree row with a variant and a size.
#[derive(Clone)]
pub struct TreeItem {
    /// The entry's name.
    name: String,
    /// How many folders hold it.
    depth: usize,
    /// Whether it is a folder, and then whether it is unfolded.
    folder: Option<bool>,
    /// Its change mark, such as `M`.
    mark: Option<char>,
    /// Whether it is chosen.
    selected: bool,
    /// How it reads.
    variant: TreeItemVariant,
    /// Its height.
    size: Size,
    /// The box its name is typed in, while it is.
    editor: Option<Editor>,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
    /// What a secondary click does.
    on_menu: Option<Rc<dyn Fn()>>,
    /// The path it carries when dragged.
    drag: Option<String>,
    /// What dropping a dragged entry's path on it does.
    on_drop: Option<Rc<dyn Fn(String)>>,
    /// What dropping a file from another application on it does.
    on_file_drop: Option<Rc<dyn Fn(PathBuf)>>,
}

impl TreeItem {
    /// A plain row for the file called `name`, `depth` folders deep.
    pub fn new(name: impl Into<String>, depth: usize) -> Self {
        Self {
            name: name.into(),
            depth,
            folder: None,
            mark: None,
            selected: false,
            variant: TreeItemVariant::Plain,
            size: Size::Sm,
            editor: None,
            on_click: None,
            on_menu: None,
            drag: None,
            on_drop: None,
            on_file_drop: None,
        }
    }

    /// The row of a folder, unfolded when `expanded`.
    pub fn folder(mut self, expanded: bool) -> Self {
        self.folder = Some(expanded);
        self
    }

    /// The row marked with the change mark `mark`.
    pub fn mark(mut self, mark: Option<char>) -> Self {
        self.mark = mark;
        self
    }

    /// The row highlighted as chosen when `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The row in `variant`.
    pub fn variant(mut self, variant: TreeItemVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The row at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The row holding a box with `text` in place of its name, calling
    /// `on_change` as it is typed in, `on_submit` on Enter and `on_cancel` on Escape.
    pub fn editor(
        mut self,
        text: impl Into<String>,
        on_change: impl Fn(String) + 'static,
        on_submit: impl Fn() + 'static,
        on_cancel: impl Fn() + 'static,
    ) -> Self {
        self.editor = Some(Editor {
            text: text.into(),
            on_change: Rc::new(on_change),
            on_submit: Rc::new(on_submit),
            on_cancel: Rc::new(on_cancel),
        });
        self
    }

    /// The row calling `on_click` when clicked.
    pub fn on_click(mut self, on_click: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// The row calling `on_menu` on a secondary click.
    pub fn on_menu(mut self, on_menu: impl Fn() + 'static) -> Self {
        self.on_menu = Some(Rc::new(on_menu));
        self
    }

    /// The row carrying `path` when dragged.
    pub fn draggable(mut self, path: impl Into<String>) -> Self {
        self.drag = Some(path.into());
        self
    }

    /// The row calling `on_drop` with the path of an entry dropped on it.
    pub fn on_drop(mut self, on_drop: impl Fn(String) + 'static) -> Self {
        self.on_drop = Some(Rc::new(on_drop));
        self
    }

    /// The row calling `on_file_drop` with each file dropped on it from another application.
    pub fn on_file_drop(mut self, on_file_drop: impl Fn(PathBuf) + 'static) -> Self {
        self.on_file_drop = Some(Rc::new(on_file_drop));
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
        let height = crate::row_height(height);
        let fill = if self.selected {
            scheme.secondary_container
        } else if hovered.get() {
            scheme.surface_container_high
        } else {
            Color::TRANSPARENT
        };
        let mut modifier = Modifier::new()
            .fill_max_width()
            .height(height)
            .background_rounded(fill, 4.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if let Some(path) = self.drag.clone() {
            modifier = modifier.drag_source(path);
        }
        if let Some(on_drop) = self.on_drop.clone() {
            modifier = modifier.drop_target(move |payload| {
                if let Some(path) = payload.downcast_ref::<String>() {
                    on_drop(path.clone());
                }
            });
        }
        if let Some(on_file_drop) = self.on_file_drop.clone() {
            modifier = modifier.on_file_drop(move |file| on_file_drop(file));
        }
        if let Some(on_menu) = self.on_menu.clone() {
            modifier = modifier.on_context_menu(move |_| on_menu());
        }
        if let Some(on_click) = self.on_click.clone() {
            modifier = modifier
                .cursor(CursorIcon::Pointer)
                .clickable(move || on_click());
        }
        let tint = mark_colour(&scheme, &extended, self.mark);
        let name_variant = match self.variant {
            TreeItemVariant::Plain => LabelVariant::Body,
            TreeItemVariant::Dimmed => LabelVariant::Subtle,
        };
        let icon_variant = match self.variant {
            TreeItemVariant::Plain => FileIconVariant::Coloured,
            TreeItemVariant::Dimmed => FileIconVariant::Muted,
        };
        row_arranged(
            modifier.padding_xy(6.0, 0.0),
            Arrangement::SpacedBy(crate::spacing(4.0)),
            Alignment::Center,
            move || {
                spacer(Modifier::new().width(self.depth as f32 * INDENT));
                match self.folder {
                    Some(expanded) => icon(
                        if expanded {
                            phosphor::CARET_DOWN
                        } else {
                            phosphor::CARET_RIGHT
                        },
                        10.0,
                        scheme.on_surface_variant,
                    ),
                    None => spacer(Modifier::new().width(10.0)),
                }
                let mut file_icon = FileIcon::new(self.name.clone())
                    .size(Size::Sm)
                    .variant(icon_variant);
                if let Some(expanded) = self.folder {
                    file_icon = file_icon.folder(expanded);
                }
                file_icon.show();
                spacer(Modifier::new().width(2.0));
                match self.editor {
                    Some(editor) => {
                        let on_submit = editor.on_submit.clone();
                        let on_cancel = editor.on_cancel.clone();
                        let on_change = editor.on_change.clone();
                        stack_with(
                            Modifier::new().weight(1.0).on_key_event(move |event| {
                                if event.is(Key::Escape, Modifiers::NONE) {
                                    on_cancel();
                                    return true;
                                }
                                false
                            }),
                            || {
                                TextInput::new(editor.text)
                                    .size(Size::Xs)
                                    .autofocus()
                                    .modifier(Modifier::new().fill_max_width())
                                    .on_change(move |text| on_change(text))
                                    .on_submit(move |_| on_submit())
                                    .show();
                            },
                        );
                    }
                    None => {
                        let mut label = Label::new(self.name)
                            .variant(name_variant)
                            .size(Size::Sm)
                            .modifier(Modifier::new().weight(1.0));
                        if let (Some(tint), TreeItemVariant::Plain) = (tint, self.variant) {
                            label = label.colour(tint);
                        }
                        label.show();
                        if let (Some(mark), Some(tint)) = (self.mark, tint) {
                            Label::new(mark.to_string())
                                .variant(LabelVariant::Code)
                                .colour(tint)
                                .size(Size::Xs)
                                .show();
                        }
                    }
                }
            },
        );
    }
}

/// The colour of the change mark `mark`: added in the success tone, removed
/// in the danger tone, anything else changed in the warning tone.
pub fn mark_colour(
    scheme: &ColorScheme,
    extended: &ExtendedColors,
    mark: Option<char>,
) -> Option<Color> {
    mark.map(|mark| match mark {
        'A' | '?' | 'U' => extended.success,
        'D' => scheme.error,
        _ => extended.warning,
    })
}
