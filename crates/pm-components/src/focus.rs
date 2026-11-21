//! Keyboard focus as the views see it: rustle's [`Focus`](rustle::Focus), one for each
//! control, kept in a registry the app holds.
//!
//! Rustle owns focus: Tab walks the focus targets in the order they were painted, a press
//! on a text field gives it focus, Escape and a press elsewhere take it away. The app
//! holds a [`FocusRequesters`] with one rustle focus for each [`Target`], the way a
//! Compose screen holds its `FocusRequester`s, and provides it to the views with
//! [`provide`]. A view attaches the focus of its control to its focus target, focusable
//! or text field; the app asks which control holds focus by reading rustle's state of
//! those focuses, asks for focus with [`FocusRequesters::request_focus`] and lets it go
//! with [`FocusRequesters::free_focus`].

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use rustle::{
    CompositionLocal, Focus, Modifier, Offset, PlatformTextInputMethodRequest, Preedit, Rect,
};

use crate::target::Target;

/// The rustle focus of every control, by its [`Target`], made the first time it is asked
/// for and kept for as long as the registry lives, so a control has the same focus
/// wherever and whenever it is composed, and focus can be asked for before it is.
#[derive(Clone, Default)]
pub struct FocusRequesters {
    /// The focus of each control asked for so far.
    entries: Rc<RefCell<BTreeMap<Target, Focus>>>,
}

impl PartialEq for FocusRequesters {
    /// Whether both are the same registry.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.entries, &other.entries)
    }
}

impl FocusRequesters {
    /// A registry holding no focus yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The rustle focus of the control `id`, made now if it was never asked for.
    pub fn focus_of(&self, id: Target) -> Focus {
        self.entries.borrow_mut().entry(id).or_default().clone()
    }

    /// Whether rustle's focus is on the control `id`. Read while composing, it composes
    /// the reader again when that changes.
    pub fn is_focused(&self, id: Target) -> bool {
        self.focus_of(id).is_focused()
    }

    /// The control rustle's focus is on, the first in target order when windows beside
    /// the main one each hold one.
    pub fn focused(&self) -> Option<Target> {
        self.focused_where(|_| true)
    }

    /// The first control, in target order, that rustle's focus is on and `keep` accepts.
    pub fn focused_where(&self, keep: impl Fn(Target) -> bool) -> Option<Target> {
        let entries: Vec<(Target, Focus)> = self
            .entries
            .borrow()
            .iter()
            .map(|(id, focus)| (*id, focus.clone()))
            .collect();
        entries
            .into_iter()
            .find(|(id, focus)| keep(*id) && focus.is_focused())
            .map(|(id, _)| id)
    }

    /// Asks rustle to give focus to the control `id`, in the window that painted it.
    pub fn request_focus(&self, id: Target) {
        self.focus_of(id).request_focus();
    }

    /// Asks rustle to take focus from the control holding it, if any, so nothing holds it.
    pub fn free_focus(&self) {
        if let Some(id) = self.focused() {
            self.free_focus_of(id);
        }
    }

    /// Asks rustle to take focus from the control `id`, if it holds it then.
    pub fn free_focus_of(&self, id: Target) {
        self.focus_of(id).free_focus();
    }
}

thread_local! {
    /// The registry views composed with none provided share.
    static RESTING: FocusRequesters = FocusRequesters::new();
}

/// The focus registry the views composed below see.
static LOCAL_FOCUS_REQUESTERS: CompositionLocal<FocusRequesters> =
    CompositionLocal::new(|| RESTING.with(FocusRequesters::clone));

/// Composes `content` with the focus registry `requesters`.
pub fn provide<R>(requesters: FocusRequesters, content: impl FnOnce() -> R) -> R {
    LOCAL_FOCUS_REQUESTERS.provide(requesters, content)
}

/// The focus registry the view being composed sees.
pub fn requesters() -> FocusRequesters {
    LOCAL_FOCUS_REQUESTERS.current()
}

/// The rustle focus of the control `id`, from the registry the view being composed sees.
pub fn focus_of(id: Target) -> Focus {
    requesters().focus_of(id)
}

/// Whether rustle's focus is on the control `id`, read so the view being composed is
/// composed again when that changes.
pub fn is_focused(id: Target) -> bool {
    focus_of(id).is_focused()
}

/// The modifier that makes the control `id` a focus target holding its registry focus:
/// Tab reaches it in paint order, and a press does not give it focus, as in Compose.
pub fn focus_target(id: Target) -> Modifier {
    Modifier::new().focus_target(&focus_of(id))
}

/// The modifier that lets the area `id` hold rustle's keyboard focus, for an area that
/// takes text the app handles itself, such as the code editor, a shell or a web page: the
/// window keeps its input method on while the area has the keyboard, what the input method
/// composes and commits there goes to the app through the area's text input session, and
/// the app's own key handler sees the keys first.
#[track_caller]
pub fn keyboard_focus(id: Target) -> Modifier {
    let focus = focus_of(id);
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
    Modifier::new()
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
