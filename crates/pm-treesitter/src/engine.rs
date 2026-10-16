//! The WebAssembly engine grammars run in, and the parser each thread parses with.

use std::cell::RefCell;
use std::sync::LazyLock;

use tree_sitter::{Language, Parser, WasmStore, wasmtime};

/// The engine every grammar is compiled for.
static ENGINE: LazyLock<wasmtime::Engine> = LazyLock::new(wasmtime::Engine::default);

thread_local! {
    /// The parser of this thread, with a store to run WebAssembly grammars in, made on first use.
    static PARSER: RefCell<Option<Parser>> = const { RefCell::new(None) };
}

/// A parser that can run grammars compiled to WebAssembly.
fn wasm_parser() -> Result<Parser, String> {
    let store = WasmStore::new(&ENGINE).map_err(|error| error.to_string())?;
    let mut parser = Parser::new();
    parser
        .set_wasm_store(store)
        .map_err(|error| error.to_string())?;
    Ok(parser)
}

/// Runs `work` with this thread's parser.
pub(crate) fn with_parser<T>(work: impl FnOnce(&mut Parser) -> T) -> Result<T, String> {
    PARSER.with(|held| {
        let mut held = held.borrow_mut();
        if held.is_none() {
            *held = Some(wasm_parser()?);
        }
        let parser = held.as_mut().ok_or("the parser could not be made")?;
        Ok(work(parser))
    })
}

/// Compiles the grammar named `name` from the WebAssembly module `wasm`.
pub(crate) fn load_language(name: &str, wasm: &[u8]) -> Result<Language, String> {
    with_parser(|parser| {
        let mut store = parser
            .take_wasm_store()
            .ok_or("the parser has no WebAssembly store")?;
        let language = store
            .load_language(name, wasm)
            .map_err(|error| error.to_string());
        parser
            .set_wasm_store(store)
            .map_err(|error| error.to_string())?;
        language
    })?
}
