//! The page that offers a newer release: its notes, and the button that downloads it and restarts into it.

use pm_input::FocusId;
use pm_style::{
    Role, bg, flex, flex_1, flex_col, gap, items_center, min_h, min_w_px, overflow_hidden, px, py,
    w_full,
};
use pm_text::Shaper;
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::parts::{line, spacer, wrap};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::update::{RUNNING, Update};
use crate::{Button, Markdown};

/// Downloads the release and restarts into it.
pub const UPDATE_NOW: FocusId = FocusId(7_019);

/// Opens the page of the release.
pub const UPDATE_PAGE: FocusId = FocusId(7_025);

/// The tab of the release notes in a pane's strip.
pub const TAB_UPDATE: FocusId = FocusId(7_411_000);

/// Closes the tab of the release notes.
pub const UPDATE_CLOSE: FocusId = FocusId(7_411_001);

/// Closes the tab of the release notes, or cancels the download first.
pub const UPDATE_LATER: FocusId = FocusId(7_411_002);

/// The box the notes of the release scroll in.
pub const UPDATE_SCROLL: FocusId = FocusId(7_026);

/// The space around the page's column, in logical pixels, which the notes' width is taken from.
const MARGIN: f32 = 48.0;

/// How far an update has got.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum UpdateRun {
    /// Nothing was started.
    #[default]
    Idle,
    /// The download is under way, with this many bytes of this many received; the total is zero when not known.
    Downloading {
        /// Bytes that have arrived.
        received: u64,
        /// Bytes there are in all.
        total: u64,
    },
    /// The download is done and the new version is being put in place.
    Installing,
    /// It stopped, for this reason.
    Failed(String),
}

impl WorkspaceState {
    /// Opens the tab that offers the newer release found, when one was.
    pub(crate) fn open_release(&mut self) {
        if matches!(self.settings_page.release, Some(Update::Newer { .. })) {
            if !self.update_open {
                self.update_scroll = 0.0;
            }
            self.update_open = true;
            self.tab = crate::state::Tab::Update;
        }
    }

    /// Closes the tab of the release notes, stopping a download under way.
    fn close_release(&mut self) {
        if matches!(self.update_run, UpdateRun::Downloading { .. }) {
            self.outbox.push(Outgoing::CancelUpdate);
        }
        if !matches!(self.update_run, UpdateRun::Installing) {
            self.update_run = UpdateRun::Idle;
            self.update_open = false;
            if self.tab == crate::state::Tab::Update {
                self.tab = crate::state::Tab::Chat;
            }
        }
    }

    /// The state after the notes of the update were scrolled by `delta` logical pixels, with `room` pixels to scroll.
    pub fn scrolled_update(mut self, delta: f32, room: f32) -> Self {
        self.update_scroll = (self.update_scroll + delta).clamp(0.0, room);
        self
    }

    /// The state knowing how far the download of the update has got.
    pub fn with_update_progress(mut self, received: u64, total: u64) -> Self {
        if matches!(self.update_run, UpdateRun::Downloading { .. }) {
            self.update_run = UpdateRun::Downloading { received, total };
        }
        self
    }

    /// The state knowing the update is downloaded and is being put in place.
    pub fn with_update_installing(mut self) -> Self {
        self.update_run = UpdateRun::Installing;
        self
    }

    /// The state knowing the update stopped, for `reason`.
    pub fn with_update_failed(mut self, reason: String) -> Self {
        self.update_run = UpdateRun::Failed(reason);
        self
    }

    /// The state ready to close, the new version having started.
    pub fn with_update_installed(mut self) -> Self {
        self.quitting = true;
        self
    }

    /// Carries out `action` if it presses the tab of an update, and says whether it did.
    pub(crate) fn press_update(&mut self, action: FocusId) -> bool {
        let Some(Update::Newer {
            url,
            version,
            blocker,
            ..
        }) = self.settings_page.release.clone()
        else {
            return false;
        };
        match action {
            TAB_UPDATE => self.tab = crate::state::Tab::Update,
            UPDATE_PAGE => self.outbox.push(Outgoing::Open { address: url }),
            UPDATE_NOW if blocker.is_none() => {
                self.update_run = UpdateRun::Downloading {
                    received: 0,
                    total: 0,
                };
                self.outbox.push(Outgoing::DownloadUpdate {
                    channel: self.window.general.channel,
                    version,
                });
            }
            UPDATE_CLOSE | UPDATE_LATER => self.close_release(),
            _ => return false,
        }
        true
    }
}

/// A button of the window, saying `label`.
fn button(label: &str, id: FocusId, variant: Variant) -> Node {
    Button::new(label, id)
        .variant(variant)
        .size(Size::Sm)
        .build()
}

/// What the window says under the notes about how the update is going, when it says anything.
fn status(state: &WorkspaceState, blocker: Option<&str>) -> Option<String> {
    match &state.update_run {
        UpdateRun::Idle => blocker.map(str::to_owned),
        UpdateRun::Downloading { received, total } if *total > 0 => Some(format!(
            "Downloading… {}%",
            (received * 100 / total).min(100)
        )),
        UpdateRun::Downloading { .. } => Some("Downloading…".to_owned()),
        UpdateRun::Installing => Some("Installing and restarting…".to_owned()),
        UpdateRun::Failed(reason) => Some(format!("The update failed: {reason}")),
    }
}

/// The page, `width` logical pixels across, that offers `update`, the newer release found, with its notes.
pub fn page(state: &WorkspaceState, update: &Update, width: f32, shaper: &mut Shaper) -> Node {
    let Update::Newer {
        name,
        published,
        body,
        blocker,
        ..
    } = update
    else {
        return node();
    };
    let text_width = width - 2.0 * MARGIN;
    let date = published
        .as_deref()
        .map(|date| format!(", published {}", date.split('T').next().unwrap_or(date)))
        .unwrap_or_default();
    let mut column = node()
        .styles([
            flex(),
            flex_col(),
            gap(3),
            flex_1(),
            min_h(0),
            w_full(),
            py(8),
        ])
        .child(line(name, 22, 1.3, Role::Text).styles([pm_style::font_medium()]))
        .child(line(
            format!("This is {RUNNING}{date}."),
            13,
            1.4,
            Role::Muted,
        ));
    if let Some(notes) = body {
        column = column.child(
            node()
                .styles([flex_1(), min_h(0), overflow_hidden()])
                .scroll_from_start(UPDATE_SCROLL, state.update_scroll)
                .child(Markdown::new(notes, text_width).build(shaper)),
        );
    } else {
        column = column.child(node().styles([flex_1()]));
    }
    let said = status(state, blocker.as_deref());
    let busy = matches!(
        state.update_run,
        UpdateRun::Downloading { .. } | UpdateRun::Installing
    );
    let mut buttons = node()
        .styles([flex(), items_center(), gap(2)])
        .child(button("View on GitHub", UPDATE_PAGE, Variant::Ghost))
        .child(spacer());
    if !matches!(state.update_run, UpdateRun::Installing) {
        buttons = buttons.child(button(
            if busy { "Cancel" } else { "Later" },
            UPDATE_LATER,
            Variant::Ghost,
        ));
    }
    if !busy && blocker.is_none() {
        buttons = buttons.child(button("Update and Restart", UPDATE_NOW, Variant::Default));
    }
    let column = column
        .children(said.map(|said| {
            node().styles([flex(), flex_col()]).children(
                wrap(&said, 13, text_width)
                    .iter()
                    .map(|words| line(words, 13, 1.5, Role::Muted)),
            )
        }))
        .child(buttons);
    node()
        .styles([
            flex(),
            flex_1(),
            min_w_px(0),
            min_h(0),
            px(6),
            bg(Role::Background),
        ])
        .child(column)
}
