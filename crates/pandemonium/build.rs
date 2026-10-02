//! Build script: gives the Windows executable the editor's icon.

/// The editor's icon, relative to this crate.
const ICON: &str = "../../assets/brand/pandemonium.ico";

/// Embeds the icon in the executable when building for Windows.
fn main() {
    embed_extensions();
    println!("cargo:rerun-if-changed={ICON}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    winresource::WindowsResource::new()
        .set_icon(ICON)
        .compile()
        .expect("could not embed the icon in the executable");
}

/// Embeds catalogue packages as a generic offline fallback for first installation.
fn embed_extensions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions/packages");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut entries = std::fs::read_dir(&root)
        .expect("extension packages directory")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "zip"))
        .collect::<Vec<_>>();
    entries.sort();
    let mut source = String::from(
        "/// Bundled extension packages keyed by archive name.\npub const PACKAGES: &[(&str, &[u8])] = &[\n",
    );
    for path in entries {
        source.push_str(&format!(
            "({:?}, include_bytes!({:?})),\n",
            path.file_name().unwrap().to_string_lossy(),
            path
        ));
    }
    source.push_str("];\n");
    std::fs::write(
        std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("extension_packages.rs"),
        source,
    )
    .expect("write extension package includes");
}
