//! File and folder icons: the artwork for the kind of file a path names, tinted by kind.

use pm_icon::IconName;
use pm_style::{Color, Role};
use pm_ui::Node;

use crate::parts::glyph;

/// The icon and colour of a file of the kind `path` names, a plain muted file when it is of no known kind.
pub fn file_kind(path: &str) -> (IconName, Color) {
    let rgb = |red, green, blue| Color::rgb(red, green, blue);
    match path.rsplit('.').next().unwrap_or_default() {
        "rs" => (IconName::FileRs, rgb(0xe8, 0x95, 0x6a)),
        "ts" => (IconName::FileTs, rgb(0x5a, 0xa9, 0xe6)),
        "tsx" => (IconName::FileTsx, rgb(0x5a, 0xa9, 0xe6)),
        "js" => (IconName::FileJs, rgb(0xe6, 0xc8, 0x4f)),
        "jsx" => (IconName::FileJsx, rgb(0xe6, 0xc8, 0x4f)),
        "md" => (IconName::FileMd, rgb(0x7f, 0xb7, 0xe8)),
        "css" => (IconName::FileCss, rgb(0xb5, 0x8b, 0xe0)),
        "html" => (IconName::FileHtml, rgb(0xe8, 0x79, 0x5f)),
        "py" => (IconName::FilePy, rgb(0x6f, 0xb0, 0xe0)),
        "toml" | "yml" | "yaml" => (IconName::FileText, rgb(0xd9, 0x8b, 0x8b)),
        "json" => (IconName::FileText, rgb(0xe6, 0xc8, 0x4f)),
        "lock" => (IconName::FileText, Color::from(Role::Subtle)),
        _ => (IconName::File, Color::from(Role::Muted)),
    }
}

/// The icon of the file `path` names, `steps` on a side, tinted by its kind.
pub fn file_icon(path: &str, steps: f32) -> Node {
    let (icon, tone) = file_kind(path);
    glyph(icon, steps, tone)
}

/// A folder, open or closed, `steps` on a side, in the folder colour.
pub fn folder_icon(open: bool, steps: f32) -> Node {
    let icon = if open {
        IconName::FolderOpen
    } else {
        IconName::Folder
    };
    glyph(icon, steps, Color::rgb(0xd9, 0xa9, 0x5b))
}
