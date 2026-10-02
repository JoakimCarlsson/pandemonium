//! Build script: gives the Windows executable the editor's icon.

/// The editor's icon, relative to this crate.
const ICON: &str = "../../assets/brand/pandemonium.ico";

/// Embeds the icon in the executable when building for Windows.
fn main() {
    println!("cargo:rerun-if-changed={ICON}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    winresource::WindowsResource::new()
        .set_icon(ICON)
        .compile()
        .expect("could not embed the icon in the executable");
}
