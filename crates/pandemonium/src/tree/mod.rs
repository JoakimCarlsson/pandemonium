//! The file tree beside the panes, and what a reader does with it.
//!
//! The tree is a view of the disk that is worked the way every editor's file
//! tree is: rows are selected with a click and marked with the secondary
//! modifier or shift, the keyboard walks them, a name is typed where the row
//! will be, and what is selected is cut, copied, pasted, dragged into
//! another directory or taken off the disk. What the disk holds is
//! [`pm_core::FileTree`]'s; what the reader is doing to it is here.

mod clipboard;
mod drag;
mod edit;
mod menu;
mod selection;
mod view;

pub use clipboard::Clipboard;
pub use drag::EntryDrag;
pub use edit::{Edit, EditKind};
pub use menu::{empty_menu, entry_menu};
pub use selection::Selection;
pub use view::{Listing, files_sidebar};
