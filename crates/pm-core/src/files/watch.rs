//! Following project files on the machine holding them.

use pm_host::Location;
use std::sync::Arc;

pub use pm_host::{Disk, Touch, Touched};

/// A project's active filesystem subscription.
pub struct Watcher {
    /// The machine's settled change subscription.
    watch: pm_host::Watcher,
}

impl Watcher {
    /// Follows the root on its machine and wakes the window for changes.
    pub fn start(root: impl Into<Location>, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let root = root.into();
        Self {
            watch: root.host.watch(&root.path, wake),
        }
    }

    /// Takes the changes settled since the window last asked.
    pub fn take(&self) -> Disk {
        self.watch.take()
    }
}
