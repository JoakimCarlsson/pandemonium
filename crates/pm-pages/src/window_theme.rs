//! The theme every window is drawn in, made from the appearance settings.

use pm_components::{AppTheme, CodeFont, CodeLook, CursorShape, Density, InterfaceFont};
use pm_config::ThemeMode;
use pm_shell::SettingsModel;

/// The theme `settings` choose: light or dark as the theme mode says, the
/// desktop's own when it follows the system and the desktop is dark when
/// `system_dark` holds; with the interface and code fonts, the line height,
/// the terminal's size, the density, the code editor's look, git's line marks and the
/// terminal's cursor they name. Also makes the editor settings the ones
/// every code editor reads its snippets from.
pub fn window_theme(settings: &SettingsModel, system_dark: bool) -> AppTheme {
    let appearance = &settings.window.appearance;
    let dark = match appearance.theme_mode {
        ThemeMode::System => system_dark,
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
    };
    let code_size = settings.editor.font_size as f32;
    let terminal_size = settings
        .window
        .terminal
        .font_size
        .map_or(code_size, |size| size as f32);
    pm_components::use_editor_settings(&settings.editor);
    AppTheme::of_dark(dark)
        .with_interface_font(InterfaceFont::named(
            &appearance.interface_font,
            appearance.interface_size as f32,
        ))
        .with_code_font(CodeFont::named(
            &appearance.code_font,
            code_size,
            settings.editor.line_height as f32 / 100.0,
            terminal_size,
        ))
        .with_code_look(
            CodeLook::of(&settings.editor)
                .with_git_gutter(settings.window.git.gutter.shows(true))
                .with_terminal_cursor(match settings.window.terminal.cursor {
                    pm_config::TerminalCursor::Block => CursorShape::Block,
                    pm_config::TerminalCursor::Bar => CursorShape::Bar,
                    pm_config::TerminalCursor::Underline => CursorShape::Underline,
                }),
        )
        .with_density(match appearance.density {
            pm_config::Density::Comfortable => Density::Comfortable,
            pm_config::Density::Compact => Density::Compact,
        })
}
