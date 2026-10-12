//! The names of the app's icons and the artwork each is drawn from.

use crate::Svg;

/// Reads one icon's artwork out of the crate's own assets.
macro_rules! include_icon {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/",
            $name,
            ".svg"
        ))
    };
}

/// One piece of artwork, at whatever size it is asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IconName {
    /// A globe: the placeholder that shows how an icon is added.
    HelloWorld,
}

impl IconName {
    /// The artwork this icon is drawn from.
    pub fn svg(self) -> Svg {
        match self {
            Self::HelloWorld => Svg::new("hello_world", include_icon!("hello_world")),
        }
    }
}
