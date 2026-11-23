//! The tab chip: one tab of a strip inside a pane, with its glyph, its words, marks for what it is and a button that closes it.

use crate::events;
use crate::events::Interactive;
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use crate::theme::{Radius, Role};
use rustle::Modifier;
use rustle::{Alignment, Arrangement};
use rustle::{Icon, phosphor};

use crate::Button;
use crate::parts::{clipped, glyph};
use crate::size::Size;

/// What a tab chip says about whose tab it is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TabVariant {
    /// A tab of the reader's own.
    #[default]
    Page,
    /// A tab an agent opened: marked with sparkles in the accent colour.
    Agent,
    /// A private tab of the reader's own, whose page keeps nothing: marked with a detective in the accent colour.
    Private,
}

/// What opens the menu of a tab chip on a secondary press, told where the pointer is in the window.
#[derive(Clone)]
pub struct ChipMenu(std::rc::Rc<dyn Fn((f32, f32))>);

impl std::fmt::Debug for ChipMenu {
    /// Writes that there is a menu, since what opens it cannot be shown.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ChipMenu")
    }
}

/// A tab chip being described: build it with [`TabChip::new`], change it with the methods that follow, and emit it with [`TabChip::build`].
#[derive(Clone, Debug)]
pub struct TabChip {
    /// The words on the tab.
    label: String,
    /// The target that selects the tab.
    select: Target,
    /// The target of the button that closes it.
    close: Target,
    /// The glyph before the words.
    icon: Icon,
    /// Whether it is the tab being shown.
    selected: bool,
    /// Whose tab it is.
    variant: TabVariant,
    /// Whether a padlock marks it as kept from agents.
    locked: bool,
    /// How wide it is, in logical pixels.
    width: f32,
    /// What it says when the pointer rests on it, if anything.
    hint: Option<String>,
    /// Whether its page plays sound, as a speaker button: whether the sound is muted, and the target that mutes or unmutes it.
    audio: Option<(bool, Target)>,
    /// Whether it is pinned: compact, its glyph alone, without words or a close button.
    pinned: bool,
    /// What opens its menu on a secondary press, if it has one.
    menu: Option<ChipMenu>,
}

impl TabChip {
    /// A tab saying `label`, selected by `select` and closed by `close`.
    pub fn new(label: impl Into<String>, select: Target, close: Target) -> Self {
        Self {
            label: label.into(),
            select,
            close,
            icon: phosphor::GLOBE,
            selected: false,
            variant: TabVariant::Page,
            locked: false,
            width: 160.0,
            hint: None,
            audio: None,
            pinned: false,
            menu: None,
        }
    }

    /// Whose tab it is.
    pub fn variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Whether it is the tab being shown.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Whether a padlock marks it as kept from agents.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// How wide it is, in logical pixels.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// What it says when the pointer rests on it.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Shows a speaker button for a page that plays sound or is muted: struck through while `muted`, and pressing `toggle` mutes or unmutes it.
    pub fn audio(mut self, audio: Option<(bool, Target)>) -> Self {
        self.audio = audio;
        self
    }

    /// Whether it is pinned: compact, showing its glyph alone, without words or a close button.
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// Opens a menu with `open`, told where the pointer is in the window, when the secondary button goes down on the tab.
    pub fn menu(mut self, open: impl Fn((f32, f32)) + 'static) -> Self {
        self.menu = Some(ChipMenu(std::rc::Rc::new(open)));
        self
    }

    /// Emits the tab.
    #[track_caller]
    pub fn build(self) {
        let tone = if self.selected {
            Role::Text
        } else {
            Role::Muted
        };
        let looks = theme::selectable(self.selected).radius(Radius::Md);
        let sized = Modifier::new().width(self.width).height(28.0).indicated(
            looks,
            theme::Focused::Control(crate::focus::is_focused(self.select)),
        );
        let own = match self.menu {
            Some(ChipMenu(open)) => {
                let press = events::send(events::Event::pressing(self.select));
                sized.cursor(rustle::CursorIcon::Pointer).control_menu(
                    self.select,
                    move |_| press(),
                    move |at| open(at),
                )
            }
            None => events::pressed(sized, self.select),
        }
        .padding_xy(theme::sp(2), 0.0);
        let (variant, icon, label, locked, close, audio, pinned) = (
            self.variant,
            self.icon,
            self.label,
            self.locked,
            self.close,
            self.audio,
            self.pinned,
        );
        let hint = self.hint;
        let row = move || {
            rustle::row_arranged(
                own,
                if pinned {
                    Arrangement::Center
                } else {
                    Arrangement::SpacedBy(theme::sp(1))
                },
                Alignment::Center,
                move || {
                    match variant {
                        TabVariant::Agent => glyph(phosphor::SPARKLE, 3.5, Role::Accent),
                        TabVariant::Private => glyph(phosphor::DETECTIVE, 3.5, Role::Accent),
                        TabVariant::Page => glyph(icon, 3.5, tone),
                    }
                    if pinned {
                        return;
                    }
                    clipped(label, 12, tone);
                    if locked {
                        glyph(phosphor::LOCK_SIMPLE, 3.0, Role::Warning);
                    }
                    if let Some((muted, toggle)) = audio {
                        Button::icon(
                            if muted {
                                phosphor::SPEAKER_SLASH
                            } else {
                                phosphor::SPEAKER_HIGH
                            },
                            toggle,
                        )
                        .size(Size::Xs)
                        .tooltip(if muted {
                            "Unmute this tab"
                        } else {
                            "Mute this tab"
                        })
                        .build();
                    }
                    Button::icon(phosphor::X, close).size(Size::Xs).build();
                },
            );
        };
        match hint {
            Some(hint) => crate::tooltip::with_tooltip(hint, row),
            None => row(),
        }
    }
}
