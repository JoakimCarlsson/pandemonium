//! Keyboard focus as the views see it: rustle's [`Focus`](rustle::Focus), driven by the
//! control the app's state holder keeps focus on.
//!
//! Rustle owns focus: Tab walks the focus targets in the order they were painted, a press
//! on a text field gives it focus, Escape and a press elsewhere take it away. The app keeps
//! which control holds it as a [`Target`], the domain key its reducers ask about (which
//! field Enter submits, which row the keys act on). The two follow each other the way a
//! Compose screen hoists focus: every focusable control remembers a rustle focus, asks for
//! it while the app keeps focus on the control and frees it otherwise (a `FocusRequester`
//! driven by state from an effect), and tells the app through
//! [`on_focus_changed`](rustle::Modifier::on_focus_changed) when rustle moved it. The app's
//! side is provided to the views with [`provide`] as [`FocusKeys`].

use std::cell::Cell;
use std::rc::Rc;

use rustle::{
    CompositionLocal, Focus, Modifier, Offset, PlatformTextInputMethodRequest, Preedit, Rect,
};

use crate::target::Target;

/// The app's focus as the views see it: the control it keeps focus on, and what it is
/// told when rustle's focus comes to a control or leaves it.
#[derive(Clone)]
pub struct FocusKeys {
    /// The control the app keeps focus on.
    pub held: Option<Target>,
    /// Called with a control and whether rustle's focus came to it (true) or left it.
    pub on_moved: Rc<dyn Fn(Target, bool)>,
    /// Asks the app to keep focus on a control that is not composed yet, such as a row of
    /// a long list scrolled into view for it: its focus is requested once it is composed.
    pub request: Rc<dyn Fn(Target)>,
}

impl FocusKeys {
    /// Focus kept on `held`, with nothing told when it moves, for views drawn with no app
    /// behind them, such as snapshots and a component's own tests.
    pub fn resting(held: Option<Target>) -> Self {
        Self {
            held,
            on_moved: Rc::new(|_, _| {}),
            request: Rc::new(|_| {}),
        }
    }
}

impl PartialEq for FocusKeys {
    /// Whether both keep focus on the same control and tell the same holder.
    fn eq(&self, other: &Self) -> bool {
        self.held == other.held
            && Rc::ptr_eq(&self.on_moved, &other.on_moved)
            && Rc::ptr_eq(&self.request, &other.request)
    }
}

/// The app's focus the views composed below see.
static LOCAL_FOCUS_KEYS: CompositionLocal<FocusKeys> =
    CompositionLocal::new(|| FocusKeys::resting(None));

/// Composes `content` with the app's focus `keys`.
pub fn provide<R>(keys: FocusKeys, content: impl FnOnce() -> R) -> R {
    LOCAL_FOCUS_KEYS.provide(keys, content)
}

/// The app's focus as the view being composed sees it.
pub fn keys() -> FocusKeys {
    LOCAL_FOCUS_KEYS.current()
}

/// The control the app keeps focus on, as the view being composed sees it.
pub fn held() -> Option<Target> {
    LOCAL_FOCUS_KEYS.current().held
}

/// Whether the app keeps focus on `id`, as the view being composed sees it.
pub fn is_held(id: Target) -> bool {
    held() == Some(id)
}

/// Remembers the rustle [`Focus`] of the control `id`, keyed by `id` so a control keeps its
/// focus wherever it is composed, asked for while the app keeps focus on `id` and `wanted`
/// holds, and freed otherwise: a `FocusRequester` driven by state from an effect.
#[track_caller]
pub fn follow_focus(id: Target, wanted: bool) -> Focus {
    let focus = rustle::key(id, rustle::remember_focus);
    let wanted = wanted && is_held(id);
    let held = focus.clone();
    rustle::side_effect(move || {
        if wanted && !held.is_focused() {
            held.request_focus();
        } else if !wanted && held.is_focused() {
            held.free_focus();
        }
    });
    focus
}

/// The modifier that tells the app when rustle's focus comes to the control `id`, or
/// leaves it. Put it before (outside) the focusable it observes.
pub fn reports_focus(id: Target) -> Modifier {
    let on_moved = LOCAL_FOCUS_KEYS.current().on_moved;
    Modifier::new().on_focus_changed(move |focused| on_moved(id, focused))
}

/// The modifier that makes the control `id` a focus target: Tab reaches it in paint order,
/// it holds rustle's focus while the app keeps focus on it, and tells the app when rustle
/// moves focus to it or away. A press does not give it focus, as in Compose.
#[track_caller]
pub fn focus_target(id: Target) -> Modifier {
    let focus = follow_focus(id, true);
    reports_focus(id).focus_target(&focus)
}

/// The modifier that lets the area `id` hold rustle's keyboard focus while the app keeps
/// focus on it, for an area that takes text the app handles itself, such as the code
/// editor, a shell or a web page: the window keeps its input method on while the area has
/// the keyboard, what the input method composes and commits there goes to the app through
/// the area's text input session, and the app's own key handler sees the keys first.
#[track_caller]
pub fn keyboard_focus(id: Target) -> Modifier {
    let focus = follow_focus(id, true);
    let session = rustle::key(id, rustle::remember_platform_text_input_session);
    let origin = rustle::key(id, || {
        rustle::remember(|| Rc::new(Cell::new(Offset::default())))
    })
    .get();
    let request = AppTextInput {
        id,
        operations: crate::events::operations(),
        origin: Rc::clone(&origin),
    };
    let started = session.clone();
    rustle::side_effect(move || started.start_input_method(request));
    reports_focus(id)
        .focusable(&focus, |_| false)
        .platform_text_input(&session)
        .on_globally_positioned(move |area| origin.set(area.origin))
}

/// The input method's side of an area that takes text the app handles itself: what it
/// composes and commits there goes to the app, which also says where its candidates go.
struct AppTextInput {
    /// The area.
    id: Target,
    /// The app's state holder, as the area's view saw it.
    operations: Rc<dyn crate::events::Operations>,
    /// Where the area was last laid out in the window, its top left corner.
    origin: Rc<Cell<Offset>>,
}

impl PlatformTextInputMethodRequest for AppTextInput {
    /// Hands what the input method is composing to the app, or that it stopped.
    fn on_preedit(&self, preedit: Option<&Preedit>) {
        let text = preedit
            .map(|preedit| preedit.text.clone())
            .unwrap_or_default();
        self.operations.composing(self.id, text);
    }

    /// Hands what the input method committed to the app.
    fn on_commit(&self, text: &str) {
        self.operations.committed(self.id, text.to_owned());
    }

    /// Where the app puts the input method's candidates, in the area's own coordinates.
    fn focused_rect(&self) -> Option<Rect> {
        let area = self.operations.caret_area(self.id)?;
        let origin = self.origin.get();
        Some(Rect::new(
            Offset::new(area.origin.x - origin.x, area.origin.y - origin.y),
            area.size,
        ))
    }
}
