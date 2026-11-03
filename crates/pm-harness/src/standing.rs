//! Where an agent stands on this machine.

/// How a login is carried out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Way {
    /// The agent logs in itself when asked, opening a browser if it must.
    Asked,
    /// The agent is run with these arguments in a terminal, where the reader logs in.
    Terminal {
        /// The arguments to run the agent with.
        args: Vec<String>,
    },
}

/// One way an agent offers to log in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Login {
    /// What the agent calls the method.
    pub id: String,
    /// What the method is called to the reader.
    pub name: String,
    /// How it is carried out.
    pub way: Way,
}

/// Where an agent stands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Standing {
    /// It started, answered and opened a conversation.
    Ready,
    /// It started but wants a login first, and offers these.
    SignedOut(Vec<Login>),
    /// It cannot be started: its program is not here and nothing can fetch it.
    Missing,
    /// It started but did not work, for this reason.
    Failed(String),
}
