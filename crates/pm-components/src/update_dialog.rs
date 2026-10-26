//! The window that offers a newer release: its notes, and the button that downloads it and restarts into it.

use pm_input::FocusId;
use pm_style::{Role, flex, flex_col, gap, h, items_center, overflow_hidden, p, pb, px};
use pm_text::Shaper;
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::git_prompts::{PROMPT_CANCEL, PROMPT_INSIDE, PROMPT_OUTSIDE};
use crate::parts::{line, spacer, wrap};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::update::{RUNNING, Update};
use crate::{Button, Markdown, Modal};

/// Downloads the release and restarts into it.
pub const UPDATE_NOW: FocusId = FocusId(7_019);

/// Opens the page of the release.
pub const UPDATE_PAGE: FocusId = FocusId(7_025);

/// The box the notes of the release scroll in.
pub const UPDATE_SCROLL: FocusId = FocusId(7_026);

/// How wide the window is, in logical pixels.
const WIDTH: f32 = 520.0;

/// How tall the box of notes is, in spacing units.
const NOTES_HEIGHT: u32 = 64;

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
    /// Opens the window that offers the newer release found, when one was.
    pub(crate) fn open_release(&mut self) {
        if matches!(self.settings_page.release, Some(Update::Newer { .. })) {
            self.update_scroll = 0.0;
            self.ask(crate::git_prompts::Prompt::Update);
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

    /// Carries out `action` if it presses the window of an update, and says whether it did.
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
            PROMPT_INSIDE | UPDATE_SCROLL => {}
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
            PROMPT_CANCEL | PROMPT_OUTSIDE => match self.update_run {
                UpdateRun::Installing => {}
                UpdateRun::Downloading { .. } => {
                    self.outbox.push(Outgoing::CancelUpdate);
                    self.update_run = UpdateRun::Idle;
                    self.git.prompt = None;
                }
                _ => self.git.prompt = None,
            },
            _ => {}
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

/// The window that offers `update`, the newer release found, with its notes.
pub fn dialog(state: &WorkspaceState, update: &Update, shaper: &mut Shaper) -> Node {
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
    let width = WIDTH - 40.0;
    let date = published
        .as_deref()
        .map(|date| format!(", published {}", date.split('T').next().unwrap_or(date)))
        .unwrap_or_default();
    let mut panel = node()
        .styles([flex(), flex_col(), gap(3), p(5)])
        .child(line(name, 15, 1.4, Role::Text).styles([pm_style::font_medium()]))
        .child(line(
            format!("This is {RUNNING}{date}."),
            13,
            1.4,
            Role::Muted,
        ));
    if let Some(notes) = body {
        panel = panel.child(
            node()
                .styles([h(NOTES_HEIGHT), overflow_hidden()])
                .scroll_from_start(UPDATE_SCROLL, state.update_scroll)
                .child(Markdown::new(notes, width).build(shaper)),
        );
    }
    let said = status(state, blocker.as_deref());
    let busy = matches!(
        state.update_run,
        UpdateRun::Downloading { .. } | UpdateRun::Installing
    );
    let mut buttons = node()
        .styles([flex(), items_center(), gap(2), px(5), pb(5)])
        .child(button("View on GitHub", UPDATE_PAGE, Variant::Ghost))
        .child(spacer());
    if !matches!(state.update_run, UpdateRun::Installing) {
        buttons = buttons.child(button(
            if busy { "Cancel" } else { "Later" },
            PROMPT_CANCEL,
            Variant::Ghost,
        ));
    }
    if !busy && blocker.is_none() {
        buttons = buttons.child(button("Update and Restart", UPDATE_NOW, Variant::Default));
    }
    Modal::new(PROMPT_OUTSIDE, PROMPT_INSIDE)
        .width(WIDTH)
        .child(panel.children(said.map(|said| {
            node().styles([flex(), flex_col()]).children(
                wrap(&said, 13, width)
                    .iter()
                    .map(|words| line(words, 13, 1.5, Role::Muted)),
            )
        })))
        .child(buttons)
        .build()
}
