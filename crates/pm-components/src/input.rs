//! The input: a box holding editable text, one line or several, with a look per variant and a measure per size.
//!
//! The text is a rustle text field editing a [`TextFieldState`] the model holds,
//! drawn in the input's type with its placeholder in the decoration. The field
//! holds the rustle focus of its control from the registry the views are
//! composed with (see [`crate::focus`]), so the app reads whether it has the
//! keyboard through that focus. The field edits, selects, copies, cuts,
//! pastes, undoes and redoes by itself, as rustle's text field does; what it
//! hands in on Enter, and on Control or Command with Enter, whether Enter
//! breaks its lines, whether it hides what it holds and what takes a paste
//! before it are declared where the input is built, as a Compose text field
//! declares its `KeyboardOptions`, `onKeyboardAction` and line limits.

use crate::events::{Event, OnClick, send};
use crate::target::Target;
use crate::theme;
use crate::theme::{FocusOutline, Indicated};
use rustle::{Alignment, Arrangement};
use rustle::{Color, Icon, phosphor};
use std::cell::RefCell;
use std::rc::Rc;

use rustle::{
    BasicTextFieldOptions, Modifier, ScrollState, TextFieldDecorator, TextFieldLineLimits,
    TextFieldState, TextObfuscationMode, TextSelectionColors,
};
use rustle::{
    CursorIcon, Key, KeyEvent, KeyboardActionHandler, LOCAL_TEXT_SELECTION_COLORS, PointerButton,
    PointerPhase,
};

use crate::events;
use crate::events::Interactive;
use crate::marked;
use crate::parts::glyph;
use crate::size::Size;
use crate::theme::{Radius, Role};

/// How tall a line of a multiline input is as a share of its type size.
const LINE_RATIO: f32 = 1.57;

/// How tall a line of a multiline input is, in logical pixels.
const LINE: f32 = 22.0;

/// How tall a multiline input is above and below its lines together, in logical pixels.
const PADDING: f32 = 18.0;

/// How many lines a multiline input shows at most; longer text scrolls to keep the caret in view.
const MAX_LINES: usize = 8;

/// The size and leading every line of a multiline input is set in.
fn typography() -> rustle::TextStyle {
    theme::text_style(14, LINE_RATIO)
}

/// How an input looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// A filled box with an outline.
    #[default]
    Default,
    /// A filled box with an outline for a line typed in ordinary text, which a [`Input::leading_icon`] stands before.
    Bordered,
    /// No fill and no outline, for a line edited in place.
    Ghost,
    /// Several wrapped lines with no fill and no outline, for a box that draws its own; give the lines with [`Input::wrap`].
    Multiline,
}

/// What takes a paste in an input before the field does, and says whether it took it.
#[derive(Clone)]
pub struct OnPaste(Rc<dyn Fn() -> bool>);

impl std::fmt::Debug for OnPaste {
    /// Writes that there is a lambda, which has nothing else to show.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OnPaste")
    }
}

/// What an input does with Enter: what it hands in, what it hands in with Control or
/// Command held, and whether Enter breaks a line of a multiline input instead.
#[derive(Clone, Debug, Default)]
struct Submits {
    /// What Enter hands in.
    plain: Option<OnClick>,
    /// What Control or Command with Enter hands in, when it is something of its own.
    hurried: Option<OnClick>,
    /// Whether Enter breaks a line of a multiline input, Control or Command with it handing in.
    breaks: bool,
}

/// An input being described: the text it edits, what it says while empty,
/// how it looks and whether it is switched off.
///
/// Build it with [`Input::new`], change it with the methods that follow, and
/// emit it with [`Input::build`].
#[derive(Clone, Debug)]
pub struct Input<'a> {
    /// The text being edited.
    field: &'a TextFieldState,
    /// What shows while the line is empty.
    placeholder: &'a str,
    /// The name it answers to.
    id: Target,
    /// How it looks.
    variant: Variant,
    /// How big it is.
    size: Size,
    /// The glyph before the line, if any.
    leading: Option<Icon>,
    /// Whether what it holds is not acceptable.
    invalid: bool,
    /// How many lines tall a multiline box is at least.
    rows: u32,
    /// Whether what it holds is shown as dots and kept off the clipboard, as a key is.
    secure: bool,
    /// The scroll state of a multiline box's lines, when its owner holds it.
    scroll: Option<&'a ScrollState>,
    /// What Enter does in it.
    submits: Submits,
    /// What takes a paste before the field does, if anything.
    on_paste: Option<OnPaste>,
    /// What the caller puts around its box: sizes, growth, margins, placement.
    modifier: Modifier,
}

impl<'a> Input<'a> {
    /// An input editing `field`, answering to `id`.
    pub fn new(field: &'a TextFieldState, id: Target) -> Self {
        Self {
            field,
            placeholder: "",
            id,
            variant: Variant::Default,
            size: Size::Md,
            leading: None,
            invalid: false,
            rows: 1,
            secure: false,
            scroll: None,
            submits: Submits::default(),
            on_paste: None,
            modifier: Modifier::new(),
        }
    }

    /// A bordered input editing `field` with a magnifying glass before the line, for what is looked for.
    pub fn search(field: &'a TextFieldState, id: Target) -> Self {
        Self::new(field, id)
            .variant(Variant::Bordered)
            .leading_icon(phosphor::MAGNIFYING_GLASS)
    }

    /// The name the input answers to.
    pub fn answers_to(&self) -> Target {
        self.id
    }

    /// The input with `icon` before its line.
    pub fn leading_icon(mut self, icon: Icon) -> Self {
        self.leading = Some(icon);
        self
    }

    /// The multiline input at least `rows` lines tall.
    pub fn rows(mut self, rows: u32) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// The input showing a dot for each character of what it holds, which can
    /// be neither copied nor cut, while `secure` is true: for keys and tokens.
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// The input running `on_submit` when Enter is pressed in it: its keyboard action,
    /// Compose's `onKeyboardAction`. In a multiline input Shift with Enter breaks a line.
    pub fn on_submit(mut self, on_submit: impl Fn() + 'static) -> Self {
        self.submits.plain = Some(OnClick::new(on_submit));
        self
    }

    /// The input handing the press of `id` to the workspace when Enter is pressed in it,
    /// as [`on_submit`](Self::on_submit) runs it.
    pub fn submits(self, id: Target) -> Self {
        self.on_submit(send(Event::pressing(id)))
    }

    /// The input running `on_hurried_submit` when Control or Command with Enter is
    /// pressed in it, instead of what Enter hands in.
    pub fn on_hurried_submit(mut self, on_hurried_submit: impl Fn() + 'static) -> Self {
        self.submits.hurried = Some(OnClick::new(on_hurried_submit));
        self
    }

    /// The input handing the press of `id` to the workspace when Control or Command with
    /// Enter is pressed in it, as [`on_hurried_submit`](Self::on_hurried_submit) runs it.
    pub fn hurries(self, id: Target) -> Self {
        self.on_hurried_submit(send(Event::pressing(id)))
    }

    /// The multiline input breaking a line on Enter, and handing in with Control or
    /// Command held with it, as a message is written.
    pub fn breaks_lines_on_enter(mut self) -> Self {
        self.submits.breaks = true;
        self
    }

    /// The input offering a paste to `on_paste` before its field pastes the clipboard's
    /// text: when `on_paste` says it took the paste, the field pastes nothing.
    pub fn on_paste(mut self, on_paste: impl Fn() -> bool + 'static) -> Self {
        self.on_paste = Some(OnPaste(Rc::new(on_paste)));
        self
    }

    /// The multiline input with its lines scrolled by `scroll`, which its owner holds.
    pub fn scroll(mut self, scroll: &'a ScrollState) -> Self {
        self.scroll = Some(scroll);
        self
    }

    /// The input saying `placeholder` while it is empty.
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// The input with `variant`'s look.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The input at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The input outlined as holding something that is not acceptable.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// The input with `modifier` put around its box, outside its own look:
    /// sizes, growth, margins, placement, tooltips. Calls accumulate.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the input: a ring and a blinking caret while it holds keyboard
    /// focus, and the selection highlighted. It takes the keyboard back as it
    /// appears when it is the field the reader was typing in when the window
    /// last closed.
    #[track_caller]
    pub fn build(self) {
        crate::focus::restores_focus(self.id);
        if self.variant == Variant::Multiline {
            return self.build_multiline();
        }
        let (height, font) = match (self.variant, self.size) {
            (Variant::Bordered, Size::Md) => (9.0, 12),
            (Variant::Bordered, Size::Lg) => (10.0, 12),
            (_, Size::Xs) => (7.0, 12),
            (_, Size::Sm) => (8.0, 12),
            (_, Size::Md) => (10.0, 13),
            (_, Size::Lg) => (12.0, 14),
        };
        let id = self.id;
        let invalid = self.invalid;
        let boxed = matches!(self.variant, Variant::Default | Variant::Bordered);
        let mut own = Modifier::new().fill_max_width().height(theme::sp(height));
        if boxed {
            own = own.background_rounded(theme::color(Role::Surface), Radius::Lg.px());
        }
        let held = theme::Focused::Control(crate::focus::is_focused(id));
        if boxed || invalid {
            let (idle, focused) = if invalid {
                (theme::role(Role::Danger), theme::role(Role::Danger))
            } else {
                (theme::role(Role::Border), theme::role(Role::Focused))
            };
            let radius = Radius::Lg;
            own = own.focus_outline(crate::focus::is_focused(id), idle, focused, radius);
        }
        own = own.indicated(
            theme::FocusRing {
                width: 2.0,
                radius: Radius::Lg,
            },
            held,
        );
        if boxed || invalid {
            own = own.padding(1.0);
        }
        own = editing(own, id, (self.secure, false)).padding_xy(theme::sp(3), 0.0);
        let mut kind = theme::text_style(font, 1.4).color(theme::color(Role::Text));
        kind = if self.variant == Variant::Default {
            theme::in_mono(kind)
        } else {
            kind.line_height(kind.font_size * 1.57)
        };
        let (field, placeholder, leading) = (self.field, self.placeholder, self.leading);
        let secure = self.secure;
        let (submits, on_paste) = (self.submits, self.on_paste);
        rustle::row_arranged(
            self.modifier.chain(own),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            move || {
                if let Some(icon) = leading {
                    glyph(icon, 4.0, Role::Muted);
                }
                FieldText {
                    field,
                    id,
                    kind,
                    placeholder,
                    lines: None,
                    secure,
                    scroll: None,
                    submits,
                    on_paste,
                }
                .build(Modifier::new().weight(1.0));
            },
        );
    }

    /// Emits a multiline input: its wrapped lines in a column at least
    /// [`Input::rows`] lines tall, showing [`MAX_LINES`] at most and scrolled
    /// to keep the caret in view, the caret and selection placed in them.
    fn build_multiline(self) {
        let minimum = self.rows as f32 * LINE + PADDING;
        let text = FieldText {
            field: self.field,
            id: self.id,
            kind: typography().color(theme::color(Role::Text)),
            placeholder: self.placeholder,
            lines: Some(MAX_LINES),
            secure: false,
            scroll: self.scroll,
            submits: self.submits,
            on_paste: self.on_paste,
        };
        let mut own = Modifier::new()
            .fill_max_width()
            .height_in(theme::sp(minimum / 4.0), f32::INFINITY);
        own = editing(own, self.id, (false, true)).padding_each(
            theme::sp(4),
            theme::sp(3.5),
            theme::sp(4),
            theme::sp(1),
        );
        rustle::column_arranged(
            self.modifier.chain(own),
            Arrangement::Start,
            Alignment::Start,
            move || text.build(Modifier::new().fill_max_width()),
        );
    }
}

/// The text of an input: a rustle text field editing the model's state in the
/// input's type, with the placeholder behind it while it is empty.
struct FieldText<'a> {
    /// The state edited.
    field: &'a TextFieldState,
    /// The name the input answers to, whose focus the field follows.
    id: Target,
    /// The type the text is set in.
    kind: rustle::TextStyle,
    /// What shows while the text is empty.
    placeholder: &'a str,
    /// How many lines it shows at most before it scrolls, when it holds several.
    lines: Option<usize>,
    /// Whether its text is shown as dots and kept off the clipboard.
    secure: bool,
    /// The scroll state of its lines, when its owner holds it.
    scroll: Option<&'a ScrollState>,
    /// What Enter does in it.
    submits: Submits,
    /// What takes a paste before the field does, if anything.
    on_paste: Option<OnPaste>,
}

impl FieldText<'_> {
    /// Emits the field with `modifier` around it: holding the registry's focus of
    /// the input, handing in on Enter as declared, offering pastes to what takes
    /// them first, its selection in the theme's selection colour.
    #[track_caller]
    fn build(self, modifier: Modifier) {
        let id = self.id;
        let multiline = self.lines.is_some();
        let kind = self.kind;
        let style = match self.lines {
            Some(_) => {
                let style = kind.wrap(rustle::TextWrap::Word);
                rustle::TextStyle {
                    line_height: style.line_height.round(),
                    ..style
                }
            }
            None => kind,
        };
        let placeholder_style = rustle::TextStyle {
            wrap: rustle::TextWrap::None,
            max_lines: None,
            ..kind.color(theme::color(Color::from(Role::Subtle)))
        };
        let focus = crate::focus::focus_of(id);
        let composing = focus
            .preedit()
            .is_some_and(|preedit| !preedit.text.is_empty());
        let empty = self.field.with_text(str::is_empty) && !composing;
        let placeholder = self.placeholder.to_owned();
        let shaped: Rc<RefCell<Option<Rc<rustle::TextLayout>>>> =
            rustle::remember(Rc::default).get();
        let remembered = rustle::remember_scroll_state();
        let scroll = self.scroll.cloned().unwrap_or(remembered);
        scroll.set_scrollbar(match self.lines {
            Some(_) => theme::scrollbar(),
            None => rustle::ScrollbarStyle {
                visibility: rustle::ScrollbarVisibility::Hidden,
                ..theme::scrollbar()
            },
        });
        let context = context_press(id, self.field, &shaped, &scroll, (self.secure, multiline));
        let decorator: TextFieldDecorator = Rc::new(move |inner: &dyn Fn()| {
            rustle::stack_with(
                Modifier::new().fill_max_width().chain(context.clone()),
                || {
                    if empty && !placeholder.is_empty() {
                        rustle::text_ellipsized(
                            Modifier::new().fill_max_width(),
                            placeholder.clone(),
                            placeholder_style,
                        );
                    }
                    inner();
                },
            );
        });
        let options = BasicTextFieldOptions {
            text_style: Some(style),
            cursor_color: Some(style.color),
            line_limits: match self.lines {
                Some(most) => TextFieldLineLimits::MultiLine {
                    min_height_in_lines: 1,
                    max_height_in_lines: most,
                },
                None => TextFieldLineLimits::SingleLine,
            },
            decorator: Some(decorator),
            scroll_state: Some(scroll),
            on_text_layout: Some(Rc::new(move |layout| *shaped.borrow_mut() = Some(layout))),
            on_keyboard_action: self.submits.plain.clone().map(|submit| {
                let submit = submit.lambda();
                let action: KeyboardActionHandler = Rc::new(move |_| submit());
                action
            }),
            focus: Some(focus.clone()),
            ..BasicTextFieldOptions::default()
        };
        let modifier = modifier.chain(entering(&focus, self.submits, self.on_paste, multiline));
        let colors = TextSelectionColors {
            background_color: marked::selection_color(&theme::theme()),
            ..LOCAL_TEXT_SELECTION_COLORS.current()
        };
        let field = self.field;
        let secure = self.secure;
        LOCAL_TEXT_SELECTION_COLORS.provide(colors, || {
            if secure {
                rustle::basic_secure_text_field(
                    modifier,
                    field,
                    TextObfuscationMode::Hidden,
                    rustle::DEFAULT_OBFUSCATION_CHARACTER,
                    options,
                );
            } else {
                rustle::basic_text_field_with(modifier, field, options);
            }
        });
    }
}

/// Whether `event` is the paste shortcut, Control or Command with V going down.
fn is_paste(event: &KeyEvent) -> bool {
    event.pressed
        && event.modifiers.command()
        && !event.modifiers.alt
        && matches!(&event.key, Key::Character(letter) if letter.eq_ignore_ascii_case("v"))
}

/// The modifier that sees the keys of the field holding `focus` before the field does:
/// Enter hands in what `submits` says, unless it breaks a line of a `multiline` field
/// (Shift held, or Enter breaking lines without Control or Command), and the paste
/// shortcut goes to `on_paste` first, the field pasting only what it leaves. Enter in a
/// single line without Control or Command is left to the field's keyboard action.
fn entering(
    focus: &rustle::Focus,
    submits: Submits,
    on_paste: Option<OnPaste>,
    multiline: bool,
) -> Modifier {
    let focus = focus.clone();
    Modifier::new().on_preview_key_event(move |event| {
        if !event.pressed || !focus.is_focused() {
            return false;
        }
        if is_paste(event) {
            return on_paste.as_ref().is_some_and(|on_paste| (on_paste.0)());
        }
        if event.key != Key::Enter {
            return false;
        }
        let hurried = event.modifiers.command();
        let breaks = event.modifiers.shift || (submits.breaks && !hurried);
        let handing = match (&submits.hurried, &submits.plain) {
            _ if multiline && breaks => None,
            (Some(hurried_submit), _) if hurried => Some(hurried_submit),
            (_, Some(submit)) if multiline => Some(submit),
            _ => None,
        };
        match handing {
            Some(submit) => {
                submit.run();
                true
            }
            None => false,
        }
    })
}

/// The modifier that takes a secondary press on the text of the input `id`:
/// the caret of `field` goes to the character under the pointer, found in the
/// `shaped` text scrolled by `scroll`, unless the press is inside the
/// selection, and the app opens the input's menu, told whether the field is
/// secure and holds several lines, as `(secure, multiline)` says.
fn context_press(
    id: Target,
    field: &TextFieldState,
    shaped: &Rc<RefCell<Option<Rc<rustle::TextLayout>>>>,
    scroll: &ScrollState,
    (secure, multiline): (bool, bool),
) -> Modifier {
    let operations = events::operations();
    let focus = crate::focus::focus_of(id);
    let (field, shaped, scroll) = (field.clone(), Rc::clone(shaped), scroll.clone());
    Modifier::new().on_pointer_button(PointerButton::Secondary, move |event| {
        if event.phase != PointerPhase::Press {
            return;
        }
        if let Some(layout) = shaped.borrow().clone() {
            let at = rustle::Offset::new(event.position.x, event.position.y);
            let along = if layout.line_count() > 1 {
                rustle::Offset::new(at.x, at.y + scroll.offset())
            } else {
                rustle::Offset::new(at.x + scroll.offset(), at.y)
            };
            let byte = layout.hit_test(along);
            let index = layout.text()[..byte.min(layout.text().len())]
                .chars()
                .count();
            let selection = field.selection();
            if index < selection.min() || index > selection.max() || selection.collapsed() {
                field.edit(|buffer| buffer.place_cursor_before_char_at(index));
            }
        }
        focus.request_focus();
        operations.open_text_field_menu(id, crate::events::window_at(event), secure, multiline);
    })
}

/// `own` taking the presses of the input `id` with the text cursor: a click gives its
/// field the keyboard, as a press on a Compose text field's decoration does, and a
/// secondary press gives it the keyboard and opens its menu, told whether the field is
/// secure and holds several lines, as `(secure, multiline)` says.
fn editing(own: Modifier, id: Target, (secure, multiline): (bool, bool)) -> Modifier {
    let operations = crate::events::operations();
    let focus = crate::focus::focus_of(id);
    let held = focus.clone();
    let pressed = send(Event::pressing(id));
    own.cursor(CursorIcon::Text).pressable_menu(
        id,
        move || {
            pressed();
            held.request_focus();
        },
        move |at| {
            focus.request_focus();
            operations.open_text_field_menu(id, at, secure, multiline);
        },
    )
}

#[cfg(test)]
mod tests;
