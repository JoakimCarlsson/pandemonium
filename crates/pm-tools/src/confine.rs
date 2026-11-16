//! Processes started in memory-capped systemd scopes of their own on Linux, so that when memory runs out the out-of-memory killer stops the process that grew, inside its own scope, and never the editor's window.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// The program that starts a command in a transient systemd scope.
const SYSTEMD_RUN: &str = "systemd-run";

/// The variable that, when set, starts every process as it is, without a scope.
const UNCONFINED: &str = "PANDEMONIUM_UNCONFINED";

/// The least memory, in bytes, any scope is allowed, however small the machine.
const FLOOR: u64 = 2 << 30;

/// The size of a memory page, to which the kernel rounds every cap down.
const PAGE: u64 = 4096;

/// How a process is confined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confinement {
    /// One heavy helper, such as a language server or a debugger: capped at half the machine and stopped whole when it goes past that.
    Helper,
    /// The server with everything it starts, agents, terminals and builds among them: capped below the machine's memory, and when it goes past that only the largest process in it is killed while the rest goes on.
    Server,
}

impl Confinement {
    /// The scope's memory cap in bytes for a machine with `total` bytes of memory.
    fn limit(self, total: u64) -> u64 {
        let share = match self {
            Self::Helper => total / 2,
            Self::Server => total / 100 * 85,
        };
        share.max(FLOOR) / PAGE * PAGE
    }

    /// Where a scope capped at `limit` bytes starts being slowed down: for the server, a tenth below the cap, so that [`crate::shield_from_children`] has time to stop what grew before the kernel does; for a helper, nowhere, since nothing watches it and it is better stopped than stalled.
    fn soft_limit(self, limit: u64) -> String {
        match self {
            Self::Helper => "infinity".to_owned(),
            Self::Server => (limit / 10 * 9 / PAGE * PAGE).to_string(),
        }
    }

    /// What systemd does with the scope when the out-of-memory killer stops a process in it.
    fn oom_policy(self) -> &'static str {
        match self {
            Self::Helper => "kill",
            Self::Server => "continue",
        }
    }

    /// The name the scope is shown under in `systemctl --user`.
    fn description(self, program: &Path) -> String {
        let name = program.file_name().map_or_else(
            || program.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        match self {
            Self::Helper => format!("Pandemonium helper {name}"),
            Self::Server => "Pandemonium server".to_owned(),
        }
    }

    /// A unit name for a new scope of `program` that says what runs in it, such as `pandemonium-rust-analyzer-4242-3.scope`, so that a notice of the scope being stopped names the program; unique by this process's id and a count.
    fn unit(self, program: &Path) -> String {
        static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let name: String = match self {
            Self::Helper => program
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
                .chars()
                .map(|held| {
                    if held.is_ascii_alphanumeric() || held == '-' || held == '_' {
                        held
                    } else {
                        '-'
                    }
                })
                .collect(),
            Self::Server => "server".to_owned(),
        };
        let count = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("pandemonium-{name}-{}-{count}.scope", std::process::id())
    }
}

/// The program and arguments that start `program` with `arguments` confined as `confinement`: inside a memory-capped systemd scope when the user's systemd can make one, otherwise unchanged. The scope runs `program` in the same process, so its id, input, output, folder and variables stay those of the process started.
pub fn confined(
    program: &Path,
    arguments: &[String],
    confinement: Confinement,
) -> (PathBuf, Vec<String>) {
    match confinable_memory() {
        Some(total) => scoped(program, arguments, confinement, confinement.limit(total)),
        None => (program.to_path_buf(), arguments.to_vec()),
    }
}

/// The program and arguments that start `program` with `arguments` in a systemd scope confined as `confinement` with a cap of `limit` bytes.
fn scoped(
    program: &Path,
    arguments: &[String],
    confinement: Confinement,
    limit: u64,
) -> (PathBuf, Vec<String>) {
    let mut wrapped = vec![
        "--user".to_owned(),
        "--scope".to_owned(),
        "--quiet".to_owned(),
        "--collect".to_owned(),
        format!("--unit={}", confinement.unit(program)),
        format!("--description={}", confinement.description(program)),
        format!("--property=MemoryMax={limit}"),
        format!("--property=MemoryHigh={}", confinement.soft_limit(limit)),
        "--property=MemorySwapMax=0".to_owned(),
        format!("--property=OOMPolicy={}", confinement.oom_policy()),
        "--".to_owned(),
        program.display().to_string(),
    ];
    wrapped.extend(arguments.iter().cloned());
    (PathBuf::from(SYSTEMD_RUN), wrapped)
}

/// The memory cap, in bytes, a helper is started with on this machine; nothing when helpers are started without one.
pub fn helper_limit() -> Option<u64> {
    confinable_memory().map(|total| Confinement::Helper.limit(total))
}

/// The machine's memory in bytes, worked out once, when processes can be confined here: nothing off Linux, when [`UNCONFINED`] is set, or when the user's systemd cannot make scopes.
fn confinable_memory() -> Option<u64> {
    static TOTAL: OnceLock<Option<u64>> = OnceLock::new();
    *TOTAL.get_or_init(|| {
        if !cfg!(target_os = "linux") || std::env::var_os(UNCONFINED).is_some() {
            return None;
        }
        let total = total_memory()?;
        scope_works().then_some(total)
    })
}

/// The machine's memory in bytes, read from `/proc/meminfo`.
fn total_memory() -> Option<u64> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = info.lines().find(|line| line.starts_with("MemTotal:"))?;
    let kilobytes: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kilobytes * 1024)
}

/// Whether the user's systemd starts a command in a scope of its own, tried with `true`.
fn scope_works() -> bool {
    Command::new(SYSTEMD_RUN)
        .args(["--user", "--scope", "--quiet", "--collect", "--", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cap the tests give scopes, small so that they never press on the machine's memory.
    const TEST_LIMIT: u64 = 256 << 20;

    /// What prints the memory cap of the scope a process runs in, as the kernel reports it.
    const REPORT: &str =
        "cat /proc/self/cgroup; cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/memory.max";

    /// Runs `script` with `sh` in a scope confined as `confinement` with the test cap, and returns whether it succeeded and what it printed.
    fn run(script: &str, confinement: Confinement) -> (bool, String) {
        let (program, arguments) = scoped(
            Path::new("sh"),
            &["-c".to_owned(), script.to_owned()],
            confinement,
            TEST_LIMIT,
        );
        let output = Command::new(program).args(arguments).output().unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    }

    /// What uses twice the test cap.
    fn hog() -> String {
        format!("head -c {} /dev/zero | tail > /dev/null", TEST_LIMIT * 2)
    }

    /// What uses twice the test cap once it has been running for a moment, as a build that grows does, rather than at once.
    fn slow_hog() -> String {
        format!("sleep 0.5; {}", hog())
    }

    /// Caps are a share of the machine, never under the floor, and whole pages.
    #[test]
    fn caps_follow_the_machine() {
        let total = 32 << 30;
        assert_eq!(Confinement::Helper.limit(total), 16 << 30);
        assert!(Confinement::Server.limit(total) > 27 << 30);
        assert_eq!(Confinement::Helper.limit(1 << 30), FLOOR);
        assert_eq!(Confinement::Server.limit(total + 1) % PAGE, 0);
    }

    /// A confined command keeps its program and arguments, in order, after `--`.
    #[test]
    fn keeps_the_command() {
        let (program, arguments) = confined(
            Path::new("/bin/server"),
            &["--stdio".to_owned()],
            Confinement::Helper,
        );
        if confinable_memory().is_none() {
            assert_eq!(program, Path::new("/bin/server"));
            return;
        }
        assert_eq!(program, Path::new(SYSTEMD_RUN));
        assert!(
            arguments
                .iter()
                .any(|held| held.starts_with("--unit=pandemonium-server-"))
        );
        assert_eq!(
            &arguments[arguments.len() - 3..],
            ["--", "/bin/server", "--stdio"]
        );
    }

    /// A helper runs in a scope of its own with its cap, and one that goes past it is killed.
    #[test]
    fn helper_past_its_cap_is_killed() {
        if confinable_memory().is_none() {
            return;
        }
        let (_, said) = run(REPORT, Confinement::Helper);
        assert!(said.contains(".scope"), "{said}");
        assert!(said.contains(&TEST_LIMIT.to_string()), "{said}");
        assert!(!run(&hog(), Confinement::Helper).0);
    }

    /// The server's scope keeps going when a process in it is killed, and slows growth a tenth below its cap.
    #[test]
    fn server_scope_survives_a_kill_and_has_a_soft_limit() {
        if confinable_memory().is_none() {
            return;
        }
        let script = format!(
            "{REPORT}; cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/memory.high; {}; echo survived",
            slow_hog()
        );
        let (program, arguments) = scoped(
            Path::new("sh"),
            &["-c".to_owned(), script],
            Confinement::Server,
            TEST_LIMIT,
        );
        let mut child = Command::new(program)
            .args(arguments)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let said = crate::shield::guard_until_done(&mut child);
        assert!(said.contains(&TEST_LIMIT.to_string()), "{said}");
        assert!(
            said.contains(&Confinement::Server.soft_limit(TEST_LIMIT)),
            "{said}"
        );
        assert!(said.contains("survived"), "{said}");
    }
}
