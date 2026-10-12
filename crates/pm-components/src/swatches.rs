//! Rows that lay a scale out: colours, radii, shadows, opacity and transforms.

use pm_style::{
    Color, Radius, bg, flex, flex_col, flex_wrap, gap, grid, grid_cols, h, h_full, items_center,
    opacity, rotate, rounded, scale, shadow_2xl, shadow_lg, shadow_md, shadow_sm, shadow_xl, size,
    skew_x, w_full,
};
use pm_ui::{Node, node};

/// The eleven shades of the hue at `hue` in the palette, as a row of tiles.
pub fn palette(hue: usize) -> Node {
    let start = 3 + hue * 11;
    node()
        .styles([grid(), grid_cols(11), gap(1), w_full()])
        .children(
            Color::PALETTE[start..start + 11]
                .iter()
                .map(|shade| node().styles([h(6), rounded(Radius::Sm), bg(*shade)])),
        )
}

/// A tile at each named radius.
pub fn radius_steps(tone: Color) -> Node {
    let steps = [
        Radius::Xs,
        Radius::Sm,
        Radius::Md,
        Radius::Lg,
        Radius::Xl,
        Radius::Xl2,
    ];
    node().styles([flex(), items_center(), gap(2)]).children(
        steps
            .into_iter()
            .map(|step| node().styles([size(9), rounded(step), bg(tone)])),
    )
}

/// A tile at each shadow depth, on the surface that shows them.
pub fn shadow_steps() -> Node {
    let tile = |shadow| node().styles([size(9), rounded(Radius::Md), bg(Color::Zinc100), shadow]);
    node().styles([flex(), items_center(), gap(3)]).children([
        tile(shadow_sm()),
        tile(shadow_md()),
        tile(shadow_lg()),
        tile(shadow_xl()),
        tile(shadow_2xl()),
    ])
}

/// A tile at four opacities.
pub fn opacity_steps(tone: Color) -> Node {
    node().styles([flex(), items_center(), gap(2)]).children(
        [100, 75, 50, 25].map(|percent| {
            node().styles([size(9), rounded(Radius::Md), bg(tone), opacity(percent)])
        }),
    )
}

/// A tile turned, scaled and skewed.
pub fn transforms(tone: Color) -> Node {
    let tile = |transform| node().styles([size(8), rounded(Radius::Md), bg(tone), transform]);
    node()
        .styles([
            flex(),
            flex_wrap(),
            items_center(),
            gap(6),
            h_full(),
            pm_style::p(2),
        ])
        .children([tile(rotate(15)), tile(scale(125)), tile(skew_x(15))])
}

/// A column of `rows`, for a section that stacks scales.
pub fn stack(rows: impl IntoIterator<Item = Node>) -> Node {
    node().styles([flex(), flex_col(), gap(3)]).children(rows)
}
