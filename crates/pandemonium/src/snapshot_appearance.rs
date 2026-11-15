//! The snapshot words that change the appearance settings a snapshot is drawn
//! with: `compact`, `interface-<px>`, `codesize-<px>`, `line-<times>`,
//! `terminal-<px>`, `font-<family>` and `mono-<family>`, where a family's
//! spaces are written as `_`; and the editor words `relative`, `no-numbers`,
//! `wrap-guide`, `no-minimap` and `bar-terminal`.

use pm_config::{Density, TerminalCursor};
use pm_shell::SettingsModel;

/// The prefixes of the appearance words that take a value.
const PREFIXES: &[&str] = &[
    "interface-",
    "codesize-",
    "line-",
    "terminal-",
    "font-",
    "mono-",
];

/// The appearance words that take no value.
const FLAGS: &[&str] = &[
    "compact",
    "relative",
    "no-numbers",
    "wrap-guide",
    "no-minimap",
    "bar-terminal",
];

/// Whether `word` is an appearance word.
pub fn known(word: &str) -> bool {
    FLAGS.contains(&word) || PREFIXES.iter().any(|prefix| word.starts_with(prefix))
}

/// Changes the appearance settings in `model` as the appearance words among `words` say.
pub fn apply(model: &mut SettingsModel, words: &[&str]) {
    for word in words {
        apply_word(model, word);
    }
}

/// Changes the appearance settings in `model` as `word` says, when it is an appearance word.
fn apply_word(model: &mut SettingsModel, word: &str) {
    let appearance = &mut model.window.appearance;
    match word {
        "compact" => appearance.density = Density::Compact,
        "relative" => model.editor.relative_line_numbers = true,
        "no-numbers" => model.editor.line_numbers = false,
        "wrap-guide" => model.editor.wrap_guide = true,
        "no-minimap" => model.editor.minimap = false,
        "bar-terminal" => model.window.terminal.cursor = TerminalCursor::Bar,
        _ => apply_valued(model, word),
    }
}

/// Changes the appearance settings in `model` as `word` says, when it is an appearance word that takes a value.
fn apply_valued(model: &mut SettingsModel, word: &str) {
    let appearance = &mut model.window.appearance;
    if let Some(size) = word.strip_prefix("interface-").and_then(|n| n.parse().ok()) {
        appearance.interface_size = size;
    } else if let Some(size) = word.strip_prefix("codesize-").and_then(|n| n.parse().ok()) {
        model.editor.font_size = size;
    } else if let Some(times) = word
        .strip_prefix("line-")
        .and_then(|n| n.parse::<f32>().ok())
    {
        model.editor.line_height = (times * 100.0).round() as u32;
    } else if let Some(size) = word.strip_prefix("terminal-").and_then(|n| n.parse().ok()) {
        model.window.terminal.font_size = Some(size);
    } else if let Some(family) = word.strip_prefix("font-") {
        appearance.interface_font = family.replace('_', " ");
    } else if let Some(family) = word.strip_prefix("mono-") {
        appearance.code_font = family.replace('_', " ");
    }
}
