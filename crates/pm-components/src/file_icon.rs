//! The icon of a file or folder, picked and tinted by the file's name.

use rustle::prelude::*;

use crate::size::Size;
use rustle::color_scheme;

/// How a file icon is tinted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileIconVariant {
    /// In the colour of the file's kind.
    #[default]
    Coloured,
    /// Muted, for an entry git ignores.
    Muted,
}

/// A file or folder icon with a variant and a size.
#[derive(Clone)]
pub struct FileIcon {
    /// The file's name, whose ending picks the icon.
    name: String,
    /// Whether it is a folder, and then whether it is open.
    folder: Option<bool>,
    /// How it is tinted.
    variant: FileIconVariant,
    /// Its size.
    size: Size,
}

impl FileIcon {
    /// The icon of the file called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            folder: None,
            variant: FileIconVariant::Coloured,
            size: Size::Sm,
        }
    }

    /// The icon of a folder, open when `open`.
    pub fn folder(mut self, open: bool) -> Self {
        self.folder = Some(open);
        self
    }

    /// The icon in `variant`.
    pub fn variant(mut self, variant: FileIconVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The icon at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the icon.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let side = match self.size {
            Size::Xs => 12.0,
            Size::Sm => 14.0,
            Size::Md => 16.0,
            Size::Lg => 18.0,
        };
        let (glyph, tint) = match self.folder {
            Some(true) => (phosphor::FOLDER_OPEN, Some(scheme.on_surface_variant)),
            Some(false) => (phosphor::FOLDER, Some(scheme.on_surface_variant)),
            None => file_glyph(&self.name),
        };
        let tint = match self.variant {
            FileIconVariant::Muted => scheme.on_surface_variant,
            FileIconVariant::Coloured => tint.unwrap_or(scheme.on_surface_variant),
        };
        icon(glyph, side, tint);
    }
}

/// The glyph of the file called `name`, and the colour of its kind when it has one.
pub fn file_glyph(name: &str) -> (Icon, Option<Color>) {
    let lower = name.to_lowercase();
    let ending = lower.rsplit_once('.').map_or("", |(_, ending)| ending);
    let tone = |r, g, b| Some(Color::rgb(r, g, b));
    match (lower.as_str(), ending) {
        ("cargo.lock" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml", _) => {
            (phosphor::FILE_LOCK, None)
        }
        ("makefile" | "dockerfile" | "justfile", _) => {
            (phosphor::FILE_CODE, tone(0x6d, 0x8c, 0xb3))
        }
        (_, "rs") => (phosphor::FILE_RS, tone(0xde, 0x8a, 0x5a)),
        (_, "ts" | "mts" | "cts") => (phosphor::FILE_TS, tone(0x31, 0x78, 0xc6)),
        (_, "tsx") => (phosphor::FILE_TSX, tone(0x31, 0x78, 0xc6)),
        (_, "js" | "mjs" | "cjs") => (phosphor::FILE_JS, tone(0xe8, 0xc5, 0x4a)),
        (_, "jsx") => (phosphor::FILE_JSX, tone(0xe8, 0xc5, 0x4a)),
        (_, "py") => (phosphor::FILE_PY, tone(0x4b, 0x8b, 0xbe)),
        (_, "c" | "h") => (phosphor::FILE_C, tone(0x5c, 0x6b, 0xc0)),
        (_, "cpp" | "cc" | "cxx" | "hpp" | "hh") => (phosphor::FILE_CPP, tone(0x5c, 0x6b, 0xc0)),
        (_, "cs") => (phosphor::FILE_C_SHARP, tone(0x9b, 0x4f, 0x96)),
        (_, "css" | "scss" | "sass" | "less") => (phosphor::FILE_CSS, tone(0x56, 0x3d, 0x7c)),
        (_, "html" | "htm") => (phosphor::FILE_HTML, tone(0xe3, 0x4c, 0x26)),
        (_, "vue") => (phosphor::FILE_VUE, tone(0x41, 0xb8, 0x83)),
        (_, "md" | "markdown" | "mdx") => (phosphor::FILE_MD, tone(0x51, 0x9a, 0xba)),
        (_, "sql") => (phosphor::FILE_SQL, tone(0xda, 0xa5, 0x2e)),
        (_, "csv" | "tsv") => (phosphor::FILE_CSV, tone(0x4c, 0xaf, 0x50)),
        (_, "toml" | "ini" | "cfg" | "conf" | "yaml" | "yml" | "json" | "jsonc") => {
            (phosphor::FILE_INI, tone(0x8f, 0x9b, 0xa8))
        }
        (_, "svg") => (phosphor::FILE_SVG, tone(0xff, 0xb1, 0x3b)),
        (_, "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico") => {
            (phosphor::FILE_IMAGE, tone(0xa0, 0x74, 0xc4))
        }
        (_, "pdf") => (phosphor::FILE_PDF, tone(0xd9, 0x4c, 0x3c)),
        (_, "zip" | "tar" | "gz" | "tgz" | "xz" | "7z") => (phosphor::FILE_ZIP, None),
        (_, "mp3" | "wav" | "ogg" | "flac") => (phosphor::FILE_AUDIO, None),
        (_, "mp4" | "mov" | "mkv" | "webm") => (phosphor::FILE_VIDEO, None),
        (_, "txt" | "log") => (phosphor::FILE_TXT, None),
        (_, "sh" | "bash" | "zsh" | "fish" | "ps1" | "go" | "java" | "kt" | "rb" | "lua") => {
            (phosphor::FILE_CODE, tone(0x6d, 0x8c, 0xb3))
        }
        _ => (phosphor::FILE, None),
    }
}
