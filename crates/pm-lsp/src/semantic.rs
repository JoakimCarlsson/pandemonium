//! Rules that say what colour the names a language server reports are: one per type of token a server names, from the user's settings, from the language's `semantic_token_rules.json` and, where neither says, from what the editor knows of the standard types.

use serde::{Deserialize, Serialize};

use crate::types::TokenKind;

/// One rule: the tokens it matches and the styles it gives them.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenRule {
    /// The type of token it matches, as the server names it; every type when nothing.
    #[serde(default)]
    pub token_type: Option<String>,
    /// The modifiers a token must all have for the rule to match.
    #[serde(default)]
    pub token_modifiers: Vec<String>,
    /// The styles the token is given, by the name of a highlight capture such as `keyword` or `type`; the first the editor knows is used.
    #[serde(default)]
    pub style: Vec<String>,
}

impl TokenRule {
    /// Whether the rule matches a token of type `name` that has the modifiers `held`.
    fn matches(&self, name: &str, held: &[&str]) -> bool {
        self.token_type
            .as_deref()
            .is_none_or(|wanted| wanted == name)
            && self
                .token_modifiers
                .iter()
                .all(|wanted| held.contains(&wanted.as_str()))
    }

    /// The kind its first style the editor knows stands for.
    fn kind(&self) -> Option<TokenKind> {
        self.style.iter().find_map(|style| kind_of_style(style))
    }
}

/// The rules written in `text`, a JSON array, or none when it is not one.
pub fn parse_rules(text: &str) -> Vec<TokenRule> {
    serde_json::from_str(text).unwrap_or_default()
}

/// The kind the style `style`, named as a highlight capture is, stands for.
fn kind_of_style(style: &str) -> Option<TokenKind> {
    let style = style.strip_prefix("syntax.").unwrap_or(style);
    let head = style.split('.').next().unwrap_or(style);
    Some(match head {
        "keyword" | "conditional" | "repeat" | "include" | "storageclass" => TokenKind::Keyword,
        "function" | "method" | "constructor" | "macro" => TokenKind::Function,
        "type" | "namespace" | "module" => TokenKind::Type,
        "string" | "character" | "escape" => TokenKind::String,
        "number" | "float" | "boolean" | "constant" | "label" => TokenKind::Constant,
        "comment" => TokenKind::Comment,
        "property" | "field" | "attribute" => TokenKind::Property,
        "operator" | "punctuation" => TokenKind::Operator,
        "variable" | "parameter" => TokenKind::Variable,
        _ => return None,
    })
}

/// The kind of a token of type `name` with the modifiers `held` by the first of `rules` that matches it and names a style the editor knows; nothing when none does.
pub fn kind_by_rules(rules: &[TokenRule], name: &str, held: &[&str]) -> Option<TokenKind> {
    rules
        .iter()
        .filter(|rule| rule.matches(name, held))
        .find_map(TokenRule::kind)
}

/// The rules that apply to the tokens of a file written in the installed language `language`: the reader's own `user` rules, then those of the language's `semantic_token_rules.json`, which are ahead of what the editor knows of the standard types.
pub fn rules_for(language: &str, user: &[TokenRule]) -> Vec<TokenRule> {
    let mut rules = user.to_vec();
    rules.extend(
        crate::inherit::language_file(language, "semantic_token_rules.json")
            .and_then(|file| std::fs::read_to_string(file).ok())
            .map(|text| parse_rules(&text))
            .unwrap_or_default(),
    );
    rules
}
