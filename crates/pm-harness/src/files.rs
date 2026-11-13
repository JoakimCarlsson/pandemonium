//! What the harness answers about the files of a turn: what it changed, and that the files were put back or the turn's changes taken out of them.

pub use omc_wire::TurnChange;

/// What every failure of the harness's commands about a turn's files starts with.
const FAILED: &str = "files:";

/// An answer of the harness to a request about the files of a turn; every request gets exactly one, in the order the requests were made.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileNews {
    /// What a turn changed.
    Changes {
        /// The id the turn is saved under.
        turn: String,
        /// The files it changed, each with its diff.
        files: Vec<TurnChange>,
    },
    /// The files were put back as they were before a turn.
    Restored {
        /// The id the turn is saved under.
        turn: String,
        /// The files that changed, from the root of the repository.
        files: Vec<String>,
    },
    /// What a turn changed was taken back out of the files.
    Undone {
        /// The id the turn is saved under.
        turn: String,
        /// The files that changed, from the root of the repository.
        files: Vec<String>,
    },
    /// The request could not be carried out, for this reason; the conversation goes on.
    Failed(String),
}

impl FileNews {
    /// The answer `reported` is, when it is one about the files of a turn.
    pub fn from_harness(reported: &omc_wire::Event) -> Option<Self> {
        use omc_wire::Event as Wire;
        match reported {
            Wire::Changes { turn, files, .. } => Some(Self::Changes {
                turn: turn.clone(),
                files: files.clone(),
            }),
            Wire::Restored { turn, files } => Some(Self::Restored {
                turn: turn.clone(),
                files: files.clone(),
            }),
            Wire::Undone { turn, files } => Some(Self::Undone {
                turn: turn.clone(),
                files: files.clone(),
            }),
            Wire::Failed(reason) => reason
                .strip_prefix(FAILED)
                .map(|reason| Self::Failed(reason.trim().to_owned())),
            _ => None,
        }
    }
}

#[cfg(test)]
/// Tests of reading the harness's answers about files.
mod tests {
    use super::*;

    /// A failure about files is an answer, with its marker taken off, and any other failure is not.
    #[test]
    fn only_file_failures_are_answers() {
        assert_eq!(
            FileNews::from_harness(&omc_wire::Event::Failed(
                "files: no files were kept before the turn 4".to_owned()
            )),
            Some(FileNews::Failed(
                "no files were kept before the turn 4".to_owned()
            ))
        );
        assert_eq!(
            FileNews::from_harness(&omc_wire::Event::Failed("out of memory".to_owned())),
            None
        );
        assert_eq!(
            FileNews::from_harness(&omc_wire::Event::Restored {
                turn: "4".to_owned(),
                files: vec!["a".to_owned()]
            }),
            Some(FileNews::Restored {
                turn: "4".to_owned(),
                files: vec!["a".to_owned()]
            })
        );
    }
}
