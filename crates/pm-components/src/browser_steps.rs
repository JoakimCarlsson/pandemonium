//! The steps a recording of the agents' browser work kept, shown beside the page so the reader can see what they did: a list of the steps with thumbnails, and above it the screenshot of one step, stepped through one by one to replay the work.

use crate::Button;
use crate::areas::BrowserEvent;
use crate::browser::Browser;
use crate::button::{self, Variant};
use crate::parts::{clipped, glyph, line};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::target::Target;
use crate::theme::{self, Indicated, Radius, Role};
use rustle::phosphor;
use rustle::{Alignment, Arrangement, ImageData, Modifier};

/// The most steps the pane keeps; the oldest are dropped past it.
const MOST_STEPS: usize = 500;

/// How wide the panel of steps is, in logical pixels.
pub const PANEL_WIDTH: f32 = 320.0;

/// How tall the screenshot of the step shown may be, in logical pixels.
const PICTURE_HEIGHT: f32 = 200.0;

/// How wide a thumbnail in the list is, in logical pixels.
const THUMB_WIDTH: f32 = 72.0;

/// How tall a thumbnail in the list is, in logical pixels.
const THUMB_HEIGHT: f32 = 45.0;

/// One step an agent took with the browser while a recording was under way.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentStep {
    /// Its number in the recording, from 1.
    pub index: u32,
    /// The tool that was called, such as `browser_click`.
    pub tool: String,
    /// What the agent gave the tool, as JSON text.
    pub arguments: String,
    /// Whether the tool worked.
    pub ok: bool,
    /// The start of what the tool answered.
    pub text: String,
    /// The address of the page after the tool ran.
    pub url: String,
    /// The key of the agent session whose recording kept the step; empty when the helper named none.
    pub session: String,
    /// What that session is called.
    pub session_name: String,
    /// The screenshot taken after the tool ran, shrunk for the list; nothing when none was taken.
    pub thumbnail: Option<ImageData>,
    /// The screenshot taken after the tool ran, as large as the panel shows it; nothing when none was taken.
    pub picture: Option<ImageData>,
}

impl AgentStep {
    /// The tool's name without the `browser_` every tool starts with, such as `click`.
    pub fn short_tool(&self) -> &str {
        self.tool.strip_prefix("browser_").unwrap_or(&self.tool)
    }
}

/// The steps of the latest recording of each agent session, and how the reader looks at them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Steps {
    /// The steps, in the order they ended, of every session's latest recording.
    pub list: Vec<AgentStep>,
    /// The folder of the latest recording of each session, by the session's key.
    pub folders: std::collections::BTreeMap<String, String>,
    /// The key of the session whose steps the panel lists; nothing to list every session's.
    pub filter: Option<String>,
    /// Whether the panel is open beside the page.
    pub open: bool,
    /// The place in `list` of the step being shown; nothing to follow the newest the panel lists.
    pub shown: Option<usize>,
    /// How far the list is scrolled.
    pub scroll: rustle::ScrollState,
    /// How far the row of sessions to choose from is scrolled.
    pub choices_scroll: rustle::ScrollState,
}

impl Steps {
    /// Adds `step` of the recording in `folder`: a step of another recording of its session than the one held drops that session's older steps and opens the panel; other sessions' steps stay.
    pub fn add(&mut self, folder: &str, step: AgentStep) {
        if self.folders.get(&step.session).map(String::as_str) != Some(folder) {
            let session = step.session.clone();
            let shown = self.shown.and_then(|at| self.list.get(at)).cloned();
            self.list.retain(|held| held.session != session);
            self.shown = shown
                .filter(|held| held.session != session)
                .and_then(|held| self.list.iter().position(|kept| *kept == held));
            self.folders.insert(session, folder.to_owned());
            self.open = true;
        }
        self.list.push(step);
        if self.list.len() > MOST_STEPS {
            let dropped = self.list.len() - MOST_STEPS;
            self.list.drain(..dropped);
            self.shown = self.shown.and_then(|at| at.checked_sub(dropped));
        }
    }

    /// The sessions whose steps are held, key and name, in the order their first step arrived.
    pub fn sessions(&self) -> Vec<(String, String)> {
        let mut found: Vec<(String, String)> = Vec::new();
        for step in &self.list {
            if !found.iter().any(|(key, _)| *key == step.session) {
                found.push((step.session.clone(), step.session_name.clone()));
            }
        }
        found
    }

    /// The places in `list` of the steps the panel lists: those of the session chosen, or every one.
    pub fn visible(&self) -> Vec<usize> {
        self.list
            .iter()
            .enumerate()
            .filter(|(_, step)| {
                self.filter
                    .as_ref()
                    .is_none_or(|session| step.session == *session)
            })
            .map(|(at, _)| at)
            .collect()
    }

    /// Lists only the steps of the session at `place` in [`Steps::sessions`], or every session's with nothing, and follows the newest of them.
    pub fn choose_session(&mut self, place: Option<usize>) {
        self.filter = place.and_then(|at| self.sessions().into_iter().nth(at).map(|(key, _)| key));
        self.shown = None;
    }

    /// The place in `list` of the step the panel shows: the one chosen, when the panel lists it, or the newest it lists.
    pub fn current(&self) -> Option<usize> {
        let visible = self.visible();
        match self.shown {
            Some(at) if visible.contains(&at) => Some(at),
            _ => visible.last().copied(),
        }
    }

    /// Shows the step `by` places after the one shown among those the panel lists, or before it when `by` is negative, staying within them.
    pub fn step_by(&mut self, by: isize) {
        let Some(at) = self.current() else {
            return;
        };
        let visible = self.visible();
        let Some(place) = visible.iter().position(|held| *held == at) else {
            return;
        };
        let last = visible.len() - 1;
        self.shown = Some(visible[place.saturating_add_signed(by).min(last)]);
    }
}

impl WorkspaceState {
    /// Adds `step` of the recording in `folder` to the steps the browser shows; nothing when the browser is closed.
    pub fn browser_step_recorded(&mut self, folder: &str, step: AgentStep) {
        if let Some(browser) = self.browser.as_mut() {
            browser.steps.add(folder, step);
        }
    }

    /// Carries out what pressing `id` of the panel of steps, or the button opening it, does; says whether `id` was one of them.
    pub(crate) fn browser_steps_pressed(&mut self, id: Target) -> bool {
        let Target::Browser(event) = id else {
            return false;
        };
        let Some(steps) = self.browser.as_mut().map(|browser| &mut browser.steps) else {
            return false;
        };
        match event {
            BrowserEvent::BrowserSteps => steps.open = !steps.open,
            BrowserEvent::BrowserStep(at) => steps.shown = Some(at),
            BrowserEvent::BrowserStepPrevious => steps.step_by(-1),
            BrowserEvent::BrowserStepNext => steps.step_by(1),
            BrowserEvent::BrowserStepLatest => steps.shown = None,
            BrowserEvent::BrowserStepSession(place) => steps.choose_session(Some(place)),
            BrowserEvent::BrowserStepAllSessions => steps.choose_session(None),
            _ => return false,
        }
        true
    }
}

/// Emits the button of the toolbar that opens and closes the panel of steps, while a recording kept any.
pub(crate) fn steps_button(browser: &Browser) {
    if browser.steps.list.is_empty() {
        return;
    }
    Button::icon(
        phosphor::FILM_STRIP,
        Target::Browser(BrowserEvent::BrowserSteps),
    )
    .variant(if browser.steps.open {
        Variant::Secondary
    } else {
        Variant::Ghost
    })
    .tooltip("What the agents did: the steps of the latest browser recording")
    .build();
}

/// Emits `picture` scaled to fit within `width` by `height`, keeping its shape.
fn fitted(picture: &ImageData, width: f32, height: f32) {
    let scale = (width / picture.width().max(1) as f32)
        .min(height / picture.height().max(1) as f32)
        .min(1.0);
    rustle::image(
        Modifier::new()
            .width((picture.width() as f32 * scale).floor().max(1.0))
            .height((picture.height() as f32 * scale).floor().max(1.0))
            .clip(),
        picture,
        rustle::ContentScale::Crop,
    );
}

/// Emits text of `pixels` size in `tone`, at most `most` lines, ending in an ellipsis when cut.
fn lines(content: &str, pixels: u32, tone: Role, most: usize) {
    rustle::basic_text(
        Modifier::new().fill_max_width(),
        content,
        theme::line(pixels, tone).max_lines(most),
        rustle::TextOverflow::Ellipsis,
    );
}

/// Emits the mark of whether a step worked.
fn outcome_mark(ok: bool) {
    if ok {
        glyph(phosphor::CHECK_CIRCLE, 3.5, Role::Success);
    } else {
        glyph(phosphor::X_CIRCLE, 3.5, Role::Danger);
    }
}

/// Emits the step being shown: its screenshot, the buttons that step through the steps, and what it did.
#[track_caller]
fn replay(steps: &Steps, at: usize) {
    let step = &steps.list[at];
    let visible = steps.visible();
    let place = visible.iter().position(|held| *held == at).unwrap_or(0);
    let room = PANEL_WIDTH - 2.0 * theme::sp(2);
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .height(PICTURE_HEIGHT)
            .background(theme::color(Role::Background)),
        Arrangement::Center,
        Alignment::Center,
        || match step.picture.as_ref() {
            Some(picture) => fitted(picture, room, PICTURE_HEIGHT),
            None => line("No screenshot of this step", 12, 1.4, Role::Muted),
        },
    );
    rustle::row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            Button::icon(
                phosphor::CARET_LEFT,
                Target::Browser(BrowserEvent::BrowserStepPrevious),
            )
            .size(Size::Xs)
            .disabled(place == 0)
            .build();
            rustle::row_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Center,
                Alignment::Center,
                || {
                    clipped(
                        format!("Step {} of {}", place + 1, visible.len()),
                        12,
                        Role::Text,
                    )
                },
            );
            Button::icon(
                phosphor::CARET_RIGHT,
                Target::Browser(BrowserEvent::BrowserStepNext),
            )
            .size(Size::Xs)
            .disabled(place + 1 >= visible.len())
            .build();
            if steps.shown.is_some() {
                button::small_button(
                    "Latest",
                    Target::Browser(BrowserEvent::BrowserStepLatest),
                    Variant::Ghost,
                )
                .build();
            }
        },
    );
    rustle::row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            outcome_mark(step.ok);
            clipped(step.tool.clone(), 12, Role::Text);
        },
    );
    if !step.session_name.is_empty() {
        lines(&step.session_name, 11, Role::Muted, 1);
    }
    lines(&step.arguments, 11, Role::Muted, 3);
    lines(&step.text, 11, Role::Text, 3);
    if !step.url.is_empty() {
        lines(&step.url, 11, Role::Muted, 1);
    }
}

/// Emits one step of the list: its thumbnail, number, tool and arguments; pressing it shows the step.
#[track_caller]
fn step_row(step: &AgentStep, at: usize, selected: bool) {
    let id = Target::Browser(BrowserEvent::BrowserStep(at));
    rustle::row_arranged(
        crate::events::pressed(
            Modifier::new().fill_max_width().indicated(
                theme::selectable(selected).radius(Radius::Sm),
                theme::Focused::Control(crate::focus::is_focused(id)),
            ),
            id,
        )
        .padding(theme::sp(1)),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            rustle::column_arranged(
                Modifier::new()
                    .width(THUMB_WIDTH)
                    .height(THUMB_HEIGHT)
                    .background(theme::color(Role::Background)),
                Arrangement::Center,
                Alignment::Center,
                || {
                    if let Some(thumbnail) = step.thumbnail.as_ref() {
                        fitted(thumbnail, THUMB_WIDTH, THUMB_HEIGHT);
                    }
                },
            );
            rustle::column_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    rustle::row_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(theme::sp(1)),
                        Alignment::Center,
                        || {
                            outcome_mark(step.ok);
                            clipped(
                                format!("{}. {}", step.index, step.short_tool()),
                                12,
                                Role::Text,
                            );
                        },
                    );
                    lines(&step.arguments, 11, Role::Muted, 2);
                },
            );
        },
    );
}

/// Emits the row that chooses whose steps the panel lists, every session's or one session's, while steps of more than one agent session are held; nothing otherwise.
#[track_caller]
fn session_choices(steps: &Steps) {
    let sessions = steps.sessions();
    if sessions.len() < 2 {
        return;
    }
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .horizontal_scroll(&steps.choices_scroll),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            let variant = |chosen: bool| {
                if chosen {
                    Variant::Secondary
                } else {
                    Variant::Ghost
                }
            };
            button::small_button(
                "All sessions",
                Target::Browser(BrowserEvent::BrowserStepAllSessions),
                variant(steps.filter.is_none()),
            )
            .build();
            for (place, (key, name)) in sessions.iter().enumerate() {
                let label = if name.is_empty() {
                    key.clone()
                } else {
                    name.clone()
                };
                rustle::key(key.as_str(), || {
                    button::small_button(
                        label,
                        Target::Browser(BrowserEvent::BrowserStepSession(place)),
                        variant(steps.filter.as_ref() == Some(key)),
                    )
                    .build();
                });
            }
        },
    );
}

/// Emits the panel of steps beside the page, as tall as its row and [`PANEL_WIDTH`] wide: a heading, the step shown and the list of every step; nothing while it is closed or empty.
#[track_caller]
pub(crate) fn steps_panel(steps: &Steps) {
    let Some(at) = steps.current() else {
        return;
    };
    if !steps.open {
        return;
    }
    rustle::column_arranged(
        Modifier::new()
            .width(1.0)
            .fill_max_height()
            .background(theme::color(Role::Border)),
        Arrangement::Start,
        Alignment::Start,
        || {},
    );
    rustle::column_arranged(
        Modifier::new()
            .width(PANEL_WIDTH)
            .fill_max_height()
            .background(theme::color(Role::Surface))
            .padding(theme::sp(2)),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(1)),
                Alignment::Center,
                || {
                    rustle::row_arranged(
                        Modifier::new().weight(1.0),
                        Arrangement::Start,
                        Alignment::Center,
                        || clipped("Agent steps", 13, Role::Text),
                    );
                    Button::icon(phosphor::X, Target::Browser(BrowserEvent::BrowserSteps))
                        .size(Size::Xs)
                        .build();
                },
            );
            session_choices(steps);
            replay(steps, at);
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(1.0)
                    .background(theme::color(Role::Border)),
                Arrangement::Start,
                Alignment::Start,
                || {},
            );
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .vertical_scroll(&steps.scroll),
                Arrangement::Start,
                Alignment::Start,
                || {
                    for place in steps.visible() {
                        let step = &steps.list[place];
                        rustle::key((step.session.as_str(), step.index), || {
                            step_row(step, place, place == at);
                        });
                    }
                },
            );
        },
    );
}

#[cfg(test)]
mod tests;
