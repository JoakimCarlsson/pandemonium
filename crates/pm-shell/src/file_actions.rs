//! What can be done with an open file besides editing it: showing it in the
//! Files panel, going through the states it was saved in, comparing it with
//! the clipboard, with what was last saved or committed of it or with
//! another open file, and the changes its language server makes to the
//! whole file.

/// Something done with an open file, named by its position among the open files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileAction {
    /// Shows the file in the Files panel.
    Reveal,
    /// Lists the states the file was saved in, to put one back.
    History,
    /// Compares the file with this text, taken from the clipboard.
    CompareClipboard(String),
    /// Compares the file with what the folder holds of it.
    CompareSaved,
    /// Compares the file with what the last commit holds of it.
    CompareCommitted,
    /// Lists the files of the project, to compare the file with one.
    ChooseCompare,
    /// Compares the file with the open file at this position.
    CompareWith(usize),
    /// Has the language server sort the file's imports.
    OrganizeImports,
    /// Has the language server fix every problem of the file it can fix at once.
    FixAll,
    /// Has the language server format the file.
    Format,
    /// Installs the language with this id, which the file is written in.
    InstallLanguage(String),
    /// Stops offering to install the language with this id.
    DeclineLanguage(String),
}
