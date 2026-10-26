//! Closing the tags that are opened: what closes the tag a `>` typed at a place ends, `</>` for an opening tag with no name.

use crate::syntax::Syntax;

impl Syntax {
    /// The closing tag to write after a `>` typed at the byte `offset` of `text`, the text as it is before it is typed, when it ends an opening tag that nothing closes; none for a self-closing tag, a `>` that is not a tag's and a tag that is closed already.
    pub fn closing_tag(&self, text: &str, offset: usize) -> Option<String> {
        let tags = self.grammar().editing().tags.as_ref()?;
        if !text.is_char_boundary(offset) {
            return None;
        }
        let mut typed = String::with_capacity(text.len() + 1);
        typed.push_str(&text[..offset]);
        typed.push('>');
        typed.push_str(&text[offset..]);
        let parsed = Self::parse(self.grammar().clone(), &typed)?;
        let greater = parsed
            .tree
            .root_node()
            .descendant_for_byte_range(offset, offset + 1)?;
        if greater.kind() != ">" || greater.start_byte() != offset {
            return None;
        }
        let tag = greater.parent()?;
        if tag.kind() != tags.open {
            return None;
        }
        let name_of = |tag: tree_sitter::Node<'_>| {
            (0..tag.child_count())
                .filter_map(|at| tag.child(at))
                .find(|child| tags.names.iter().any(|held| held == child.kind()))
                .and_then(|name| typed.get(name.start_byte()..name.end_byte()))
        };
        let element = tag.parent().filter(|parent| parent.kind() == tags.element);
        let closed = element.is_some_and(|element| {
            (0..element.child_count())
                .filter_map(|at| element.child(at))
                .any(|child| {
                    child.kind() == tags.close
                        && !child.is_missing()
                        && name_of(child) == name_of(tag)
                })
        });
        if closed {
            return None;
        }
        let written = name_of(tag).unwrap_or_default();
        Some(format!("</{written}>"))
    }
}
