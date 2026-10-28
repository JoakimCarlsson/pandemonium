//! Where installed languages and language servers live in the data folder.

use std::path::PathBuf;

/// The folder installed languages are kept in.
pub(crate) fn languages_root() -> Result<PathBuf, String> {
    pm_store::data_directory()
        .map(|data| data.join("languages"))
        .ok_or_else(|| "this machine has no data folder".to_owned())
}

/// The folder you put language folders of your own in, which are listed beside the ones the list offers.
pub fn local_root() -> Result<PathBuf, String> {
    pm_store::data_directory()
        .map(|data| data.join("local-languages"))
        .ok_or_else(|| "this machine has no data folder".to_owned())
}

/// The folder the language `id` is installed in.
pub fn language_folder(id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || id.starts_with('.') || id.contains(['/', '\\']) {
        return Err(format!("{id} is not the name of a language"));
    }
    Ok(languages_root()?.join(id))
}

/// The grammar of the installed language `id`: the one in its folder, or the one of the language it builds on.
pub fn grammar_file(id: &str) -> Result<PathBuf, String> {
    Ok(
        crate::inherit::language_file(id, "grammar.wasm").unwrap_or_else(|| {
            language_folder(id)
                .map_or_else(|_| PathBuf::new(), |folder| folder.join("grammar.wasm"))
        }),
    )
}

/// The folder the versions of the language server `id` are installed in.
pub(crate) fn server_root(id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || id.starts_with('.') || id.contains(['/', '\\']) {
        return Err(format!("{id} is not the name of a language server"));
    }
    pm_store::data_directory()
        .map(|data| data.join("language-servers").join(id))
        .ok_or_else(|| "this machine has no data folder".to_owned())
}

/// The platform this build runs on, as manifests name it: `linux-x86_64`, `macos-aarch64`, `windows-x86_64`.
pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// `name` as a program file of this platform, with `.exe` on Windows.
pub(crate) fn program_name(name: &str) -> String {
    if cfg!(windows) && !name.ends_with(".exe") {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}
