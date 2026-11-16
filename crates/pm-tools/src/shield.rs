//! Keeping the server alive in its own scope when what it started runs out of memory: the largest process it started is stopped before the scope reaches its cap, and every process it started is marked as preferred over it by the kernel's out-of-memory killer for when something grows too fast for that.
//!
//! The kernel alone is not enough. When a process in the scope goes past the cap, the kernel kills it, but its memory is freed only a moment later, and an allocation in that moment makes the kernel kill again, choosing among what is left, which may be the server or a terminal.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// The variable the launcher sets on a server it started in a scope of its own.
pub const OWN_SCOPE: &str = "PANDEMONIUM_OWN_SCOPE";

/// The out-of-memory adjustment of every process the server started: the most there is, so that the kernel picks any of them before the server, and the largest of them first.
const PREFERRED: i32 = 1000;

/// How often the scope is looked at.
const LOOK_AGAIN: Duration = Duration::from_millis(100);

/// Starts a thread that, for as long as this process lives, watches its scope: it marks every other process in it as preferred by the out-of-memory killer, and once the scope's memory passes its soft limit it kills the largest of them. Does nothing off Linux or unless [`OWN_SCOPE`] says the launcher gave this process a scope of its own.
pub fn shield_from_children() {
    if !cfg!(target_os = "linux") || std::env::var_os(OWN_SCOPE).is_none() {
        return;
    }
    let Some(group) = own_group() else {
        return;
    };
    let own = std::process::id();
    let _ = std::thread::Builder::new()
        .name("oom-shield".to_owned())
        .spawn(move || {
            loop {
                guard(&group, own);
                std::thread::sleep(LOOK_AGAIN);
            }
        });
}

/// Looks at `group` once on behalf of `own`: marks the other processes in it, and kills the largest of them when the group is past its soft limit.
fn guard(group: &Path, own: u32) {
    let others = others(group, own);
    mark(&others);
    if over_soft_limit(group) {
        stop_largest(&others);
    }
}

/// Guards the scope of `child`, which stands in for the server, until it ends, and returns what it printed.
#[cfg(test)]
pub(crate) fn guard_until_done(child: &mut std::process::Child) -> String {
    use std::io::Read;
    let own = child.id();
    let mut output = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut said = String::new();
        let _ = output.read_to_string(&mut said);
        said
    });
    while child.try_wait().ok().flatten().is_none() {
        if let Some(group) = group_of(own) {
            guard(&group, own);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    reader.join().unwrap()
}

/// The folder of the control group of the process `pid`, when it is a scope.
fn group_of(pid: u32) -> Option<PathBuf> {
    let line = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    let relative = line.lines().find_map(|line| line.strip_prefix("0::"))?;
    (relative.ends_with(".scope"))
        .then(|| Path::new("/sys/fs/cgroup").join(relative.trim_start_matches('/')))
}

/// The folder of this process's control group, when it is a scope.
fn own_group() -> Option<PathBuf> {
    group_of(std::process::id())
}

/// The processes in `group` other than `own`.
fn others(group: &Path, own: u32) -> Vec<u32> {
    std::fs::read_to_string(group.join("cgroup.procs"))
        .map(|listed| {
            listed
                .lines()
                .filter_map(|line| line.parse::<u32>().ok())
                .filter(|pid| *pid != own)
                .collect()
        })
        .unwrap_or_default()
}

/// Raises the out-of-memory adjustment of each of `pids` to [`PREFERRED`].
fn mark(pids: &[u32]) {
    for pid in pids {
        if adjustment(*pid).is_some_and(|held| held < PREFERRED) {
            let _ = std::fs::write(format!("/proc/{pid}/oom_score_adj"), PREFERRED.to_string());
        }
    }
}

/// Whether the memory `group` uses is past its soft limit; never when it has none.
fn over_soft_limit(group: &Path) -> bool {
    let read = |name: &str| -> Option<u64> {
        std::fs::read_to_string(group.join(name))
            .ok()?
            .trim()
            .parse()
            .ok()
    };
    match (read("memory.current"), read("memory.high")) {
        (Some(current), Some(high)) => current >= high,
        _ => false,
    }
}

/// Kills the process of `pids` that holds the most memory, so that the memory comes back before the scope reaches its cap.
fn stop_largest(pids: &[u32]) {
    let Some(largest) = pids.iter().copied().max_by_key(|pid| resident(*pid)) else {
        return;
    };
    let _ = Command::new("kill")
        .args(["-KILL", &largest.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// The memory, in pages, the process `pid` holds.
fn resident(pid: u32) -> u64 {
    std::fs::read_to_string(format!("/proc/{pid}/statm"))
        .ok()
        .and_then(|held| held.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0)
}

/// The out-of-memory adjustment of the process `pid`.
fn adjustment(pid: u32) -> Option<i32> {
    std::fs::read_to_string(format!("/proc/{pid}/oom_score_adj"))
        .ok()?
        .trim()
        .parse()
        .ok()
}
