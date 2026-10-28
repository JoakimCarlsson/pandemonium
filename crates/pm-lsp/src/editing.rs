//! How a language's files are edited, as its manifest says, in the form the editor reads.

use std::collections::BTreeMap;

use pm_treesitter::{Aligned, Editing, IndentRules, Pattern, Scope, Tags};

use crate::manifest::{Manifest, ScopeSpec, TagSpec};

/// The scopes a language's manifest declares, as the editor reads them.
pub fn scopes_of(declared: &BTreeMap<String, ScopeSpec>) -> Vec<Scope> {
    declared
        .iter()
        .map(|(name, scope)| Scope {
            name: name.clone(),
            nodes: scope.nodes.clone(),
            line_comment: scope.line_comment.clone(),
            block_comment: scope
                .block_comment
                .clone()
                .map(|[open, close]| (open, close)),
            emmet: scope.emmet,
        })
        .collect()
}

/// The tag nodes a language's manifest declares, as the editor reads them.
pub fn tags_of(declared: Option<&TagSpec>) -> Option<Tags> {
    declared.map(|tags| Tags {
        element: tags.element.clone(),
        open: tags.open.clone(),
        close: tags.close.clone(),
        names: tags.names.clone(),
    })
}

/// How the files of the language `manifest` describes are edited.
pub fn editing_of(manifest: &Manifest) -> Editing {
    let mut pairs = Editing::pairs_of(&manifest.pairs);
    let mut not_in = Vec::new();
    for bracket in &manifest.brackets {
        let (Some(open), Some(close)) = (bracket.start.chars().next(), bracket.end.chars().next())
        else {
            continue;
        };
        pairs.retain(|(held, _)| *held != open);
        if bracket.close {
            pairs.push((open, close));
        }
        not_in.push((open, bracket.not_in.clone()));
    }
    Editing {
        indent: manifest.indent,
        tabs: manifest.tabs,
        line_comment: manifest.line_comment.clone(),
        pairs,
        scopes: scopes_of(&manifest.scopes),
        tags: tags_of(manifest.tags.as_ref()),
        emmet_dialect: manifest.emmet_dialect.clone(),
        run_lenses: Vec::new(),
        word_characters: manifest.word_characters.concat(),
        not_in,
        indent_rules: IndentRules {
            increase: manifest
                .increase_indent_pattern
                .as_deref()
                .and_then(Pattern::new),
            decrease: manifest
                .decrease_indent_pattern
                .as_deref()
                .and_then(Pattern::new),
            aligned: manifest
                .decrease_indent_patterns
                .iter()
                .filter_map(|rule| {
                    Some(Aligned {
                        pattern: Pattern::new(&rule.pattern)?,
                        valid_after: rule.valid_after.clone(),
                    })
                })
                .collect(),
        },
    }
}
