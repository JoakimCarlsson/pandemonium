//! A file read in the window: its lines, numbered, as they are in the folder or in the last commit.

use pm_icon::IconName;
use pm_style::{
    Role, bg, border_b, border_color, flex, flex_1, flex_col, flex_none, font_mono, gap, h,
    items_center, justify_end, leading_ratio, min_h, min_w_px, overflow_hidden, pr, px, py,
    text_color, text_px, truncate, w,
};
use pm_ui::{Node, node, text};

use crate::Button;
use crate::file_icon::file_icon;
use crate::git_controls::{Contents, VIEWER_CLOSE, VIEWER_SCROLL, Viewer};
use crate::parts::{line, spacer};
use crate::state::WorkspaceState;

/// How tall a line of the file is, in logical pixels.
const ROW: f32 = 20.0;

/// How far beyond the view, above and below, lines are still built, in logical pixels.
const OVERSCAN: f32 = 100.0;

/// How tall the strip above the lines is, in logical pixels.
const STRIP: f32 = 40.0;

/// A box as tall as `lines` lines, standing in for those left out.
fn blank(lines: usize) -> Node {
    node().styles([h(lines as f32 * ROW / 4.0), flex_none()])
}

/// One numbered line of the file.
fn file_line(number: usize, content: &str) -> Node {
    node()
        .styles([flex(), h(5), flex_none(), overflow_hidden()])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    justify_end(),
                    flex_none(),
                    w(14),
                    pr(3),
                ])
                .child(text(number.to_string()).styles([
                    text_px(12),
                    leading_ratio(1.4),
                    font_mono(),
                    text_color(Role::Subtle),
                ])),
        )
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden(), px(2)])
                .child(text(content.replace('\t', "    ")).styles([
                    text_px(12),
                    leading_ratio(1.4),
                    font_mono(),
                    text_color(Role::Text),
                    truncate(),
                ])),
        )
}

/// The lines of `lines` that are in or near a view `height` tall scrolled `scroll` down, with a blank for those above and below.
fn visible(lines: &[String], scroll: f32, height: f32) -> Vec<Node> {
    let first = (((scroll - OVERSCAN) / ROW).floor().max(0.0) as usize).min(lines.len());
    let last = ((((scroll + height + OVERSCAN) / ROW).ceil()) as usize).min(lines.len());
    let mut rows = vec![blank(first)];
    rows.extend(
        lines[first..last]
            .iter()
            .enumerate()
            .map(|(offset, content)| file_line(first + offset + 1, content)),
    );
    rows.push(blank(lines.len() - last));
    rows
}

/// The file being read, in a view `height` logical pixels tall: a strip naming it, then its lines.
pub fn viewer(state: &WorkspaceState, height: f32) -> Node {
    let Some(Viewer {
        path,
        head,
        contents,
        scroll,
    }) = state.git.viewer.as_ref()
    else {
        return node();
    };
    let mut strip = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(10),
            px(4),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(file_icon(path, 4.0))
        .child(line(path, 13, 1.4, Role::Text).styles([font_mono()]));
    if *head {
        strip = strip.child(line("HEAD", 11, 1.4, Role::Muted));
    }
    strip = strip
        .child(spacer())
        .child(Button::icon(IconName::Close, VIEWER_CLOSE).build());
    let body = match contents {
        Contents::Loading => {
            node()
                .styles([px(4), py(4)])
                .child(line("Reading the file.", 13, 1.4, Role::Subtle))
        }
        Contents::Message(message) => {
            node()
                .styles([px(4), py(4)])
                .child(line(message, 13, 1.4, Role::Subtle))
        }
        Contents::Text { lines, truncated } => {
            let mut rows = node()
                .styles([flex(), flex_col(), flex_none(), py(2)])
                .children(visible(lines, *scroll, height - STRIP));
            if *truncated {
                rows = rows.child(node().styles([px(4), py(2)]).child(line(
                    "The rest of the file is not shown.",
                    12,
                    1.4,
                    Role::Subtle,
                )));
            }
            rows
        }
    };
    node()
        .styles([
            flex(),
            flex_col(),
            flex_1(),
            min_w_px(0),
            min_h(0),
            bg(Role::Background),
        ])
        .child(strip)
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
                .scroll_from_start(VIEWER_SCROLL, *scroll)
                .child(body),
        )
}
