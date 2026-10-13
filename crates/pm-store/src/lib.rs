//! Persistence for the projects the reader has added.

use std::fs;
use std::io;
use std::path::PathBuf;

/// The folder the app keeps its data in, under the desktop's data directory.
fn directory() -> Option<PathBuf> {
    dirs::data_dir().map(|data| data.join("pandemonium"))
}

/// The file the project list is kept in.
fn file() -> Option<PathBuf> {
    directory().map(|directory| directory.join("projects"))
}

/// The paths of the projects saved by [`save_projects`], oldest first, or
/// nothing when none was ever saved: the app has not been set up yet.
pub fn load_projects() -> Option<Vec<String>> {
    let contents = fs::read_to_string(file()?).ok()?;
    Some(
        contents
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// Saves `paths` as the projects to open with next time, one per line.
pub fn save_projects(paths: &[String]) -> io::Result<()> {
    let (Some(directory), Some(file)) = (directory(), file()) else {
        return Err(io::Error::other("the desktop has no data directory"));
    };
    fs::create_dir_all(directory)?;
    let mut contents = paths.join("\n");
    contents.push('\n');
    fs::write(file, contents)
}
