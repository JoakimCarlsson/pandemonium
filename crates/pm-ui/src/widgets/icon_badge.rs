//! A small status icon with detail shared by list and sidebar rows.

use pm_gfx::Rgba;

use crate::{Div, IconName, IconSize, Styled, h_flex, icon};

/// Draws a status icon and its tooltip, leaving unknown states empty.
pub fn icon_badge<M: 'static>(status: Option<(IconName, Rgba)>, detail: &str) -> Div<M> {
    let badge = h_flex().items_center().pl(1).tooltip(detail);
    match status {
        Some((name, color)) => badge.child(icon(name).size(IconSize::Small).color(color)),
        None => badge,
    }
}
