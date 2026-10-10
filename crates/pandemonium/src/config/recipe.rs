//! Declarative server recipes shared by extensions and user settings.

use pm_text::install::{Build, Recipe};
use serde::{Deserialize, Serialize};

/// A release asset in a server declaration.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StoredBuild {
    /// Operating system and architecture.
    pub platform: String,
    /// HTTPS URL with an optional version placeholder.
    pub url: String,
    /// SHA-256 of the compressed asset.
    pub sha256: String,
}

/// A pinned installation recipe supplied as data.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StoredRecipe {
    /// An executable or archive with checksums per platform.
    Release {
        /// Pinned release version.
        version: String,
        /// Available platform assets.
        builds: Vec<StoredBuild>,
    },
    /// An npm package and its pinned companion packages.
    Npm {
        /// Package name.
        package: String,
        /// Pinned package version.
        version: String,
        /// Additional pinned packages.
        #[serde(default)]
        extra: Vec<String>,
    },
    /// A pinned Go module.
    Go {
        /// Module import path.
        module: String,
        /// Module version.
        version: String,
    },
    /// A pinned Python package.
    Pip {
        /// Package name.
        package: String,
        /// Package version.
        version: String,
    },
}

impl StoredRecipe {
    /// Rejects unsafe destinations, unpinned versions and invalid release assets.
    pub fn validate(&self) -> Result<(), String> {
        let version = match self {
            Self::Release { version, .. }
            | Self::Npm { version, .. }
            | Self::Go { version, .. }
            | Self::Pip { version, .. } => version,
        };
        if version.is_empty()
            || ["latest", "*", ".", ".."].contains(&version.as_str())
            || version.contains(['/', '\\'])
            || version.starts_with(['-', '^', '~'])
        {
            return Err("Server recipes require a pinned version without path components.".into());
        }
        if let Self::Release { builds, .. } = self {
            if builds.is_empty() {
                return Err("A release recipe needs at least one platform build.".into());
            }
            let mut platforms = std::collections::HashSet::new();
            for build in builds {
                if !build.url.starts_with("https://")
                    || build.sha256.len() != 64
                    || !build.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                    || !platforms.insert(&build.platform)
                {
                    return Err(
                        "Release builds need HTTPS URLs, SHA-256 checksums and unique platforms."
                            .into(),
                    );
                }
            }
        }
        if let Self::Npm { extra, .. } = self
            && extra.iter().any(|package| {
                package.rsplit_once('@').is_none_or(|(_, version)| {
                    version.is_empty() || version == "latest" || version.contains(['*', '^', '~'])
                })
            })
        {
            return Err("Extra npm packages must have pinned versions.".into());
        }
        Ok(())
    }

    /// Keeps a validated recipe alive for the language registry.
    pub fn into_recipe(self) -> Recipe {
        match self {
            Self::Release { version, builds } => Recipe::Release {
                version: version.leak(),
                builds: builds
                    .into_iter()
                    .map(|build| Build {
                        platform: build.platform.leak(),
                        url: build.url.leak(),
                        sha256: build.sha256.leak(),
                    })
                    .collect::<Vec<_>>()
                    .leak(),
            },
            Self::Npm {
                package,
                version,
                extra,
            } => Recipe::Npm {
                package: package.leak(),
                version: version.leak(),
                extra: extra
                    .into_iter()
                    .map(|value| &*value.leak())
                    .collect::<Vec<_>>()
                    .leak(),
            },
            Self::Go { module, version } => Recipe::Go {
                module: module.leak(),
                version: version.leak(),
            },
            Self::Pip { package, version } => Recipe::Pip {
                package: package.leak(),
                version: version.leak(),
            },
        }
    }

    /// Serializes a runtime recipe without losing extension installation metadata.
    pub fn of(recipe: Recipe) -> Self {
        match recipe {
            Recipe::Release { version, builds } => Self::Release {
                version: version.into(),
                builds: builds
                    .iter()
                    .map(|build| StoredBuild {
                        platform: build.platform.into(),
                        url: build.url.into(),
                        sha256: build.sha256.into(),
                    })
                    .collect(),
            },
            Recipe::Npm {
                package,
                version,
                extra,
            } => Self::Npm {
                package: package.into(),
                version: version.into(),
                extra: extra.iter().map(|value| (*value).into()).collect(),
            },
            Recipe::Go { module, version } => Self::Go {
                module: module.into(),
                version: version.into(),
            },
            Recipe::Pip { package, version } => Self::Pip {
                package: package.into(),
                version: version.into(),
            },
        }
    }
}
