//! Teaches the file icons what is installed on this machine: the icon of
//! each installed language and the icons of the icon themes extensions bring.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pm_components::{IconSource, learn_language_icons, learn_theme_icons};
use pm_shell::SettingsModel;

/// Learns the file icons the languages and icon themes `settings` lists give, reading them again only when what is installed changed.
pub fn learn_file_icons(settings: &SettingsModel) {
    let mut languages = DefaultHasher::new();
    for extension in settings
        .extensions
        .iter()
        .filter(|extension| extension.installed)
    {
        extension.id.hash(&mut languages);
        extension.icon.hash(&mut languages);
        extension.color.hash(&mut languages);
        extension.suffixes.hash(&mut languages);
        extension.names.hash(&mut languages);
    }
    learn_language_icons(languages.finish(), || {
        settings
            .extensions
            .iter()
            .filter(|extension| extension.installed)
            .filter_map(|extension| {
                Some(IconSource {
                    suffixes: extension.suffixes.clone(),
                    names: extension.names.clone(),
                    svg: extension.icon.clone()?,
                    color: extension.color.clone(),
                })
            })
            .collect()
    });
    let mut themes = DefaultHasher::new();
    for theme in &settings.assets.icon_themes {
        theme.extension.hash(&mut themes);
        theme.name.hash(&mut themes);
        for icon in &theme.icons {
            icon.suffix.hash(&mut themes);
            icon.svg.hash(&mut themes);
            icon.color.hash(&mut themes);
        }
    }
    learn_theme_icons(themes.finish(), || {
        settings
            .assets
            .icon_themes
            .iter()
            .flat_map(|theme| &theme.icons)
            .map(|icon| IconSource {
                suffixes: vec![icon.suffix.clone()],
                names: Vec::new(),
                svg: icon.svg.clone(),
                color: icon.color.clone(),
            })
            .collect()
    });
}
