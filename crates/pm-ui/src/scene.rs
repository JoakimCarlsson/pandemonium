//! A complete screen whose construction and layout survive pointer-only frames.

use pm_renderer::DrawList;
use pm_style::layout::{Placed, solve};
use pm_text::Shaper;
use pm_theme::Theme;

use crate::{Input, Node, Output, paint};

/// A node tree and its geometry, rebuilt by the caller when its content or viewport changes.
pub struct Scene {
    /// The described screen, including its conditional interaction styles.
    root: Node,
    /// The geometry shared by subsequent painting passes.
    placed: Placed,
    /// The logical viewport used to lay out the screen.
    size: (f32, f32),
}

impl Scene {
    /// Builds the geometry for `root` in the logical viewport `width` by `height`.
    pub fn new(root: Node, width: f32, height: f32, shaper: &mut Shaper) -> Self {
        let placed = solve(&root.item(shaper), width, height);
        Self {
            root,
            placed,
            size: (width, height),
        }
    }

    /// Repaints the screen with the latest pointer, focus, selection, clock and theme.
    pub fn paint(
        &self,
        input: &Input,
        theme: &Theme,
        shaper: &mut Shaper,
        list: &mut DrawList,
    ) -> Output {
        paint::paint(
            &self.root,
            &self.placed,
            self.size.0,
            self.size.1,
            input,
            theme,
            shaper,
            list,
        )
    }
}

#[cfg(test)]
mod tests {
    use pm_input::{FocusId, PointerState};
    use pm_renderer::Primitive;
    use pm_style::{Role, Roles, bg, flex, flex_col, h, w_full};

    use super::*;
    use crate::{draw, node};

    /// Retained geometry still repaints hover and focus immediately, exactly as a fresh layout does.
    #[test]
    fn retained_scene_resolves_new_input() {
        let mut shaper = Shaper::new();
        let theme = pm_theme::default_family().dark;
        let root = node()
            .styles([flex(), flex_col()])
            .children((0..3).map(|index| {
                node()
                    .styles([
                        h(6),
                        w_full(),
                        bg(Role::SurfaceHover).hover(),
                        bg(Role::SurfaceSelected).focus(),
                    ])
                    .focusable(FocusId(index))
            }));
        let scene = Scene::new(root.clone(), 200.0, 200.0, &mut shaper);
        let mut retained = DrawList::new();
        let mut fresh = DrawList::new();
        for y in [10.0, 35.0, 60.0] {
            let input = Input {
                pointer: PointerState {
                    position: Some((10.0, y)),
                    ..Default::default()
                },
                focused: Some(FocusId(2)),
                ..Default::default()
            };
            retained.clear();
            fresh.clear();
            let actual = scene.paint(&input, &theme, &mut shaper, &mut retained);
            let expected = draw(&root, 200.0, 200.0, &input, &theme, &mut shaper, &mut fresh);
            assert_eq!(actual.regions, expected.regions);
            let quads = |list: &DrawList| {
                list.primitives()
                    .iter()
                    .filter_map(|primitive| match primitive {
                        Primitive::Quad(quad) => Some((quad.bounds, quad.fill)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(quads(&retained), quads(&fresh));
            assert!(quads(&retained).iter().any(|(_, fill)| *fill
                == theme.role(if y >= 48.0 {
                    Role::SurfaceSelected
                } else {
                    Role::SurfaceHover
                })));
        }
    }
}
