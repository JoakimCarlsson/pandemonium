//! A bounded cache of file hierarchies shared by drawing and tree commands.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;

use crate::tree::{Entry, Layout, build};

/// How many recently displayed projects or sessions retain their hierarchy.
const CAPACITY: usize = 4;

/// A hierarchy and the exact input ordering its file indices refer to.
struct Cached {
    /// The files, in the server's order.
    files: Vec<String>,
    /// The explicitly reported folders.
    folders: Vec<String>,
    /// The sorted hierarchy built from these paths.
    entries: Arc<Vec<Entry>>,
}

thread_local! {
    /// Recently used hierarchies, most recent first, bounded independently of frame count.
    static TREES: RefCell<VecDeque<Cached>> = const { RefCell::new(VecDeque::new()) };
}

/// Gets the hierarchy, rebuilding only when the files or explicit folders change.
pub(super) fn tree(files: &[String], folders: &[String]) -> Arc<Vec<Entry>> {
    TREES.with_borrow_mut(|trees| {
        if let Some(index) = trees
            .iter()
            .position(|tree| tree.files == files && tree.folders == folders)
        {
            let found = trees.remove(index).expect("entry found above");
            let entries = found.entries.clone();
            trees.push_front(found);
            return entries;
        }
        let paths: Vec<String> = files
            .iter()
            .cloned()
            .chain(folders.iter().map(|folder| format!("{folder}/")))
            .collect();
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let entries = Arc::new(build(
            &paths,
            Layout {
                compact: false,
                sorted: true,
            },
        ));
        trees.push_front(Cached {
            files: files.to_vec(),
            folders: folders.to_vec(),
            entries: entries.clone(),
        });
        trees.truncate(CAPACITY);
        entries
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reusing input preserves the tree; reordering files or changing folders invalidates its indices.
    #[test]
    fn reuse_and_invalidation() {
        let files = vec!["b.rs".to_owned(), "a.rs".to_owned()];
        let first = tree(&files, &[]);
        assert!(Arc::ptr_eq(&first, &tree(&files, &[])));
        let reversed = tree(&[files[1].clone(), files[0].clone()], &[]);
        assert!(!Arc::ptr_eq(&first, &reversed));
        assert!(matches!(&reversed[0], Entry::File { name, index: 0 } if name == "a.rs"));
        assert!(!Arc::ptr_eq(&first, &tree(&files, &["empty".to_owned()])));
        assert!(!Arc::ptr_eq(&first, &tree(&files[..1], &[])));
    }
}
