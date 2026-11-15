//! The composer: a card with a text area, the chips of what is attached, and
//! a toolbar of the agent's settings, usage rings and the send button; with
//! the slash command menu, the setting pickers and the text field's menu.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, TextFieldOptions, TextFieldState, basic_text_field_in, remember_focus};

use crate::button::{Button, ButtonVariant};
use crate::card::{Card, CardVariant};
use crate::chip::Chip;
use crate::composer_menus::{
    CommandMenu, FieldMenu, MentionMenu, SettingMenu, mention_query, paste_attachments,
    with_mention,
};
use crate::icon_button::{IconButton, IconButtonVariant};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::usage_ring::UsageRing;
use rustle::color_scheme;

/// One value a toolbar setting can take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerPick {
    /// What the setting's owner calls the value.
    pub value: String,
    /// What it is called to the reader.
    pub name: String,
    /// What choosing it means, or nothing.
    pub description: String,
}

/// A choice shown in the composer's toolbar, such as the model.
#[derive(Clone, Debug, PartialEq)]
pub struct ComposerChoice {
    /// Its glyph, before the text.
    pub icon: Option<Icon>,
    /// Its text.
    pub label: String,
    /// What its owner calls it, handed back when a value is picked; empty for a choice without a menu.
    pub id: String,
    /// The heading of its menu.
    pub heading: String,
    /// The value it has now.
    pub current: String,
    /// The values its menu offers.
    pub picks: Vec<ComposerPick>,
}

/// A command offered behind a slash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerCommand {
    /// What is typed after the slash.
    pub name: String,
    /// What it does.
    pub description: String,
}

/// A menu of the composer, open when it is first drawn and whenever it, or the number asking for it, changes.
#[derive(Clone, Debug, PartialEq)]
pub enum ComposerMenu {
    /// The menu of the toolbar setting with this id.
    Setting(String),
    /// The text field's menu, at this point inside the text area.
    Field(f32, f32),
}

/// A chip of something attached to the prompt.
#[derive(Clone)]
struct Attached {
    /// Its glyph.
    icon: Icon,
    /// What it says.
    label: String,
    /// What its cross does.
    on_remove: Rc<dyn Fn()>,
}

/// How a composer is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ComposerVariant {
    /// Replying in a running conversation.
    #[default]
    Reply,
    /// Starting a new conversation, taller.
    Start,
}

/// What pasting a picture calls with its width, height and RGBA rows.
pub(crate) type PictureHandler = Rc<dyn Fn(u32, u32, Vec<u8>)>;

/// What the composer's controls call.
#[derive(Clone, Default)]
pub(crate) struct ComposerCallbacks {
    /// What sending does with the text.
    pub(crate) on_send: Option<Rc<dyn Fn(String)>>,
    /// What sending in place of the turn does with the text.
    pub(crate) on_send_now: Option<Rc<dyn Fn(String)>>,
    /// What stopping the turn does.
    pub(crate) on_stop: Option<Rc<dyn Fn()>>,
    /// What picking a value of a setting does, with the setting's id and the value.
    pub(crate) on_pick: Option<Rc<dyn Fn(String, String)>>,
    /// What attaching files does with their paths.
    pub(crate) on_attach: Option<Rc<dyn Fn(Vec<String>)>>,
    /// What pasting a picture does with its width, height and RGBA rows.
    pub(crate) on_picture: Option<PictureHandler>,
    /// What the `+` button does: ask the desktop for files to attach.
    pub(crate) on_browse: Option<Rc<dyn Fn()>>,
    /// What starting to mention a file does, such as listing the files that can be mentioned.
    pub(crate) on_mention: Option<Rc<dyn Fn()>>,
    /// What Shift+Tab does: move the agent to its next mode.
    pub(crate) on_cycle_mode: Option<Rc<dyn Fn()>>,
}

/// How many lines the text area grows to before it scrolls.
const MOST_LINES: usize = 10;

/// How tall one line of the text area is, in logical pixels.
const LINE_HEIGHT: f32 = 20.0;

/// The composer with a variant and a size.
#[derive(Clone)]
pub struct Composer {
    /// The text shown while it is empty.
    placeholder: String,
    /// The choices in the toolbar.
    choices: Vec<ComposerChoice>,
    /// The commands behind a slash.
    commands: Vec<ComposerCommand>,
    /// The usage rings at the end of the toolbar.
    rings: Vec<UsageRing>,
    /// The chips of what is attached.
    attached: Vec<Attached>,
    /// Whether the agent is on a turn, so that sending queues the prompt.
    working: bool,
    /// The menu open when the composer is first drawn.
    menu: Option<ComposerMenu>,
    /// How it is drawn.
    variant: ComposerVariant,
    /// Its size.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What its controls call.
    callbacks: ComposerCallbacks,
    /// Text put into the composer once each time it changes, such as a command to finish.
    prefill: Option<String>,
    /// A number that, each time it changes, puts the keyboard in the composer.
    focus_request: u64,
    /// A number that, each time it changes, opens the menu given with [`Composer::menu`] again.
    menu_request: u64,
    /// The paths of the files an `@` can mention.
    mentions: Rc<Vec<String>>,
}

impl Composer {
    /// A reply composer showing `placeholder` while empty.
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            placeholder: placeholder.into(),
            choices: Vec::new(),
            commands: Vec::new(),
            rings: Vec::new(),
            attached: Vec::new(),
            working: false,
            menu: None,
            variant: ComposerVariant::Reply,
            size: Size::Sm,
            modifier: Modifier::new(),
            callbacks: ComposerCallbacks::default(),
            prefill: None,
            mentions: Rc::new(Vec::new()),
            focus_request: 0,
            menu_request: 0,
        }
    }

    /// The composer opening its menu given with [`Composer::menu`] again whenever `request` differs from the number it was last given, even when the menu asked for is the same.
    pub fn menu_request(mut self, request: u64) -> Self {
        self.menu_request = request;
        self
    }

    /// The composer taking the keyboard whenever `request` differs from the number it was last given.
    pub fn focus_request(mut self, request: u64) -> Self {
        self.focus_request = request;
        self
    }

    /// The composer offering `paths` to mention after an `@`.
    pub fn mentions(mut self, paths: Vec<String>) -> Self {
        self.mentions = Rc::new(paths);
        self
    }

    /// The composer calling `on_browse` when its `+` button is pressed, to pick files to attach.
    pub fn on_browse(mut self, on_browse: impl Fn() + 'static) -> Self {
        self.callbacks.on_browse = Some(Rc::new(on_browse));
        self
    }

    /// The composer calling `on_mention` when an `@` starts a mention, so the files can be listed.
    pub fn on_mention(mut self, on_mention: impl Fn() + 'static) -> Self {
        self.callbacks.on_mention = Some(Rc::new(on_mention));
        self
    }

    /// The composer calling `on_cycle_mode` when Shift+Tab is pressed in it.
    pub fn on_cycle_mode(mut self, on_cycle_mode: impl Fn() + 'static) -> Self {
        self.callbacks.on_cycle_mode = Some(Rc::new(on_cycle_mode));
        self
    }

    /// The composer filled with `prefill` whenever it is given anew.
    pub fn prefill(mut self, prefill: Option<String>) -> Self {
        self.prefill = prefill;
        self
    }

    /// The composer with `choice` added to its toolbar, without a menu.
    pub fn choice(mut self, icon: Option<Icon>, label: impl Into<String>) -> Self {
        self.choices.push(ComposerChoice {
            icon,
            label: label.into(),
            id: String::new(),
            heading: String::new(),
            current: String::new(),
            picks: Vec::new(),
        });
        self
    }

    /// The composer with the setting `choice` added to its toolbar, whose menu offers its picks.
    pub fn setting(mut self, choice: ComposerChoice) -> Self {
        self.choices.push(choice);
        self
    }

    /// The composer offering the command `name` doing `description` behind a slash.
    pub fn command(mut self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.commands.push(ComposerCommand {
            name: name.into(),
            description: description.into(),
        });
        self
    }

    /// The composer with `ring` at the end of its toolbar.
    pub fn ring(mut self, ring: UsageRing) -> Self {
        self.rings.push(ring);
        self
    }

    /// The composer with a chip of something attached, reading `label` after
    /// `icon`, whose cross calls `on_remove`.
    pub fn attached(
        mut self,
        icon: Icon,
        label: impl Into<String>,
        on_remove: impl Fn() + 'static,
    ) -> Self {
        self.attached.push(Attached {
            icon,
            label: label.into(),
            on_remove: Rc::new(on_remove),
        });
        self
    }

    /// The composer queueing what is sent while `working`, offering to send it in place of the turn, or to stop the turn while nothing is written.
    pub fn working(mut self, working: bool) -> Self {
        self.working = working;
        self
    }

    /// The composer with `menu` open when it is first drawn.
    pub fn menu(mut self, menu: Option<ComposerMenu>) -> Self {
        self.menu = menu;
        self
    }

    /// The composer in `variant`.
    pub fn variant(mut self, variant: ComposerVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The composer at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The composer laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The composer calling `on_send` with the text when Enter or the send
    /// button is pressed.
    pub fn on_send(mut self, on_send: impl Fn(String) + 'static) -> Self {
        self.callbacks.on_send = Some(Rc::new(on_send));
        self
    }

    /// The composer calling `on_send_now` with the text when it is sent in place of the turn.
    pub fn on_send_now(mut self, on_send_now: impl Fn(String) + 'static) -> Self {
        self.callbacks.on_send_now = Some(Rc::new(on_send_now));
        self
    }

    /// The composer calling `on_stop` when the turn is stopped.
    pub fn on_stop(mut self, on_stop: impl Fn() + 'static) -> Self {
        self.callbacks.on_stop = Some(Rc::new(on_stop));
        self
    }

    /// The composer calling `on_pick` with a setting's id and the value picked from its menu.
    pub fn on_pick(mut self, on_pick: impl Fn(String, String) + 'static) -> Self {
        self.callbacks.on_pick = Some(Rc::new(on_pick));
        self
    }

    /// The composer calling `on_attach` with the paths of files pasted or dropped on it.
    pub fn on_attach(mut self, on_attach: impl Fn(Vec<String>) + 'static) -> Self {
        self.callbacks.on_attach = Some(Rc::new(on_attach));
        self
    }

    /// The composer calling `on_picture` with the width, height and RGBA rows of a picture pasted into it.
    pub fn on_picture(mut self, on_picture: impl Fn(u32, u32, Vec<u8>) + 'static) -> Self {
        self.callbacks.on_picture = Some(Rc::new(on_picture));
        self
    }

    /// Emits the composer.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let state = remember_text_field_state("");
        let filled = remember(|| None::<String>);
        if self.prefill != filled.get() {
            if let Some(text) = &self.prefill {
                state.set_text(text.clone());
            }
            filled.set(self.prefill.clone());
        }
        let initial = self.menu.clone();
        let menu = remember(move || initial);
        let asked_menu = remember(|| (self.menu.clone(), self.menu_request));
        let asked = (self.menu.clone(), self.menu_request);
        if asked_menu.get() != asked {
            asked_menu.set(asked);
            menu.set(self.menu.clone());
        }
        let focus = remember_focus();
        let asked_focus = remember(|| self.focus_request);
        if asked_focus.get() != self.focus_request {
            asked_focus.set(self.focus_request);
            focus.request_focus();
        }
        let slash_closed = remember(|| None::<String>);
        let focused = remember(|| false);
        let least = match self.variant {
            ComposerVariant::Reply => 22.0,
            ComposerVariant::Start => 54.0,
        };
        let callbacks = self.callbacks.clone();
        let working = self.working;
        let has_attached = !self.attached.is_empty();
        let send: Rc<dyn Fn(bool)> = {
            let state = state.clone();
            let callbacks = callbacks.clone();
            Rc::new(move |now| {
                let text = state.text();
                if text.trim().is_empty() && !has_attached {
                    return;
                }
                let target = if now {
                    callbacks.on_send_now.as_ref()
                } else {
                    callbacks.on_send.as_ref()
                };
                if let Some(target) = target {
                    target(text);
                }
                state.clear();
            })
        };
        let text = state.text();
        let empty = text.is_empty();
        let lines = text.split('\n').count().clamp(1, MOST_LINES);
        let height = (lines as f32 * LINE_HEIGHT).max(least);
        let matching = CommandMenu::matching(&self.commands, &text);
        let slash_open = !matching.is_empty() && slash_closed.get().as_deref() != Some(&text);
        let first_command = matching.first().map(|command| command.name.clone());
        let mention = mention_query(&text);
        let mentioned = remember(|| false);
        if mention.is_some() != mentioned.get() {
            mentioned.set(mention.is_some());
            if let (true, Some(on_mention)) = (mention.is_some(), &callbacks.on_mention) {
                on_mention();
            }
        }
        let mention_matches = mention
            .as_deref()
            .map(|query| MentionMenu::matching(&self.mentions, query))
            .unwrap_or_default();
        let mention_open = !mention_matches.is_empty()
            && !slash_open
            && slash_closed.get().as_deref() != Some(&text);
        let first_mention = mention_matches.first().cloned();
        let key_send = send.clone();
        let key_state = state.clone();
        let key_callbacks = callbacks.clone();
        let modifier = self.modifier.on_preview_key_event(move |event| {
            if !event.pressed {
                return false;
            }
            let paste = (event.modifiers.ctrl || event.modifiers.meta)
                && event.key == Key::Character("v".into());
            if paste {
                return paste_attachments(&key_callbacks);
            }
            if event.key == Key::Tab && event.modifiers.shift {
                if let Some(cycle) = &key_callbacks.on_cycle_mode {
                    cycle();
                }
                return true;
            }
            let enter = event.key == Key::Enter && !event.modifiers.shift;
            if enter {
                match (&first_command, slash_open, &first_mention, mention_open) {
                    (Some(name), true, _, _) => key_state.set_text(format!("/{name} ")),
                    (_, _, Some(path), true) => {
                        key_state.set_text(with_mention(&key_state.text(), path))
                    }
                    _ => key_send(event.modifiers.ctrl && working),
                }
            }
            enter
        });
        let drop_callbacks = callbacks.clone();
        let modifier = match drop_callbacks.on_attach.clone() {
            Some(on_attach) => modifier
                .on_file_drop(move |path| on_attach(vec![path.to_string_lossy().into_owned()])),
            None => modifier,
        };
        let commands_state = state.clone();
        let commands_closed = slash_closed.clone();
        let field_menu = menu.clone();
        let field_state = state.clone();
        let field_callbacks = callbacks.clone();
        let mention_state = state.clone();
        let mention_closed = slash_closed.clone();
        let toolbar_state = state.clone();
        Card::new()
            .variant(CardVariant::Outlined)
            .size(Size::Md)
            .focused(focused.get() || !empty)
            .modifier(modifier)
            .show(move || {
                if slash_open {
                    let typed = text.clone();
                    CommandMenu {
                        commands: matching,
                        on_pick: Rc::new(move |name: String| {
                            commands_state.set_text(format!("/{name} "))
                        }),
                        on_dismiss: Rc::new(move || commands_closed.set(Some(typed.clone()))),
                    }
                    .show();
                }
                if mention_open {
                    let typed = text.clone();
                    MentionMenu {
                        paths: mention_matches,
                        on_pick: Rc::new(move |path: String| {
                            mention_state.set_text(with_mention(&mention_state.text(), &path))
                        }),
                        on_dismiss: Rc::new(move || mention_closed.set(Some(typed.clone()))),
                    }
                    .show();
                }
                if !self.attached.is_empty() {
                    attached_chips(self.attached);
                }
                let opener = field_menu.clone();
                stack_with(
                    Modifier::new()
                        .fill_max_width()
                        .height(height)
                        .on_context_menu(move |event| {
                            opener.set(Some(ComposerMenu::Field(
                                event.position.x,
                                event.position.y,
                            )))
                        }),
                    || {
                        if empty {
                            Label::new(self.placeholder.clone())
                                .variant(LabelVariant::Subtle)
                                .show();
                        }
                        let style = Label::new("").wrap().style();
                        LOCAL_TEXT_STYLE.provide(style, || {
                            basic_text_field_in(
                                Modifier::new().fill_max_size().clip(),
                                Modifier::new(),
                                &state,
                                &focus,
                                TextFieldOptions {
                                    multiline: true,
                                    ..TextFieldOptions::default()
                                },
                            );
                        });
                        if let Some(ComposerMenu::Field(across, down)) = field_menu.get() {
                            let closer = field_menu.clone();
                            FieldMenu {
                                state: field_state.clone(),
                                callbacks: field_callbacks.clone(),
                                at: (across, down),
                                on_dismiss: Rc::new(move || closer.set(None)),
                            }
                            .show();
                        }
                    },
                );
                spacer(Modifier::new().height(8.0));
                toolbar(
                    self.choices,
                    self.rings,
                    &menu,
                    &callbacks,
                    &send,
                    &toolbar_state,
                    empty && !has_attached,
                    working,
                    scheme,
                );
            });
    }
}

/// `text` with `trigger` typed at its end, after a space unless it is empty or already ends in one.
fn with_trigger(text: &str, trigger: char) -> String {
    if text.is_empty() || text.ends_with(char::is_whitespace) {
        format!("{text}{trigger}")
    } else {
        format!("{text} {trigger}")
    }
}

/// Emits the chips of `attached` in rows that wrap.
#[track_caller]
fn attached_chips(attached: Vec<Attached>) {
    flow_row_with(
        Modifier::new()
            .fill_max_width()
            .padding_each(0.0, 0.0, 0.0, 8.0),
        FlowOptions::new(6.0, 6.0),
        move || {
            for (place, chip) in attached.into_iter().enumerate() {
                key(place, move || {
                    let on_remove = chip.on_remove.clone();
                    Chip::new(chip.label)
                        .icon(chip.icon)
                        .size(Size::Xs)
                        .on_remove(move || on_remove())
                        .show();
                });
            }
        },
    );
}

/// Emits the toolbar: attaching, the `choices` with the menu `menu` names
/// open, the `rings`, and the buttons that send with `send` or stop the turn
/// through `callbacks`, drawn for an `empty` prompt and an agent `working` or
/// not in `scheme`.
#[track_caller]
#[allow(clippy::too_many_arguments)]
fn toolbar(
    choices: Vec<ComposerChoice>,
    rings: Vec<UsageRing>,
    menu: &State<Option<ComposerMenu>>,
    callbacks: &ComposerCallbacks,
    send: &Rc<dyn Fn(bool)>,
    state: &TextFieldState,
    empty: bool,
    working: bool,
    scheme: ColorScheme,
) {
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(crate::spacing(2.0)),
        Alignment::Center,
        || {
            let browse = callbacks.on_browse.clone();
            IconButton::new(phosphor::PLUS)
                .size(Size::Xs)
                .tooltip("Attach files")
                .on_click(move || {
                    if let Some(browse) = &browse {
                        browse();
                    }
                })
                .show();
            let at = state.clone();
            IconButton::new(phosphor::AT)
                .size(Size::Xs)
                .tooltip("Mention a file")
                .on_click(move || at.set_text(with_trigger(&at.text(), '@')))
                .show();
            let slash = state.clone();
            stack_aligned(
                Modifier::new()
                    .size(20.0, 20.0)
                    .cursor(CursorIcon::Pointer)
                    .clickable(move || {
                        if slash.text().trim().is_empty() {
                            slash.set_text("/".to_owned());
                        }
                    }),
                BoxAlignment::Center,
                || Label::new("/").variant(LabelVariant::Muted).show(),
            );
            for (place, choice) in choices
                .into_iter()
                .filter(|choice| !choice.label.is_empty())
                .enumerate()
            {
                let menu = menu.clone();
                let on_pick = callbacks.on_pick.clone();
                key(place, move || choice_button(choice, &menu, on_pick));
            }
            spacer(Modifier::new().weight(1.0));
            for (place, ring) in rings.into_iter().enumerate() {
                key(place, move || ring.show());
            }
            send_buttons(callbacks, send, empty, working, scheme);
        },
    );
}

/// Emits the button of `choice`, and its menu while `menu` names it, calling `on_pick` with a value picked.
#[track_caller]
fn choice_button(
    choice: ComposerChoice,
    menu: &State<Option<ComposerMenu>>,
    on_pick: Option<Rc<dyn Fn(String, String)>>,
) {
    column_with(Modifier::new(), || {
        let mut button = Button::new(choice.label.clone()).size(Size::Xs);
        if let Some(glyph) = &choice.icon {
            button = button.icon(glyph.clone());
        }
        let has_menu = !choice.id.is_empty() && !choice.picks.is_empty();
        if has_menu {
            let opener = menu.clone();
            let id = choice.id.clone();
            button = button.on_click(move || {
                let open = opener.get() == Some(ComposerMenu::Setting(id.clone()));
                opener.set((!open).then(|| ComposerMenu::Setting(id.clone())));
            });
        }
        button.variant(ButtonVariant::Ghost).show();
        if has_menu && menu.get() == Some(ComposerMenu::Setting(choice.id.clone())) {
            let closer = menu.clone();
            SettingMenu {
                choice,
                on_pick,
                on_dismiss: Rc::new(move || closer.set(None)),
            }
            .show();
        }
    });
}

/// Emits the buttons at the end of the toolbar: one that sends the prompt,
/// or, while the agent is `working`, one that stops it when the prompt is
/// `empty`, and otherwise one that sends it in place of the turn and one
/// that queues it behind; each calling `send` or `callbacks`.
#[track_caller]
fn send_buttons(
    callbacks: &ComposerCallbacks,
    send: &Rc<dyn Fn(bool)>,
    empty: bool,
    working: bool,
    scheme: ColorScheme,
) {
    if working && empty {
        let stop = callbacks.on_stop.clone();
        IconButton::new(phosphor::STOP)
            .variant(IconButtonVariant::Primary)
            .size(Size::Xs)
            .tooltip("Stop")
            .tint(scheme.background)
            .on_click(move || {
                if let Some(stop) = &stop {
                    stop();
                }
            })
            .show();
        return;
    }
    if working && callbacks.on_send_now.is_some() {
        let now = send.clone();
        IconButton::new(phosphor::LIGHTNING)
            .size(Size::Xs)
            .tooltip("Send now")
            .on_click(move || now(true))
            .show();
    }
    let queue = send.clone();
    IconButton::new(if working {
        phosphor::QUEUE
    } else {
        phosphor::ARROW_UP
    })
    .variant(IconButtonVariant::Primary)
    .size(Size::Xs)
    .tooltip(if working { "Queue message" } else { "Send" })
    .tint(if empty {
        scheme.surface_container_high
    } else {
        scheme.background
    })
    .on_click(move || queue(false))
    .show();
}
