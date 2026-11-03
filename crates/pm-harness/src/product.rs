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

    /// The folder the agent keeps its config in: the account folder in use, or the product's own when there is none; nothing for a product the app does not know; an account folder in use is first brought up to date with the user's own folder, so what is read from it is what the agent would load now.
    pub fn config_folder(self) -> Option<PathBuf> {
        let home = self.home.and_then(|_| self.home());
        if let Some(home) = &home {
            crate::profile::share_setup(self, home);
        }
        home.or_else(|| self.product()?.default_folder())
    }

    /// Whether the agent can be told to read its skills again while a conversation is open, which the Claude and Grok agents can, by the same command.
    pub fn reloads_skills(self) -> bool {
        matches!(self.product(), Some(Product::Claude | Product::Grok))
    }
}
