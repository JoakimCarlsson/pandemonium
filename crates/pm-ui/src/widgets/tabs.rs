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
use crate::style::Styled;
use crate::text::text;
use crate::theme::Theme;
use crate::widgets::icon_button;
use crate::widgets::rule;

/// Height of a bar of tabs, hairline included.
pub const TAB_BAR_HEIGHT: f32 = 32.0;

/// Longest name a tab shows before it is cut short.
const NAME_CHARS: usize = 20;

/// Diameter of the dot marking that what a tab holds is unsaved.
const DOT_SIZE: f32 = 7.0;

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
pub fn tab_bar<M: Clone + 'static>(theme: &Theme, tabs: Vec<Tab<M>>, actions: Div<M>) -> Div<M> {
    v_flex()
        .w_full()
        .h_px(TAB_BAR_HEIGHT)
        .child(
            h_flex()
                .w_full()
                .flex_1()
                .items_stretch()
                .overflow_hidden()
                .bg(theme.colors.surface)
                .children(tabs.into_iter().map(|tab| one_tab(theme, tab)))
                .child(h_flex().flex_1())
                .child(actions),
        )
        .child(rule(theme))
}

/// Builds one tab, measured when the caller asked to be told where it lands.
fn one_tab<M: Clone + 'static>(theme: &Theme, tab: Tab<M>) -> Box<dyn crate::Element<M>> {
    match tab.bounds.clone() {
        Some(bounds) => Box::new(measured(bounds, pane_tab(theme, tab))),
        None => Box::new(pane_tab(theme, tab)),
    }
}

/// Builds one tab: what it holds, and the control that closes it.
fn pane_tab<M: Clone + 'static>(theme: &Theme, tab: Tab<M>) -> Div<M> {
    let (background, color) = if tab.active {
        (theme.colors.background, theme.colors.text)
    } else {
        (theme.colors.surface, theme.colors.text_muted)
    };
    let name = text(truncated(&tab.name, NAME_CHARS))
        .text_sm()
        .font_light()
        .color(color);
    let name = if tab.preview { name.italic() } else { name };

    h_flex()
        .h_full()
        .px(1)
        .gap(1)
        .items_center()
        .overflow_hidden()
        .bg(background)
        .when(!tab.active, |tab| tab.hover_bg(theme.colors.surface_hover))
        .when_some(tab.drag, |row, on_drag| {
            row.on_drag(move |event| on_drag(event))
        })
        .on_click(tab.select)
        .on_secondary_click(tab.menu)
        .child(
            icon(tab.icon)
                .size(IconSize::XSmall)
                .color(theme.colors.text_subtle),
        )
        .child(name)
        .when(tab.dirty, |row| row.child(unsaved_dot(color)))
        .child(icon_button(theme, IconName::Close, tab.close))
}

/// Builds the mark a tab carries while what it holds is not on disk.
///
/// The mark is drawn in the tab's own text colour: it says something about
/// the name beside it, and it dims with that name when the tab is not the
/// one in front.
fn unsaved_dot<M>(color: Rgba) -> Div<M> {
    v_flex().size_px(DOT_SIZE).rounded(DOT_SIZE / 2.0).bg(color)
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
