//! File icons learned from what is installed: the icon of each installed
//! language and the icons of icon themes, read from their SVG documents into
//! outlines once and looked up by a file's name.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use resvg::tiny_skia::{PathSegment, Transform};
use resvg::usvg::{Group, Node, Options, Tree};
use rustle::{Color, FillRule, Icon, PathCommand};

/// The icon a language or an icon theme gives some files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconSource {
    /// The endings of the file names it is for, without the dot.
    pub suffixes: Vec<String>,
    /// The whole file names it is for, such as `Makefile`.
    pub names: Vec<String>,
    /// The icon, as an SVG document.
    pub svg: String,
    /// The colour it is tinted, as `#rrggbb`, when it names one.
    pub color: Option<String>,
}

/// An icon read from its SVG document, with its tint.
#[derive(Clone)]
struct Learned {
    /// The outline.
    icon: Icon,
    /// Its tint, when it names one.
    tint: Option<Color>,
}

/// The icons learned from one kind of source, by lower-case ending and by lower-case whole name.
#[derive(Default)]
struct Table {
    /// What stands for the sources it was read from, to skip reading them again.
    key: Option<u64>,
    /// By the ending of a file name.
    endings: HashMap<String, Learned>,
    /// By the whole name of a file.
    names: HashMap<String, Learned>,
}

/// What was learned: icon themes win over languages.
#[derive(Default)]
struct Registry {
    /// From the icon themes installed.
    themes: Table,
    /// From the languages installed.
    languages: Table,
}

/// The icons learned so far.
static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Default::default);

/// Takes in the icons of the languages installed, which `sources` lists, in place of those learned before; does nothing when `key`, which stands for what they are, is the one they were learned under.
pub fn learn_language_icons(key: u64, sources: impl FnOnce() -> Vec<IconSource>) {
    if let Ok(mut registry) = REGISTRY.lock() {
        registry.languages.learn(key, sources);
    }
}

/// Takes in the icons of the icon themes installed, which `sources` lists, a later one's icon for an ending replacing an earlier one's, in place of those learned before; does nothing when `key` is the one they were learned under.
pub fn learn_theme_icons(key: u64, sources: impl FnOnce() -> Vec<IconSource>) {
    if let Ok(mut registry) = REGISTRY.lock() {
        registry.themes.learn(key, sources);
    }
}

/// The learned icon of the file called `name`, and its tint, when an icon theme or an installed language gives one.
pub(crate) fn learned_glyph(name: &str) -> Option<(Icon, Option<Color>)> {
    let lower = name.to_lowercase();
    let ending = lower.rsplit_once('.').map(|(_, ending)| ending);
    let registry = REGISTRY.lock().ok()?;
    [&registry.themes, &registry.languages]
        .into_iter()
        .find_map(|table| {
            table
                .names
                .get(&lower)
                .or_else(|| ending.and_then(|ending| table.endings.get(ending)))
                .cloned()
        })
        .map(|learned| (learned.icon, learned.tint))
}

impl Table {
    /// Reads `sources` in place of what the table held, unless `key` is what it was read under.
    fn learn(&mut self, key: u64, sources: impl FnOnce() -> Vec<IconSource>) {
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        let sources = sources();
        self.endings.clear();
        self.names.clear();
        for source in &sources {
            let Some(icon) = outline_of(&source.svg) else {
                continue;
            };
            let learned = Learned {
                icon,
                tint: source.color.as_deref().and_then(colour_of),
            };
            for suffix in &source.suffixes {
                self.endings.insert(suffix.to_lowercase(), learned.clone());
            }
            for name in &source.names {
                self.names.insert(name.to_lowercase(), learned.clone());
            }
        }
    }
}

/// The colour `hex`, written `#rrggbb`, names.
fn colour_of(hex: &str) -> Option<Color> {
    let digits = hex.trim().strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16)
        .ok()
        .filter(|_| digits.len() == 6)?;
    Some(Color::rgb(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ))
}

/// The SVG document `svg` as one filled outline: its filled shapes, and its stroked ones as the outlines of their strokes, in the document's own units.
pub fn outline_of(svg: &str) -> Option<Icon> {
    let tree = Tree::from_str(svg, &Options::default()).ok()?;
    let mut commands = Vec::new();
    add_group(tree.root(), &mut commands);
    if commands.is_empty() {
        return None;
    }
    let size = tree.size();
    Some(Icon::new(
        commands,
        size.width(),
        size.height(),
        FillRule::NonZero,
    ))
}

/// Adds the outlines of the shapes inside `group` to `commands`.
fn add_group(group: &Group, commands: &mut Vec<PathCommand>) {
    for node in group.children() {
        match node {
            Node::Group(inner) => add_group(inner, commands),
            Node::Path(path) if path.is_visible() => {
                let transform = path.abs_transform();
                if path.fill().is_some() {
                    add_path(path.data(), transform, commands);
                }
                if let Some(stroke) = path.stroke()
                    && let Some(outline) = path.data().stroke(&stroke.to_tiny_skia(), 1.0)
                {
                    add_path(&outline, transform, commands);
                }
            }
            _ => {}
        }
    }
}

/// Adds the segments of `path`, moved by `transform`, to `commands`.
fn add_path(path: &resvg::tiny_skia::Path, transform: Transform, commands: &mut Vec<PathCommand>) {
    let Some(path) = path.clone().transform(transform) else {
        return;
    };
    for segment in path.segments() {
        commands.push(match segment {
            PathSegment::MoveTo(point) => PathCommand::MoveTo(point.x, point.y),
            PathSegment::LineTo(point) => PathCommand::LineTo(point.x, point.y),
            PathSegment::QuadTo(control, end) => {
                PathCommand::QuadTo(control.x, control.y, end.x, end.y)
            }
            PathSegment::CubicTo(first, second, end) => {
                PathCommand::CubicTo(first.x, first.y, second.x, second.y, end.x, end.y)
            }
            PathSegment::Close => PathCommand::Close,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square drawn in SVG becomes an outline the size of the document.
    #[test]
    fn reads_an_svg_outline() {
        let icon = outline_of(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16"><rect x="2" y="2" width="12" height="12" fill="#000"/><path d="M1 1 L15 1" stroke="#000" fill="none"/></svg>"##,
        )
        .unwrap();
        assert_eq!(icon.size(), (16.0, 16.0));
        assert!(icon.commands().len() > 5);
        assert!(outline_of("not svg").is_none());
    }

    /// A learned language icon is found by the file's ending and its whole name, and an icon theme's wins.
    #[test]
    fn learned_icons_are_found() {
        let square = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect width="16" height="16"/></svg>"##;
        learn_language_icons(1, || {
            vec![IconSource {
                suffixes: vec!["zzlang".to_owned()],
                names: vec!["Zzfile".to_owned()],
                svg: square.to_owned(),
                color: Some("#112233".to_owned()),
            }]
        });
        learn_language_icons(1, Vec::new);
        assert!(learned_glyph("zzfile").is_some());
        assert_eq!(
            learned_glyph("main.ZZLANG").and_then(|(_, tint)| tint),
            Some(Color::rgb(0x11, 0x22, 0x33))
        );
        assert!(learned_glyph("zzfile").is_some());
        learn_theme_icons(1, || {
            vec![IconSource {
                suffixes: vec!["zzlang".to_owned()],
                names: Vec::new(),
                svg: square.to_owned(),
                color: Some("#445566".to_owned()),
            }]
        });
        assert_eq!(
            learned_glyph("a.zzlang").and_then(|(_, tint)| tint),
            Some(Color::rgb(0x44, 0x55, 0x66))
        );
        assert!(learned_glyph("a.unknown-ending").is_none());
        learn_theme_icons(2, Vec::new);
        learn_language_icons(2, Vec::new);
    }
}
