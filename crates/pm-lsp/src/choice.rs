//! Which language server reads the files of a language, when the reader chose other than the one it comes with.

use crate::manifest::ServerSpec;

/// What the reader chose for the language server of one language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerChoice {
    /// No language server reads its files.
    Off,
    /// This command, written as on a command line, is started in place of the one it comes with.
    Command(String),
    /// No language server is started, for this reason: the extension that brought the language is not allowed to run programs.
    Refused(String),
}

impl ServerChoice {
    /// The server a language whose own is `own`, written in `language`, is read by under this choice.
    pub fn applied(&self, language: &str, own: Option<ServerSpec>) -> Option<ServerSpec> {
        match self {
            Self::Off | Self::Refused(_) => None,
            Self::Command(line) => {
                let mut words = line.split_whitespace().map(str::to_owned);
                let command = words.next()?;
                let base = own.unwrap_or_else(|| ServerSpec {
                    language_id: language.to_owned(),
                    ..ServerSpec::default()
                });
                Some(ServerSpec {
                    id: format!("{}:{line}", base.id),
                    command,
                    args: words.collect(),
                    check: Vec::new(),
                    install: None,
                    ..base
                })
            }
        }
    }
}
