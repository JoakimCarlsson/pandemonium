//! Where a page is in its life: loading, failed to load, crashed or not answering, how far its history goes, and the windows it opened.

/// Why the main frame of a page did not load an address.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadFailure {
    /// The address that did not load.
    pub url: String,
    /// The engine's number for the error, such as -105.
    pub code: i32,
    /// The engine's name for it, such as `ERR_NAME_NOT_RESOLVED`.
    pub name: String,
}

/// What the helper last said of a page's loading, history and health.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lifecycle {
    /// Whether the page is loading.
    pub loading: bool,
    /// How far the load got, from 0 to 1, while it is loading.
    pub progress: f32,
    /// Whether the helper reported the page's history at all, which a helper older than these reports never does.
    pub history_known: bool,
    /// Whether the page's history has a page before the one shown.
    pub back: bool,
    /// Whether the page's history has a page after the one shown.
    pub forward: bool,
    /// Why the last load of the main frame failed, until the next load starts.
    pub failure: Option<LoadFailure>,
    /// Why the page's content process went away, until the next load starts.
    pub crashed: Option<String>,
    /// Whether the page stopped answering.
    pub unresponsive: bool,
    /// Whether the page closed itself, as a window a script opened does with `window.close()`.
    pub closed: bool,
}

/// A window a page opened with `window.open` that the helper holds until it is made a page with [`crate::View::adopt_popup`] or dropped with [`crate::View::drop_popup`].
#[derive(Clone, Debug, PartialEq)]
pub struct Popup {
    /// The helper's number for the window.
    pub popup: u64,
    /// Where it goes.
    pub url: String,
    /// Whether a tool of an agent was running on the page that opened it.
    pub agent: bool,
}
