//! The agents the app knows the files of, recognised by what runs them rather than by the name they are listed under.

use std::path::PathBuf;

use crate::agent::Agent;

pub use pm_agent_library::Product;

impl Agent {
    /// The product the agent runs, when it is one the app knows.
    pub fn product(self) -> Option<Product> {
        let package = match self.source {
            crate::agent::Source::Package(package) => package,
            _ => "",
        };
        Product::of(&crate::mcp::command_line(
            std::iter::once(self.program)
                .chain(self.arguments.iter().copied())
                .chain(std::iter::once(package)),
        ))
    }

    /// The folder the agent keeps its config in: the account folder in use, or the product's own when there is none; nothing for a product the app does not know.
    pub fn config_folder(self) -> Option<PathBuf> {
        self.home
            .and_then(|_| self.home())
            .or_else(|| self.product()?.default_folder())
    }
}
