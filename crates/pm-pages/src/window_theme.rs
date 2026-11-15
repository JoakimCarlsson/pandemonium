//! The theme every window is drawn in, made from the appearance settings.

use pm_components::{
    AppTheme, CodeFont, CodeLook, CursorShape, Density, InterfaceFont, ThemeFamily, family_named,
};
use pm_config::ThemeMode;
use pm_shell::SettingsModel;

/// The theme families the extensions installed on this machine bring, as `settings` lists them.
pub fn brought_families(settings: &SettingsModel) -> Vec<ThemeFamily> {
    settings
        .assets
        .themes
        .iter()
        .map(|theme| ThemeFamily::from_colours(&theme.name, &theme.dark, &theme.light))
        .collect()
}

/// The theme `settings` choose: the family named for the appearance, light or dark as the theme mode says, the
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
    let name = if dark {
        &appearance.dark_theme
    } else {
        &appearance.light_theme
    };
    family_named(name, &brought_families(settings))
        .theme(dark)
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
                .with_git_marks(
                    settings.window.git.blame.shows(true),
                    settings.window.git.word_diff,
                )
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The settings list the same shipped theme families the components paint.
    #[test]
    fn shipped_names_agree() {
        let painted: Vec<String> = pm_components::shipped_families()
            .into_iter()
            .map(|family| family.name)
            .collect();
        assert_eq!(
            painted,
            pm_shell::SHIPPED_THEMES.map(str::to_owned).to_vec()
        );
    }

    /// Choosing Dusk for the dark appearance paints the window in Dusk's colours.
    #[test]
    fn dark_theme_paints_dusk() {
        let mut settings = SettingsModel::default();
        settings.window.appearance.theme_mode = ThemeMode::Dark;
        settings.window.appearance.dark_theme = "Dusk".to_owned();
        let theme = window_theme(&settings, false);
        assert_eq!(
            theme.color_scheme.background,
            pm_components::Palette::dusk_dark().background
        );
    }
}
