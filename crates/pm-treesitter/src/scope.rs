//! The scope of the language the caret is in: which named set of syntax nodes holds it.

use crate::grammar::Scope;
use crate::syntax::Syntax;

impl Syntax {
    /// The scope of the innermost node of any scope the byte `offset` is inside, where being inside a node means being after its start and before its end; none when no scope holds it.
    pub fn scope_at(&self, offset: usize) -> Option<&Scope> {
        let scopes = &self.grammar().editing().scopes;
        if scopes.is_empty() {
            return None;
        }
        let mut node = self
            .tree
            .root_node()
            .descendant_for_byte_range(offset, offset)?;
        loop {
            if node.start_byte() < offset && offset < node.end_byte() {
                let kind = node.kind();
                if let Some(scope) = scopes
                    .iter()
                    .find(|scope| scope.nodes.iter().any(|held| held == kind))
                {
                    return Some(scope);
                }
            }
            node = node.parent()?;
        }
    }
}
