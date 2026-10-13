//! The showcase screen: one card per family of utilities.

use pm_input::FocusId;
use pm_style::{
    Color, Ease, Role, bg, col_span, flex, flex_col, font_medium, gap, grid, grid_cols, h_full, p,
    text_color, text_lg, w_full,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node, text};

use crate::swatches::stack;
use crate::{
    avatar_stack, badge, bouncing_dots, button, card, opacity_steps, palette, ping_dot,
    progress_bar, pulse_dot, radius_steps, shadow_steps, spinner, switch, text_input, transforms,
};

/// Presses the counter up by one.
pub const INCREMENT: FocusId = FocusId(1);

/// Sets the counter back to zero.
pub const RESET: FocusId = FocusId(2);

/// Flips the switch.
pub const TOGGLE: FocusId = FocusId(3);

/// The name field.
pub const NAME: FocusId = FocusId(10);

/// The notes field.
pub const NOTES: FocusId = FocusId(11);

/// What the showcase shows that the user can change.
#[derive(Clone, Debug, PartialEq)]
pub struct ShowcaseState {
    /// How many times the counter has been pressed.
    pub clicks: u32,
    /// Whether the switch is on.
    pub switch_on: bool,
    /// When the switch last flipped, in milliseconds of the application clock.
    pub switch_flipped_ms: f32,
    /// The text of the name field.
    pub name: Field,
    /// The text of the notes field.
    pub notes: Field,
}

impl Default for ShowcaseState {
    /// A counter at zero, the switch off, a name filled in and empty notes.
    fn default() -> Self {
        Self {
            clicks: 0,
            switch_on: false,
            switch_flipped_ms: 0.0,
            name: Field::new("Ada Lovelace"),
            notes: Field::default(),
        }
    }
}

impl ShowcaseState {
    /// The state after `action`, which happened at `now_ms`.
    pub fn after(mut self, action: FocusId, now_ms: f32) -> Self {
        match action {
            INCREMENT => self.clicks += 1,
            RESET => self.clicks = 0,
            TOGGLE => {
                self.switch_on = !self.switch_on;
                self.switch_flipped_ms = now_ms;
            }
            _ => {}
        }
        self
    }

    /// Whether `id` names a text field.
    pub fn is_field(id: FocusId) -> bool {
        id == NAME || id == NOTES
    }

    /// The state after `edit` is made to the field named `id`.
    pub fn edited(mut self, id: FocusId, edit: Edit) -> Self {
        match id {
            NAME => self.name.apply(edit),
            NOTES => self.notes.apply(edit),
            _ => {}
        }
        self
    }

    /// How far along the track the knob is at `now_ms`, from 0.0 to 1.0,
    /// easing over a quarter of a second after each flip.
    fn knob(&self, now_ms: f32) -> f32 {
        let progress = Ease::InOut
            .curve()
            .progress((now_ms - self.switch_flipped_ms) / 250.0);
        if self.switch_on {
            progress
        } else {
            1.0 - progress
        }
    }
}

/// Three shades of a tone for a control: hovered, resting, pressed.
fn tone(light: Color, base: Color, dark: Color) -> [Color; 3] {
    [light, base, dark]
}

/// How far a looping effect has got at `elapsed_ms`, easing out and back
/// over a `period_ms` loop, from 0.0 to 1.0.
fn swing(elapsed_ms: f32, period_ms: f32) -> f32 {
    let phase = (elapsed_ms / period_ms).fract();
    let leg = if phase < 0.5 {
        phase * 2.0
    } else {
        2.0 - phase * 2.0
    };
    Ease::InOut.curve().progress(leg)
}

/// The whole screen, for `state` at `elapsed_ms` of the application clock.
pub fn showcase(state: &ShowcaseState, elapsed_ms: f32) -> Node {
    let counter = format!("Pressed {} times", state.clicks);
    node()
        .styles([
            grid(),
            grid_cols(3),
            gap(5),
            p(6),
            w_full(),
            h_full(),
            bg(Role::Background),
        ])
        .child(
            card("Controls")
                .child(node().styles([flex(), gap(3)]).children([
                    button(
                        "Press me",
                        tone(
                            Role::AccentHover.into(),
                            Role::Accent.into(),
                            Color::from(Role::Accent).alpha(0.8),
                        ),
                        Role::OnAccent.into(),
                        INCREMENT,
                    ),
                    button(
                        "Reset",
                        tone(
                            Role::SurfaceActive.into(),
                            Role::SurfaceHover.into(),
                            Role::SurfaceSelected.into(),
                        ),
                        Role::Text.into(),
                        RESET,
                    ),
                ]))
                .child(text(counter).styles([text_lg(), font_medium(), text_color(Role::Text)]))
                .child(node().styles([flex(), gap(2)]).children([
                    badge("Active", Color::Amber400),
                    badge("Failed", Color::Rose400),
                    badge("Queued", Color::Sky400),
                ]))
                .child(switch(state.knob(elapsed_ms), TOGGLE))
                .child(avatar_stack(&[
                    Color::Rose500,
                    Color::Amber500,
                    Color::Emerald500,
                    Color::Sky500,
                    Color::Violet500,
                ])),
        )
        .child(
            card("Progress and motion")
                .child(progress_bar(swing(elapsed_ms, 4000.0), Color::Blue500))
                .child(progress_bar(0.62, Color::Emerald500))
                .child(progress_bar(0.25, Color::Amber500))
                .child(node().styles([flex(), gap(8)]).children([
                    spinner(Color::Sky400),
                    pulse_dot(Color::Emerald400),
                    ping_dot(Color::Rose400),
                    bouncing_dots(Color::Violet400),
                ])),
        )
        .child(
            card("Text input")
                .child(text_input(&state.name, "Your name", NAME))
                .child(text_input(&state.notes, "Notes", NOTES))
                .child(text(format!("Hello, {}", state.name.text())).styles([
                    text_lg(),
                    font_medium(),
                    text_color(Role::Text),
                ])),
        )
        .child(card("Radius, opacity, transform").child(stack([
            radius_steps(Color::Indigo500),
            opacity_steps(Color::Fuchsia500),
            transforms(Color::Teal400),
        ])))
        .child(
            card("Palette")
                .styles([col_span(2), flex_col()])
                .children([15, 10, 20, 5, 12, 16].map(palette)),
        )
        .child(
            card("Shadows")
                .styles([bg(Color::Zinc200)])
                .child(shadow_steps()),
        )
}
