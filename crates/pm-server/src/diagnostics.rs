//! The app's crash log, the files a server gives for a bug report, and the report itself.

use std::path::{Path, PathBuf};

use omc_diagnostics::{CrashLog, Facts, Report};
use pm_protocol::DiagnosticFile;

pub use omc_diagnostics::{Crash, Telemetry, now_ms};

/// The version of this build.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The name the server goes by in the crash log, in bug reports and in its telemetry.
pub const SERVER: &str = "pandemonium-server";

/// The name the window goes by in the crash log and in bug reports.
pub const WINDOW: &str = "pandemonium";

/// The crash log the window and the server of this machine share, when the desktop has a data folder.
fn crash_log() -> Option<CrashLog> {
    pm_store::crash_file().map(CrashLog::at)
}

/// Records every panic of this process in the app's crash log, as a crash of `program`, such as `pandemonium` or `pandemonium-server`.
///
/// A panic an agent's conversation or another part catches and lives through is recorded as well.
pub fn record_crashes(program: &'static str) {
    if let Some(log) = crash_log() {
        omc_diagnostics::install(log, program, VERSION);
    }
}

/// The newest crash of the app on this machine the reader has not been told about, from the last seven days; it is not given again.
pub fn take_crash() -> Option<Crash> {
    crash_log()?.take_unnoticed(now_ms())
}

/// Starts sending this process's traces and metrics as `program`, when the `OTEL_*` variables ask for it and the build can.
pub fn start_telemetry(program: &'static str) -> Option<Telemetry> {
    Telemetry::start(program, VERSION)
}

/// The files this machine gives for a bug report about `program`: `facts.json`, with the build, the operating system and the architecture, and `crashes.json`, the crash log, when it holds any.
pub fn files(program: &str) -> Vec<DiagnosticFile> {
    let facts = serde_json::to_string_pretty(&Facts::now(program, VERSION)).unwrap_or_default();
    let mut files = vec![DiagnosticFile {
        name: "facts.json".to_owned(),
        text: facts,
    }];
    let crashes = crash_log().map(|log| log.crashes()).unwrap_or_default();
    if !crashes.is_empty() {
        files.push(DiagnosticFile {
            name: "crashes.json".to_owned(),
            text: serde_json::to_string_pretty(&crashes).unwrap_or_default(),
        });
    }
    files
}

/// Writes a bug report about `program` into `folder` and returns the zip's path: the facts about this build and machine, then the files each machine gave, each machine's in a folder of its name, or why it gave none as `trouble.txt` there.
///
/// # Errors
///
/// Fails when the folder cannot be made or the zip cannot be written.
pub fn write_report(
    program: &str,
    machines: Vec<(String, Result<Vec<DiagnosticFile>, String>)>,
    folder: &Path,
) -> std::io::Result<PathBuf> {
    let mut report = Report::new(Facts::now(program, VERSION));
    for (machine, answer) in machines {
        let machine = machine.replace(['/', '\\', ':'], "_");
        match answer {
            Ok(files) => {
                for file in files {
                    report =
                        report.file(format!("{machine}/{}", file.name), file.text.into_bytes());
                }
            }
            Err(trouble) => {
                report = report.file(format!("{machine}/trouble.txt"), trouble.into_bytes());
            }
        }
    }
    report.write(folder)
}

#[cfg(test)]
/// Tests for bug reports.
mod tests {
    use super::*;

    /// A report holds every machine's files in its own folder, and the trouble of a machine that gave none.
    #[test]
    fn writes_each_machine_apart() {
        let folder = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../build/test-bug-report"
        ));
        let _ = std::fs::remove_dir_all(&folder);
        let path = write_report(
            "pandemonium",
            vec![
                ("this machine".to_owned(), Ok(files("pandemonium"))),
                ("ssh://box".to_owned(), Err("not connected".to_owned())),
            ],
            &folder,
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("this machine/facts.json"));
        assert!(text.contains("ssh___box/trouble.txt"));
    }
}
