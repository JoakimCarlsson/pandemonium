//! The settings panel: a modal with the sections of settings down its left side and the one chosen on its right.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, border_b, border_color, border_r, flex, flex_1, flex_col, flex_none, gap, h, h_px,
    items_center, justify_between, p, px, w_px,
};
use pm_ui::{Node, node};

use crate::button::{Size, Variant};
use crate::parts::{glyph, line};
use crate::state::WorkspaceState;
use crate::tile::Tile;
use crate::{Button, Modal};

/// Closes the panel, which is what pressing outside it does.
pub const SETTINGS_SCRIM: FocusId = FocusId(7100);

/// The panel itself, which keeps a press on it from counting as outside.
pub const SETTINGS_BODY: FocusId = FocusId(7101);

/// Closes the panel.
pub const SETTINGS_CLOSE: FocusId = FocusId(7102);

/// The first of the sections down the left side; the rest follow it.
pub const SETTINGS_SECTION_BASE: u64 = 7110;

/// How wide the panel is, in logical pixels.
const WIDTH: f32 = 1080.0;

/// How tall the panel is, in logical pixels.
const HEIGHT: f32 = 700.0;

/// How wide the list of sections is, in logical pixels.
const NAVIGATION_WIDTH: f32 = 220.0;

/// A group of settings, named down the left side of the panel.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Section {
    /// How the window looks and behaves.
    #[default]
    General,
}

impl Section {
    /// Every section, in the order the list shows them.
    pub const ALL: [Section; 1] = [Section::General];

    /// What the list and the heading call it.
    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
        }
    }

    /// The glyph before its name in the list.
    fn icon(self) -> IconName {
        match self {
            Self::General => IconName::Settings,
        }
    }

    /// The name of pressing its row in the list.
    fn id(self) -> FocusId {
        let position = Self::ALL.iter().position(|section| *section == self);
        FocusId(SETTINGS_SECTION_BASE + position.unwrap_or_default() as u64)
    }
}

/// Whether `id` names something of the panel.
pub fn is_settings(id: FocusId) -> bool {
    let FocusId(number) = id;
    (SETTINGS_SCRIM.0..SETTINGS_SECTION_BASE + Section::ALL.len() as u64).contains(&number)
}

impl WorkspaceState {
    /// Whether the settings panel is open.
    pub fn settings_open(&self) -> bool {
        self.settings.is_some()
    }

    /// The state with the settings panel open on its first section.
    pub(crate) fn with_settings(mut self) -> Self {
        self.menu = None;
        self.picker = None;
        self.settings = Some(Section::default());
        self
    }

    /// The state after something of the settings panel was pressed: `action` is the name of what.
    pub(crate) fn settings_pressed(mut self, action: FocusId) -> Self {
        match action {
            SETTINGS_SCRIM | SETTINGS_CLOSE => self.settings = None,
            action => {
                if let Some(section) = Section::ALL
                    .into_iter()
                    .find(|section| section.id() == action)
                {
                    self.settings = Some(section);
                }
            }
        }
        self
    }
}

/// The node that draws the settings panel, when it is open.
pub fn settings(state: &WorkspaceState) -> Option<Node> {
    let section = state.settings?;
    Some(
        Modal::new(SETTINGS_SCRIM, SETTINGS_BODY)
            .width(WIDTH)
            .child(
                node()
                    .styles([flex(), h_px(HEIGHT)])
                    .child(navigation(section))
                    .child(content(section)),
            )
            .build(),
    )
}

/// The list of sections, with the one showing marked.
fn navigation(showing: Section) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            gap(1),
            p(3),
            w_px(NAVIGATION_WIDTH),
            border_r(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([flex(), items_center(), h(9), px(2)])
                .child(line("Settings", 13, 1.4, Role::Subtle)),
        )
        .children(Section::ALL.map(|section| entry(section, section == showing)))
}

/// One row of the list of sections.
fn entry(section: Section, showing: bool) -> Node {
    let tone = if showing { Role::Text } else { Role::Muted };
    Tile::new(section.id())
        .selected(showing)
        .build()
        .styles([px(2)])
        .child(glyph(section.icon(), 4.0, tone))
        .child(line(section.title(), 13, 1.4, tone))
}

/// The settings of `section` under its heading and the button that closes the panel.
fn content(section: Section) -> Node {
    node()
        .styles([flex(), flex_col(), flex_1()])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    justify_between(),
                    h(14),
                    px(6),
                    flex_none(),
                    border_b(),
                    border_color(Role::Border),
                ])
                .child(line(section.title(), 15, 1.4, Role::Text).styles([pm_style::font_medium()]))
                .child(
                    Button::icon(IconName::Close, SETTINGS_CLOSE)
                        .variant(Variant::Ghost)
                        .size(Size::Icon)
                        .build(),
                ),
        )
        .child(node().styles([flex_1()]))
}
