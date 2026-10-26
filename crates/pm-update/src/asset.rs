//! Picking the files of a release that hold the build for this platform and the checksums to check it by.

/// One file attached to a release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    /// Its name, as the release lists it.
    pub name: String,
    /// The address it is downloaded from.
    pub url: String,
    /// How many bytes it is, as the release says; zero when it does not.
    pub size: u64,
}

/// The files of a release that an update takes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chosen {
    /// The archive holding the build.
    pub archive: Asset,
    /// The file listing the SHA-256 of every archive of the release.
    pub sums: Asset,
    /// The name of the program inside the archive, apart from macOS where the archive holds an app bundle.
    pub program: String,
}

/// The target this build was made for, as the names of the release's archives spell it.
const TARGET: &str = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    "x86_64-unknown-linux-gnu"
} else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
    "aarch64-unknown-linux-gnu"
} else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
    "x86_64-apple-darwin"
} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    "aarch64-apple-darwin"
} else {
    "x86_64-pc-windows-msvc"
};

/// What the name of this platform's archive ends in.
const SUFFIX: &str = if cfg!(target_os = "macos") {
    ".app.zip"
} else if cfg!(windows) {
    ".zip"
} else {
    ".tar.gz"
};

/// The name the files of a channel start with: `pandemonium` for `stable`, `pandemonium-preview` for `preview`, which a client from before the rename names `nightly`.
pub fn slug(channel: &str) -> &'static str {
    if matches!(channel, "preview" | "nightly") {
        "pandemonium-preview"
    } else {
        "pandemonium"
    }
}

/// The archive of `version` on `channel` made for this platform, and the checksums beside it, out of the `assets` of the release.
pub fn choose_here(assets: &[Asset], channel: &str, version: &str) -> Result<Chosen, String> {
    choose(assets, channel, version, TARGET, SUFFIX)
}

/// The archive of `version` on `channel` made for `target` and ending in `suffix`, and the checksums beside it, out of the `assets` of the release.
pub fn choose(
    assets: &[Asset],
    channel: &str,
    version: &str,
    target: &str,
    suffix: &str,
) -> Result<Chosen, String> {
    let slug = slug(channel);
    let wanted = format!("{slug}-{version}-{target}{suffix}");
    let found = |name: &str| assets.iter().find(|asset| asset.name == name).cloned();
    let archive = found(&wanted)
        .ok_or_else(|| format!("this release has no build for this platform ({wanted})"))?;
    let sums = found("SHA256SUMS").ok_or("this release has no SHA256SUMS to check the build by")?;
    let program = if cfg!(windows) {
        format!("{slug}.exe")
    } else {
        slug.to_owned()
    };
    Ok(Chosen {
        archive,
        sums,
        program,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An asset called `name`.
    fn asset(name: &str) -> Asset {
        Asset {
            name: name.to_owned(),
            url: format!("https://example.test/{name}"),
            size: 1,
        }
    }

    /// The files of a release of `slug`, as the release workflow publishes them for Linux and macOS.
    fn release(slug: &str) -> Vec<Asset> {
        [
            format!("{slug}-1.0.0-x86_64-unknown-linux-gnu.tar.gz"),
            format!("{slug}-server-1.0.0-x86_64-unknown-linux-gnu.tar.gz"),
            format!("{slug}-1.0.0-aarch64-apple-darwin.tar.gz"),
            format!("{slug}-1.0.0-aarch64-apple-darwin.app.zip"),
            "SHA256SUMS".to_owned(),
        ]
        .iter()
        .map(|name| asset(name))
        .collect()
    }

    #[test]
    fn picks_the_archive_of_the_channel_and_target() {
        let stable = choose(
            &release("pandemonium"),
            "stable",
            "1.0.0",
            "x86_64-unknown-linux-gnu",
            ".tar.gz",
        )
        .unwrap();
        assert_eq!(
            stable.archive.name,
            "pandemonium-1.0.0-x86_64-unknown-linux-gnu.tar.gz"
        );
        let app = choose(
            &release("pandemonium-preview"),
            "preview",
            "1.0.0",
            "aarch64-apple-darwin",
            ".app.zip",
        )
        .unwrap();
        assert_eq!(
            app.archive.name,
            "pandemonium-preview-1.0.0-aarch64-apple-darwin.app.zip"
        );
    }

    #[test]
    fn refuses_a_release_without_this_platform_or_checksums() {
        let assets = release("pandemonium");
        assert!(choose(&assets, "stable", "1.0.0", "x86_64-pc-windows-msvc", ".zip").is_err());
        assert!(
            choose(
                &assets,
                "preview",
                "1.0.0",
                "x86_64-unknown-linux-gnu",
                ".tar.gz"
            )
            .is_err()
        );
        let bare = &assets[..4];
        assert!(
            choose(
                bare,
                "stable",
                "1.0.0",
                "x86_64-unknown-linux-gnu",
                ".tar.gz"
            )
            .is_err()
        );
    }
}
