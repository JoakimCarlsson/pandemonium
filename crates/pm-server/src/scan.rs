//! Reading files on the server's machine with the installed languages: the places of a manifest that a language marks as runnable, and the task templates the languages ship.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use pm_task::Template;
use pm_treesitter::{Editing, Grammar, Queries, Syntax, registry};

/// The grammar of the installed language `id`, loaded into this process's registry the first time it is asked for.
fn grammar_of(id: &str) -> Option<Arc<Grammar>> {
    if let Some(loaded) = registry::called(id) {
        return Some(loaded);
    }
    let bundle = crate::languages::grammar(id).ok()?;
    let queries = Queries {
        highlights: bundle.highlights,
        indents: bundle.indents,
        outline: bundle.outline,
        injections: bundle.injections,
        brackets: bundle.brackets,
        runnables: bundle.runnables,
        ..Queries::default()
    };
    let grammar = Grammar::load(
        &bundle.id,
        &bundle.symbol,
        &bundle.wasm,
        &queries,
        Editing::default(),
    )
    .ok()?;
    let mut aliases = bundle.aliases;
    aliases.push(bundle.symbol);
    registry::add(grammar.clone(), &aliases);
    Some(grammar)
}

/// What each place of the file at `path` that carries any of `tags` captured, in the order the places start; none when no installed language reads the file or the file cannot be read.
pub(crate) fn places(path: &Path, tags: &[String]) -> Vec<BTreeMap<String, String>> {
    let Some(language) = path.to_str().and_then(pm_lsp::language_of) else {
        return Vec::new();
    };
    let (Some(grammar), Ok(text)) = (grammar_of(&language), std::fs::read_to_string(path)) else {
        return Vec::new();
    };
    Syntax::parse(grammar, &text)
        .map(|syntax| {
            syntax
                .runnables_tagged(&text, tags)
                .into_iter()
                .map(|place| place.variables)
                .collect()
        })
        .unwrap_or_default()
}

/// The task templates of every installed language, in the order of the languages' folder names; a language whose `tasks.json` cannot be read ships none.
pub(crate) fn templates() -> Vec<Template> {
    pm_lsp::installed()
        .into_iter()
        .filter_map(|(id, _)| {
            let text =
                std::fs::read_to_string(pm_lsp::language_folder(&id).ok()?.join("tasks.json"))
                    .ok()?;
            pm_task::parse_templates(&text).ok()
        })
        .flatten()
        .collect()
}
