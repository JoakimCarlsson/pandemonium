//! The products the app knows the files of, which an agent's customizations are read from.

use std::path::PathBuf;

use crate::agent::Agent;

pub use pm_agent_library::Product;

impl Agent {
    /// The product the agent runs, when it is one the app knows.
    pub fn product(self) -> Option<Product> {
        self.product
    }

    /// The folder the agent's customizations are read from: its product's own, when it has a product.
    pub fn config_folder(self) -> Option<PathBuf> {
        self.product?.default_folder()
    }

    /// Whether the agent can be told to read its skills again while a conversation is open.
    pub fn reloads_skills(self) -> bool {
        false
    }
}
