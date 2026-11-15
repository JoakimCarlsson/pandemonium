//! Grammars the window highlights files with: compiled from what a server
//! sends, or read from the languages installed on this machine for a
//! snapshot, and kept in the process's registry.

use std::sync::Arc;

use pm_protocol::GrammarBundle;
use pm_treesitter::{Editing, Grammar, Queries, registry};

/// The manifest of the language `bundle` was made from, as far as it says how the language is edited.
fn manifest_of(bundle: &GrammarBundle) -> pm_lsp::Manifest {
    pm_lsp::Manifest {
        indent: bundle.indent,
        tabs: bundle.tabs,
        line_comment: bundle.line_comment.clone(),
        pairs: bundle
            .pairs
            .clone()
            .unwrap_or_else(pm_treesitter::default_pairs),
        scopes: bundle.scopes.clone(),
        tags: bundle.tags.clone(),
        emmet_dialect: bundle.more.emmet_dialect.clone(),
        word_characters: bundle.more.word_characters.clone(),
        brackets: bundle.more.bracket_pairs.clone(),
        increase_indent_pattern: bundle.more.increase_indent_pattern.clone(),
        decrease_indent_pattern: bundle.more.decrease_indent_pattern.clone(),
        decrease_indent_patterns: bundle.more.decrease_indent_patterns.clone(),
        ..pm_lsp::Manifest::default()
    }
}

/// Compiles the grammar `bundle` carries and keeps it in the registry under
/// its names, or returns the one kept already.
///
/// # Errors
///
/// Fails with why the grammar or one of its queries does not compile.
pub fn learn_grammar(bundle: &GrammarBundle) -> Result<Arc<Grammar>, String> {
    if let Some(loaded) = registry::called(&bundle.id) {
        return Ok(loaded);
    }
    let grammar = Grammar::load(
        &bundle.id,
        &bundle.symbol,
        &bundle.wasm,
        &Queries {
            highlights: bundle.highlights.clone(),
            indents: bundle.indents.clone(),
            outline: bundle.outline.clone(),
            injections: bundle.injections.clone(),
            brackets: bundle.brackets.clone(),
            runnables: bundle.runnables.clone(),
            overrides: bundle.more.overrides.clone(),
            redactions: bundle.more.redactions.clone(),
            textobjects: bundle.more.textobjects.clone(),
            debugger: bundle.more.debugger.clone(),
            dependencies: String::new(),
        },
        Editing {
            run_lenses: bundle
                .more
                .lenses
                .iter()
                .map(|lens| (lens.tags.clone(), lens.label.clone()))
                .collect(),
            ..pm_lsp::editing_of(&manifest_of(bundle))
        },
    )?;
    let mut aliases = bundle.aliases.clone();
    aliases.push(bundle.symbol.clone());
    registry::add(grammar.clone(), &aliases);
    Ok(grammar)
}

/// The grammar of the language `id` installed on this machine, loaded into
/// the registry, for a snapshot that has no server to ask; nothing when the
/// language is not installed.
pub fn installed_grammar(id: &str) -> Option<Arc<Grammar>> {
    if let Some(loaded) = registry::called(id) {
        return Some(loaded);
    }
    let manifest = pm_lsp::installed_manifest(id)?;
    let wasm = std::fs::read(pm_lsp::grammar_file(id).ok()?).ok()?;
    let read = |file: &str| {
        pm_lsp::language_file(id, file)
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_default()
    };
    let queries = Queries {
        highlights: read("highlights.scm"),
        indents: read("indents.scm"),
        outline: read("outline.scm"),
        injections: read("injections.scm"),
        brackets: read("brackets.scm"),
        runnables: read("runnables.scm"),
        overrides: read("overrides.scm"),
        redactions: read("redactions.scm"),
        textobjects: read("textobjects.scm"),
        debugger: read("debugger.scm"),
        dependencies: String::new(),
    };
    let editing = pm_lsp::editing_of(&manifest);
    let grammar = Grammar::load(id, &manifest.grammar, &wasm, &queries, editing).ok()?;
    let mut aliases = manifest.aliases.clone();
    aliases.push(manifest.grammar.clone());
    registry::add(grammar.clone(), &aliases);
    Some(grammar)
}
