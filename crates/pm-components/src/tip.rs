//! A tooltip: a small label shown below a control while the pointer rests on
//! it, moved back inside the window at its right edge and shown above the
//! control when there is no room below.

use rustle::prelude::*;

/// How far from the control the label hangs.
const GAP: f32 = 6.0;

/// Emits `content` and, while the pointer is over it, `label` in a small
/// raised box below it, kept inside the window.
#[track_caller]
pub fn tip(label: &str, content: impl FnOnce()) {
    let scheme = color_scheme();
    let hovered = remember(|| false);
    let height = remember(|| 0.0_f32);
    let hover = hovered.clone();
    let tall = height.clone();
    stack_with(
        Modifier::new()
            .on_hover(move |over| hover.set(over))
            .on_size_changed(move |size| {
                if tall.get() != size.height {
                    tall.set(size.height);
                }
            }),
        || {
            content();
            if !hovered.get() {
                return;
            }
            let measures = crate::window_area::remember_measures();
            let anchor = height.get();
            let (across, down) = match (
                crate::window_area::window_area(),
                measures.origin,
                measures.size,
            ) {
                (Some(area), Some(origin), Some(size)) => {
                    crate::window_area::fit_beside(origin, anchor, size, GAP, area)
                }
                _ => (0.0, anchor + GAP),
            };
            let style = crate::label::Label::new("")
                .size(crate::size::Size::Xs)
                .style()
                .color(scheme.inverse_on_surface)
                .max_lines(1);
            let label = label.to_owned();
            floating(Modifier::new(), move || {
                spacer(measures.marker);
                stack_with(
                    measures
                        .measured
                        .offset(across, down)
                        .shadow(4.0, 6.0)
                        .background_rounded(scheme.inverse_surface, 6.0)
                        .padding_xy(8.0, 4.0),
                    move || text_styled(Modifier::new(), label, style),
                );
            });
        },
    );
}
