//! What the settings window asks the shell to do when the person acts.

use std::rc::Rc;

use pm_shell::SettingsChange;

/// The callbacks the settings window calls; the shell decides what each does.
#[derive(Clone)]
pub struct SettingsActions {
    /// Changes a setting, or what the window shows.
    pub change: Rc<dyn Fn(SettingsChange)>,
    /// Closes the settings window.
    pub close: Rc<dyn Fn()>,
    /// Starts moving the settings window with the pointer.
    pub drag: Rc<dyn Fn()>,
    /// Opens a web address in the system's browser.
    pub open_url: Rc<dyn Fn(String)>,
}

impl SettingsActions {
    /// Actions that do nothing, for snapshots.
    pub fn inert() -> Self {
        Self {
            change: Rc::new(|_| {}),
            close: Rc::new(|| {}),
            drag: Rc::new(|| {}),
            open_url: Rc::new(|_| {}),
        }
    }

    /// A callback that asks for the change `make` builds from what it is given.
    pub fn with<T: 'static, F: Fn(T) -> SettingsChange + 'static>(
        &self,
        make: F,
    ) -> impl Fn(T) + 'static + use<T, F> {
        let change = self.change.clone();
        move |value| change(make(value))
    }

    /// A callback that asks for `change`.
    pub fn send(&self, change: SettingsChange) -> impl Fn() + 'static + use<> {
        let send = self.change.clone();
        move || send(change.clone())
    }
}
