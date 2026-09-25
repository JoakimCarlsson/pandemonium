//! The bar of tabs above a pane, and one tab in it.
//!
//! Every pane in the window wears the same bar: whatever is open in it, one
//! tab each, the one in front lit, and the pane's own controls at the end of
//! the row. The bar knows nothing of what the tabs hold — a file, a shell, a
//! diff — only what each one is called and what clicking it sends.

use std::sync::Arc;

use pm_gfx::Rgba;

use crate::div::{Div, h_flex, v_flex};
use crate::icons::{IconName, IconSize, icon};
use crate::measured::{Bounds, measured};
use crate::resize::ResizeEvent;
use crate::style::{Side, Styled};
use crate::text::text;
use crate::theme::Theme;
use crate::widgets::{icon_button, tinted_icon_button};

/// Longest name a tab shows before it is cut short.
const NAME_CHARS: usize = 20;

/// Diameter of the dot marking that what a tab holds is unsaved.
const DOT_SIZE: f32 = 7.0;

/// Thickness of the line across the top of the tab in front of the pane
/// that has the keyboard.
const LIT_EDGE: f32 = 2.0;

/// What carrying one tab across the window sends, event by event.
type OnDrag<M> = Arc<dyn Fn(ResizeEvent) -> M>;

/// One tab in a pane's bar of them, whichever kind of pane it is.
pub struct Tab<M> {
    /// What the tab shows before its name.
    pub icon: IconName,
    /// What the tab calls what is in it.
    pub name: String,
    /// Whether this is the one the pane is showing.
    pub active: bool,
    /// Whether what is in it has changes that are not on disk.
    pub dirty: bool,
    /// Whether it holds something that is only being previewed.
    pub preview: bool,
    /// Whether it is kept in the bar whatever else the bar is showing.
    pub pinned: bool,
    /// What pinning it, or letting it go again, sends.
    pub pin: Option<M>,
    /// What clicking the tab sends.
    pub select: M,
    /// What closing the tab sends.
    pub close: M,
    /// What clicking it with the secondary button sends.
    pub menu: M,
    /// What carrying it sends, when it is one that can be carried at all.
    ///
    /// A tab that can be dragged is not clicked: its own press, travel and
    /// release all arrive through this, and a press that went nowhere is the
    /// caller's to read as the click that selects it.
    pub drag: Option<OnDrag<M>>,
    /// Where to leave the tab's bounds, for a drop to be resolved against.
    pub bounds: Option<Bounds>,
}

/// One tab that is only ever clicked: no dragging, nothing measured.
pub fn tab<M>(icon: IconName, name: impl Into<String>, select: M, close: M, menu: M) -> Tab<M> {
    Tab {
        icon,
        name: name.into(),
        active: false,
        dirty: false,
        preview: false,
        pinned: false,
        pin: None,
        select,
        close,
        menu,
        drag: None,
        bounds: None,
    }
}

impl<M> Tab<M> {
    /// Returns this tab shown as the one its pane is in front of.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Returns this tab marked as holding changes that are not on disk.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    /// Returns this tab marked as holding something only being previewed.
    pub fn preview(mut self, preview: bool) -> Self {
        self.preview = preview;
        self
    }

    /// Returns this tab wearing the pin that `pin` turns on and off.
    ///
    /// A bar whose tabs can be pinned wears the control on every one of
    /// them: the pin is how a tab is kept as well as how it is let go of,
    /// and a tab that only showed one once it was pinned would leave no way
    /// to pin it in the first place.
    pub fn pinned(mut self, pinned: bool, pin: M) -> Self {
        self.pinned = pinned;
        self.pin = Some(pin);
        self
    }

    /// Returns this tab carried by the pointer, reporting through `on_drag`.
    pub fn on_drag(mut self, bounds: Bounds, on_drag: impl Fn(ResizeEvent) -> M + 'static) -> Self {
        self.bounds = Some(bounds);
        self.drag = Some(Arc::new(on_drag));
        self
    }
}

/// Builds a pane's bar: one tab per thing open in it, then its own actions.
///
/// The empty stretch after the last tab is part of the bar rather than a gap
/// in it: a tab let go of there belongs at the end of the row, and the bar
/// is what the window measures to know that.
///
/// The hairline under the bar is drawn by each part of it rather than across
/// it, so the tab in front leaves a gap in it and reads as one piece with
/// what is drawn beneath. The tab in front of a `focused` pane is also lit
/// along its top edge, which is how the window says where keystrokes go.
pub fn tab_bar<M: Clone + 'static>(
    theme: &Theme,
    tabs: Vec<Tab<M>>,
    actions: Div<M>,
    focused: bool,
) -> Div<M> {
    let floor = theme.colors.border_variant;
    h_flex()
        .w_full()
        .h_px(theme.size.tab_bar)
        .items_stretch()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .children(tabs.into_iter().map(|tab| one_tab(theme, tab, focused)))
        .child(h_flex().flex_1().border_side(Side::Bottom, 1.0, floor))
        .child(actions.border_side(Side::Bottom, 1.0, floor))
}

/// Builds one tab, measured when the caller asked to be told where it lands.
fn one_tab<M: Clone + 'static>(
    theme: &Theme,
    tab: Tab<M>,
    focused: bool,
) -> Box<dyn crate::Element<M>> {
    match tab.bounds.clone() {
        Some(bounds) => Box::new(measured(bounds, pane_tab(theme, tab, focused))),
        None => Box::new(pane_tab(theme, tab, focused)),
    }
}

/// Builds one tab: its edges, what it holds, and the control that closes it.
fn pane_tab<M: Clone + 'static>(theme: &Theme, tab: Tab<M>, focused: bool) -> Div<M> {
    let (background, color) = if tab.active {
        (theme.colors.background, theme.colors.text)
    } else {
        (theme.colors.surface, theme.colors.text_muted)
    };
    let name = text(truncated(&tab.name, NAME_CHARS))
        .text_sm()
        .color(color);
    let name = if tab.preview { name.italic() } else { name };

    h_flex()
        .h_full()
        .pl(2)
        .pr(1)
        .gap(1.5)
        .items_center()
        .overflow_hidden()
        .bg(background)
        .border_side(Side::Right, 1.0, theme.colors.border_variant)
        .when(tab.active && focused, |tab| {
            tab.border_side(Side::Top, LIT_EDGE, theme.colors.border_focused)
        })
        .when(!tab.active, |tab| {
            tab.hover_bg(theme.colors.surface_hover).border_side(
                Side::Bottom,
                1.0,
                theme.colors.border_variant,
            )
        })
        .when_some(tab.drag, |row, on_drag| {
            row.on_drag(move |event| on_drag(event))
        })
        .on_click(tab.select)
        .on_secondary_click(tab.menu)
        .child(
            icon(tab.icon)
                .size(IconSize::Medium)
                .color(theme.colors.text_subtle),
        )
        .child(name)
        .when(tab.dirty, |row| row.child(unsaved_dot(theme, color)))
        .when_some(tab.pin, |row, pin| {
            let (glyph, tint) = if tab.pinned {
                (IconName::PinFilled, color)
            } else {
                (IconName::Pin, theme.colors.text_subtle)
            };
            row.child(tinted_icon_button(theme, glyph, tint, pin))
        })
        .child(icon_button(theme, IconName::Close, tab.close))
}

/// Builds the mark a tab carries while what it holds is not on disk.
///
/// The mark is drawn in the tab's own text colour: it says something about
/// the name beside it, and it dims with that name when the tab is not the
/// one in front.
fn unsaved_dot<M>(theme: &Theme, color: Rgba) -> Div<M> {
    v_flex()
        .size_px(DOT_SIZE)
        .rounded(theme.radius.full)
        .bg(color)
}

/// `name` cut to `chars` characters, ending in an ellipsis when it was cut.
fn truncated(name: &str, chars: usize) -> String {
    if name.chars().count() <= chars {
        return name.to_owned();
    }
    name.chars()
        .take(chars.saturating_sub(1))
        .collect::<String>()
        + "…"
}
