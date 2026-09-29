//! WASM grammars share one engine and give each parser its own store.

use std::sync::LazyLock;

use tree_sitter::{Language, Parser, WasmStore, wasmtime::Engine};

/// The engine shared by every extension grammar and parser store.
static ENGINE: LazyLock<Engine> = LazyLock::new(Engine::default);

/// Loads a grammar from its bytes, keeping its language alive for the editor.
pub(crate) fn load(name: &str, bytes: &[u8]) -> Result<&'static Language, String> {
    if name.contains('\0') {
        return Err("language id contains a NUL byte".into());
    }
    let mut store = WasmStore::new(&ENGINE).map_err(|error| error.to_string())?;
    let language = store
        .load_language(name, bytes)
        .map_err(|error| error.to_string())?;
    Ok(Box::leak(Box::new(language)))
}

/// Gives a parser the store required to run a WASM grammar.
pub(crate) fn prepare(parser: &mut Parser) -> Result<(), String> {
    let store = WasmStore::new(&ENGINE).map_err(|error| error.to_string())?;
    parser
        .set_wasm_store(store)
        .map_err(|error| error.to_string())
}
