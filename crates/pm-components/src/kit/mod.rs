//! The kit: what views are built from on rustle.
//!
//! Views are rustle composables that emit what they draw. They read the
//! model the host hands them: a control is named by a [`FocusId`], and what
//! the user does to it reaches the app's state holder through the
//! [`Handler`](crate::events::Handler) the views are composed under. Each frame the kit reports what was
//! painted where in a [`Report`], which gives the host the tab order and the
//! geometry its logic needs.
//!
//! - [`theme`]: colour roles, tones, radii, the spacing step, the text scale
//!   and font families, provided for a frame with [`theme::provide`].
//! - [`layout`]: the [`Styled`] modifier helpers over the spacing scale, roles
//!   and radii.
//! - [`text`]: [`Type`], lines, paragraphs, measuring and wrapping.
//! - [`icon`]: [`IconName`], drawn from rustle's Phosphor paths, and artwork
//!   that arrives at run time.
//! - [`interact`]: hover, press, focus and group-hover styles resolved when painting.
//! - [`frame`]: what the host tells the views about the frame: focus, clock.
//! - [`report`]: what the frame painted where.
//! - [`scroll`]: boxes scrolled by an offset kept in the model.
//! - [`field`]: the text of an editable field drawn from its model.
//! - [`select`]: text selectable across many boxes.
//! - [`overlay`]: popovers, menus at a point, tooltips.
//! - [`progress`]: the spinner and the turning icon on the frame's clock.
//! - [`focus`], [`keys`], [`pointer`]: what names a control, the keys the
//!   logic matches on, and the pointer as the host follows it.

pub mod field;
pub mod focus;
pub mod frame;
pub mod icon;
pub mod interact;
pub mod keys;
pub mod layout;
pub mod overlay;
pub mod pointer;
pub mod progress;
pub mod report;
pub mod scroll;
pub mod select;
pub mod text;
pub mod theme;

pub use crate::events::Interactive;
pub use field::FieldText;
pub use focus::{Focus, FocusId};
pub use frame::Frame;
pub use icon::{Artwork, IconName, artwork_with, icon, icon_with, path_with};
pub use interact::{Hovered, Looks};
pub use keys::{Key, KeyEvent, Modifiers};
pub use layout::{Side, Styled};
pub use overlay::{Placement, Tooltip, anchored, at_window, tooltips};
pub use pointer::{Button, PointerEvent, PointerState};
pub use progress::{spinner, spinning_icon};
pub use report::{FieldLine, Report, Step, root};
pub use scroll::{SCROLLBAR_GRIP, Scroll, scroll_area};
pub use select::{TextPoint, TextSelection, selectable, selectable_scope};
pub use text::{
    Line, Type, code_ligatures, font_families, font_key, label, label_with, measure, paragraph,
    set_code_ligatures, wrap_lines,
};
pub use theme::{
    Appearance, Fonts, Radius, Role, Roles, Srgba, Theme, ThemeFamily, Tone, color, role, sp,
};
